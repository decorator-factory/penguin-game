use arrayvec::ArrayVec;
use macroquad::prelude::*;
use miniquad::{TextureWrap, window::screen_size};
use parry2d::shape::ConvexPolygon;
use std::f32::consts::PI;

mod draw_utils;
mod levels;
mod wasm;
use levels::Level;

const UPS_NORMAL: f64 = 240.;
const UPS_FAST: f64 = 1200.;

const PENGUIN_RADIUS: f32 = 24.0;
const ROCKET_RADIUS: f32 = 4.0;

const ROCKET_SPEED: f32 = 3.0;
const ROCKET_TTL: u16 = 240;
const ROCKET_SHOOT_COOLDOWN: u16 = 45;

const EXPLOSION_RADIUS: f32 = 48.0;
const EXPLOSION_TTL: u16 = 10;
const EXPLOSION_FORCE: f32 = 1.0;

const COYOTE_DURATION: u8 = 12;
const GRAVITY: f32 = 0.03;
const MAX_GRAVITY_SPEED: f32 = 10.;
const MAX_WALK_SPEED: f32 = 0.8;

const FRICTION_AIR: f32 = 0.999;
const FRICTION_GROUND: f32 = 0.98;

const WALK_ACCEL_GROUND: f32 = 0.04;
const WALK_ACCEL_AIR: f32 = 0.03;

fn window_conf() -> Conf {
    Conf {
        window_title: "penguin".to_string(),
        window_width: 1280,
        window_height: 720,
        window_resizable: true,
        sample_count: 2,
        high_dpi: true,
        ..Default::default()
    }
}

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
    level: Level,
    rockets: Vec<Rocket>,
    explosions: Vec<Explosion>,
    debug_strings: Vec<String>,
}

