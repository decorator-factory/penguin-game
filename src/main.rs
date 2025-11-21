use std::{
    borrow::Cow,
    io::Read,
};

use macroquad::prelude::*;
use miniquad::TextureWrap;
use parry2d::shape::ConvexPolygon;

mod demo;
mod draw_utils;
mod input;
mod levels;
mod wasm;

#[cfg(not(target_family = "wasm"))]
mod pico_args;

const UPS_NORMAL: f64 = 240.;
const UPS_FAST: f64 = 1200.;

const PENGUIN_RADIUS: f32 = 24.0;
const ROCKET_RADIUS: f32 = 4.0;

#[derive(Copy, Clone, Debug)]
struct Penguin {
    pos: Vec2,
    vel: Vec2,
    rocket_cooldown: u16,
    coyote_time: u8,
}

#[derive(Copy, Clone, Debug)]
struct Rocket {
    pos: Vec2,
    vel: Vec2,
    ttl: u16,
}

#[derive(Copy, Clone, Debug)]
struct Explosion {
    pos: Vec2,
    radius: f32,
    force: f32,
    ttl: u16,
    initial_ttl: u16,
}

struct GameState {
    penguin: Penguin,
    level: levels::Level,
    rockets: Vec<Rocket>,
    explosions: Vec<Explosion>,
    debug_strings: Vec<String>,
}

fn init_game_state() -> GameState {
    let level = build_level();

    let penguin =
        Penguin { pos: level.start_pos(), vel: Vec2::ZERO, coyote_time: 0, rocket_cooldown: 0 };

    GameState {
        penguin,
        level,
        rockets: Vec::with_capacity(32),
        explosions: Vec::with_capacity(32),
        debug_strings: Vec::with_capacity(16),
    }
}

fn main() {
    let conf = Conf {
        window_title: "penguin".to_string(),
        window_width: 1280,
        window_height: 720,
        window_resizable: true,
        sample_count: 2,
        high_dpi: true,
        ..Default::default()
    };

    let args = match cli::parse_args() {
        Ok(cli::Args::ShowHelp) => {
            // do not open the game window
            eprintln!("{}", cli::HELP);
            std::process::exit(1);
        }
        Ok(args) => args,
        Err(e) => {
            eprintln!("Invalid command-line arguments: {e}");
            return;
        }
    };
    macroquad::Window::from_config(conf, amain(args));
}

async fn amain(args: cli::Args) {
    fn try_read_demo_movie(input_path: &std::path::Path) -> Result<demo::DemoMovie, String> {
        let mut input_file = std::fs::File::open(input_path)
            .map_err(|e| format!("Could not open input file {input_path:?}: {e}"))?;

        let mut buf = Vec::with_capacity(1 << 20);
        if let Err(e) = input_file.read_to_end(&mut buf) {
            return Err(format!("Could not read input file {input_path:?}: {e}"));
        };
        drop(input_file);

        demo::parse_movie(buf.as_ref())
            .map_err(|e| format!("Problem in demo file {input_path:?}: {e}"))
    }

    match args {
        cli::Args::ShowHelp => unreachable!(),
        cli::Args::JustPlay => {
            run_game(&mut input::MacroquadInput).await;
        }
        cli::Args::PlayDemo { input_path } => {
            let input_movie = match input_path {
                Some(path) => match try_read_demo_movie(&path) {
                    Ok(movie) => movie,
                    Err(e) => {
                        eprintln!("{e}");
                        std::process::exit(1);
                    }
                },
                None => demo::make_default_demo_movie(),
            };

            let mut device = demo::DemoPlayback::new(input_movie);
            run_game(&mut device).await;
        }
        cli::Args::RecordDemo { output_path } => {
            let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&output_path);
            let mut file = match file {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Could not open output file {output_path:?}: {e}");
                    std::process::exit(1);
                }
            };

            let mut device = demo::DemoRecorder::new(input::MacroquadInput, 0);
            run_game(&mut device).await;
            let movie = device.collect_recording();

            if let Err(e) = demo::unparse_movie(&movie, &mut file) {
                eprintln!("Failed to write demo movie to {output_path:?}: {e}");
                std::process::exit(1);
            };
            drop(file);
            println!("Wrote {output_path:?} successfully!");
        }
        cli::Args::KeepRecordingDemo { input_path, output_path } => {
            let input_movie = match try_read_demo_movie(&input_path) {
                Ok(movie) => movie,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            };

            let output_file =
                std::fs::OpenOptions::new().write(true).create_new(true).open(&output_path);
            let mut output_file = match output_file {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Could not open output file {output_path:?}: {e}");
                    std::process::exit(1);
                }
            };

            let mut device = {
                let threshold_frame = input_movie.last_frame() + 1;
                let playback = demo::DemoPlayback::new(input_movie);
                let recorder = demo::DemoRecorder::new(input::MacroquadInput, threshold_frame);
                input::ComposedInput::new(
                    playback,
                    recorder,
                    threshold_frame,
                    (Cow::Borrowed("playback"), Cow::Borrowed("recording")),
                )
            };
            run_game(&mut device).await;
            let output_movie = device.into_inner().1.collect_recording();

            if let Err(e) = demo::unparse_movie(&output_movie, &mut output_file) {
                eprintln!("Failed to write demo movie to {output_path:?}: {e}");
                std::process::exit(1);
            };
            drop(output_file);
            println!("Wrote {output_path:?} successfully!");
        }
    }
}

