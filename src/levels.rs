use glam::Vec2;
use macroquad::math::Rect;

use crate::draw_utils::{DrawOpts, draw_textured_poly, draw_textured_rect};

pub struct Level {
    graphics: Vec<Graphic>,
    rect_colliders: Vec<Rect>,
    poly_colliders: Vec<parry2d::shape::ConvexPolygon>,
}

pub enum Graphic {
    Rect { pos: Vec2, wh: Vec2, draw: DrawOpts },
    Polygon { points: Vec<Vec2>, draw: DrawOpts },
}

impl Graphic {
    pub fn macroquad_draw(&self) {
        match self {
            Graphic::Rect { pos, wh, draw } => {
                draw_textured_rect(*pos, *wh, draw.clone());
            }
            Graphic::Polygon { points, draw } => {
                draw_textured_poly(points, draw.clone());
            }
        }
    }
}

impl Level {
    pub fn new(rects: &[(Vec2, Vec2, DrawOpts)], polygons: &[(&[Vec2], DrawOpts)]) -> Level {
        let mut graphics = Vec::with_capacity(rects.len() + polygons.len());
        let mut rect_colliders = Vec::with_capacity(rects.len());
        let mut poly_colliders = Vec::with_capacity(polygons.len());

        for (pos, wh, draw) in rects {
            graphics.push(Graphic::Rect { pos: *pos, wh: *wh, draw: draw.clone() });
            rect_colliders.push(Rect { x: pos.x, y: pos.y, w: wh.x, h: wh.y });
        }

        for (points, draw) in polygons {
            graphics.push(Graphic::Polygon { points: points.to_vec(), draw: draw.clone() });

            let parry2d_points: Vec<_> =
                points.iter().map(|p| nalgebra::Point2::new(p.x, p.y)).collect();
            poly_colliders.push(
                parry2d::shape::ConvexPolygon::from_convex_hull(&parry2d_points)
                    .expect("invalid polygon"),
            );
        }

        Level { graphics, rect_colliders, poly_colliders }
    }

    pub fn graphics(&self) -> &[Graphic] {
        &self.graphics
    }

    pub fn rect_colliders(&self) -> &[Rect] {
        &self.rect_colliders
    }

    pub fn poly_colliders(&self) -> &[parry2d::shape::ConvexPolygon] {
        &self.poly_colliders
    }
}
