use std::{
    collections::{
        HashMap,
        VecDeque,
    },
    fmt::Write,
};

use macroquad::prelude::*;
use miniquad::TextureWrap;
use parry2d::shape::ConvexPolygon;

use crate::{
    draw_utils::DrawOpts,
    levels,
};

const UPS_NORMAL: f64 = 240.;
const UPS_FAST: f64 = 1200.;

const PENGUIN_RADIUS: f32 = 24.0;
const ROCKET_RADIUS: f32 = 4.0;

const FUEL_MAX: u16 = 240;
const FUEL_ROCKET_COST: u16 = 100;

#[derive(Copy, Clone, Debug)]
struct Penguin {
    pos: Vec2,
    vel: Vec2,
    rocket_cooldown: u16,
    fuel: u16,
    is_grounded: bool,
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
    level: crate::levels::Level,
    rockets: Vec<Rocket>,
    explosions: Vec<Explosion>,
    debug_strings: Vec<String>,
}

impl std::fmt::Debug for GameState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameState")
            .field("penguin", &self.penguin)
            .field("rockets", &self.rockets)
            .field("explosions", &self.explosions)
            .field("debug_strings", &self.debug_strings)
            .finish_non_exhaustive()
    }
}

impl GameState {
    fn new(level: levels::Level) -> GameState {
        let penguin = Penguin {
            pos: level.start_pos(),
            vel: Vec2::ZERO,
            is_grounded: false,
            fuel: FUEL_MAX,
            rocket_cooldown: 0,
        };

        GameState {
            penguin,
            level,
            rockets: Vec::with_capacity(32),
            explosions: Vec::with_capacity(32),
            debug_strings: Vec::with_capacity(16),
        }
    }
}

pub async fn run_game(device: &mut impl crate::input::InputDevice, new_level: bool) {
    let builder = levels::LevelBuilder::new(load_textures());
    let level = if new_level {
        // this is very MVP quality...
        crate::generated_levels::level_test::build(builder)
    } else {
        build_default_level(builder)
    };
    let mut state = GameState::new(level);

    macroquad::logging::info!("Initialized penguin-game state!");

    let mut time_bank: f64 = 0.0;
    let mut last_time = get_time();

    let mut update_number = 0u64;
    let mut frame_number = 0u64;
    let mut speed_up = false;

    let mut stats = Stats::new();

    // Handling the quit event manually allows us to save the demo recording
    prevent_quit();
    while !is_quit_requested() {
        if is_key_pressed(KeyCode::R) {
            speed_up = !speed_up;
        }

        #[expect(clippy::manual_assert)]
        if is_key_down(miniquad::KeyCode::P) {
            panic!("You pressed P, the Panic button. Game state: {state:#?}");
        }

        // Update debt logic
        let now = get_time();
        time_bank += now - last_time;
        last_time = now;
        let ups = if speed_up { UPS_FAST } else { UPS_NORMAL };
        let update_time = 1.0 / ups;
        if time_bank >= update_time * 60.0 {
            // We "bankrupt" the time bank and assume we have 1 update left to do.
            // This can happen due to several reasons:
            // - lag spikes in other programs
            // - using very high UPS (like when pressing R) using a debug build and a low end device
            // - on Linux I only get one update per second when the application is minimized
            // and we don't want to run a million updates in a single frame
            time_bank = update_time;
        }

        stats.measure_update(|| {
            while time_bank >= update_time {
                device.next_update();
                update_number += 1;
                updates::fixed_update(&mut state, device);
                time_bank -= update_time;
            }
        });

        let fps = get_fps();
        let stats_line = &format!(
            "up:{} fr:{} dev:{} perf:{}",
            update_number,
            frame_number,
            device.device_info(),
            stats
        );
        stats.measure_graphics(|| {
            graphics::draw_state(&state, device.look_angle_radians());
            draw_text(&format!("FPS: {fps:03}, target UPS: {ups:04}"), 32., 32., 16., WHITE);
            draw_text(stats_line, 32., 48., 16., WHITE);
            let mut y = 64.0;
            for string in &state.debug_strings {
                draw_text(string, 32.0, y, 16.0, WHITE);
                y += 16.0;
            }
        });
        next_frame().await;
        frame_number += 1;
        #[expect(clippy::cast_sign_loss)]
        if frame_number.is_multiple_of((fps / 4) as u64) {
            stats.sample();
        }
    }

    macroquad::logging::warn!("Closing penguin-game window");
}