async fn run_game(device: &mut impl input::InputDevice) {
    let mut state = init_game_state();

    let mut time_bank: f64 = 0.0;
    let mut last_time = get_time();

    let mut frame = 0u64;

    let mut speed_up = false;

    // Handling the quit event manually allows us to save the demo recording
    prevent_quit();
    while !is_quit_requested() {
        if is_key_pressed(KeyCode::R) {
            speed_up = !speed_up;
        }

        // Frame debt logic
        let now = get_time();
        time_bank += now - last_time;
        last_time = now;
        let ups = if speed_up { UPS_FAST } else { UPS_NORMAL };
        let update_frame_time = 1.0 / ups;
        while time_bank >= update_frame_time {
            device.next_frame();
            frame += 1;
            updates::fixed_update(&mut state, device);
            time_bank -= update_frame_time;
        }

        graphics::draw_state(&state, device.look_angle_radians());

        // Debug information
        draw_text(&format!("FPS: {:03}, target_ups: {:04}", get_fps(), ups), 32., 32., 16., WHITE);
        let debug_line = &format!("frame: {}, input: {}", frame, device.device_info());
        draw_text(debug_line, 32., 48., 16., WHITE);
        let mut y = 64.0;
        for string in state.debug_strings.iter() {
            draw_text(string, 32.0, y, 16.0, WHITE);
            y += 16.0;
        }
        next_frame().await;
    }
}

mod updates {
    use super::{
        ConvexPolygon,
        Explosion,
        GameState,
        PENGUIN_RADIUS,
        Penguin,
        ROCKET_RADIUS,
        Rocket,
    };
    use crate::input::{
        Input,
        InputDevice,
    };
    use arrayvec::ArrayVec;
    use glam::{
        Vec2,
        vec2,
    };
    use macroquad::math::{
        Circle,
        Rect,
    };
    use std::f32::consts::PI;

    const ROCKET_SPEED: f32 = 3.0;
    const ROCKET_TTL: u16 = 240;
    const ROCKET_SHOOT_COOLDOWN: u16 = 45;

    const EXPLOSION_RADIUS: f32 = 48.0;
    const EXPLOSION_TTL: u16 = 10;
    const EXPLOSION_FORCE: f32 = 1.0;

    // XXX: grounding detection is currently broken, but don't fix it yet to keep
    //      the demo movie working
    const COYOTE_DURATION: u8 = 12;
    const GRAVITY: f32 = 0.03;
    const MAX_GRAVITY_SPEED: f32 = 10.;
    const MAX_WALK_SPEED: f32 = 0.8;

