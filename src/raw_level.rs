/// A "raw level" is a kind of a "data transfer object", a dumb data structure that
/// represents the things that make up a level. Parsing a file from a binary or text
/// file should produce a [`RawLevel`] which will then be used to initialize a real level.
///
use std::rc::Rc;

use crate::levels::TriggerKind;
use glam::Vec2;

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Rect { pos: Vec2, size: Vec2 },
    Polygon(Rc<[Vec2]>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Graphic {
    pub shape: Shape,
    pub texture: Rc<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Trigger {
    pub shape: Shape,
    pub kind: TriggerKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawLevel {
    pub start_pos: Vec2,
    pub graphics: Box<[Graphic]>,
    pub colliders: Box<[Shape]>,
    pub triggers: Box<[Trigger]>,
}
