use macroquad::prelude::*;
use miniquad::TextureWrap;
use parry2d::shape::ConvexPolygon;
use std::{
    collections::{
        HashMap,
        VecDeque,
    },
    fmt::Write,
    rc::Rc,
};

use crate::{
    compat::performance_timer,
    draw_utils::draw_text_bold,
    input::InputDevice,
    level_parsing,
    levels,
};

const UPS_PRESETS: [f64; 12] =
    [1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 240.0, 480.0, 1200.0, 2400.0, 12000.0, 24000.0];

const INVERSE_UPS: [f64; UPS_PRESETS.len()] = {
    // UPS_PRESETS.map(f64::recip)
    let mut rv = UPS_PRESETS;
    let mut i = 0;
    while i < UPS_PRESETS.len() {
        rv[i] = rv[i].recip();
        i += 1;
    }
    rv
};

const DEFAULT_UPS_PRESET: usize = 6;

#[allow(clippy::float_cmp_const)]
const _: () = assert!(UPS_PRESETS[DEFAULT_UPS_PRESET] == 240.0, "");

const PENGUIN_RADIUS: f32 = 24.0;
const ROCKET_RADIUS: f32 = 4.0;

const FUEL_MAX: u16 = 240;
const FUEL_ROCKET_COST: u16 = 100;

const TOOLTIP_TTL_MAX: u16 = 300;
const TOOLTIP_TTL_GROW_RATE: u16 = 9;
const TOOLTIP_TTL_FADE_BEGIN: u16 = 120;
const _: () = assert!(TOOLTIP_TTL_FADE_BEGIN < TOOLTIP_TTL_MAX, "");

const STATUS_TTL_MAX: u16 = 360;
const STATUS_TTL_FADE_BEGIN: u16 = 180;
const _: () = assert!(STATUS_TTL_FADE_BEGIN < STATUS_TTL_MAX, "");

#[derive(Default, Clone, Debug)]
pub enum LevelSource {
    #[default]
    Default,
    New,
    Big,
}

fn load_level_from_disk(source: &LevelSource) -> Vec<u8> {
    let path = match source {
        LevelSource::Default => "../levels/default.bin",
        LevelSource::New => "../levels/new.bin",
        LevelSource::Big => "../levels/big.bin",
    };
    let path = std::path::PathBuf::from(file!()).parent().unwrap().join(path);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read path {}: {}", path.display(), e))
}

fn load_bundled_level(source: &LevelSource) -> &'static [u8] {
    match source {
        LevelSource::Default => include_bytes!("../levels/default.bin"),
        LevelSource::New => include_bytes!("../levels/new.bin"),
        LevelSource::Big => include_bytes!("../levels/big.bin"),
    }
}

pub async fn run_game(
    device: &mut dyn InputDevice,
    level_source: LevelSource,
    skip_until_update: u64,
) {
    let textures = load_textures();

    let level_src = load_bundled_level(&level_source);
    let raw_level = level_parsing::parse(level_src).expect("bundled level is corrupted");

    let level = levels::build_level(raw_level, &textures).expect("bundled level is invalid");
    let mut state = GameState::new(level);

    loop {
        match game_loop(device, &mut state, skip_until_update).await {
            GameResult::Stop => break,
            GameResult::ReloadLevel => {
                let level_src = load_level_from_disk(&level_source);
                let raw_level =
                    level_parsing::parse(&level_src).expect("bundled level is corrupted");
                let level =
                    levels::build_level(raw_level, &textures).expect("bundled level is invalid");
                state = GameState::new(level);
            }
        }
    }

    macroquad::logging::warn!("Closing RJP window");
}

#[must_use]
enum GameResult {
    Stop,
    ReloadLevel,
}