    const FRICTION_AIR: f32 = 0.999;
    const FRICTION_GROUND: f32 = 0.98;

    const WALK_ACCEL_GROUND: f32 = 0.04;
    const WALK_ACCEL_AIR: f32 = 0.03;

    pub fn fixed_update(state: &mut GameState, device: &dyn InputDevice) {
        state.debug_strings.clear();

        update_rockets_movement(
            &mut state.rockets,
            &mut state.explosions,
            state.level.rect_colliders(),
            state.level.poly_colliders(),
        );
        update_explosions(&mut state.explosions);
        update_penguin_movement(
            device,
            &mut state.penguin,
            state.level.rect_colliders(),
            state.level.poly_colliders(),
            &mut state.explosions,
            |debug| state.debug_strings.push(debug),
        );

        state.penguin.rocket_cooldown = state.penguin.rocket_cooldown.saturating_sub(1);
        // Spawn rocket
        if device.is_input_down(Input::Shoot) && state.penguin.rocket_cooldown == 0 {
            state.penguin.rocket_cooldown = ROCKET_SHOOT_COOLDOWN;
            let dir = Vec2::from_angle(device.look_angle_radians());
            state.rockets.push(Rocket {
                pos: state.penguin.pos,
                vel: dir * ROCKET_SPEED,
                ttl: ROCKET_TTL,
            });
        }
    }

    fn update_penguin_movement(
        device: &dyn InputDevice,
        penguin: &mut Penguin,
        rects: &[Rect],
        polygons: &[ConvexPolygon],
        explosions: &mut [Explosion],
        mut debug: impl FnMut(String),
    ) {
        let (accel, mut friction) = if penguin.coyote_time < COYOTE_DURATION {
            (WALK_ACCEL_AIR, FRICTION_AIR)
        } else {
            (WALK_ACCEL_GROUND, FRICTION_GROUND)
        };

        if penguin.vel.x.abs() < 0.1 {
            friction *= 0.8;
        }

        penguin.vel.x *= friction;
        if device.is_input_down(Input::Right) {
            if penguin.vel.x + accel < MAX_WALK_SPEED {
                penguin.vel.x += accel;
            } else if penguin.vel.x < MAX_WALK_SPEED {
                penguin.vel.x = MAX_WALK_SPEED;
            }
        }
        if device.is_input_down(Input::Left) {
            if penguin.vel.x - accel > -MAX_WALK_SPEED {
                penguin.vel.x -= accel;
            } else if penguin.vel.x > -MAX_WALK_SPEED {
                penguin.vel.x = -MAX_WALK_SPEED;
            }
        }

        if penguin.coyote_time == COYOTE_DURATION {
            penguin.vel.y = 0.;
        }
        if penguin.coyote_time < COYOTE_DURATION && penguin.vel.y < MAX_GRAVITY_SPEED {
            penguin.vel.y += GRAVITY;
        }

        // Penguin collision detection
        let penguin_circle = Circle::new(penguin.pos.x, penguin.pos.y, PENGUIN_RADIUS);
        for exp in explosions {
            let exp_circle = Circle::new(exp.pos.x, exp.pos.y, exp.radius);
            if let Some((dir, scale)) = penguin_impacts_explosion(penguin_circle, exp_circle) {
                penguin.vel += dir * exp.force * scale;
            }
        }

        let penguin_circle = Circle::new(penguin.pos.x, penguin.pos.y, PENGUIN_RADIUS);
        // TODO: I really have no idea what I'm doing when it comes to collision detection.
        //       Detecting if we're grounded seems super janky with polygons. Fix later please
        let penguin_if_it_were_to_fall = penguin_circle.offset(vec2(0.0, GRAVITY));
        let mut dv_for_grounded = Vec2::ZERO;
        let mut any_point_upwards = false;

        for rect in rects {
            if let Some(dv) = circle_impacts_rect(penguin_circle.offset(penguin.vel), *rect)
                && dv.length_squared() >= 0.001
            {
                penguin.pos += dv / 2.;

                let cancel_vec = penguin.vel.project_onto_normalized(dv.normalize_or_zero());
                penguin.vel -= cancel_vec / 2.;
            }

            if let Some(dv) = circle_impacts_rect(penguin_if_it_were_to_fall, *rect)
                && dv.length_squared() >= 0.001
            {
                dv_for_grounded += dv;

                let angle = dv.to_angle();
                any_point_upwards =
                    any_point_upwards || (-PI / 2. - 0.34 < angle && angle < -PI / 2. + 0.34);
            }
        }

        for poly in polygons {
            if let Some(dv) = circle_impacts_convex(penguin_circle.offset(penguin.vel), poly)
                && dv.length_squared() >= 0.001
            {
                penguin.pos += dv / 2.;

                let cancel_vec = penguin.vel.project_onto_normalized(dv.normalize_or_zero());
                penguin.vel -= cancel_vec / 2.;
            }

            if let Some(dv) = circle_impacts_convex(penguin_if_it_were_to_fall, poly)
                && dv.length_squared() >= 0.001
            {
                dv_for_grounded += dv;

                let angle = dv.to_angle();
                any_point_upwards =
                    any_point_upwards || (-PI / 2. - 0.67 < angle && angle < -PI / 2. + 0.67);
            }
        }

        let is_grounded = {
            let dv_angle = dv_for_grounded.to_angle();
            any_point_upwards || -PI / 2. - 0.34 < dv_angle && dv_angle < -PI / 2. + 0.34
        };

        if is_grounded {
            penguin.coyote_time = COYOTE_DURATION;
        } else {
            penguin.coyote_time = penguin.coyote_time.saturating_sub(1);
        }

        debug(format!("speed: x={:+.2}, y={:+.2}", penguin.vel.x, penguin.vel.y));

        penguin.pos += penguin.vel
    }

