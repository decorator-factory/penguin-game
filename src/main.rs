use arrayvec::ArrayVec;
use macroquad::prelude::*;
use std::f32::consts::PI;

const WINDOW_WIDTH: u32 = 1600;
const WINDOW_HEIGHT: u32 = 900;

const SCREEN_SIZE: Vec2 = vec2(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);

const UPDATES_PER_SECOND: f64 = 120.;
const UPDATE_FRAME_TIME: f64 = 1. / UPDATES_PER_SECOND;

const DUCKY_RADIUS: f32 = 24.0;
const ROCKET_RADIUS: f32 = 4.0;

const ROCKET_SPEED: f32 = 5.0;
const ROCKET_TTL: u16 = 180;
const ROCKET_SHOOT_COOLDOWN: u16 = 30;

const EXPLOSION_RADIUS: f32 = 32.0;
const EXPLOSION_TTL: u16 = 8;
const EXPLOSION_FORCE: f32 = 1.5;

const COYOTE_DURATION: u8 = 12;
const GRAVITY: f32 = 0.1;
const JUMP_SPEED: f32 = 4.;
const MAX_GRAVITY_SPEED: f32 = 12.;
const MAX_WALK_SPEED: f32 = 1.5;

const FRICTION_AIR: f32 = 0.999;
const FRICTION_GROUND: f32 = 0.99;

const WALK_ACCEL_GROUND: f32 = 0.1;
const WALK_ACCEL_AIR: f32 = 0.05;

fn window_conf() -> Conf {
    Conf {
        window_title: "ducky".to_string(),
        window_width: WINDOW_WIDTH as i32,
        window_height: WINDOW_HEIGHT as i32,
        window_resizable: false,
        sample_count: 4,
        ..Default::default()
    }
}

#[derive(Copy, Clone, Debug)]
struct Ducky {
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

struct Level {
    rects: Vec<Rect>,
}

struct GameState {
    ducky: Ducky,
    level: Level,
    rockets: Vec<Rocket>,
    explosions: Vec<Explosion>,
}

fn viewport_offset_to_camera(offset: Vec2) -> Camera2D {
    let w = WINDOW_WIDTH as f32;
    let h = WINDOW_HEIGHT as f32;

    // I don't understand why, but `from_display_rect` flips the height portion by
    // default, or something like that
    Camera2D::from_display_rect(Rect { x: offset.x.round(), y: offset.y.round() + h, w, h: -h })
}

fn init_game_state() -> GameState {
    let ducky =
        Ducky { pos: vec2(100., 100.), vel: Vec2::ZERO, coyote_time: 0, rocket_cooldown: 0 };

    #[rustfmt::skip]
    let rects = [
        (0., 20., 100., 4.),

        (100., 15., 1., 5.),
        (20.5, 16., 3., 2.),
        (24., 13., 3., 2.),
        (28., 10.5, 3., 6.),

        (30., 25., 1., 5.),
        (40., 25., 1., 5.),
        (50., 25., 1., 5.),
        (60., 25., 1., 5.),
        (70., 25., 1., 5.),
        (80., 25., 1., 5.),
        (90., 25., 1., 5.),

    ].map(|(x, y, w, h)| Rect::new(x * 32., y * 32., w * 32., h * 32.)).to_vec();

    GameState {
        ducky,
        level: Level { rects },
        rockets: Vec::with_capacity(32),
        explosions: Vec::with_capacity(32),
    }
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

        // Frame debt logic
        let now = get_time();
        time_bank += now - last_time;
        last_time = now;

        while time_bank >= UPDATE_FRAME_TIME {
            fixed_update(&mut state, mouse_pos);
            time_bank -= UPDATE_FRAME_TIME;
        }
        viewport_offset = state.ducky.pos - SCREEN_SIZE / 2.;

        // Main rendering
        set_camera(&viewport_offset_to_camera(viewport_offset));
        clear_background(DARKBLUE);
        for &Rect { x, y, w, h } in &state.level.rects {
            draw_rectangle(x, y, w, h, GRAY);
        }
        draw_ducky(state.ducky.pos, state.ducky.vel);
        for &rocket in &state.rockets {
            draw_rocket(rocket.pos, rocket.vel);
        }
        for &explosion in &state.explosions {
            draw_explosion(explosion);
        }
        set_default_camera();

        // Debug information();
        draw_text(&format!("FPS: {}", get_fps()), 32., 32., 16., WHITE);
        next_frame().await
    }
}

