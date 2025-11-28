use std::f32::consts::PI;

use arrayvec::ArrayVec;
use macroquad::prelude::*;
use miniquad::{
    RenderingBackend,
    TextureWrap,
};

#[derive(Debug, Clone)]
pub struct DrawOpts {
    color: Color,
    texture: Texture2D,
}

impl From<Color> for DrawOpts {
    fn from(val: Color) -> Self {
        DrawOpts { color: val, texture: Texture2D::empty() }
    }
}

impl From<Texture2D> for DrawOpts {
    fn from(val: Texture2D) -> Self {
        DrawOpts { color: WHITE, texture: val }
    }
}

impl From<&Texture2D> for DrawOpts {
    fn from(val: &Texture2D) -> Self {
        DrawOpts { color: WHITE, texture: val.weak_clone() }
    }
}

pub fn draw_textured_rect(pos: Vec2, wh: Vec2, opts: &DrawOpts) {
    // SAFETY: internal context does not escape this function
    let ctx = unsafe { get_internal_gl() };
    let gl = ctx.quad_gl;
    let DrawOpts { color, texture } = opts;

    #[rustfmt::skip]
    let vertices = if is_texture_repeating(ctx.quad_context, texture) {
        // this draws the texture as if it started at (0, 0)
        let points = [pos, pos + wh.with_y(0.0), pos + wh, pos + wh.with_x(0.0)];
        points.map(|point| {
            let (u, v) = (point.x / texture.width(), point.y / texture.height());
            Vertex::new(point.x, point.y, 0., u, v, *color)
        })
    } else {
        [
            Vertex::new(pos.x,        pos.y,        0.0, 0.0, 0.0, *color),
            Vertex::new(pos.x + wh.x, pos.y,        0.0, 1.0, 0.0, *color),
            Vertex::new(pos.x + wh.x, pos.y + wh.y, 0.0, 1.0, 1.0, *color),
            Vertex::new(pos.x,        pos.y + wh.y, 0.0, 0.0, 1.0, *color),
        ]
    };

    gl.texture(Some(texture));
    gl.draw_mode(DrawMode::Triangles);
    gl.geometry(&vertices, &[0, 1, 3, 1, 2, 3]);
}

pub fn draw_textured_poly(points: &[Vec2], opts: &DrawOpts) {
    debug_assert!(points.len() < 1024, "polygon is suspiciously large");

    // SAFETY: internal context does not escape this function
    let ctx = unsafe { get_internal_gl() };
    let gl = ctx.quad_gl;

    let mut vertices = Vec::<Vertex>::with_capacity(points.len());
    let mut indices = Vec::<u16>::with_capacity(points.len() * 3);

    let DrawOpts { color, texture } = opts;

    for (i, point) in points.iter().enumerate() {
        // Texturing a polygon probably doesn't make sense with a non-repeating texture, right?
        let (u, v) = (point.x / texture.width(), point.y / texture.height());
        vertices.push(Vertex::new(point.x, point.y, 0., u, v, *color));

        #[expect(clippy::cast_possible_truncation, reason = "see debug_assert")]
        if i != 0 && i != points.len() - 1 {
            indices.extend_from_slice(&[0, i as u16, i as u16 + 1]);
        }
    }

    gl.texture(Some(texture));
    gl.draw_mode(DrawMode::Triangles);
    gl.geometry(&vertices, &indices);
}

/// Draw a circle that's partially cut off on the top.
/// `ratio` must be a value from 0 to 1.
pub fn draw_vclipped_circle(x: f32, y: f32, radius: f32, ratio: f32, color: Color) {
    use core::f32::consts::PI;

    debug_assert!(radius.is_finite() && radius >= 0.0, "invalid radius: {radius}");
    debug_assert!(ratio.is_finite() && (0.0..=1.0).contains(&ratio), "invalid ratio: {ratio}");

    let theta = (2.0 * ratio - 1.0).asin();

    let mut points = ArrayVec::<Vec2, 22>::new();

    let deltas: ArrayVec<(f32, f32), 10> = (0..10u8)
        .map(|i| {
            let (sin, cos) = theta.lerp(-PI / 2.0, f32::from(i) / 10.0).sin_cos();
            (cos * radius, sin * radius)
        })
        .collect();

    for &(dx, dy) in &deltas {
        points.push(vec2(x + dx, y - dy));
    }
    points.push(vec2(x, y + radius));
    for (dx, dy) in deltas.into_iter().rev() {
        points.push(vec2(x - dx, y - dy));
    }
    points.push(points[0]);

    draw_textured_poly(&points, &color.into());
}

#[allow(dead_code, reason = "this function is useful for debugging")]
pub fn draw_arrow(start: Vec2, end: Vec2, color: Color) {
    let delta = (start - end).normalize_or_zero();

    let v1 = end;
    let v2 = end + delta * 12. + delta.perp() * 6.;
    let v3 = end + delta * 12. - delta.perp() * 6.;
    draw_line(start.x, start.y, end.x, end.y, 2., color);
    draw_triangle(v1, v2, v3, color);
}

/// TODO: handle partially transparent colors
#[rustfmt::skip]
pub fn draw_rounded_rect(pos: Vec2, wh: Vec2, r: f32, color: Color) {
    // AxxxxB
    // yyyyyy
    // CzzzzD

    draw_rectangle(pos.x + r, pos.y,            wh.x - r * 2.0, r,              color); // x
    draw_rectangle(pos.x,     pos.y + r,        wh.x,           wh.y - r * 2.0, color); // y
    draw_rectangle(pos.x + r, pos.y + wh.y - r, wh.x - r * 2.0, r,              color); // z

    let opts = color.into();
    draw_quarter_circle(pos + vec2(r, r),        r, -PI/2., &opts); // A
    draw_quarter_circle(pos + vec2(wh.x - r, r), r, 0.0,    &opts); // B
    draw_quarter_circle(pos + vec2(r, wh.y - r), r, PI,     &opts); // C
    draw_quarter_circle(pos + wh - vec2(r, r),   r, PI/2.,  &opts); // D

}

pub fn draw_quarter_circle(corner: Vec2, r: f32, rotate: f32, opts: &DrawOpts) {
    let mut points: ArrayVec<Vec2, 12> = ArrayVec::new();
    points.push(corner);
    for n in 0u8..=10 {
        let angle = -PI / 2.0 * (f32::from(n) / 10.0) + rotate;
        points.push(corner + Vec2::from_angle(angle) * r);
    }
    draw_textured_poly(&points, opts);
}

fn is_texture_repeating(backend: &dyn RenderingBackend, texture: &Texture2D) -> bool {
    let id = texture.raw_miniquad_id();
    matches!(backend.texture_params(id).wrap, TextureWrap::Repeat)
}