    fn update_rockets_movement(
        rockets: &mut Vec<Rocket>,
        explosions: &mut Vec<Explosion>,
        rects: &[Rect],
        polygons: &[ConvexPolygon],
    ) {
        let mut removed_rocket_idxs = ArrayVec::<(usize, Vec2), 8>::new();

        for rocket in rockets.iter_mut() {
            rocket.pos += rocket.vel;
        }

        for rocket in rockets.iter_mut() {
            rocket.ttl -= 1;
        }

        'outer: for (idx, rocket) in rockets.iter_mut().enumerate() {
            if rocket.ttl == 0 {
                removed_rocket_idxs.push((idx, rocket.pos));
            } else {
                let circle = Circle::new(rocket.pos.x, rocket.pos.y, ROCKET_RADIUS);
                for &rect in rects {
                    if let Some(contact) = circle_impacts_rect_alt(circle, rect) {
                        let pos = vec2(contact.point2.x, contact.point2.y);
                        removed_rocket_idxs.push((idx, pos));
                        continue 'outer;
                    }
                }

                for poly in polygons {
                    if let Some(contact) = circle_impacts_convex_alt(circle, poly) {
                        let pos = vec2(contact.point2.x, contact.point2.y);
                        removed_rocket_idxs.push((idx, pos));
                        continue 'outer;
                    }
                }
            }
        }