async fn game_loop(
    device: &mut dyn InputDevice,
    state: &mut GameState,
    skip_until_update: u64,
) -> GameResult {
    macroquad::logging::info!("Initialized RJP state!");

    let mut time_bank: f64 = 0.0;

    let mut update_number = 0u64;
    let mut frame_number = 0u64;

    let mut stats = Stats::new();

    let mut ups_index: usize = if skip_until_update == 0 {
        DEFAULT_UPS_PRESET
    } else {
        0 // slowest
    };

    while update_number < skip_until_update {
        // TODO: duplication with main loop?
        device.next_update();
        update_number += 1;
        updates::fixed_update(state, device);
    }

    let mut last_time = performance_timer();
    // Handling the quit event manually allows us to save the demo recording
    prevent_quit();
    while !is_quit_requested() {
        if cfg!(not(target_family = "wasm"))
            && is_key_down(KeyCode::LeftControl)
            && is_key_pressed(KeyCode::R)
            && update_number != skip_until_update
        {
            // if we don't check `update_number`, we enter an infinite loop
            return GameResult::ReloadLevel;
        }

        if is_key_pressed(KeyCode::Q) && ups_index > 0 {
            ups_index -= 1;
        } else if is_key_pressed(KeyCode::E) && ups_index < UPS_PRESETS.len() - 1 {
            ups_index += 1;
        }
        let ups = UPS_PRESETS[ups_index];
        let inverse_ups = INVERSE_UPS[ups_index];

        // Update debt logic
        let now = performance_timer();
        time_bank += now - last_time;
        last_time = now;
        if time_bank >= 0.5 && ups > 2.0 {
            // We "bankrupt" the time bank and assume we have 1 update left to do.
            // This can happen due to several reasons:
            // - lag spikes in other programs
            // - using very high UPS (like when pressing R) using a debug build and a low end device
            // - on Linux I only get one update per second when the application is minimized
            // and we don't want to run a million updates in a single frame
            time_bank = inverse_ups;
        }

        let times = (time_bank * ups).trunc() as u32;
        stats.measure_update(times, || {
            for _ in 0..times {
                device.next_update();
                updates::fixed_update(state, device);
            }
        });
        update_number += u64::from(times);
        time_bank -= inverse_ups * f64::from(times);

        let fps = get_fps();
        let stats_line = format!(
            "perf:{} up:{} fr:{} dev:{}",
            stats,
            update_number,
            frame_number,
            device.device_info(),
        );
        let fps_line = format!("FPS: {fps:03}, target UPS: {ups:04}");

        let font_size = 16.0;
        stats.measure_graphics(|| {
            graphics::draw_state(state, device.look_angle_radians());
            let mut y = font_size * 1.25;
            y += draw_text_bold(&fps_line, 8.0, y, font_size, BLACK).height + 2.0;

            y += draw_text_bold(&stats_line, 8.0, y, font_size, BLACK).height + 2.0;
            for string in &state.debug_strings {
                for line in string.lines() {
                    y += draw_text_bold(line, 8.0, y, font_size, BLACK).height + 2.0;
                }
            }
        });
        next_frame().await;
        frame_number += 1;
        if frame_number.is_multiple_of((fps / 4) as u64) {
            stats.sample();
        }
    }

    GameResult::Stop
}

#[derive(Clone, Debug)]
struct Penguin {
    pos: Vec2,
    vel: Vec2,
    rocket_cooldown: u16,
    fuel: u16,
    is_grounded: bool,
    has_eyepatch: bool,
    status: levels::StatusIcon,
    status_ttl: u16,
}

