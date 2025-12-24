use std::{
    collections::HashMap,
    rc::Rc,
};

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

use crate::{
    draw_utils::{
        draw_textured_poly,
        draw_textured_rect,
    },
    raw_level::{
        RawLevel,
        Shape,
    },
};

pub struct Level {
    start_pos: Vec2,

    collision_bvh: Bvh, // ids: rects first, then polys
    rect_colliders: Box<[Rect]>,
    poly_colliders: Box<[ConvexPolygon]>,

    triggers_bvh: Bvh,
    triggers: Box<[(TriggerKind, ConvexPolygon)]>,

    graphics_background_threshold: u32, // HACK
    graphics: Box<[Graphic]>,
    graphics_bvh: Bvh,
}

pub enum Graphic {
    Rect { pos: Vec2, wh: Vec2, texture: Texture2D },
    Polygon { points: Box<[Vec2]>, texture: Texture2D },
}

impl Graphic {
    fn macroquad_draw(&self) {
        match self {
            Graphic::Rect { pos, wh, texture } => {
                draw_textured_rect(*pos, *wh, &texture.into());
            }
            Graphic::Polygon { points, texture } => {
                draw_textured_poly(points, &texture.into());
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum TriggerKind {
    Panic,
    Hello,
    ShowText(Rc<str>),
    SetEyepatch(bool),
    Goto(Vec2, StatusIcon),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum StatusIcon {
    Wrong,
    Nice,
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
                rv.push((kind.clone(), poly));
            }
        }
        rv
    }

    pub fn collision_candidates_at(&self, rect: Rect) -> (Vec<Rect>, Vec<&ConvexPolygon>) {
        let aabb = rect_aabb(rect);
        let mut rects = Vec::new();
        let mut polys = Vec::new();

        let rect_count = self.rect_colliders.len();
        for index in self.collision_bvh.intersect_aabb(&aabb) {
            if (index as usize) < rect_count {
                rects.push(self.rect_colliders[index as usize]);
            } else {
                polys.push(&self.poly_colliders[index as usize - rect_count]);
            }
        }

        (rects, polys)
    }

    pub fn start_pos(&self) -> Vec2 {
        self.start_pos
    }
}

struct LevelBuilder {
    rect_colliders: Vec<(usize, Vec2, Vec2)>,
    poly_colliders: Vec<(usize, Vec<Vec2>)>,
    rects_graphics: Vec<(usize, Vec2, Vec2, Texture2D)>,
    polygons_graphics: Vec<(usize, Vec<Vec2>, Texture2D)>,
    triggers: Vec<(TriggerKind, ConvexPolygon)>,
    level_start: Vec2,
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum LevelError {
    #[error("Invalid shape in {0} (index {1}): {2:?}")]
    InvalidShape(&'static str, usize, Vec<Vec2>),

    #[error("Unknown texture referenced: {0}")]
    UnknownTexture(String),
}

pub fn build_level(
    raw: &RawLevel,
    textures: &HashMap<&str, Texture2D>,
) -> Result<Level, LevelError> {
    let mut builder = LevelBuilder::new();
    builder.level_start = raw.start_pos;

    for (i, graphic) in raw.graphics.iter().enumerate() {
        let tex = lookup_texture(textures, &graphic.texture)?;
        match &graphic.shape {
            Shape::Rect { pos, size } => builder.rects_graphics.push((i, *pos, *size, tex)),
            Shape::Polygon(points) => builder.polygons_graphics.push((i, points.to_vec(), tex)),
        }
    }

    for (i, shape) in raw.colliders.iter().enumerate() {
        match shape {
            Shape::Rect { pos, size } => {
                builder.rect_colliders.push((i, *pos, *size));
            }
            Shape::Polygon(points) => {
                builder.poly_colliders.push((i, points.to_vec()));
            }
        }
    }

    for (i, trigger) in raw.triggers.iter().enumerate() {
        let points: &[Vec2] = match &trigger.shape {
            Shape::Rect { pos, size } => {
                &[*size, size.with_y(0.0), vec2(0.0, 0.0), size.with_x(0.0)].map(|p| p + *pos)
            }
            Shape::Polygon(points) => points,
        };
        let parry2d_points: Vec<_> = points.iter().copied().map(vec_to_parry).collect();
        let poly = ConvexPolygon::from_convex_hull(&parry2d_points)
            .ok_or_else(|| LevelError::InvalidShape("triggers", i, points.to_vec()))?;
        builder.triggers.push((trigger.kind.clone(), poly));
    }

    builder.build()
}

fn lookup_texture<'a>(
    textures: &'a HashMap<&'a str, Texture2D>,
    name: &str,
) -> Result<Texture2D, LevelError> {
    textures.get(name).ok_or_else(|| LevelError::UnknownTexture(name.to_string())).cloned()
}

impl LevelBuilder {
    pub fn new() -> LevelBuilder {
        LevelBuilder {
            rect_colliders: Vec::with_capacity(64),
            poly_colliders: Vec::with_capacity(64),
            rects_graphics: Vec::with_capacity(64),
            polygons_graphics: Vec::with_capacity(64),
            level_start: Vec2::ZERO,
            triggers: Vec::with_capacity(64),
        }
    }

    pub fn build(self) -> Result<Level, LevelError> {
        let start_pos = self.level_start;
        let mut rect_colliders = Vec::with_capacity(self.rect_colliders.len());
        let mut poly_colliders = Vec::with_capacity(self.poly_colliders.len());
        let mut collision_aabbs: Vec<Aabb> =
            Vec::with_capacity(self.rect_colliders.len() + self.poly_colliders.len());

        let rect_aabb = |pos: Vec2, wh: Vec2| Aabb::new(vec_to_parry(pos), vec_to_parry(pos + wh));

        let graphics_background_threshold =
            (self.rects_graphics.len() + self.polygons_graphics.len()) as u32;
        let mut graphics =
            Vec::with_capacity(self.rects_graphics.len() + self.polygons_graphics.len());
        let mut graphics_aabbs: Vec<Aabb> = Vec::with_capacity(graphics.capacity());

        for (_i, pos, wh, texture) in self.rects_graphics {
            graphics.push(Graphic::Rect { pos, wh, texture });
            graphics_aabbs.push(rect_aabb(pos, wh));
        }

        for (_i, points, texture) in self.polygons_graphics {
            graphics_aabbs.push(Aabb::from_points(points.iter().copied().map(vec_to_parry)));
            graphics.push(Graphic::Polygon { points: points.into(), texture });
        }

        for (_i, pos, wh) in &self.rect_colliders {
            rect_colliders.push(Rect { x: pos.x, y: pos.y, w: wh.x, h: wh.y });
            collision_aabbs.push(rect_aabb(*pos, *wh));
        }

        for (i, points) in self.poly_colliders {
            let parry2d_points: Vec<_> = points.iter().copied().map(vec_to_parry).collect();
            let poly = ConvexPolygon::from_convex_hull(&parry2d_points)
                .ok_or(LevelError::InvalidShape("triggers", i, points))?;
            poly_colliders.push(poly);
            collision_aabbs.push(Aabb::from_points(parry2d_points));
        }
        let graphics_bvh = Bvh::from_leaves(BvhBuildStrategy::Ploc, &graphics_aabbs);
        let collision_bvh = Bvh::from_leaves(BvhBuildStrategy::Ploc, &collision_aabbs);

        let triggers_bvh = {
            let aabbs = self.triggers.iter().map(|(_, poly)| poly.aabb(&Isometry2::default()));
            Bvh::from_iter(BvhBuildStrategy::Ploc, aabbs.enumerate())
        };

        Ok(Level {
            start_pos,
            rect_colliders: rect_colliders.into(),
            poly_colliders: poly_colliders.into(),
            collision_bvh,
            graphics: graphics.into(),
            graphics_bvh,
            graphics_background_threshold,
            triggers_bvh,
            triggers: self.triggers.into(),
        })
    }
}

fn vec_to_parry(v: Vec2) -> Point2<f32> {
    Point2::new(v.x, v.y)
}

fn rect_aabb(rect: Rect) -> Aabb {
    Aabb::new(Point2::new(rect.x, rect.y), Point2::new(rect.right(), rect.bottom()))
}