        for (idx, explosion_pos) in removed_rocket_idxs.into_iter().rev() {
            rockets.swap_remove(idx);
            explosions.push(Explosion {
                pos: explosion_pos,
                radius: EXPLOSION_RADIUS,
                ttl: EXPLOSION_TTL,
                initial_ttl: EXPLOSION_TTL,
                force: EXPLOSION_FORCE,
            });
        }
    }

    fn update_explosions(explosions: &mut Vec<Explosion>) {
        let mut removed_idxs = ArrayVec::<_, 8>::new();

        for (idx, exp) in explosions.iter().enumerate() {
            if exp.ttl == 0 {
                removed_idxs.push(idx);
            }
        }

        for exp in explosions.iter_mut() {
            exp.ttl = exp.ttl.wrapping_sub(1); // if this wraps, then it will be removed anyway!
        }

        for &idx in removed_idxs.iter().rev() {
            explosions.swap_remove(idx);
        }
    }

    /// If an intersection occurs, return how much to move the circle
    fn circle_impacts_rect(circle: Circle, rect: Rect) -> Option<Vec2> {
        circle_impacts_rect_alt(circle, rect).map(|c| {
            let delta = c.point1 - c.point2;
            vec2(delta.x, delta.y)
        })
    }

    /// Circle is the "second object"
    fn circle_impacts_rect_alt(circle: Circle, rect: Rect) -> Option<parry2d::query::Contact> {
        use nalgebra::{
            Isometry2,
            Vector2,
        };
        use parry2d::query;
        use parry2d::shape::{
            Ball,
            Cuboid,
        };

        let cuboid = Cuboid::new(Vector2::new(rect.w / 2., rect.h / 2.));
        let ball = Ball::new(circle.radius());

        let cuboid_pos = Isometry2::translation(rect.x + rect.w / 2., rect.y + rect.h / 2.);
        let ball_pos = Isometry2::translation(circle.x, circle.y);
        query::contact(&cuboid_pos, &cuboid, &ball_pos, &ball, 0.0).unwrap()
    }

    /// If an intersection occurs, return how much to move the circle
    fn circle_impacts_convex(circle: Circle, poly: &ConvexPolygon) -> Option<Vec2> {
        circle_impacts_convex_alt(circle, poly).and_then(|c| {
            let delta = c.point1 - c.point2;
            let rv = vec2(delta.x, delta.y);
            rv.is_finite().then_some(rv)
        })
    }

    /// Circle is the "second object"
    fn circle_impacts_convex_alt(
        circle: Circle,
        poly: &ConvexPolygon,
    ) -> Option<parry2d::query::Contact> {
        use nalgebra::Isometry2;
        use parry2d::query;
        use parry2d::shape::Ball;

        let ball = Ball::new(circle.radius());
        let ball_pos = Isometry2::translation(circle.x, circle.y);

        query::contact(&Isometry2::default(), poly, &ball_pos, &ball, 0.0).unwrap()
    }

    /// If an intersection occurs, returns a normal vector and how close
    /// the penguin was to the explosion center (1 is the closest, 0 is the farthest)
    fn penguin_impacts_explosion(penguin: Circle, exp: Circle) -> Option<(Vec2, f32)> {
        let delta = penguin.point() - exp.point();
        let dist = delta.length();

        if dist > 0.001 && dist <= penguin.radius() + exp.radius() {
            let strength = 1.0 - dist / (penguin.radius() + exp.radius());
            Some((delta / dist, strength))
        } else {
            None
        }
    }
}

mod graphics {
    use super::{
        Explosion,
        GameState,
        PENGUIN_RADIUS,
        ROCKET_RADIUS,
    };
    use macroquad::prelude::*;

    pub fn draw_state(state: &GameState, look_angle: f32) {
        let screen_size = {
            let (w, h) = miniquad::window::screen_size();
            vec2(w, h)
        };
        let viewport_offset = state.penguin.pos - screen_size / 2.;
        set_camera(&viewport_offset_to_camera(viewport_offset, screen_size));
        clear_background(DARKBLUE);
        for graphic in state.level.graphics() {
            graphic.macroquad_draw();
        }
        draw_penguin(state.penguin.pos, state.penguin.vel, Vec2::from_angle(look_angle));
        for &rocket in &state.rockets {
            draw_rocket(rocket.pos, rocket.vel);
        }
        for &explosion in &state.explosions {
            draw_explosion(explosion);
        }
        set_default_camera();
    }

    fn viewport_offset_to_camera(offset: Vec2, screen_size: Vec2) -> Camera2D {
        let [w, h] = screen_size.to_array();

        // I don't understand why, but `from_display_rect` flips the height portion by
        // default, or something like that
        Camera2D::from_display_rect(Rect { x: offset.x.round(), y: offset.y.round() + h, w, h: -h })
    }

