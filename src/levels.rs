use std::collections::HashMap;

use glam::{
    Vec2,
    vec2,
};
use macroquad::{
    math::{
        Circle,
        Rect,
    },
    texture::Texture2D,
};
use nalgebra::{
    Isometry2,
    Point2,
};
use parry2d::{
    bounding_volume::Aabb,
    partitioning::{
        Bvh,
        BvhBuildStrategy,
    },
    query::details::intersection_test_ball_point_query,
    shape::{
        Ball,
        ConvexPolygon,
    },
};

use crate::draw_utils::{
    draw_textured_poly,
    draw_textured_rect,
};

pub struct Level {
    rect_colliders: Vec<Rect>,
    poly_colliders: Vec<ConvexPolygon>,
    start_pos: Vec2,

    triggers_bvh: Bvh,
    triggers: Vec<(TriggerKind, ConvexPolygon)>,

    graphics_background_threshold: u32, // HACK
    graphics: Vec<Graphic>,
    graphics_bvh: Bvh,
}

pub enum Graphic {
    Rect { pos: Vec2, wh: Vec2, texture: Texture2D },
    Polygon { points: Vec<Vec2>, texture: Texture2D },
}

impl Graphic {
    fn macroquad_draw(&self) {
        match self {
            Graphic::Rect { pos, wh, texture } => {
                draw_textured_rect(*pos, *wh, texture.into());
            }
            Graphic::Polygon { points, texture } => {
                draw_textured_poly(points, texture.into());
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TriggerKind {
    Panic,
    Hello,
    DebugText(&'static str),
}

impl Level {
    pub fn macroquad_draw(&self, rect: Rect) {
        let aabb = rect_aabb(rect);
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

    pub fn triggers_at(&self, circle: Circle) -> Vec<(TriggerKind, &ConvexPolygon)> {
        let mut rv = Vec::new();
        let start = circle.point() - vec2(circle.r, circle.r);
        let end = circle.point() + vec2(circle.r, circle.r);
        let aabb = Aabb::new(vec_to_parry(start), vec_to_parry(end));

        let ball = Ball::new(circle.radius());
        let ball_pos_inv = Isometry2::translation(-circle.x, -circle.y);
        for index in self.triggers_bvh.intersect_aabb(&aabb) {
            let (kind, poly) = &self.triggers[index as usize];
            if intersection_test_ball_point_query(&ball_pos_inv, &ball, poly) {
                rv.push((*kind, poly));
            }
        }
        rv
    }

    pub fn rect_colliders(&self) -> &[Rect] {
        &self.rect_colliders
    }

    pub fn poly_colliders(&self) -> &[ConvexPolygon] {
        &self.poly_colliders
    }

    pub fn start_pos(&self) -> Vec2 {
        self.start_pos
    }
}

pub struct LevelBuilder {
    textures: HashMap<&'static str, Texture2D>,
    rects: Vec<(Vec2, Vec2, Option<&'static str>)>,
    polygons: Vec<(Vec<Vec2>, Option<&'static str>)>,
    rects_graphics: Vec<(Vec2, Vec2, &'static str)>,
    polygons_graphics: Vec<(Vec<Vec2>, &'static str)>,
    triggers: Vec<(TriggerKind, ConvexPolygon)>,
    level_start: Vec2,
}

impl LevelBuilder {
    pub fn new(textures: HashMap<&'static str, Texture2D>) -> LevelBuilder {
        LevelBuilder {
            textures,
            rects: Vec::with_capacity(64),
            polygons: Vec::with_capacity(64),
            rects_graphics: Vec::with_capacity(32),
            polygons_graphics: Vec::with_capacity(32),
            level_start: Vec2::ZERO,
            triggers: Vec::new(),
        }
    }

    pub fn lookup_texture_or_die(&self, name: &'static str) -> Texture2D {
        self.textures
            .get(name)
            .unwrap_or_else(|| panic!("Unknown texture referenced: {name}"))
            .clone()
    }

    pub fn polygon(&mut self, texture: Option<&'static str>, points: &[Vec2]) {
        // TODO: skip drawing stuff when texture is None
        self.polygons.push((points.to_vec(), texture));
    }

    pub fn rect(&mut self, texture: Option<&'static str>, xy: Vec2, wh: Vec2) {
        // TODO: skip drawing stuff when texture is None
        self.rects.push((xy, wh, texture));
    }

    pub fn rect_trigger(&mut self, action: TriggerKind, xy: Vec2, wh: Vec2) {
        debug_assert!(wh.is_finite() && wh.x >= 0.0 && wh.y >= 0.0, "Invalid 'wh': {wh}");
        let points = [xy + wh, xy + wh.with_y(0.0), xy, xy + wh.with_x(0.0)];
        self.polygon_trigger(action, &points);
    }

    pub fn polygon_graphics(&mut self, texture: &'static str, points: &[Vec2]) {
        self.polygons_graphics.push((points.to_vec(), texture));
    }

    pub fn rect_graphics(&mut self, texture: &'static str, xy: Vec2, wh: Vec2) {
        self.rects_graphics.push((xy, wh, texture));
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

        let rect_aabb = |pos: Vec2, wh: Vec2| Aabb::new(vec_to_parry(pos), vec_to_parry(pos + wh));

        for (pos, wh, tex_name) in &self.rects_graphics {
            graphics.push(Graphic::Rect {
                pos: *pos,
                wh: *wh,
                texture: self.lookup_texture_or_die(tex_name),
            });
            graphics_aabbs.push(rect_aabb(*pos, *wh));
        }

        for (points, tex_name) in &self.polygons_graphics {
            graphics.push(Graphic::Polygon {
                points: points.clone(),
                texture: self.lookup_texture_or_die(tex_name),
            });
            graphics_aabbs.push(Aabb::from_points(points.iter().copied().map(vec_to_parry)));
        }

        for (pos, wh, tex_name) in &self.rects {
            if let Some(tex_name) = tex_name {
                graphics.push(Graphic::Rect {
                    pos: *pos,
                    wh: *wh,
                    texture: self.lookup_texture_or_die(tex_name),
                });
                graphics_aabbs.push(rect_aabb(*pos, *wh));
            }
            rect_colliders.push(Rect { x: pos.x, y: pos.y, w: wh.x, h: wh.y });
        }

        for (points, tex_name) in &self.polygons {
            let points = points.clone();
            let parry2d_points: Vec<_> = points.iter().copied().map(vec_to_parry).collect();

            poly_colliders
                .push(ConvexPolygon::from_convex_hull(&parry2d_points).expect("invalid polygon"));

            if let Some(tex_name) = tex_name {
                graphics_aabbs.push(Aabb::from_points(points.iter().copied().map(vec_to_parry)));
                graphics.push(Graphic::Polygon {
                    points,
                    texture: self.lookup_texture_or_die(tex_name),
                });
            }
        }
        let graphics_bvh = Bvh::from_leaves(BvhBuildStrategy::Ploc, &graphics_aabbs);

        let triggers_bvh = {
            let aabbs = self.triggers.iter().map(|(_, poly)| poly.aabb(&Isometry2::default()));
            Bvh::from_iter(BvhBuildStrategy::Ploc, aabbs.enumerate())
        };

        #[expect(clippy::cast_possible_truncation)]
        Level {
            rect_colliders,
            poly_colliders,
            start_pos,
            graphics,
            graphics_bvh,
            graphics_background_threshold: (self.rects_graphics.len()
                + self.polygons_graphics.len()) as u32,

            triggers_bvh,
            triggers: self.triggers,
        }
    }

    pub fn polygon_trigger(&mut self, action: TriggerKind, points: &[Vec2]) {
        let parry2d_points: Vec<_> = points.iter().copied().map(vec_to_parry).collect();
        let poly = ConvexPolygon::from_convex_hull(&parry2d_points)
            .unwrap_or_else(|| panic!("Invalid polygon provided: {points:?}"));
        self.triggers.push((action, poly));
    }
}

fn vec_to_parry(v: Vec2) -> Point2<f32> {
    Point2::new(v.x, v.y)
}

fn rect_aabb(rect: Rect) -> Aabb {
    Aabb::new(Point2::new(rect.x, rect.y), Point2::new(rect.right(), rect.bottom()))
}