struct Stats {
    update_total: f64,
    update_buffer: VecDeque<f64>,
    graphics_total: f64,
    graphics_buffer: VecDeque<f64>,
    display: String,
}

impl Stats {
    const BUFFER_LEN: usize = 60;

    fn new() -> Stats {
        Stats {
            update_total: 0.0,
            update_buffer: VecDeque::from_iter([0.0; Self::BUFFER_LEN]),
            graphics_total: 0.0,
            graphics_buffer: VecDeque::from_iter([0.0; Self::BUFFER_LEN]),
            display: String::with_capacity(64),
        }
    }

    fn measure_update(&mut self, f: impl FnOnce()) {
        let start = get_time();
        f();
        let delta = get_time() - start;
        let subtract = self.update_buffer.pop_back().unwrap();
        self.update_total -= subtract;
        self.update_total += delta;

        self.update_buffer.push_front(delta);
    }

    fn measure_graphics(&mut self, f: impl FnOnce()) {
        let start = get_time();
        f();
        let delta = get_time() - start;

        let subtract = self.graphics_buffer.pop_back().unwrap();
        self.graphics_total -= subtract;
        self.graphics_total += delta;

        self.graphics_buffer.push_front(delta);
    }

    #[allow(clippy::cast_precision_loss)]
    fn sample(&mut self) {
        let update_micros = self.update_total * 1_000_000.0 / (Self::BUFFER_LEN as f64);
        let graphics_micros = self.graphics_total * 1_000_000.0 / (Self::BUFFER_LEN as f64);

        self.display.clear();
        write!(self.display, "(upd {update_micros:.2}us, draw {graphics_micros:.2}us)").unwrap();
    }
}

