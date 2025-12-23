/// A "raw level" is a kind of a "data transfer object", a dumb data structure that
/// represents the things that make up a level. Parsing a file from a binary or text
/// file should produce a [`RawLevel`] which will then be used to initialize a real level.
///
use std::rc::Rc;

use crate::levels::TriggerKind;
use glam::{
    Vec2,
    vec2,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Rect { pos: Vec2, size: Vec2 },
    Polygon(Box<[Vec2]>), // TODO: switch to Rc<[Vec2]> to share polygons
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

impl RawLevel {
    pub fn build(self, mut builder: crate::levels::LevelBuilder) -> crate::levels::Level {
        builder.level_start(self.start_pos);

        for graphic in &self.graphics {
            match &graphic.shape {
                Shape::Rect { pos, size } => {
                    builder.rect_graphics(Rc::clone(&graphic.texture), *pos, *size);
                }

                Shape::Polygon(points) => {
                    builder.polygon_graphics(Rc::clone(&graphic.texture), points);
                }
            }
        }

        for shape in &self.colliders {
            match shape {
                Shape::Rect { pos, size } => builder.rect(Option::<Rc<str>>::None, *pos, *size),
                Shape::Polygon(points) => builder.polygon(Option::<Rc<str>>::None, points),
            }
        }

        for trigger in &self.triggers {
            match &trigger.shape {
                Shape::Rect { pos, size } => {
                    builder.rect_trigger(trigger.kind.clone(), *pos, *size);
                }
                Shape::Polygon(points) => builder.polygon_trigger(trigger.kind.clone(), points),
            }
        }

        builder.build_or_die()
    }
}

pub fn make_test_raw_level() -> RawLevel {
    let rect1 = Shape::Rect { pos: vec2(-200.0, 0.0), size: vec2(400.0, 24.0) };
    let rect2 = Shape::Rect { pos: vec2(-200.0, -400.0), size: vec2(24.0, 400.0) };

    let poly = Shape::Polygon([vec2(200.0, 0.0), vec2(300.0, 0.0), vec2(260.0, -100.0)].into());

    RawLevel {
        start_pos: vec2(0.0, -60.0),
        graphics: [Graphic { shape: rect1.clone(), texture: "wood".into() }, Graphic {
            shape: poly.clone(),
            texture: "bricks".into(),
        }]
        .into(),
        colliders: [rect1.clone(), rect2.clone()].into(),
        triggers: [Trigger {
            shape: poly.clone(),
            kind: TriggerKind::ShowText(Rc::from("test123")),
        }]
        .into(),
    }
}
