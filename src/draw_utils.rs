use arrayvec::ArrayVec;
use macroquad::prelude::*;

#[derive(Debug, Clone)]
pub struct DrawOpts(Color, Texture2D);

impl From<Color> for DrawOpts {
    fn from(val: Color) -> Self {
        DrawOpts(val, Texture2D::empty())
    }
}

impl From<Texture2D> for DrawOpts {
    fn from(val: Texture2D) -> Self {
        DrawOpts(WHITE, val)
    }
}

impl From<&Texture2D> for DrawOpts {
    fn from(val: &Texture2D) -> Self {
        DrawOpts(WHITE, val.clone())
    }
}

pub fn draw_textured_rect(pos: Vec2, wh: Vec2, opts: impl Into<DrawOpts>) {
    draw_textured_poly(&[pos, pos + wh.with_y(0.), pos + wh, pos + wh.with_x(0.)], opts);
}

pub fn draw_textured_poly(points: &[Vec2], opts: impl Into<DrawOpts>) {
    debug_assert!(points.len() < 1024, "polygon is suspiciously large");

    // SAFETY: internal context does not escape this function
    let gl = unsafe { get_internal_gl() }.quad_gl;

    // can't use ArrayVec because `N * 3` isn't a thing. Boo
    // https://github.com/rust-lang/rust/issues/76560
    let mut vertices = Vec::<Vertex>::with_capacity(points.len());
    let mut indices = Vec::<u16>::with_capacity(points.len() * 3);

    let (min_x, min_y) = {
        let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
        for &Vec2 { x, y } in points {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
        }
        (min_x, min_y)
    };

    // what's a few clone() calls between friends
    let DrawOpts(color, texture) = opts.into().clone();

    for (i, point) in points.iter().enumerate() {
        let (dx, dy) = (point.x - min_x, point.y - min_y);
        let (u, v) = (dx / texture.width(), dy / texture.height());
        vertices.push(Vertex::new(point.x, point.y, 0., u, v, color));

        #[allow(clippy::cast_possible_truncation, reason = "see debug_assert")]
        if i != 0 && i != points.len() - 1 {
            indices.extend_from_slice(&[0, i as u16, i as u16 + 1]);
        }
    }

    gl.texture(Some(&texture));
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

    draw_textured_poly(&points, color);
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