fn viewport_offset_to_camera(offset: Vec2, screen_size: Vec2) -> Camera2D {
    let [w, h] = screen_size.to_array();

    // I don't understand why, but `from_display_rect` flips the height portion by
    // default, or something like that
    Camera2D::from_display_rect(Rect { x: offset.x.round(), y: offset.y.round() + h, w, h: -h })
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

fn make_wrapping_png_texture(png_bytes: &[u8]) -> Texture2D {
    let ctx = unsafe { get_internal_gl() }.quad_context;
    let img = image::load_from_memory_with_format(png_bytes, ImageFormat::Png).unwrap();
    let bytes = img.to_rgba8().into_raw();

    assert!(
        img.width().is_power_of_two() && img.height().is_power_of_two(),
        "WebGL doesn't support repeating non-power-of-two textures",
    );

    let texture_id = ctx.new_texture_from_data_and_format(
        &bytes,
        miniquad::TextureParams {
            width: img.width(),
            height: img.height(),
            wrap: TextureWrap::Repeat,
            ..Default::default()
        },
    );
    Texture2D::from_miniquad_texture(texture_id)
}

fn build_level() -> Level {
    let tex_bricks = make_wrapping_png_texture(include_bytes!("./assets/bricks.png"));
    let tex_bricks_dark = make_wrapping_png_texture(include_bytes!("./assets/bricks_dark.png"));
    let tex_wood = make_wrapping_png_texture(include_bytes!("./assets/wood.png"));
    let tex_wood_dark = make_wrapping_png_texture(include_bytes!("./assets/wood_dark.png"));
    let tex_arrow_left = make_wrapping_png_texture(include_bytes!("./assets/arrow_left.png"));

    let level_width: f32 = 6000.0;
    let level_height: f32 = 12000.0;

    // TODO: make a god damn level editor
    let mut level = Level::new(
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

#[macroquad::main(window_conf)]
async fn main() {
    rand::srand(miniquad::date::now() as u64);

    let mut state = init_game_state();

    let mut time_bank: f64 = 0.0;
    let mut last_time = get_time();

    let mut frame = 0u64;

    #[cfg(target_family = "wasm")]
    let is_demo = wasm::is_wasm_demo();
    #[cfg(not(target_family = "wasm"))]
    let is_demo = std::env::var_os("PENGUIN_DEMO").is_some_and(|s| !s.is_empty());

    let demo_banner = if is_demo { "[DEMO] " } else { "" };

    let device: &mut dyn InputDevice = if is_demo {
        &mut DemoInput::new(DEMO_MOVIE.to_vec().into_boxed_slice())
    } else {
        &mut MacroquadInput
    };

    let mut speed_up = false;

    loop {
        // Mouse input
        let screen_size = {
            let (w, h) = screen_size();
            vec2(w, h)
        };
        // let device = MacroquadInput;

        // Frame debt logic
        let now = get_time();
        time_bank += now - last_time;
        last_time = now;

        if is_key_pressed(KeyCode::R) {
            speed_up = !speed_up;
        }

        let ups = if speed_up { UPS_FAST } else { UPS_NORMAL };
        let update_frame_time = 1.0 / ups;
        while time_bank >= update_frame_time {
            device.next_frame();
            frame += 1;
            fixed_update(&mut state, device);
            time_bank -= update_frame_time;
        }

        // Main rendering
        let viewport_offset = state.penguin.pos - screen_size / 2.;
        set_camera(&viewport_offset_to_camera(viewport_offset, screen_size));
        clear_background(DARKBLUE);
        for graphic in state.level.graphics() {
            graphic.macroquad_draw();
        }
        draw_penguin(state.penguin.pos, state.penguin.vel, Vec2::from_angle(device.look_angle()));
        for &rocket in &state.rockets {
            draw_rocket(rocket.pos, rocket.vel);
        }
        for &explosion in &state.explosions {
            draw_explosion(explosion);
        }
        set_default_camera();

        // Debug information
        draw_text(&format!("FPS: {}", get_fps()), 32., 32., 16., WHITE);
        draw_text(&format!("{}frame: {}", demo_banner, frame), 32., 48., 16., WHITE);
        let mut y = 64.0;
        for string in state.debug_strings.iter() {
            draw_text(string, 32.0, y, 16.0, WHITE);
            y += 16.0;
        }

        next_frame().await
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Input {
    Left,
    Right,
    Shoot,
}

trait InputDevice {
    fn next_frame(&mut self);
    fn is_input_down(&self, input: Input) -> bool;
    fn look_angle(&self) -> f32;
}

struct MacroquadInput;

impl InputDevice for MacroquadInput {
    fn next_frame(&mut self) {}

    fn is_input_down(&self, input: Input) -> bool {
        match input {
            Input::Left => is_key_down(KeyCode::A),
            Input::Right => is_key_down(KeyCode::D),
            Input::Shoot => is_mouse_button_down(MouseButton::Left),
        }
    }

    fn look_angle(&self) -> f32 {
        let (sx, sy) = screen_size();
        let (mx, my) = mouse_position();

        vec2(mx - sx / 2., my - sy / 2.).to_angle()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DemoAction {
    InputOn(Input),
    InputOff(Input),
    SetLookAngle(f32), // degrees!
}

struct DemoInput {
    frame: u64,
    actions: Box<[(u64, DemoAction)]>, // should be sorted by u64
    action_index: usize,
    look_angle: f32,
    // TODO: use enumset or something like that
    is_left_on: bool,
    is_right_on: bool,
    is_shoot_on: bool,
}

impl DemoInput {
    pub fn new(actions: Box<[(u64, DemoAction)]>) -> DemoInput {
        {
            // ensure frame numbers are non-decreasing
            let mut last_frame = 0u64;
            for (i, (frame, action)) in actions.iter().enumerate() {
                assert!(*frame >= last_frame, "Instruction out of place: {:?}", (i, frame, action));
                last_frame = *frame;
            }
        }

        DemoInput {
            actions,
            frame: 0,
            action_index: 0,
            look_angle: 0.0,
            is_left_on: false,
            is_right_on: false,
            is_shoot_on: false,
        }
    }

    fn handle_action(&mut self, action: DemoAction) {
        match action {
            DemoAction::InputOn(input) => match input {
                Input::Left => self.is_left_on = true,
                Input::Right => self.is_right_on = true,
                Input::Shoot => self.is_shoot_on = true,
            },
            DemoAction::InputOff(input) => match input {
                Input::Left => self.is_left_on = false,
                Input::Right => self.is_right_on = false,
                Input::Shoot => self.is_shoot_on = false,
            },
            DemoAction::SetLookAngle(angle) => self.look_angle = angle.to_radians(),
        }
    }
}

impl InputDevice for DemoInput {
    fn is_input_down(&self, input: Input) -> bool {
        match input {
            Input::Left => self.is_left_on,
            Input::Right => self.is_right_on,
            Input::Shoot => self.is_shoot_on,
        }
    }

    fn look_angle(&self) -> f32 {
        self.look_angle
    }

    fn next_frame(&mut self) {
        self.frame += 1;

        if self.action_index >= self.actions.len() {
            return;
        }

        loop {
            let Some((frame, action)) = self.actions.get(self.action_index) else { return };
            if *frame == self.frame {
                self.handle_action(*action);
                self.action_index += 1;
            } else {
                return;
            }
        }
    }
}

fn fixed_update(state: &mut GameState, device: &dyn InputDevice) {
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
        let dir = Vec2::from_angle(device.look_angle());
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
    use nalgebra::{Isometry2, Vector2};
    use parry2d::query;
    use parry2d::shape::{Ball, Cuboid};

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

fn draw_rocket(pos: Vec2, vel: Vec2) {
    let dir = vel.normalize_or_zero();

    draw_triangle(pos - dir * 24., pos - dir * 4. + dir.perp() * 3., pos - dir.perp() * 3., RED);
    draw_triangle(pos - dir * 18., pos - dir * 3. + dir.perp() * 2., pos - dir.perp() * 2., WHITE);
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

// Holy text wall!
// TODO: implement special file format for demo movies
// TODO: implement demo recording and better replay
/*
We may want to

*/
static DEMO_MOVIE: &[(u64, DemoAction)] = &[
    //
    // Get on top of the house
    (150, DemoAction::SetLookAngle(30.0)),
    (200, DemoAction::SetLookAngle(60.0)),
    (250, DemoAction::SetLookAngle(75.0)),
    (300, DemoAction::SetLookAngle(90.0)),
    (300, DemoAction::InputOn(Input::Shoot)),
    (304, DemoAction::InputOff(Input::Shoot)),
    (316, DemoAction::InputOn(Input::Left)),
    (332, DemoAction::InputOff(Input::Left)),
    (336, DemoAction::InputOn(Input::Right)),
    (344, DemoAction::InputOff(Input::Right)),
    (600, DemoAction::InputOn(Input::Shoot)),
    (604, DemoAction::InputOff(Input::Shoot)),
    (604, DemoAction::InputOn(Input::Right)),
    (612, DemoAction::InputOff(Input::Right)),
    (660, DemoAction::InputOn(Input::Left)),
    (792, DemoAction::InputOff(Input::Left)),
    //
    // Get onto the first bridge
    (800, DemoAction::SetLookAngle(100.0)),
    (820, DemoAction::SetLookAngle(110.0)),
    (840, DemoAction::SetLookAngle(120.0)),
    (860, DemoAction::InputOn(Input::Right)),
    (872, DemoAction::InputOn(Input::Shoot)),
    (876, DemoAction::InputOff(Input::Shoot)),
    (1000, DemoAction::InputOff(Input::Right)),
    (1000, DemoAction::SetLookAngle(110.0)),
    (1020, DemoAction::InputOn(Input::Left)),
    (1020, DemoAction::SetLookAngle(100.0)),
    (1040, DemoAction::SetLookAngle(95.0)),
    (1060, DemoAction::SetLookAngle(90.0)),
    (1080, DemoAction::InputOff(Input::Left)),
    //
    // Get onto the second bridge
    (1100, DemoAction::InputOn(Input::Right)),
    (1300, DemoAction::InputOn(Input::Shoot)),
    (1304, DemoAction::InputOff(Input::Shoot)),
    (1464, DemoAction::InputOff(Input::Right)),
    (1464, DemoAction::InputOn(Input::Left)),
    (1520, DemoAction::InputOff(Input::Left)),
    //
    // Get onto the third bridge
    (1540, DemoAction::SetLookAngle(95.0)),
    (1552, DemoAction::SetLookAngle(100.0)),
    (1564, DemoAction::InputOn(Input::Right)),
    (1700, DemoAction::SetLookAngle(110.0)),
    (1740, DemoAction::SetLookAngle(120.0)),
    (1900, DemoAction::InputOn(Input::Shoot)),
    (1904, DemoAction::InputOff(Input::Shoot)),
    (2208, DemoAction::InputOff(Input::Right)),
    //
    // Get onto the fourth bridge
    (2216, DemoAction::SetLookAngle(110.0)),
    (2232, DemoAction::SetLookAngle(100.0)),
    (2300, DemoAction::InputOn(Input::Right)),
    (2316, DemoAction::SetLookAngle(90.0)),
    (2440, DemoAction::InputOn(Input::Shoot)),
    (2444, DemoAction::InputOff(Input::Shoot)),
    (2600, DemoAction::InputOff(Input::Right)),
    (2700, DemoAction::InputOn(Input::Left)),
    (2752, DemoAction::InputOff(Input::Left)),
    //
    // Get into the tower
    (2780, DemoAction::SetLookAngle(85.0)),
    (2800, DemoAction::SetLookAngle(80.0)),
    (2800, DemoAction::InputOn(Input::Right)),
    (2832, DemoAction::InputOff(Input::Right)),
    (2840, DemoAction::InputOn(Input::Left)),
    (2900, DemoAction::InputOn(Input::Shoot)),
    (2904, DemoAction::InputOff(Input::Shoot)),
    (2924, DemoAction::SetLookAngle(85.0)),
    (2940, DemoAction::SetLookAngle(90.0)),
    (3200, DemoAction::InputOff(Input::Left)),
    //
    // Sync onto the crazy ledge
    (3240, DemoAction::InputOn(Input::Shoot)),
    (3244, DemoAction::InputOff(Input::Shoot)),
    (3448, DemoAction::InputOn(Input::Shoot)),
    (3452, DemoAction::InputOff(Input::Shoot)),
    (3584, DemoAction::InputOn(Input::Shoot)),
    (3588, DemoAction::InputOff(Input::Shoot)),
    (3700, DemoAction::InputOn(Input::Right)),
    (3872, DemoAction::InputOff(Input::Right)),
    (3916, DemoAction::InputOn(Input::Left)),
    (3924, DemoAction::InputOff(Input::Left)),
    //
    // Jump onto the tiny leg
    (3952, DemoAction::SetLookAngle(95.0)),
    (3956, DemoAction::InputOn(Input::Right)),
    (3980, DemoAction::InputOff(Input::Right)),
    (4000, DemoAction::InputOn(Input::Shoot)),
    (4004, DemoAction::InputOff(Input::Shoot)),
    (4120, DemoAction::InputOn(Input::Left)),
    (4164, DemoAction::InputOff(Input::Left)),
    //
    // Jump onto the finish ledge
    (4200, DemoAction::SetLookAngle(90.0)),
    (4240, DemoAction::SetLookAngle(85.0)),
    (4320, DemoAction::InputOn(Input::Shoot)),
    (4324, DemoAction::InputOff(Input::Shoot)),
    (4340, DemoAction::InputOn(Input::Left)),
    (4444, DemoAction::InputOff(Input::Left)),
    (4452, DemoAction::InputOn(Input::Right)),
    (4500, DemoAction::InputOff(Input::Right)),
    //
    // Victory spin
    (4540, DemoAction::SetLookAngle(80.0)),
    (4552, DemoAction::SetLookAngle(100.0)),
    (4564, DemoAction::SetLookAngle(120.0)),
    (4576, DemoAction::SetLookAngle(140.0)),
    (4588, DemoAction::SetLookAngle(160.0)),
    (4600, DemoAction::SetLookAngle(180.0)),
    (4612, DemoAction::SetLookAngle(200.0)),
    (4624, DemoAction::SetLookAngle(220.0)),
    (4636, DemoAction::SetLookAngle(240.0)),
    (4648, DemoAction::SetLookAngle(260.0)),
    (4660, DemoAction::SetLookAngle(280.0)),
    (4672, DemoAction::SetLookAngle(300.0)),
    (4684, DemoAction::SetLookAngle(320.0)),
    (4696, DemoAction::SetLookAngle(340.0)),
    (4708, DemoAction::SetLookAngle(0.0)),
    (4720, DemoAction::SetLookAngle(20.0)),
    (4732, DemoAction::SetLookAngle(40.0)),
    (4744, DemoAction::SetLookAngle(60.0)),
    (4756, DemoAction::SetLookAngle(80.0)),
];