impl Penguin {
    fn circle(&self) -> Circle {
        Circle::new(self.pos.x, self.pos.y, PENGUIN_RADIUS)
    }
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

#[derive(Clone, Debug)]
struct Tooltip {
    text: Option<Rc<str>>,
    ttl: u16,
    origin: Vec2,
}

impl Tooltip {
    /// Get shared reference to text (empty if text is missing)
    fn text(&self) -> &str {
        match &self.text {
            Some(rc) => rc,
            None => "",
        }
    }
}

struct GameState {
    penguin: Penguin,
    level: levels::Level,
    rockets: Vec<Rocket>,
    explosions: Vec<Explosion>,
    debug_strings: Vec<String>,
    tooltip: Tooltip,
}

impl std::fmt::Debug for GameState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameState")
            .field("penguin", &self.penguin)
            .field("rockets", &self.rockets)
            .field("explosions", &self.explosions)
            .field("debug_strings", &self.debug_strings)
            .field("tooltip", &self.tooltip)
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
            has_eyepatch: false,
            status: levels::StatusIcon::Wrong,
            status_ttl: 0,
        };

        GameState {
            penguin,
            level,
            rockets: Vec::with_capacity(32),
            explosions: Vec::with_capacity(32),
            debug_strings: Vec::with_capacity(16),
            tooltip: Tooltip { text: None, ttl: 0, origin: vec2(0.0, 0.0) },
        }
    }
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

    fn measure_update(&mut self, times: u32, f: impl FnOnce()) {
        if times == 0 {
            return;
        }

        let start = performance_timer();
        f();
        let delta = (performance_timer() - start) / f64::from(times);
        let subtract = self.update_buffer.pop_back().unwrap();
        self.update_total -= subtract;
        self.update_total += delta;

        self.update_buffer.push_front(delta);
    }

    fn measure_graphics(&mut self, f: impl FnOnce()) {
        let start = performance_timer();
        f();
        let delta = performance_timer() - start;

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
        ROCKET_RADIUS,
        Rocket,
    };
    use crate::{
        game::{
            FUEL_MAX,
            FUEL_ROCKET_COST,
            STATUS_TTL_MAX,
            TOOLTIP_TTL_GROW_RATE,
            TOOLTIP_TTL_MAX,
            circle_impacts_rect,
            circle_impacts_rect_alt,
        },
        input::{
            Input,
            InputDevice,
        },
        levels,
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
    pub(super) const ROCKET_TTL: u16 = 200;
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
        update_rockets_movement(state);
        update_penguin_movement(state, device);
        update_shooting(state, device);
        apply_penguin_triggers(state);

        update_ui(state);
    }

    fn update_ui(state: &mut GameState) {
        state.tooltip.ttl = state.tooltip.ttl.saturating_sub(1);
        state.penguin.status_ttl = state.penguin.status_ttl.saturating_sub(1);
    }

    fn update_shooting(state: &mut GameState, device: &dyn InputDevice) {
        state.penguin.rocket_cooldown = state.penguin.rocket_cooldown.saturating_sub(1);
        // Spawn rocket
        if state.penguin.fuel >= FUEL_ROCKET_COST
            && state.penguin.rocket_cooldown == 0
            && device.is_input_down(Input::Shoot)
            && (!state.penguin.has_eyepatch || state.rockets.is_empty())
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

    fn update_penguin_movement(state: &mut GameState, device: &dyn InputDevice) {
        let penguin = &mut state.penguin;

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
        let (rects, polygons) = {
            let mut aabb_circle = penguin.circle();
            aabb_circle.scale(2.0);
            state.level.collision_candidates_at(circle_aabb(aabb_circle))
        };

        state.debug_strings.extend_from_slice(&[
            format!(
                "speed: x={:+.2}, y={:+.2}, g={}",
                penguin.vel.x, penguin.vel.y, penguin.is_grounded
            ),
            format!("collision candidates: rects={}, polygons={}", rects.len(), polygons.len()),
        ]);

        for exp in &mut state.explosions {
            let exp_circle = Circle::new(exp.pos.x, exp.pos.y, exp.radius);
            if let Some((dir, scale)) = penguin_impacts_explosion(penguin.circle(), exp_circle) {
                penguin.vel += dir * exp.force * scale;
            }
        }

        let penguin_circle = Circle::new(penguin.pos.x, penguin.pos.y, PENGUIN_RADIUS);
        let penguin_if_it_were_to_fall = penguin_circle.offset(penguin.vel + vec2(0.0, GRAVITY));
        let mut dv_for_grounded = Vec2::ZERO; // how much we moved, as far as grounding logic is concerned
        let mut any_delta_points_upwards = false; // has any of the collisions pushed us upwards?

        for rect in rects {
            if let Some(dv) = circle_impacts_rect(penguin_circle.offset(penguin.vel), rect) {
                penguin.pos += dv;

                if dv.length_squared() > 1e-3 {
                    let cancel_vec = penguin.vel.project_onto(dv);
                    penguin.vel -= cancel_vec / 1.5;
                }
            }

            if let Some(dv) = circle_impacts_rect(penguin_if_it_were_to_fall, rect) {
                dv_for_grounded += dv;

                let angle = dv.to_angle();
                any_delta_points_upwards =
                    any_delta_points_upwards || (-PI / 2. - 0.1 < angle && angle < -PI / 2. + 0.1);
            }
        }

        for poly in polygons {
            if let Some(dv) = circle_impacts_convex(penguin_circle.offset(penguin.vel), poly) {
                penguin.pos += dv;

                if dv.length_squared() > 1e-3 {
                    let cancel_vec = penguin.vel.project_onto(dv);
                    penguin.vel -= cancel_vec / 1.5;
                }
            }

            if let Some(dv) = circle_impacts_convex(penguin_if_it_were_to_fall, poly) {
                dv_for_grounded += dv;

                let angle = dv.to_angle();
                any_delta_points_upwards =
                    any_delta_points_upwards || (-PI / 2. - 0.1 < angle && angle < -PI / 2. + 0.1);
            }
        }

        let is_grounded = {
            let dv_angle = dv_for_grounded.to_angle();
            dv_for_grounded.length_squared() > 1e-4
                && (any_delta_points_upwards
                    || (-PI / 2. - 0.1 < dv_angle && dv_angle < -PI / 2. + 0.1))
        };
        penguin.is_grounded = is_grounded;

        penguin.pos += penguin.vel;
    }

    fn update_rockets_movement(state: &mut GameState) {
        for rocket in &mut state.rockets {
            rocket.pos += rocket.vel;
        }

        'outer: for rocket in &mut state.rockets {
            let circle = Circle::new(rocket.pos.x, rocket.pos.y, ROCKET_RADIUS);

            let mut aabb_circle = circle;
            aabb_circle.scale(2.0);
            let (rects, polygons) = state.level.collision_candidates_at(circle_aabb(aabb_circle));

            for rect in rects {
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

    fn apply_penguin_triggers(state: &mut GameState) {
        let triggers = state.level.triggers_at(state.penguin.circle().offset(state.penguin.vel));

        for (kind, poly) in triggers {
            match kind {
                levels::TriggerKind::Panic => {
                    panic!(
                        "You have entered a Panic trigger. This is not a bug. Game state: {state:#?}"
                    )
                }
                levels::TriggerKind::Hello => {
                    state.debug_strings.push(format!("Hello from {poly:?}"));
                }
                levels::TriggerKind::ShowText(text) => {
                    let center_x = parry2d::utils::center(poly.points()).x;
                    let min_y = poly.points().iter().map(|p| p.y).min_by(f32::total_cmp).unwrap();

                    state.tooltip.text = Some(text);
                    state.tooltip.origin = vec2(center_x, min_y);
                    state.tooltip.ttl =
                        (state.tooltip.ttl + TOOLTIP_TTL_GROW_RATE).min(TOOLTIP_TTL_MAX);
                }
                levels::TriggerKind::SetEyepatch(yes) => {
                    state.penguin.has_eyepatch = yes;
                }
                levels::TriggerKind::Goto(new_pos, status_icon) => {
                    state.penguin.pos = new_pos;
                    state.penguin.vel = vec2(0.0, 0.0);
                    state.penguin.status = status_icon;
                    state.penguin.status_ttl = STATUS_TTL_MAX;
                }
            }
        }
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

    fn circle_aabb(circle: Circle) -> Rect {
        Rect::new(circle.x - circle.r, circle.y - circle.r, circle.r * 2.0, circle.r * 2.0)
    }
}

mod graphics {
    use super::{
        Explosion,
        GameState,
        PENGUIN_RADIUS,
        ROCKET_RADIUS,
    };
    use crate::{
        draw_utils::{
            draw_rounded_rect,
            draw_vclipped_circle,
            measure_multiline_text,
        },
        game::{
            Penguin,
            STATUS_TTL_FADE_BEGIN,
            TOOLTIP_TTL_FADE_BEGIN,
            Tooltip,
            circle_impacts_rect,
        },
        levels::StatusIcon,
    };
    use macroquad::prelude::*;

    const BG_COLOR: Color = Color::new(0.7, 0.8, 0.9, 1.0);

    pub fn draw_state(state: &GameState, look_angle: f32) {
        push_camera_state();
        let screen_size = {
            let (w, h) = miniquad::window::screen_size();
            vec2(w, h)
        };
        let viewport_offset = state.penguin.pos - screen_size / 2.;
        set_camera(&viewport_offset_to_camera(viewport_offset, screen_size));
        clear_background(BG_COLOR);

        let rect = Rect::new(viewport_offset.x, viewport_offset.y, screen_size.x, screen_size.y);
        state.level.macroquad_draw(rect);
        draw_tooltip(viewport_offset, &state.tooltip, &state.penguin);

        draw_penguin(&state.penguin, Vec2::from_angle(look_angle), !state.rockets.is_empty());
        for &rocket in &state.rockets {
            let factor = f32::from(rocket.ttl) / f32::from(super::updates::ROCKET_TTL);
            draw_rocket(rocket.pos, rocket.vel, factor);
        }
        for &explosion in &state.explosions {
            draw_explosion(explosion);
        }
        pop_camera_state();
    }

    fn draw_tooltip(offset: Vec2, tooltip: &Tooltip, penguin: &Penguin) {
        const FONT_SIZE: u16 = 32;
        let text = tooltip.text();

        if tooltip.ttl == 0 {
            return;
        }
        let alpha = (f32::from(tooltip.ttl) / f32::from(TOOLTIP_TTL_FADE_BEGIN)).clamp(0.0, 1.0);

        let default_anchor = tooltip.origin.round() - vec2(0.0, 4.0);

        // `measure_text` doesn't handle multiline text. Argh!
        let text_dims = measure_multiline_text(text, FONT_SIZE);
        let padding = vec2(6.0, 6.0);

        let compute_pos_and_wh = |anchor| {
            let top_left = anchor - vec2(text_dims.width / 2.0, text_dims.height) - padding * 2.0;
            let dimensions = vec2(text_dims.width, text_dims.height) + padding * 2.0;
            (top_left, dimensions)
        };

        let (mut top_left, dimensions) = compute_pos_and_wh(default_anchor);

        // move tooltip away if it would intersect with the penguin
        let dpos = {
            let rect = Rect::new(top_left.x, top_left.y, dimensions.x, dimensions.y);
            let circle = Circle::new(penguin.pos.x, penguin.pos.y, PENGUIN_RADIUS + 8.0);
            circle_impacts_rect(circle, rect)
        };
        if let Some(mut dpos) = dpos {
            dpos = dpos.round();
            top_left -= dpos;
            draw_line(
                default_anchor.x,
                default_anchor.y,
                default_anchor.x - dpos.x,
                default_anchor.y - dpos.y,
                2.0,
                BLACK.with_alpha(alpha),
            );
        }

        draw_rounded_rect(top_left, dimensions, 6.0, WHITE.with_alpha(alpha));

        // needed to ensure that the text is rendered at an integral pixel boundary
        let fraction = offset.fract_gl();

        draw_multiline_text(
            text,
            top_left.x + padding.x - fraction.x,
            top_left.y + padding.y + text_dims.offset_y - fraction.y,
            f32::from(FONT_SIZE),
            None,
            BLACK.with_alpha(alpha),
        );
    }

    fn viewport_offset_to_camera(offset: Vec2, screen_size: Vec2) -> Camera2D {
        let [w, h] = screen_size.to_array();

        // I don't understand why, but `from_display_rect` flips the height portion by
        // default, or something like that
        Camera2D::from_display_rect(Rect { x: offset.x, y: offset.y + h, w, h: -h })
    }

    fn draw_rocket(pos: Vec2, vel: Vec2, ttl_frac: f32) {
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

        let bulb_color = Color::new(1.0 - ttl_frac, 1.0 - ttl_frac, 1.0 - ttl_frac, 1.0);
        draw_circle(pos.x, pos.y, ROCKET_RADIUS + 2.0, BLACK.with_alpha((1.0 - ttl_frac).powi(2)));
        draw_circle(pos.x, pos.y, ROCKET_RADIUS, bulb_color);
    }

    fn draw_explosion(Explosion { pos, radius, ttl, initial_ttl, .. }: Explosion) {
        draw_circle(pos.x, pos.y, radius, RED);
        draw_circle(pos.x, pos.y, (radius - 1.) * f32::from(ttl) / f32::from(initial_ttl), WHITE);
    }

    const PENGUINGRAY: Color = Color::new(0.12, 0.12, 0.22, 1.0);
    const PENGUINGRAY_EMPTY: Color = Color::new(0.4, 0.4, 0.5, 0.8);
    const DARKRED: Color = Color::new(0.7, 0.0, 0.2, 1.0);

    fn draw_penguin(penguin: &Penguin, eyes_dir: Vec2, any_rockets: bool) {
        #[expect(clippy::float_cmp_const)]
        const {
            assert!(PENGUIN_RADIUS == 24.0, "");
        }

        let Penguin { pos, vel, fuel, has_eyepatch, status, status_ttl, .. } = penguin;
        let (cx, cy) = (pos.x, pos.y);

        // Draw trail when moving at high speed
        if vel.length() > 1.8 {
            let steps = ((vel.length() - 1.5) / 0.33).min(100.0) as u16;
            for i in 0..steps {
                let fade_factor = 1. - f32::from(i) / f32::from(steps);
                let dpos = *pos - vel.normalize() * (1. + f32::from(i)) * 4.0;
                draw_circle_lines(
                    dpos.x,
                    dpos.y,
                    12.0 + 10.0 * fade_factor,
                    1.0,
                    WHITE.with_alpha(fade_factor * 0.3),
                );
            }
        }

        // Draw body
        {
            let fill_fraction = f32::from(*fuel) / f32::from(super::FUEL_MAX);
            draw_circle(cx, cy, 24.0, PENGUINGRAY_EMPTY);
            draw_circle(cx, cy, 16.0, PENGUINGRAY);
            draw_vclipped_circle(cx, cy, 24.0, fill_fraction, PENGUINGRAY);
            draw_circle_lines(cx, cy, 22.0, 2.0, PENGUINGRAY);

            draw_ellipse(cx, cy + 10.0, 16.0, 10.0, 0.0, LIGHTGRAY);
            draw_rectangle(cx - 14.0, cy - 5.0, 27.0, 10.0, PENGUINGRAY);
        }

        // Draw eyes and eyepatch
        {
            let look = eyes_dir * 2.5;
            let [vx, vy] = (vel.clamp_length_max(16.0) * 0.0125 * 24.0).round().to_array();

            draw_circle(cx - 7.0 - vx, cy - 6.0 - vy, 5., WHITE);
            draw_circle(cx - 7.0 - vx + look.x, cy - 6.0 + look.y - vy, 2., BLACK);
            if *has_eyepatch {
                let color = if any_rockets { RED } else { DARKRED };
                let [px, py] = vec2(cx + 7.0 - vx, cy - 5.0 - vy).to_array();
                draw_circle(px, py, 6., color);
                draw_line(px, py, px - 16.0, py - 16.0, 5.0, color);
                draw_line(px, py, px + 16.0, py + 7.0, 5.0, color);
            } else {
                draw_circle(cx + 7.0 - vx, cy - 6.0 - vy, 5., WHITE);
                draw_circle(cx + 7.0 - vx + look.x, cy - 6.0 + look.y - vy, 2., BLACK);
            }
        }

        draw_status_icon(*pos - vec2(0.0, 29.0), *status_ttl, *status);

        // Beak
        draw_triangle(
            vec2(cx - 10.0, cy + 4.0),
            vec2(cx + 10.0, cy + 4.0),
            vec2(cx, cy + 12.0),
            ORANGE,
        );
    }

    fn draw_status_icon(pos: Vec2, ttl: u16, icon: StatusIcon) {
        if ttl == 0 {
            return;
        }

        let alpha = f32::from(ttl.min(STATUS_TTL_FADE_BEGIN)) / f32::from(STATUS_TTL_FADE_BEGIN);

        match icon {
            StatusIcon::Wrong => {
                let color = RED.with_alpha(alpha);
                draw_triangle_lines(
                    pos - vec2(10.0, 0.0),
                    pos + vec2(10.0, 0.0),
                    pos - vec2(0.0, 18.0),
                    2.0,
                    color,
                );
                draw_line(pos.x, pos.y - 13.0, pos.x, pos.y - 6.0, 2.0, color);
                draw_circle(pos.x, pos.y - 3.0, 2.0, color);
            }
            StatusIcon::Nice => {
                let color = LIME.with_alpha(alpha);
                draw_line(pos.x, pos.y, pos.x - 6.0, pos.y - 6.0, 4.0, color);
                draw_line(pos.x, pos.y, pos.x + 10.0, pos.y - 10.0, 4.0, color);
                draw_circle(pos.x, pos.y, 2.0, color);
            }
        }
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

// Level stuff

macro_rules! include_with_name {
    ($name:expr) => {
        ($name, include_bytes!($name))
    };
}

#[rustfmt::skip]
fn load_textures() -> HashMap<&'static str, Texture2D> {
    HashMap::from([
        ("arrow_left", include_texture(include_with_name!("../assets/arrow_left.png"), true)),
        ("barrier", include_texture(include_with_name!("../assets/barrier.png"), true)),
        ("barrier_eyepatch", include_texture(include_with_name!("../assets/barrier_eyepatch.png"), true)),
        ("barrier_no_eyepatch", include_texture(include_with_name!("../assets/barrier_no_eyepatch.png"), true)),
        ("barrier_danger", include_texture(include_with_name!("../assets/barrier_danger.png"), true)),
        ("barrier_danger_transparent", include_texture(include_with_name!("../assets/barrier_danger_transparent.png"), true)),
        ("barrier_move", include_texture(include_with_name!("../assets/barrier_move.png"), true)),
        ("bricks", include_texture(include_with_name!("../assets/bricks.png"), true)),
        ("bricks_dark", include_texture(include_with_name!("../assets/bricks_dark.png"), true)),
        ("caution", include_texture(include_with_name!("../assets/caution.png"), true)),
        ("water", include_texture(include_with_name!("../assets/water.png"), true)),
        ("white", include_texture(include_with_name!("../assets/white.png"), true)),
        ("wood", include_texture(include_with_name!("../assets/wood.png"), true)),
        ("wood_dark", include_texture(include_with_name!("../assets/wood_dark.png"), true)),

        ("crocodile4", include_texture(include_with_name!("../assets/crocodile4.png"), false)),
        ("pepper32", include_texture(include_with_name!("../assets/pepper32.png"), false)),
        ("question_mark", include_texture(include_with_name!("../assets/question_mark.png"), false)),
        ("question_mark32", include_texture(include_with_name!("../assets/question_mark32.png"), false)),
    ])
}

fn include_texture((path, png_bytes): (&str, &[u8]), repeating: bool) -> Texture2D {
    let img = image::load_from_memory_with_format(png_bytes, ImageFormat::Png)
        .unwrap_or_else(|e| panic!("Could not read PNG from {path}: {e}"));
    let bytes = img.to_rgba8().into_raw();

    let wrap = if repeating {
        assert!(
            img.width().is_power_of_two() && img.height().is_power_of_two(),
            "Repeating textures must be a power of two on WebGL. But the size of {} is {}x{}",
            path,
            img.width(),
            img.height()
        );
        TextureWrap::Repeat
    } else {
        // WebGL doesn't support repeating non-power-of-two textures
        TextureWrap::Clamp
    };

    // SAFETY: internal context does not escape this function
    let ctx = unsafe { get_internal_gl() }.quad_context;
    let texture_id = ctx.new_texture_from_data_and_format(&bytes, miniquad::TextureParams {
        width: img.width(),
        height: img.height(),
        wrap,
        ..Default::default()
    });
    Texture2D::from_miniquad_texture(texture_id)
}