fn fixed_update(state: &mut GameState, mouse_pos: Vec2) {
    update_rockets_movement(&mut state.rockets, &mut state.explosions, &state.level.rects);
    update_explosions(&mut state.explosions);
    update_ducky_movement(&mut state.ducky, &state.level.rects, &state.explosions);

    state.ducky.rocket_cooldown = state.ducky.rocket_cooldown.saturating_sub(1);
    // Spawn rocket
    if is_mouse_button_down(MouseButton::Left) && state.ducky.rocket_cooldown == 0 {
        state.ducky.rocket_cooldown = ROCKET_SHOOT_COOLDOWN;
        let dir = (mouse_pos - state.ducky.pos).normalize_or(vec2(1., 0.));
        state.rockets.push(Rocket {
            pos: state.ducky.pos,
            vel: dir * ROCKET_SPEED,
            ttl: ROCKET_TTL,
        });
    }

    // Ensure that we're not grounded when we've already lifted off
    // otherwise you can normal-jump after you've rocket-jumped
    if state.ducky.vel.y < -0.001 {
        state.ducky.coyote_time = 0;
    }
}

fn update_ducky_movement(
    ducky: &mut Ducky,
    rects: &[Rect],
    explosions: &[Explosion],
) {
    let (accel, friction) = if ducky.coyote_time == 0 {
        (WALK_ACCEL_AIR, FRICTION_AIR)
    } else {
        (WALK_ACCEL_GROUND, FRICTION_GROUND)
    };

    ducky.vel.x *= friction;
    if is_key_down(KeyCode::D) {
        if ducky.vel.x + accel < MAX_WALK_SPEED {
            ducky.vel.x += accel;
        } else if ducky.vel.x < MAX_WALK_SPEED {
            ducky.vel.x = MAX_WALK_SPEED;
        }
    }
    if is_key_down(KeyCode::A) {
        if ducky.vel.x - accel > -MAX_WALK_SPEED {
            ducky.vel.x -= accel;
        } else if ducky.vel.x > -MAX_WALK_SPEED {
            ducky.vel.x = -MAX_WALK_SPEED;
        }
    }

    if ducky.coyote_time == COYOTE_DURATION {
        ducky.vel.y = 0.;
    }
    if ducky.coyote_time == 0 && ducky.vel.y < MAX_GRAVITY_SPEED {
        ducky.vel.y += GRAVITY;
    }

    // Jumping
    if ducky.coyote_time > 0 && is_key_down(KeyCode::Space) {
        ducky.coyote_time = 0;
        ducky.vel += vec2(0., -JUMP_SPEED);
    }

    // Ducky collision detection
    let mut is_grounded = false;
    let ducky_circle = Circle::new(ducky.pos.x, ducky.pos.y, DUCKY_RADIUS);
    for rect in rects {
        if let Some(dv) = circle_impacts_rect(ducky_circle.offset(ducky.vel), *rect) {
            ducky.pos += dv / 2.;

            if dv.length_squared() > 0.05 {
                let cancel_vec = ducky.vel.project_onto_normalized(dv.normalize());
                ducky.vel -= cancel_vec / 4.;
            }

            if !is_grounded
                && let Some(dv_when_slightly_lower) =
                    circle_impacts_rect(ducky_circle.offset(ducky.vel + vec2(0., 0.1)), *rect)
            {
                let angle = dv_when_slightly_lower.to_angle();
                is_grounded = -PI / 2. - 0.1 < angle && angle < -PI / 2. + 0.1;
            }
        }
    }
    if is_grounded {
        ducky.coyote_time = COYOTE_DURATION;
    } else {
        ducky.coyote_time = ducky.coyote_time.saturating_sub(1);
    }

    let ducky_circle = Circle::new(ducky.pos.x, ducky.pos.y, DUCKY_RADIUS);
    for &exp in explosions {
        let exp_circle = Circle::new(exp.pos.x, exp.pos.y, exp.radius);
        if let Some((dir, scale)) = ducky_impacts_explosion(ducky_circle, exp_circle) {
            ducky.vel += dir * exp.force * scale;
        }
    }

    ducky.pos += ducky.vel
}

