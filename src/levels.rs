use std::collections::HashMap;

use glam::Vec2;
use macroquad::math::Rect;

use crate::draw_utils::{
    DrawOpts,
    draw_textured_poly,
    draw_textured_rect,
};

pub struct Level {
    graphics: Vec<Graphic>,
    rect_colliders: Vec<Rect>,
    poly_colliders: Vec<parry2d::shape::ConvexPolygon>,
    start_pos: Vec2,
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
    pub fn new(
        start_pos: Vec2,
        rects: &[(Vec2, Vec2, DrawOpts)],
        polygons: &[(Vec<Vec2>, DrawOpts)],
    ) -> Level {
        let mut graphics = Vec::with_capacity(rects.len() + polygons.len());
        let mut rect_colliders = Vec::with_capacity(rects.len());
        let mut poly_colliders = Vec::with_capacity(polygons.len());

        for (pos, wh, draw) in rects {
            graphics.push(Graphic::Rect { pos: *pos, wh: *wh, draw: draw.clone() });
            rect_colliders.push(Rect { x: pos.x, y: pos.y, w: wh.x, h: wh.y });
        }

        for (points, draw) in polygons {
            let points = points.clone();
            let parry2d_points: Vec<_> =
                points.iter().map(|p| nalgebra::Point2::new(p.x, p.y)).collect();

            poly_colliders.push(
                parry2d::shape::ConvexPolygon::from_convex_hull(&parry2d_points)
                    .expect("invalid polygon"),
            );
            graphics.push(Graphic::Polygon { points, draw: draw.clone() });
        }

        Level { graphics, rect_colliders, poly_colliders, start_pos }
    }

    pub fn insert_graphics(
        &mut self,
        rects: &[(Vec2, Vec2, DrawOpts)],
        polygons: &[(Vec<Vec2>, DrawOpts)],
    ) {
        for (pos, wh, draw) in rects {
            self.graphics.push(Graphic::Rect { pos: *pos, wh: *wh, draw: draw.clone() });
        }

        for (points, draw) in polygons {
            self.graphics.push(Graphic::Polygon { points: points.clone(), draw: draw.clone() });
        }

        // make sure the new graphics are in the background
        self.graphics.rotate_right(rects.len() + polygons.len());
    }

    pub fn insert_colliders(&mut self, rects: &[(Vec2, Vec2)], polygons: &[&[Vec2]]) {
        for (pos, wh) in rects {
            self.rect_colliders.push(Rect { x: pos.x, y: pos.y, w: wh.x, h: wh.y });
        }

        for points in polygons {
            let parry2d_points: Vec<_> =
                points.iter().map(|p| nalgebra::Point2::new(p.x, p.y)).collect();
            self.poly_colliders.push(
                parry2d::shape::ConvexPolygon::from_convex_hull(&parry2d_points)
                    .expect("invalid polygon"),
            );
        }
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

    pub fn start_pos(&self) -> Vec2 {
        self.start_pos
    }
}

pub struct LevelBuilder {
    draw_opts: HashMap<&'static str, DrawOpts>,
    rects: Vec<(Vec2, Vec2, DrawOpts)>,
    polygons: Vec<(Vec<Vec2>, DrawOpts)>,
    rects_graphics: Vec<(Vec2, Vec2, DrawOpts)>,
    polygons_graphics: Vec<(Vec<Vec2>, DrawOpts)>,
    level_start: Vec2,
}

impl LevelBuilder {
    pub fn new(draw_opts: HashMap<&'static str, DrawOpts>) -> LevelBuilder {
        LevelBuilder {
            draw_opts,
            rects: Vec::with_capacity(64),
            polygons: Vec::with_capacity(64),
            rects_graphics: Vec::with_capacity(32),
            polygons_graphics: Vec::with_capacity(32),
            level_start: Vec2::ZERO,
        }
    }

    fn texture_to_draw_opts(&self, texture: Option<&'static str>) -> DrawOpts {
        match texture {
            Some(name) => self.draw_opts[name].clone(),
            None => macroquad::color::Color::new(0.0, 0.0, 0.0, 0.0).into(),
        }
    }

    pub fn polygon(&mut self, texture: Option<&'static str>, points: &[Vec2]) {
        self.polygons.push((points.to_vec(), self.texture_to_draw_opts(texture)));
    }

    pub fn rect(&mut self, texture: Option<&'static str>, xy: Vec2, wh: Vec2) {
        self.rects.push((xy, wh, self.texture_to_draw_opts(texture)));
    }

    pub fn polygon_graphics(&mut self, texture: &'static str, points: &[Vec2]) {
        self.polygons_graphics.push((points.to_vec(), self.texture_to_draw_opts(Some(texture))));
    }

    pub fn rect_graphics(&mut self, texture: &'static str, xy: Vec2, wh: Vec2) {
        self.rects_graphics.push((xy, wh, self.texture_to_draw_opts(Some(texture))));
    }

    pub fn level_start(&mut self, point: Vec2) {
        self.level_start = point;
    }

    pub fn build_or_die(self) -> Level {
        let mut level = Level::new(self.level_start, &self.rects, &self.polygons);
        level.insert_graphics(&self.rects_graphics, &self.polygons_graphics);
        level
    }
}
