use std::collections::HashMap;

use glam::Vec2;
use macroquad::math::Rect;
use nalgebra::Point2;
use parry2d::{
    bounding_volume::Aabb,
    partitioning::{
        Bvh,
        BvhBuildStrategy,
    },
};

use crate::draw_utils::{
    DrawOpts,
    draw_textured_poly,
    draw_textured_rect,
};

pub struct Level {
    rect_colliders: Vec<Rect>,
    poly_colliders: Vec<parry2d::shape::ConvexPolygon>,
    start_pos: Vec2,
    graphics_background_threshold: u32, // HACK
    graphics: Vec<Graphic>,
    graphics_bvh: parry2d::partitioning::Bvh,
}

pub enum Graphic {
    Rect { pos: Vec2, wh: Vec2, draw: DrawOpts },
    Polygon { points: Vec<Vec2>, draw: DrawOpts },
}

impl Graphic {
    fn macroquad_draw(&self) {
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
    pub fn macroquad_draw(&self, rect: Rect) {
        let aabb =
            Aabb::new(Point2::new(rect.x, rect.y), Point2::new(rect.x + rect.w, rect.y + rect.h));

        let indices: Vec<u32> = self.graphics_bvh.intersect_aabb(&aabb).collect();

        // TODO: implement proper layers
        for &index in &indices {
            if index < self.graphics_background_threshold {
                self.graphics[index as usize].macroquad_draw();
            }
        }
        for index in indices {
            if index >= self.graphics_background_threshold {
                self.graphics[index as usize].macroquad_draw();
            }
        }
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
        // TODO: skip drawing stuff when texture is None
        self.polygons.push((points.to_vec(), self.texture_to_draw_opts(texture)));
    }

    pub fn rect(&mut self, texture: Option<&'static str>, xy: Vec2, wh: Vec2) {
        // TODO: skip drawing stuff when texture is None
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
        let start_pos = self.level_start;
        let mut graphics = Vec::with_capacity(
            self.rects.len()
                + self.polygons.len()
                + self.rects_graphics.len()
                + self.polygons_graphics.len(),
        );
        let mut rect_colliders = Vec::with_capacity(self.rects.len());
        let mut poly_colliders = Vec::with_capacity(self.polygons.len());
        let mut graphics_aabbs: Vec<Aabb> = Vec::with_capacity(graphics.capacity());

        let rect_aabb = |pos: Vec2, wh: Vec2| {
            Aabb::new(Point2::new(pos.x, pos.y), Point2::new(pos.x + wh.x, pos.y + wh.y))
        };

        for (pos, wh, draw) in &self.rects_graphics {
            graphics.push(Graphic::Rect { pos: *pos, wh: *wh, draw: draw.clone() });
            graphics_aabbs.push(rect_aabb(*pos, *wh));
        }

        for (points, draw) in &self.polygons_graphics {
            graphics.push(Graphic::Polygon { points: points.clone(), draw: draw.clone() });
            graphics_aabbs.push(Aabb::from_points(points.iter().map(|p| Point2::new(p.x, p.y))));
        }

        for (pos, wh, draw) in &self.rects {
            graphics.push(Graphic::Rect { pos: *pos, wh: *wh, draw: draw.clone() });
            rect_colliders.push(Rect { x: pos.x, y: pos.y, w: wh.x, h: wh.y });
            graphics_aabbs.push(rect_aabb(*pos, *wh));
        }

        for (points, draw) in &self.polygons {
            let points = points.clone();
            let parry2d_points: Vec<_> =
                points.iter().map(|p| nalgebra::Point2::new(p.x, p.y)).collect();

            poly_colliders.push(
                parry2d::shape::ConvexPolygon::from_convex_hull(&parry2d_points)
                    .expect("invalid polygon"),
            );
            graphics_aabbs.push(Aabb::from_points(points.iter().map(|p| Point2::new(p.x, p.y))));
            graphics.push(Graphic::Polygon { points, draw: draw.clone() });
        }

        let graphics_bvh = Bvh::from_leaves(BvhBuildStrategy::Ploc, &graphics_aabbs);

        #[expect(clippy::cast_possible_truncation)]
        Level {
            rect_colliders,
            poly_colliders,
            start_pos,
            graphics,
            graphics_bvh,
            graphics_background_threshold: (self.rects_graphics.len()
                + self.polygons_graphics.len()) as u32,
        }
    }
}