    fn draw_rocket(pos: Vec2, vel: Vec2) {
        let dir = vel.normalize_or_zero();

        draw_triangle(
            pos - dir * 24.,
            pos - dir * 4. + dir.perp() * 3.,
            pos - dir.perp() * 3.,
            RED,
        );
        draw_triangle(
            pos - dir * 18.,
            pos - dir * 3. + dir.perp() * 2.,
            pos - dir.perp() * 2.,
            WHITE,
        );
        draw_circle(pos.x, pos.y, ROCKET_RADIUS, BLACK);
    }

    fn draw_explosion(Explosion { pos, radius, ttl, initial_ttl, .. }: Explosion) {
        draw_circle(pos.x, pos.y, radius, RED);
        draw_circle(pos.x, pos.y, (radius - 1.) * (ttl as f32) / (initial_ttl as f32), WHITE);
    }

    const PENGUINGRAY: Color = Color::new(0.15, 0.15, 0.25, 1.0);

    fn draw_penguin(pos: Vec2, vel: Vec2, eyes_dir: Vec2) {
        const RAD: f32 = PENGUIN_RADIUS;
        let (cx, cy) = (pos.x, pos.y);

        if vel.length() > 2.0 {
            let steps = ((vel.length() - 1.5) / 0.5) as u16;
            for i in 0..steps {
                let fade_factor = 1. - i as f32 / steps as f32;
                let dpos = pos - vel.normalize() * (1. + i as f32) * 4.0;
                draw_circle_lines(
                    dpos.x,
                    dpos.y,
                    RAD * 0.5 + RAD * 0.4 * fade_factor,
                    1.0,
                    WHITE.with_alpha(fade_factor * 0.3),
                );
            }
        }
        draw_circle(cx, cy, RAD, PENGUINGRAY);

        draw_ellipse(cx, cy + RAD * 0.4, RAD * 0.7, RAD * 0.4, 0.0, LIGHTGRAY);
        draw_rectangle(cx - RAD * 0.6, cy - RAD * 0.2, RAD * 1.2, RAD * 0.4, PENGUINGRAY);

        {
            let [look_x, look_y] = (eyes_dir * 2.5).to_array();
            let [vx, vy] = (vel.clamp_length_max(16.0) * 0.0125 * RAD).round().to_array();

            draw_circle(cx - RAD * 0.3 - vx, cy - RAD * 0.2 - vy, 5., WHITE);
            draw_circle(cx + RAD * 0.3 - vx, cy - RAD * 0.2 - vy, 5., WHITE);
            draw_circle(cx - RAD * 0.3 - vx + look_x, cy - RAD * 0.2 + look_y - vy, 2., BLACK);
            draw_circle(cx + RAD * 0.3 - vx + look_x, cy - RAD * 0.2 + look_y - vy, 2., BLACK);
        }

        draw_triangle(
            vec2(cx - RAD / 2.5, cy + RAD * 0.15),
            vec2(cx + RAD / 2.5, cy + RAD * 0.15),
            vec2(cx, cy + RAD * 0.5),
            ORANGE,
        );
    }
}

mod cli {
    use std::path::PathBuf;

    pub const HELP: &str = "\
penguin-game

Usage:
    penguin-game
    penguin-game demo [--input-file <path>]
    penguin-game record-demo --output-file <path>
    penguin-game keep-recording-demo --input-file <path> --output-file <path>

See the README of the project for more details.";

    #[cfg_attr(target_family = "wasm", allow(dead_code))]
    #[derive(Debug)]
    pub enum Args {
        ShowHelp,
        JustPlay,
        PlayDemo { input_path: Option<PathBuf> },
        RecordDemo { output_path: PathBuf },
        KeepRecordingDemo { input_path: PathBuf, output_path: PathBuf },
    }

