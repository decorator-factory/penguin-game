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

/// Draw a circle that's partially filled.
/// `ratio` must be a value from 0 to 1
#[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub fn draw_vclipped_circle(x: f32, y: f32, radius: f32, ratio: f32, color: Color) {
    debug_assert!(radius.is_finite() && radius >= 0.0, "invalid radius: {radius}");
    debug_assert!(ratio.is_finite() && (0.0..=1.0).contains(&ratio), "invalid ratio: {ratio}");

    let target = render_target((radius * 2.0) as u32, (radius * 2.0) as u32);
    push_camera_state();
    let camera = Camera2D { render_target: Some(target), ..Default::default() };
    set_camera(&camera);
    draw_circle(0.0, 0.0, 1.0, color);
    pop_camera_state();
    let texture = camera.render_target.unwrap().texture;

    let y_offset = (1.0 - ratio) * radius * 2.0;

    draw_texture_ex(&texture, x - radius, y - radius + y_offset, WHITE, DrawTextureParams {
        source: Some(Rect { x: 0.0, y: y_offset, w: radius * 2.0, h: radius * 2.0 - y_offset }),
        ..Default::default()
    });
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