fn update_rockets_movement(
    rockets: &mut Vec<Rocket>,
    explosions: &mut Vec<Explosion>,
    rects: &[Rect],
) {
    let mut removed_rocket_idxs = ArrayVec::<_, 8>::new();

    for rocket in rockets.iter_mut() {
        rocket.pos += rocket.vel;
    }

    for rocket in rockets.iter_mut() {
        rocket.ttl -= 1;
    }

    for (idx, rocket) in rockets.iter_mut().enumerate() {
        if rocket.ttl == 0 {
            removed_rocket_idxs.push(idx);
        } else {
            for &rect in rects {
                if circle_impacts_rect(Circle::new(rocket.pos.x, rocket.pos.y, ROCKET_RADIUS), rect)
                    .is_some()
                {
                    removed_rocket_idxs.push(idx);
                    break;
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
        exp.ttl = exp.ttl.wrapping_sub(1);  // if this wraps, then it will be removed anyway!
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

/// If an intersection occurs, returns a normal vector and how close
/// the ducky was to the explosion center (1 is the closest, 0 is the farthest)
fn ducky_impacts_explosion(ducky: Circle, exp: Circle) -> Option<(Vec2, f32)> {
    use nalgebra::Isometry2;
    use parry2d::query;
    use parry2d::shape::Ball;

    let ducky_ball = Ball::new(ducky.radius());
    let exp_ball = Ball::new(exp.radius());

    let ducky_pos = Isometry2::translation(ducky.x, ducky.y);
    let exp_pos = Isometry2::translation(exp.x, exp.y);

    let contact = query::contact(&ducky_pos, &ducky_ball, &exp_pos, &exp_ball, 0.0).unwrap();
    contact.map(|c| {
        let delta = c.point2 - c.point1;
        let vec = vec2(delta.x, delta.y);

        (vec.normalize_or_zero(), vec.length() / exp.radius())
    })
}

fn draw_arrow(start: Vec2, end: Vec2, color: Color) {
    let delta = (start - end).normalize_or_zero();

    let v1 = end;
    let v2 = end + delta * 12. + delta.perp() * 6.;
    let v3 = end + delta * 12. - delta.perp() * 6.;
    draw_line(start.x, start.y, end.x, end.y, 2., color);
    draw_triangle(v1, v2, v3, color);
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

fn draw_ducky(pos: Vec2, vel: Vec2) {
    let (cx, cy) = (pos.x, pos.y);

    if vel.length() > 2.0 {
        let steps = ((vel.length() - 1.5) / 0.5) as u16;
        for i in 0..steps {
            let fade_factor = 1. - i as f32 / steps as f32;
            let dpos = pos - vel.normalize() * (1. + i as f32) * 4.0;
            draw_circle_lines(
                dpos.x,
                dpos.y,
                DUCKY_RADIUS * 0.5 + DUCKY_RADIUS * 0.4 * fade_factor,
                1.0,
                WHITE.with_alpha(fade_factor * 0.5),
            );
        }
    }
    draw_circle(cx, cy, DUCKY_RADIUS, YELLOW);

    draw_circle(cx - DUCKY_RADIUS * 0.5, cy - DUCKY_RADIUS * 0.2, 2., BLACK);
    draw_circle(cx + DUCKY_RADIUS * 0.5, cy - DUCKY_RADIUS * 0.2, 2., BLACK);

    draw_triangle(
        vec2(cx - DUCKY_RADIUS / 2.5, cy + DUCKY_RADIUS * 0.2),
        vec2(cx + DUCKY_RADIUS / 2.5, cy + DUCKY_RADIUS * 0.2),
        vec2(cx, cy + DUCKY_RADIUS * 0.5),
        ORANGE,
    );
}