    #[cfg(not(target_family = "wasm"))]
    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[error("Expected a subcommand")]
        ExpectedSubcommand,
        #[error("Unknown subcommand: {0}")]
        UnknownSubcommand(String),
        #[error("{0}")]
        PicoArgs(#[from] crate::pico_args::Error),
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn parse_args() -> Result<Args, Error> {
        fn parse_path(s: &std::ffi::OsStr) -> Result<PathBuf, &'static str> {
            Ok(s.into())
        }

        let mut pargs = crate::pico_args::Arguments::from_env();

        if pargs.contains(["-h", "--help"]) {
            return Ok(Args::ShowHelp);
        }

        let Some(subcommand) = pargs.subcommand()? else {
            if pargs.0.is_empty() {
                return Ok(Args::JustPlay);
            } else {
                return Err(Error::ExpectedSubcommand);
            }
        };

        match subcommand.as_ref() {
            "play" => Ok(Args::JustPlay),
            "demo" => {
                let input_path = pargs.opt_value_from_os_str("--input-file", parse_path)?;
                Ok(Args::PlayDemo { input_path })
            }
            "record-demo" => {
                let output_path = pargs.value_from_os_str("--output-file", parse_path)?;
                Ok(Args::RecordDemo { output_path })
            }
            "keep-recording-demo" => {
                let input_path = pargs.value_from_os_str("--input-file", parse_path)?;
                let output_path = pargs.value_from_os_str("--output-file", parse_path)?;
                Ok(Args::KeepRecordingDemo { input_path, output_path })
            }
            s => Err(Error::UnknownSubcommand(s.to_string())),
        }
    }

    #[cfg(target_family = "wasm")]
    pub fn parse_args() -> Result<Args, &'static str> {
        Ok(if crate::wasm::is_wasm_demo() {
            Args::PlayDemo { input_path: None }
        } else {
            Args::JustPlay
        })
    }
}

//
// Level stuff
// TODO: make level editor

fn make_wrapping_png_texture(png_bytes: &[u8]) -> Texture2D {
    let ctx = unsafe { get_internal_gl() }.quad_context;
    let img = image::load_from_memory_with_format(png_bytes, ImageFormat::Png).unwrap();
    let bytes = img.to_rgba8().into_raw();

    assert!(
        img.width().is_power_of_two() && img.height().is_power_of_two(),
        "WebGL doesn't support repeating non-power-of-two textures",
    );

    let texture_id = ctx.new_texture_from_data_and_format(&bytes, miniquad::TextureParams {
        width: img.width(),
        height: img.height(),
        wrap: TextureWrap::Repeat,
        ..Default::default()
    });
    Texture2D::from_miniquad_texture(texture_id)
}

