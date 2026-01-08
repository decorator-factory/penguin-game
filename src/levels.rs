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
        TriMesh,
    },
};

use crate::{
    draw_utils::{
        DrawOpts,
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
    poly_colliders: Box<[TriMesh]>,

    triggers_bvh: Bvh,
    triggers: Box<[(TriggerKind, TriMesh)]>,

    // TODO: specify draw order
    graphics: Box<[Graphic]>,
    graphics_bvh: Bvh,
}

pub enum Graphic {
    Rect { pos: Vec2, size: Vec2, opts: DrawOpts },
    Polygon { points: Rc<[Vec2]>, opts: DrawOpts },
}

impl Graphic {
    fn macroquad_draw(&self) {
        match self {
            Graphic::Rect { pos, size, opts } => {
                draw_textured_rect(*pos, *size, opts);
            }
            Graphic::Polygon { points, opts } => {
                draw_textured_poly(points, opts);
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
        for &i in &indices {
            self.graphics[i as usize].macroquad_draw();
        }
    }

    pub fn triggers_at(&self, circle: Circle) -> Vec<(TriggerKind, &TriMesh)> {
        let mut rv = Vec::new();
        let start = circle.point() - vec2(circle.r, circle.r);
        let end = circle.point() + vec2(circle.r, circle.r);
        let aabb = Aabb::new(to_nalgebra(start), to_nalgebra(end));

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

    pub fn collision_candidates_at(&self, rect: Rect) -> (Vec<Rect>, Vec<&TriMesh>) {
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

#[derive(Clone, Debug, thiserror::Error)]
pub enum LevelError {
    #[error("Invalid shape in {0} (index {1}): {2:?}")]
    InvalidShape(&'static str, usize, Box<[Vec2]>),

    #[error("Unknown texture referenced: {0}")]
    UnknownTexture(String),
}

pub fn build_level(
    raw: RawLevel,
    textures: &HashMap<&str, Texture2D>,
) -> Result<Level, LevelError> {
    let mut graphics = Vec::with_capacity(raw.graphics.len());
    let mut graphics_aabbs: Vec<Aabb> = Vec::with_capacity(raw.graphics.len());
    for (i, graphic) in raw.graphics.into_iter().enumerate() {
        let texture = lookup_texture(textures, &graphic.texture)?;
        match graphic.shape {
            Shape::Rect { pos, size } => {
                graphics.push(Graphic::Rect { pos, size, opts: texture.into() });
                graphics_aabbs.push(poswh_aabb(pos, size));
            }
            Shape::Polygon(points) => {
                // TODO: only decompose each polygon once, for graphics, triggers, collision
                let polys = decompose_concave(&points).ok_or_else(|| {
                    LevelError::InvalidShape("graphics", i, points.to_vec().into())
                })?;
                for poly in polys {
                    graphics.push(Graphic::Polygon {
                        points: poly.iter().copied().map(from_nalgebra).collect(),
                        opts: texture.clone().into(),
                    });
                    graphics_aabbs.push(Aabb::from_points(poly));
                }
            }
        }
    }

    let mut rect_colliders = Vec::with_capacity(raw.colliders.len());
    let mut poly_colliders = Vec::with_capacity(raw.colliders.len());
    let mut rect_collider_aabbs: Vec<Aabb> = Vec::with_capacity(raw.colliders.len());
    let mut poly_collider_aabbs: Vec<Aabb> = Vec::with_capacity(raw.colliders.len());

    for (i, shape) in raw.colliders.iter().enumerate() {
        match shape {
            Shape::Rect { pos, size } => {
                rect_colliders.push(Rect { x: pos.x, y: pos.y, w: size.x, h: size.y });
                rect_collider_aabbs.push(poswh_aabb(*pos, *size));
            }
            Shape::Polygon(points) => {
                let parry2d_points: Vec<_> = points.iter().copied().map(to_nalgebra).collect();
                poly_collider_aabbs.push(Aabb::from_points(parry2d_points.iter().copied()));
                let poly = try_mesh_both_ways(parry2d_points).ok_or_else(|| {
                    LevelError::InvalidShape("colliders", i, points.to_vec().into())
                })?;
                poly_colliders.push(poly);
            }
        }
    }

    let triggers: Result<Box<[_]>, _> = raw
        .triggers
        .into_iter()
        .enumerate()
        .map(|(i, trigger)| -> Result<(TriggerKind, TriMesh), LevelError> {
            let points: &[Vec2] = match &trigger.shape {
                Shape::Rect { pos, size } => {
                    &[*size, size.with_y(0.0), vec2(0.0, 0.0), size.with_x(0.0)].map(|p| p + *pos)
                }
                Shape::Polygon(points) => points,
            };
            let parry2d_points: Vec<_> = points.iter().copied().map(to_nalgebra).collect();
            match try_mesh_both_ways(parry2d_points) {
                Some(poly) => Ok((trigger.kind.clone(), poly)),
                None => Err(LevelError::InvalidShape("triggers", i, points.into())),
            }
        })
        .collect();
    let triggers = triggers?;

    let graphics_bvh = Bvh::from_leaves(BvhBuildStrategy::Ploc, &graphics_aabbs);
    let collision_bvh = Bvh::from_iter(
        BvhBuildStrategy::Ploc,
        rect_collider_aabbs.into_iter().chain(poly_collider_aabbs).enumerate(),
    );
    let triggers_bvh = {
        let aabbs = triggers.iter().map(|(_, poly)| poly.aabb(&Isometry2::default()));
        Bvh::from_iter(BvhBuildStrategy::Ploc, aabbs.enumerate())
    };

    Ok(Level {
        start_pos: raw.start_pos,
        rect_colliders: rect_colliders.into(),
        poly_colliders: poly_colliders.into(),
        collision_bvh,
        graphics: graphics.into(),
        graphics_bvh,
        triggers_bvh,
        triggers,
    })
}

fn decompose_concave(points: &[Vec2]) -> Option<Vec<Vec<Point2<f32>>>> {
    let points: Vec<_> = points.iter().copied().map(to_nalgebra).collect();
    let trimesh = try_mesh_both_ways(points)?;
    Some(parry2d::transformation::hertel_mehlhorn(trimesh.vertices(), trimesh.indices()))
}

/// Try creating a `TriMesh` from either counter-clockwise or clockwise vertices.
fn try_mesh_both_ways(points: Vec<Point2<f32>>) -> Option<TriMesh> {
    let mut copy = points.clone();
    if let Some(mesh) = TriMesh::from_polygon(points) {
        Some(mesh)
    } else {
        copy.reverse();
        TriMesh::from_polygon(copy)
    }
}

fn lookup_texture<'a>(
    textures: &'a HashMap<&'a str, Texture2D>,
    name: &str,
) -> Result<Texture2D, LevelError> {
    textures.get(name).ok_or_else(|| LevelError::UnknownTexture(name.to_string())).cloned()
}

#[inline(always)]
fn to_nalgebra(v: Vec2) -> Point2<f32> {
    Point2::new(v.x, v.y)
}

#[inline(always)]
fn from_nalgebra(v: Point2<f32>) -> Vec2 {
    Vec2::new(v.x, v.y)
}

#[inline(always)]
fn rect_aabb(rect: Rect) -> Aabb {
    Aabb::new(Point2::new(rect.x, rect.y), Point2::new(rect.right(), rect.bottom()))
}

#[inline(always)]
fn poswh_aabb(pos: Vec2, wh: Vec2) -> Aabb {
    Aabb::new(to_nalgebra(pos), to_nalgebra(pos + wh))
}
