use arrayvec::ArrayVec;
use macroquad::prelude::*;
use miniquad::TextureWrap;
use parry2d::shape::ConvexPolygon;
use std::f32::consts::PI;

mod draw_utils;
mod levels;
use levels::Level;

const WINDOW_WIDTH: u32 = 1600;
const WINDOW_HEIGHT: u32 = 900;

const SCREEN_SIZE: Vec2 = vec2(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);

const UPDATES_PER_SECOND: f64 = 120.;
const UPDATE_FRAME_TIME: f64 = 1. / UPDATES_PER_SECOND;

const PENGUIN_RADIUS: f32 = 24.0;
const ROCKET_RADIUS: f32 = 4.0;

const ROCKET_SPEED: f32 = 5.0;
const ROCKET_TTL: u16 = 120;
const ROCKET_SHOOT_COOLDOWN: u16 = 30;

const EXPLOSION_RADIUS: f32 = 48.0;
const EXPLOSION_TTL: u16 = 6;
const EXPLOSION_FORCE: f32 = 2.0;

const COYOTE_DURATION: u8 = 12;
const GRAVITY: f32 = 0.1;
const JUMP_SPEED: f32 = 4.;
const MAX_GRAVITY_SPEED: f32 = 12.;
const MAX_WALK_SPEED: f32 = 1.5;

const FRICTION_AIR: f32 = 0.999;
const FRICTION_GROUND: f32 = 0.99;

const WALK_ACCEL_GROUND: f32 = 0.1;
const WALK_ACCEL_AIR: f32 = 0.1;

