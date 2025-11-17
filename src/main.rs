use macroquad::prelude::*;
use std::f32::consts::PI;

const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 768;

const SCREEN_SIZE: Vec2 = vec2(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);

/// At what point to start scrolling the screen (x, y)
const SCROLL_MARGIN_START: Vec2 = vec2(300., 100.);
/// At what point to stop scrolling the screen (x, y)
const SCROLL_MARGIN_STOP: Vec2 = vec2(400., 300.);
const MIN_SCROLL_SPEED: Vec2 = vec2(4., 3.);

const DUCKY_RADIUS: f32 = 18.0;

const UPDATES_PER_SECOND: f64 = 120.;
const UPDATE_FRAME_TIME: f64 = 1. / UPDATES_PER_SECOND;

const COYOTE_DURATION: u8 = 12;
const GRAVITY: f32 = 0.4;
const JUMP_SPEED: f32 = 10.;
const MAX_GRAVITY_SPEED: f32 = 15.;
const MAX_WALK_SPEED: f32 = 2.;

const FRICTION_AIR: f32 = 0.99;
const FRICTION_GROUND: f32 = 0.9;

const WALK_ACCEL_GROUND: f32 = 0.2;
const WALK_ACCEL_AIR: f32 = 0.1;

fn window_conf() -> Conf {
    Conf {
        window_title: "ducky".to_string(),
        window_width: WINDOW_WIDTH as i32,
        window_height: WINDOW_HEIGHT as i32,
        window_resizable: false,
        sample_count: 0,
        ..Default::default()
    }
}

struct Ducky {
    pos: Vec2,
    vel: Vec2,
    coyote_time: u8,
}

struct Level {
    rects: Vec<(Rect, Color)>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Scroll {
    Neg,
    Pos,
    None,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct ScrollState {
    x: Scroll,
    y: Scroll,
}

struct GameState {
    viewport_offset: Vec2,
    scroll: ScrollState,
    ducky: Ducky,
    level: Level,
}

fn viewport_offset_to_camera(offset: Vec2) -> Camera2D {
    let w = WINDOW_WIDTH as f32;
    let h = WINDOW_HEIGHT as f32;

    // I don't understand why, but `from_display_rect` flips the height portion by
    // default, or something like that
    Camera2D::from_display_rect(Rect { x: offset.x.round(), y: offset.y.round() + h, w, h: -h })
}

fn init_game_state() -> GameState {
    let ducky = Ducky { pos: vec2(100., 100.), vel: Vec2::ZERO, coyote_time: 0 };
    #[rustfmt::skip]
    let rects = [
        ((0., 20., 40., 4.), BROWN),
        ((20.5, 16., 3., 2.), ORANGE),
        ((24., 13., 3., 2.), ORANGE),
        ((28., 10.5, 3., 6.), ORANGE),
    ].map(|((x, y, w, h), color)| (
        Rect::new(x * 32., y * 32., w * 32., h * 32.),
        color,
    )).to_vec();

    GameState {
        viewport_offset: Vec2::ZERO,
        ducky,
        scroll: ScrollState { x: Scroll::None, y: Scroll::None },
        level: Level { rects },
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    rand::srand(miniquad::date::now() as u64);

    let mut state = init_game_state();

    let mut time_bank: f64 = 0.0;
    let mut last_time = get_time();

    loop {
        // Frame debt logic
        let now = get_time();
        time_bank += now - last_time;
        last_time = now;

        while time_bank >= UPDATE_FRAME_TIME {
            fixed_update(&mut state, SCREEN_SIZE);
            time_bank -= UPDATE_FRAME_TIME;
        }

        // Main rendering
        set_camera(&viewport_offset_to_camera(state.viewport_offset));
        clear_background(DARKBLUE);
        for &(Rect { x, y, w, h }, color) in &state.level.rects {
            draw_rectangle(x, y, w, h, color);
        }
        draw_ducky(state.ducky.pos.x.round(), state.ducky.pos.y.round());
        set_default_camera();

        // Debug information
        draw_text(&format!("coyote={}", state.ducky.coyote_time), 32., 32., 16., WHITE);
        next_frame().await
    }
}

fn adjust_viewport_offset(
    ducky_pos: Vec2,
    ducky_speed: Vec2,
    screen_size: Vec2,
    mut offset: Vec2,
    mut scroll: ScrollState,
) -> (Vec2, ScrollState) {
    let delta_pos = ducky_pos - offset;

    if scroll.x == Scroll::None {
        scroll.x = if delta_pos.x >= screen_size.x - SCROLL_MARGIN_START.x {
            Scroll::Pos
        } else if delta_pos.x < SCROLL_MARGIN_START.x {
            Scroll::Neg
        } else {
            Scroll::None
        }
    } else {
        scroll.x = if delta_pos.x >= screen_size.x - SCROLL_MARGIN_STOP.x {
            Scroll::Pos
        } else if delta_pos.x < SCROLL_MARGIN_STOP.x {
            Scroll::Neg
        } else {
            Scroll::None
        }
    }

    // same as previous `if` but with `x` instead of `y`
    if scroll.y == Scroll::None {
        scroll.y = if delta_pos.y >= screen_size.y - SCROLL_MARGIN_START.y {
            Scroll::Pos
        } else if delta_pos.y < SCROLL_MARGIN_START.y {
            Scroll::Neg
        } else {
            Scroll::None
        }
    } else {
        scroll.y = if delta_pos.y >= screen_size.y - SCROLL_MARGIN_STOP.y {
            Scroll::Pos
        } else if delta_pos.y < SCROLL_MARGIN_STOP.y {
            Scroll::Neg
        } else {
            Scroll::None
        }
    }

    match scroll.x {
        Scroll::Pos => offset.x += ducky_speed.x.abs().max(MIN_SCROLL_SPEED.x),
        Scroll::Neg => offset.x -= ducky_speed.x.abs().max(MIN_SCROLL_SPEED.x),
        Scroll::None => {}
    }

    match scroll.y {
        Scroll::Pos => offset.y += ducky_speed.y.abs().max(MIN_SCROLL_SPEED.y),
        Scroll::Neg => offset.y -= ducky_speed.y.abs().max(MIN_SCROLL_SPEED.y),
        Scroll::None => {}
    }

    offset.x = offset.x.max(0.);
    offset.y = offset.y.min(0.);

    (offset, scroll)
}

fn fixed_update(state: &mut GameState, screen_size: Vec2) {
    let ducky = &mut state.ducky;

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

    // Collision detection
    let mut is_grounded = false;
    let ducky_circle = Circle::new(ducky.pos.x, ducky.pos.y, DUCKY_RADIUS);
    for (rect, _) in &state.level.rects {
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
    ducky.pos += ducky.vel;

    // Adjusting scrolling
    (state.viewport_offset, state.scroll) = adjust_viewport_offset(
        ducky.pos, ducky.vel, screen_size, state.viewport_offset, state.scroll,
    );
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

fn draw_arrow(start: Vec2, end: Vec2, color: Color) {
    let delta = (start - end).normalize_or_zero();

    let v1 = end;
    let v2 = end + delta * 12. + delta.perp() * 6.;
    let v3 = end + delta * 12. - delta.perp() * 6.;
    draw_line(start.x, start.y, end.x, end.y, 2., color);
    draw_triangle(v1, v2, v3, color);
}

fn draw_ducky(cx: f32, cy: f32) {
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