impl core::fmt::Display for Stats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.display)
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
    use crate::{
        game::{
            FUEL_MAX,
            FUEL_ROCKET_COST,
        },
        input::{
            Input,
            InputDevice,
        },
    };
    use arrayvec::ArrayVec;
    use core::f32::consts::PI;
    use glam::{
        Vec2,
        vec2,
    };
    use macroquad::math::{
        Circle,
        Rect,
    };

    const ROCKET_SPEED: f32 = 2.7;
    const ROCKET_TTL: u16 = 300;
    const ROCKET_SHOOT_COOLDOWN: u16 = 40;

    const EXPLOSION_RADIUS: f32 = 42.0;
    const EXPLOSION_TTL: u16 = 8;
    const EXPLOSION_FORCE: f32 = 0.85;

    const GRAVITY: f32 = 0.025;
    const MAX_GRAVITY_SPEED: f32 = 8.;
    const MAX_WALK_SPEED: f32 = 0.6;

    const FRICTION_AIR: f32 = 0.999;
    const FRICTION_GROUND: f32 = 0.98;

    const WALK_ACCEL_GROUND: f32 = 0.04;
    const WALK_ACCEL_AIR: f32 = 0.03;

    pub fn fixed_update(state: &mut GameState, device: &dyn InputDevice) {
        state.debug_strings.clear();

        update_ttl(&mut state.rockets, &mut state.explosions);
        update_rockets_movement(
            &mut state.rockets,
            state.level.rect_colliders(),
            state.level.poly_colliders(),
        );
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
        if state.penguin.fuel >= FUEL_ROCKET_COST
            && state.penguin.rocket_cooldown == 0
            && device.is_input_down(Input::Shoot)
        {
            state.penguin.rocket_cooldown = ROCKET_SHOOT_COOLDOWN;
            state.penguin.fuel -= FUEL_ROCKET_COST;
            let dir = Vec2::from_angle(device.look_angle_radians());
            state.rockets.push(Rocket {
                pos: state.penguin.pos,
                vel: dir * ROCKET_SPEED,
                ttl: ROCKET_TTL,
            });
        }

        if state.penguin.fuel < FUEL_MAX {
            state.penguin.fuel += 1;
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
        let (accel, mut friction) = if penguin.is_grounded {
            (WALK_ACCEL_GROUND, FRICTION_GROUND)
        } else {
            (WALK_ACCEL_AIR, FRICTION_AIR)
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

        if !penguin.is_grounded && penguin.vel.y < MAX_GRAVITY_SPEED {
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
        let penguin_if_it_were_to_fall = penguin_circle.offset(penguin.vel + vec2(0.0, GRAVITY));
        let mut dv_for_grounded = Vec2::ZERO; // how much we moved, as far as grounding logic is concerned
        let mut any_delta_points_upwards = false; // has any of the collisions pushed us upwards?

        for rect in rects {
            if let Some(dv) = circle_impacts_rect(penguin_circle.offset(penguin.vel), *rect) {
                penguin.pos += dv;

                if dv.length_squared() > 1e-6 {
                    let cancel_vec = penguin.vel.project_onto(dv);
                    penguin.vel -= cancel_vec / 4.;
                }
            }

            if let Some(dv) = circle_impacts_rect(penguin_if_it_were_to_fall, *rect) {
                dv_for_grounded += dv;

                let angle = dv.to_angle();
                any_delta_points_upwards = any_delta_points_upwards
                    || (-PI / 2. - 0.34 < angle && angle < -PI / 2. + 0.34);
            }
        }

        for poly in polygons {
            if let Some(dv) = circle_impacts_convex(penguin_circle.offset(penguin.vel), poly) {
                penguin.pos += dv;

                if dv.length_squared() > 1e-6 {
                    let cancel_vec = penguin.vel.project_onto(dv);
                    penguin.vel -= cancel_vec / 4.;
                }
            }

            if let Some(dv) = circle_impacts_convex(penguin_if_it_were_to_fall, poly) {
                dv_for_grounded += dv;

                let angle = dv.to_angle();
                any_delta_points_upwards = any_delta_points_upwards
                    || (-PI / 2. - 0.67 < angle && angle < -PI / 2. + 0.67);
            }
        }

        let is_grounded = {
            let dv_angle = dv_for_grounded.to_angle();
            any_delta_points_upwards || (-PI / 2. - 0.34 < dv_angle && dv_angle < -PI / 2. + 0.34)
        };
        penguin.is_grounded = is_grounded;

        debug(format!("speed: x={:+.2}, y={:+.2}", penguin.vel.x, penguin.vel.y));

        penguin.pos += penguin.vel;
    }

    fn update_rockets_movement(rockets: &mut [Rocket], rects: &[Rect], polygons: &[ConvexPolygon]) {
        for rocket in rockets.iter_mut() {
            rocket.pos += rocket.vel;
        }

        'outer: for rocket in rockets.iter_mut() {
            let circle = Circle::new(rocket.pos.x, rocket.pos.y, ROCKET_RADIUS);
            for &rect in rects {
                if circle_impacts_rect_alt(circle, rect).is_some() {
                    rocket.ttl = 0;
                    continue 'outer;
                }
            }

            for poly in polygons {
                if circle_impacts_convex_alt(circle, poly).is_some() {
                    rocket.ttl = 0;
                    continue 'outer;
                }
            }
        }
    }

    /// Update ticks on rockets and explosions. Delete expired rockets and explosions.
    /// This will also spawn extra explosions created by dying rockets.
    fn update_ttl(rockets: &mut Vec<Rocket>, explosions: &mut Vec<Explosion>) {
        for rocket in rockets.iter_mut() {
            rocket.ttl = rocket.ttl.saturating_sub(1);
        }
        for explosion in explosions.iter_mut() {
            explosion.ttl = explosion.ttl.saturating_sub(1);
        }

        let mut new_explosions = ArrayVec::<Vec2, 16>::new();
        rockets.retain_mut(|rocket| {
            if rocket.ttl == 0 {
                _ = new_explosions.try_push(rocket.pos);
            }
            rocket.ttl != 0
        });

        explosions.retain_mut(|exp| exp.ttl != 0);

        explosions.reserve_exact(new_explosions.len());
        for pos in new_explosions {
            explosions.push(Explosion {
                pos,
                radius: EXPLOSION_RADIUS,
                ttl: EXPLOSION_TTL,
                initial_ttl: EXPLOSION_TTL,
                force: EXPLOSION_FORCE,
            });
        }
    }

    /// If an intersection occurs, return how much to move the circle
    fn circle_impacts_rect(circle: Circle, rect: Rect) -> Option<Vec2> {
        let contact = circle_impacts_rect_alt(circle, rect)?;
        let delta = contact.point1 - contact.point2;
        Some(vec2(delta.x, delta.y))
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
        let contact = circle_impacts_convex_alt(circle, poly)?;
        let delta = contact.point1 - contact.point2;
        let rv = vec2(delta.x, delta.y);
        rv.is_finite().then_some(rv)
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

        (dist > 0.001 && dist <= penguin.radius() + exp.radius()).then(|| {
            let strength = 1.0 - dist / (penguin.radius() + exp.radius());
            (delta / dist, strength)
        })
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
        push_camera_state();
        let screen_size = {
            let (w, h) = miniquad::window::screen_size();
            vec2(w, h)
        };
        let viewport_offset = state.penguin.pos - screen_size / 2.;
        set_camera(&viewport_offset_to_camera(viewport_offset, screen_size));
        clear_background(DARKBLUE);

        let rect = Rect::new(viewport_offset.x, viewport_offset.y, screen_size.x, screen_size.y);
        state.level.macroquad_draw(rect);
        draw_penguin(
            state.penguin.pos,
            state.penguin.vel,
            Vec2::from_angle(look_angle),
            state.penguin.fuel,
        );
        for &rocket in &state.rockets {
            draw_rocket(rocket.pos, rocket.vel);
        }
        for &explosion in &state.explosions {
            draw_explosion(explosion);
        }
        pop_camera_state();
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
        draw_circle(pos.x, pos.y, (radius - 1.) * f32::from(ttl) / f32::from(initial_ttl), WHITE);
    }

    const PENGUINGRAY: Color = Color::new(0.12, 0.12, 0.22, 1.0);
    const PENGUINGRAY_DIM: Color = Color::new(0.25, 0.25, 0.4, 1.0);

    fn draw_penguin(pos: Vec2, vel: Vec2, eyes_dir: Vec2, fuel: u16) {
        const RAD: f32 = PENGUIN_RADIUS;
        let (cx, cy) = (pos.x, pos.y);
        let fill_fraction = f32::from(fuel) / f32::from(super::FUEL_MAX);

        // Draw trail when moving at high speed
        if vel.length() > 1.8 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let steps = ((vel.length() - 1.5) / 0.33).min(100.0) as u16;
            for i in 0..steps {
                let fade_factor = 1. - f32::from(i) / f32::from(steps);
                let dpos = pos - vel.normalize() * (1. + f32::from(i)) * 4.0;
                draw_circle_lines(
                    dpos.x,
                    dpos.y,
                    RAD * 0.5 + RAD * 0.4 * fade_factor,
                    1.0,
                    WHITE.with_alpha(fade_factor * 0.3),
                );
            }
        }
        draw_circle(cx, cy, RAD, PENGUINGRAY_DIM);
        draw_circle(cx, cy, RAD / 1.5, PENGUINGRAY);
        crate::draw_utils::draw_vclipped_circle(cx, cy, RAD, fill_fraction, PENGUINGRAY);
        draw_circle_lines(cx, cy, RAD - 1.0, 2.0, PENGUINGRAY);

        draw_ellipse(cx, cy + RAD * 0.4, RAD * 0.65, RAD * 0.4, 0.0, LIGHTGRAY);
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
//
// Level stuff
// TODO: make level editor

macro_rules! include_with_name {
    ($name:expr) => {
        ($name, include_bytes!($name))
    };
}

#[inline(never)]
#[rustfmt::skip]
fn load_textures() -> HashMap<&'static str, DrawOpts> {
    HashMap::from([
        ("arrow_left", make_wrapping_png_texture(include_with_name!("./assets/arrow_left.png")).into()),
        ("barrier", make_wrapping_png_texture(include_with_name!("./assets/barrier.png")).into()),
        ("bricks", make_wrapping_png_texture(include_with_name!("./assets/bricks.png")).into()),
        ("bricks_dark", make_wrapping_png_texture(include_with_name!("./assets/bricks_dark.png")).into()),
        ("caution", make_wrapping_png_texture(include_with_name!("./assets/caution.png")).into()),
        ("wood", make_wrapping_png_texture(include_with_name!("./assets/wood.png")).into()),
        ("wood_dark", make_wrapping_png_texture(include_with_name!("./assets/wood_dark.png")).into()),
    ])
}

fn make_wrapping_png_texture((path, png_bytes): (&str, &[u8])) -> Texture2D {
    // SAFETY: internal context does not escape this function
    let ctx = unsafe { get_internal_gl() }.quad_context;
    let img = image::load_from_memory_with_format(png_bytes, ImageFormat::Png)
        .unwrap_or_else(|e| panic!("Could not read PNG from {path}: {e}"));
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

fn build_default_level(mut builder: levels::LevelBuilder) -> levels::Level {
    builder.level_start(vec2(240.0, -48.0));

    let level_width: f32 = 6000.0;
    let level_height: f32 = 12000.0;

    // Invisible walls
    builder.rect(None, vec2(-24.0, -level_height), vec2(24.0, level_height));
    builder.rect(None, vec2(level_width, -level_height), vec2(24.0, level_height));

    // Arrow floor
    builder.rect(Some("arrow_left"), vec2(0.0, 0.0), vec2(level_width, 32.0));

    // Leftmost helper stump
    builder.rect(Some("wood"), vec2(160.0, -48.0), vec2(48.0, 48.0));

    // Leftmost house wall
    builder.rect(Some("bricks"), vec2(0.0, -300.0), vec2(64.0, 300.0));
    builder.rect(Some("bricks"), vec2(-24.0, -1200.0), vec2(24.0, 1200.0));

    // First bridge platform
    builder.rect(Some("bricks"), vec2(400.0, -536.0), vec2(432.0, 72.0));
    // First bridge pillars
    builder.rect_graphics("bricks_dark", vec2(400.0, -524.0), vec2(48.0, 524.0));
    builder.rect_graphics("bricks_dark", vec2(592.0, -524.0), vec2(48.0, 524.0));
    builder.rect_graphics("bricks_dark", vec2(784.0, -524.0), vec2(48.0, 524.0));

    // Second bridge platform
    builder.rect(Some("bricks"), vec2(1000.0, -742.0), vec2(432.0, 72.0));
    // Second bridge pillars
    builder.rect_graphics("bricks_dark", vec2(1000.0, -720.0), vec2(48.0, 720.0));
    builder.rect_graphics("bricks_dark", vec2(1192.0, -720.0), vec2(48.0, 720.0));
    builder.rect_graphics("bricks_dark", vec2(1384.0, -720.0), vec2(48.0, 720.0));

    // Third bridge platform
    builder.rect(Some("bricks"), vec2(1800.0, -792.0), vec2(240.0, 72.0));
    // Third bridge pillars
    builder.rect_graphics("bricks_dark", vec2(1800.0, -720.0), vec2(48.0, 720.0));
    builder.rect_graphics("bricks_dark", vec2(1992.0, -720.0), vec2(48.0, 720.0));

    // Fourth bridge platform
    builder.rect(Some("bricks"), vec2(2300.0, -990.0), vec2(120.0, 72.0));
    // Fourth bridge pillars
    builder.rect_graphics("bricks_dark", vec2(2336.0, -990.0), vec2(48.0, 990.0));

    // Fifth bridge platform
    builder.rect(Some("bricks"), vec2(1800.0, -1248.0), vec2(432.0, 72.0));
    // Fifth bridge pillars
    builder.rect_graphics("bricks_dark", vec2(1800.0, -1248.0), vec2(48.0, 1248.0));
    builder.rect_graphics("bricks_dark", vec2(2184.0, -1248.0), vec2(48.0, 1248.0));

    // Tower1 walls
    builder.rect(Some("bricks"), vec2(1800.0, -2350.0), vec2(12.0, 1150.0));
    builder.rect(Some("bricks"), vec2(2220.0, -2350.0), vec2(12.0, 1010.0));

    // Tower1 crazy ledge
    builder.rect(Some("bricks"), vec2(1956.0, -2180.0), vec2(120.0, 12.0));

    // Tower1 teeny weeny legs
    builder.rect(Some("bricks"), vec2(1778.0, -2350.0), vec2(22.0, 12.0));
    builder.rect(Some("bricks"), vec2(2232.0, -2350.0), vec2(22.0, 12.0));

    // Tower1 not so crazy ledge
    builder.rect(Some("bricks"), vec2(1808.0, -2600.0), vec2(268.0, 12.0));

    // Tower1 sign1
    builder.rect_graphics("wood_dark", vec2(1600.0, -1600.0), vec2(200.0, 24.0));
    builder.rect_graphics("wood", vec2(1600.0, -1640.0), vec2(60.0, 120.0));

    // Leftmost house roof
    builder.polygon(Some("wood"), &[
        vec2(0.0, -300.0),
        vec2(0.0, -420.0),
        vec2(64.0, -420.0),
        vec2(120.0, -300.0),
    ]);
    // Tower roof
    builder.polygon(Some("wood"), &[
        vec2(1752.0, -2500.0),
        vec2(1764.0, -2500.0),
        vec2(2016.0, -3000.0),
        vec2(2016.0, -3024.0),
    ]);
    builder.polygon(Some("wood"), &[
        vec2(2016.0, -3000.0),
        vec2(2016.0, -3024.0),
        vec2(2292.0, -2500.0),
        vec2(2280.0, -2500.0),
    ]);

    // Arrow sign
    builder.polygon_graphics("wood", &[
        vec2(1570.0, -1640.0),
        vec2(1630.0, -1720.0),
        vec2(1690.0, -1640.0),
    ]);
    builder.build_or_die()
}