fn window_conf() -> Conf {
    Conf {
        window_title: "penguin".to_string(),
        window_width: WINDOW_WIDTH as i32,
        window_height: WINDOW_HEIGHT as i32,
        window_resizable: false,
        sample_count: 2,
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

fn viewport_offset_to_camera(offset: Vec2) -> Camera2D {
    let w = WINDOW_WIDTH as f32;
    let h = WINDOW_HEIGHT as f32;

    // I don't understand why, but `from_display_rect` flips the height portion by
    // default, or something like that
    Camera2D::from_display_rect(Rect { x: offset.x.round(), y: offset.y.round() + h, w, h: -h })
}

fn init_game_state() -> GameState {
    let penguin = Penguin {
        pos: vec2(0.0, -PENGUIN_RADIUS * 2.0),
        vel: Vec2::ZERO,
        coyote_time: 0,
        rocket_cooldown: 0,
    };

    GameState {
        penguin,
        level: build_level(),
        rockets: Vec::with_capacity(32),
        explosions: Vec::with_capacity(32),
        debug_strings: Vec::with_capacity(16),
    }
}

fn build_level() -> Level {
    let tex_bricks = Texture2D::from_file_with_format(
        include_bytes!("./assets/bricks.png"),
        Some(ImageFormat::Png),
    );
    let tex_purple = Texture2D::from_file_with_format(
        include_bytes!("./assets/purple.png"),
        Some(ImageFormat::Png),
    );

    // Make the textures repeat instead of clamping
    let ctx = unsafe { get_internal_gl() }.quad_context;
    for tex in [&tex_bricks, &tex_purple] {
        ctx.texture_set_wrap(tex.raw_miniquad_id(), TextureWrap::Repeat, TextureWrap::Repeat);
    }

    Level::new(
        &[
            (vec2(-1200.0, 0.0), vec2(2400.0, 48.0), tex_bricks.clone().into()),
            (vec2(-128.0, -120.0), vec2(72.0, 120.0), tex_purple.clone().into()),
            (vec2(256.0, -120.0), vec2(72.0, 120.0), tex_purple.clone().into()),
        ],
        &[
            (&[vec2(400.0, 0.0), vec2(600.0, 0.0), vec2(400.0, -100.0)], tex_purple.clone().into()),
            (
                &[vec2(-600.0, 0.0), vec2(-600.0, -24.0), vec2(-400.0, 0.0)],
                tex_purple.clone().into(),
            ),
        ],
    )
}

#[macroquad::main(window_conf)]
async fn main() {
    rand::srand(miniquad::date::now() as u64);

    let mut state = init_game_state();

    let mut time_bank: f64 = 0.0;
    let mut last_time = get_time();

    let mut viewport_offset = Vec2::ZERO;

    loop {
        // Mouse input
        let (screen_mx, screen_my) = mouse_position();
        let mouse_pos = vec2(screen_mx, screen_my) + viewport_offset;
        let angle_to_mouse = (mouse_pos - state.penguin.pos).normalize_or(vec2(1.0, 0.0));

        // Frame debt logic
        let now = get_time();
        time_bank += now - last_time;
        last_time = now;

        while time_bank >= UPDATE_FRAME_TIME {
            fixed_update(&mut state, mouse_pos);
            time_bank -= UPDATE_FRAME_TIME;
        }
        viewport_offset = state.penguin.pos - SCREEN_SIZE / 2.;

        // Main rendering
        set_camera(&viewport_offset_to_camera(viewport_offset));
        clear_background(DARKBLUE);
        for graphic in state.level.graphics() {
            graphic.macroquad_draw();
        }
        draw_penguin(state.penguin.pos, state.penguin.vel, angle_to_mouse);
        for &rocket in &state.rockets {
            draw_rocket(rocket.pos, rocket.vel);
        }
        for &explosion in &state.explosions {
            draw_explosion(explosion);
        }
        set_default_camera();

        // Debug information
        draw_text(&format!("FPS: {}", get_fps()), 32., 32., 16., WHITE);
        draw_text(&format!("coyote frames: {}", state.penguin.coyote_time), 32., 48., 16., WHITE);
        let mut y = 64.0;
        for string in state.debug_strings.iter() {
            draw_text(string, 32.0, y, 16.0, WHITE);
            y += 16.0;
        }

        next_frame().await
    }
}

fn fixed_update(state: &mut GameState, mouse_pos: Vec2) {
    state.debug_strings.clear();

    update_rockets_movement(
        &mut state.rockets,
        &mut state.explosions,
        state.level.rect_colliders(),
        state.level.poly_colliders(),
    );
    update_explosions(&mut state.explosions);
    update_penguin_movement(
        &mut state.penguin,
        state.level.rect_colliders(),
        state.level.poly_colliders(),
        &state.explosions,
        |debug| state.debug_strings.push(debug),
    );

    state.penguin.rocket_cooldown = state.penguin.rocket_cooldown.saturating_sub(1);
    // Spawn rocket
    if is_mouse_button_down(MouseButton::Left) && state.penguin.rocket_cooldown == 0 {
        state.penguin.rocket_cooldown = ROCKET_SHOOT_COOLDOWN;
        let dir = (mouse_pos - state.penguin.pos).normalize_or(vec2(1., 0.));
        state.rockets.push(Rocket {
            pos: state.penguin.pos,
            vel: dir * ROCKET_SPEED,
            ttl: ROCKET_TTL,
        });
    }

    // Ensure that we're not grounded when we've already lifted off
    // otherwise you can normal-jump after you've rocket-jumped
    if state.penguin.vel.y < -0.001 {
        state.penguin.coyote_time = 0;
    }
}

fn update_penguin_movement(
    penguin: &mut Penguin,
    rects: &[Rect],
    polygons: &[ConvexPolygon],
    explosions: &[Explosion],
    mut debug: impl FnMut(String),
) {
    let (accel, mut friction) = if penguin.coyote_time == 0 {
        (WALK_ACCEL_AIR, FRICTION_AIR)
    } else {
        (WALK_ACCEL_GROUND, FRICTION_GROUND)
    };

    if penguin.vel.x.abs() < 0.1 {
        friction *= 0.8;
    }

    penguin.vel.x *= friction;
    if is_key_down(KeyCode::D) {
        if penguin.vel.x + accel < MAX_WALK_SPEED {
            penguin.vel.x += accel;
        } else if penguin.vel.x < MAX_WALK_SPEED {
            penguin.vel.x = MAX_WALK_SPEED;
        }
    }
    if is_key_down(KeyCode::A) {
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

    // Jumping
    if penguin.coyote_time > 0 && is_key_down(KeyCode::Space) {
        penguin.coyote_time = 0;
        penguin.vel += vec2(0., -JUMP_SPEED);
    }

    // Penguin collision detection
    let penguin_circle = Circle::new(penguin.pos.x, penguin.pos.y, PENGUIN_RADIUS);

    // TODO: I really have no idea what I'm doing when it comes to collision detection.
    //       Detecting if we're grounded seems super janky with polygons. Fix later please
    let penguin_if_it_were_to_fall = penguin_circle
        .offset(penguin.vel.max(vec2(f32::MIN, 0.0)) + vec2(0.0, GRAVITY * 2.0));
    let mut dv_for_grounded = Vec2::ZERO;
    let mut any_point_upwards = false;

    for rect in rects {
        if let Some(dv) = circle_impacts_rect(penguin_circle.offset(penguin.vel), *rect) {
            penguin.pos += dv / 2.;

            let cancel_vec = penguin.vel.project_onto_normalized(dv.normalize_or_zero());
            penguin.vel -= cancel_vec / 4.;
        }

        if let Some(dv) = circle_impacts_rect(penguin_if_it_were_to_fall, *rect) {
            dv_for_grounded += dv;

            let angle = dv.to_angle();
            any_point_upwards =
                any_point_upwards || (-PI / 2. - 0.34 < angle && angle < -PI / 2. + 0.34);
        }
    }

    for poly in polygons {
        if let Some(dv) = circle_impacts_convex(penguin_circle.offset(penguin.vel), poly) {
            penguin.pos += dv / 2.;

            let cancel_vec = penguin.vel.project_onto_normalized(dv.normalize_or_zero());
            penguin.vel -= cancel_vec / 4.;
        }

        if let Some(dv) = circle_impacts_convex(penguin_if_it_were_to_fall, poly) {
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

    debug(format!("dv_for_grounded: {dv_for_grounded}"));

    let penguin_circle = Circle::new(penguin.pos.x, penguin.pos.y, PENGUIN_RADIUS);
    for &exp in explosions {
        let exp_circle = Circle::new(exp.pos.x, exp.pos.y, exp.radius);
        if let Some((dir, scale)) = penguin_impacts_explosion(penguin_circle, exp_circle) {
            penguin.vel += dir * exp.force * scale;
        }
    }

    penguin.pos += penguin.vel
}

fn update_rockets_movement(
    rockets: &mut Vec<Rocket>,
    explosions: &mut Vec<Explosion>,
    rects: &[Rect],
    polygons: &[ConvexPolygon],
) {
    let mut removed_rocket_idxs = ArrayVec::<_, 8>::new();

    for rocket in rockets.iter_mut() {
        rocket.pos += rocket.vel;
    }

    for rocket in rockets.iter_mut() {
        rocket.ttl -= 1;
    }

    'outer: for (idx, rocket) in rockets.iter_mut().enumerate() {
        if rocket.ttl == 0 {
            removed_rocket_idxs.push(idx);
        } else {
            let circle = Circle::new(rocket.pos.x, rocket.pos.y, ROCKET_RADIUS);
            for &rect in rects {
                if circle_impacts_rect(circle, rect).is_some() {
                    removed_rocket_idxs.push(idx);
                    continue 'outer;
                }
            }

            for poly in polygons {
                if circle_impacts_convex(circle, poly).is_some() {
                    removed_rocket_idxs.push(idx);
                    continue 'outer;
                }
            }
        }
    }

    for &idx in removed_rocket_idxs.iter().rev() {
        let rocket = rockets.swap_remove(idx);
        explosions.push(Explosion {
            pos: rocket.pos,
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
    use nalgebra::{Isometry2, Vector2};
    use parry2d::query;
    use parry2d::shape::{Ball, Cuboid};

    let cuboid = Cuboid::new(Vector2::new(rect.w / 2., rect.h / 2.));
    let ball = Ball::new(circle.radius());

    let cuboid_pos = Isometry2::translation(rect.x + rect.w / 2., rect.y + rect.h / 2.);
    let ball_pos = Isometry2::translation(circle.x, circle.y);

    let contact = query::contact(&cuboid_pos, &cuboid, &ball_pos, &ball, 0.0).unwrap();

    contact.map(|c| {
        let delta = c.point1 - c.point2;
        vec2(delta.x, delta.y)
    })
}

/// If an intersection occurs, return how much to move the circle
fn circle_impacts_convex(circle: Circle, poly: &ConvexPolygon) -> Option<Vec2> {
    use nalgebra::Isometry2;
    use parry2d::query;
    use parry2d::shape::Ball;

    let ball = Ball::new(circle.radius());
    let ball_pos = Isometry2::translation(circle.x, circle.y);

    let contact = query::contact(&Isometry2::default(), poly, &ball_pos, &ball, 0.0).unwrap();

    contact.and_then(|c| {
        let delta = c.point1 - c.point2;
        let rv = vec2(delta.x, delta.y);
        rv.is_finite().then_some(rv)
    })
}

/// If an intersection occurs, returns a normal vector and how close
/// the penguin was to the explosion center (1 is the closest, 0 is the farthest)
fn penguin_impacts_explosion(penguin: Circle, exp: Circle) -> Option<(Vec2, f32)> {
    use nalgebra::Isometry2;
    use parry2d::query;
    use parry2d::shape::Ball;

    let penguin_ball = Ball::new(penguin.radius());
    let exp_ball = Ball::new(exp.radius());

    let penguin_pos = Isometry2::translation(penguin.x, penguin.y);
    let exp_pos = Isometry2::translation(exp.x, exp.y);

    let contact = query::contact(&penguin_pos, &penguin_ball, &exp_pos, &exp_ball, 0.0).unwrap();
    contact.map(|c| {
        let delta = c.point2 - c.point1;
        let vec = vec2(delta.x, delta.y);

        (vec.normalize_or_zero(), vec.length() / exp.radius())
    })
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
