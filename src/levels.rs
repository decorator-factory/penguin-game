use glam::Vec2;
use macroquad::math::Rect;

use crate::draw_utils::{DrawOpts, draw_textured_rect};

pub struct Level {
    graphics: Vec<Graphic>,
    rect_colliders: Vec<Rect>,
}

pub enum Graphic {
    Rect { pos: Vec2, wh: Vec2, draw: DrawOpts },
}

impl Graphic {
    pub fn macroquad_draw(&self) {
        match self {
            Graphic::Rect { pos, wh, draw } => {
                draw_textured_rect(*pos, *wh, draw.clone());
            }
        }
    }
}

impl Level {
    pub fn new(rects: impl IntoIterator<Item = (Vec2, Vec2, DrawOpts)>) -> Level {
        let mut graphics = vec![];
        let mut rect_colliders = vec![];

        for (pos, wh, draw) in rects {
            graphics.push(Graphic::Rect { pos, wh, draw });
            rect_colliders.push(Rect { x: pos.x, y: pos.y, w: wh.x, h: wh.y });
        }

        Level { graphics, rect_colliders }
    }

    pub fn graphics(&self) -> &[Graphic] {
        &self.graphics
    }

    pub fn rect_colliders(&self) -> &[Rect] {
        &self.rect_colliders
    }
}