fn build_level() -> levels::Level {
    let tex_bricks = make_wrapping_png_texture(include_bytes!("./assets/bricks.png"));
    let tex_bricks_dark = make_wrapping_png_texture(include_bytes!("./assets/bricks_dark.png"));
    let tex_wood = make_wrapping_png_texture(include_bytes!("./assets/wood.png"));
    let tex_wood_dark = make_wrapping_png_texture(include_bytes!("./assets/wood_dark.png"));
    let tex_arrow_left = make_wrapping_png_texture(include_bytes!("./assets/arrow_left.png"));

    let level_width: f32 = 6000.0;
    let level_height: f32 = 12000.0;

    let mut level = levels::Level::new(
        vec2(240.0, -48.0),
        &[
            // Arrow floor
            (vec2(0.0, 0.0), vec2(level_width, 32.0), tex_arrow_left.clone().into()),
            // Leftmost helper stump,
            (vec2(160.0, -48.0), vec2(48.0, 48.0), tex_wood.clone().into()),
            // Leftmost house wall
            (vec2(0.0, -360.0), vec2(64.0, 360.0), tex_bricks.clone().into()),
            (vec2(-24.0, -1200.0), vec2(24.0, 1200.0), tex_bricks.clone().into()),
            // First bridge platform
            (vec2(400.0, -596.0), vec2(432.0, 72.0), tex_bricks.clone().into()),
            // Second bridge platform
            (vec2(1000.0, -792.0), vec2(432.0, 72.0), tex_bricks.clone().into()),
            // Third bridge platform
            (vec2(2000.0, -792.0), vec2(240.0, 72.0), tex_bricks.clone().into()),
            // Fourth bridge platform
            (vec2(2700.0, -960.0), vec2(120.0, 72.0), tex_bricks.clone().into()),
            // Fifth bridge platform
            (vec2(2000.0, -1248.0), vec2(432.0, 72.0), tex_bricks.clone().into()),
            // Tower1 walls
            (vec2(2000.0, -2400.0), vec2(12.0, 1200.0), tex_bricks.clone().into()),
            (vec2(2420.0, -2400.0), vec2(12.0, 1060.0), tex_bricks.clone().into()),
            // Tower1 crazy ledge
            (vec2(2156.0, -2200.0), vec2(120.0, 12.0), tex_bricks.clone().into()),
            // Tower1 teeny weeny legs
            (vec2(1978.0, -2400.0), vec2(22.0, 12.0), tex_bricks.clone().into()),
            (vec2(2432.0, -2400.0), vec2(22.0, 12.0), tex_bricks.clone().into()),
            // Tower1 not so crazy ledge
            (vec2(2156.0, -2720.0), vec2(120.0, 12.0), tex_bricks.clone().into()),
        ],
        &[
            // Leftmost house roof
            (
                &[vec2(0.0, -360.0), vec2(0.0, -480.0), vec2(64.0, -480.0), vec2(120.0, -360.0)],
                tex_wood.clone().into(),
            ),
            // Tower roof
            (
                &[
                    vec2(1952.0, -2500.0),
                    vec2(1964.0, -2500.0),
                    vec2(2216.0, -3000.0),
                    vec2(2216.0, -3024.0),
                ],
                tex_wood.clone().into(),
            ),
            (
                &[
                    vec2(2216.0, -3000.0),
                    vec2(2216.0, -3024.0),
                    vec2(2492.0, -2500.0),
                    vec2(2480.0, -2500.0),
                ],
                tex_wood.clone().into(),
            ),
        ],
    );
    level.insert_colliders(
        &[
            (vec2(-24.0, -level_height), vec2(24.0, level_height)),
            (vec2(level_width, -level_height), vec2(24.0, level_height)),
        ],
        &[],
    );
    level.insert_graphics(
        &[
            // First bridge pillars
            (vec2(400.0, -524.0), vec2(48.0, 524.0), tex_bricks_dark.clone().into()),
            (vec2(592.0, -524.0), vec2(48.0, 524.0), tex_bricks_dark.clone().into()),
            (vec2(784.0, -524.0), vec2(48.0, 524.0), tex_bricks_dark.clone().into()),
            // Second bridge pillars
            (vec2(1000.0, -720.0), vec2(48.0, 720.0), tex_bricks_dark.clone().into()),
            (vec2(1192.0, -720.0), vec2(48.0, 720.0), tex_bricks_dark.clone().into()),
            (vec2(1384.0, -720.0), vec2(48.0, 720.0), tex_bricks_dark.clone().into()),
            // Third bridge pillars
            (vec2(2000.0, -720.0), vec2(48.0, 720.0), tex_bricks_dark.clone().into()),
            (vec2(2192.0, -720.0), vec2(48.0, 720.0), tex_bricks_dark.clone().into()),
            // Fourth bridge pillars
            (vec2(2736.0, -888.0), vec2(48.0, 888.0), tex_bricks_dark.clone().into()),
            // Fifth bridge pillars
            (vec2(2000.0, -1248.0), vec2(48.0, 1248.0), tex_bricks_dark.clone().into()),
            (vec2(2384.0, -1248.0), vec2(48.0, 1248.0), tex_bricks_dark.clone().into()),
            // Tower1 sign1
            (vec2(1800.0, -1600.0), vec2(200.0, 24.0), tex_wood_dark.clone().into()),
            (vec2(1800.0, -1640.0), vec2(60.0, 120.0), tex_wood.clone().into()),
        ],
        &[(
            &[vec2(1770.0, -1640.0), vec2(1830.0, -1720.0), vec2(1890.0, -1640.0)],
            tex_wood.clone().into(),
        )],
    );

    level
}
