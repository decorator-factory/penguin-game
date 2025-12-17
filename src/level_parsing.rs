#![expect(unused)]
use std::rc::Rc;

use crate::{
    raw_level::RawLevel,
    text_utils::concat_bytes,
};
use glam::Vec2;

#[derive(thiserror::Error, PartialEq, Debug)]
#[error("at byte {byte}: {detail}")]
pub struct ParseError {
    pub byte: usize,
    pub detail: ParseErrorDetail,
}

#[derive(thiserror::Error, PartialEq, Debug)]
pub enum ParseErrorDetail {
    #[error("Missing the penguin-level-bin-v0\\n header")]
    MissingHeader,

    #[error("TODO")]
    Todo,
}

pub fn parse(src: &[u8]) -> Result<RawLevel, ParseError> {
    let og_src = src;
    let Some(src) = src.strip_prefix(b"penguinlevel-bin-v0\n") else {
        return Err(ParseError { byte: 0, detail: ParseErrorDetail::MissingHeader });
    };

    Err(ParseError { byte: og_src.len() - src.len(), detail: ParseErrorDetail::Todo })
}

#[rustfmt::skip]
pub const EMPTY_LEVEL_SRC: &[u8] = concat_bytes!([
// magic header
b"penguinlevel-bin-v0\n",
// offset table
b"\x07\x00",
b"\x32\x00\x08\x00",  // level_start
b"\x3a\x00\x02\x00",  // strings
b"\x3c\x00\x02\x00",  // shape_defs/rects
b"\x3e\x00\x02\x00",  // shape_defs/polygons
b"\x40\x00\x02\x00",  // obj_defs/graphics
b"\x42\x00\x02\x00",  // obj_defs/colliders
b"\x44\x00\x02\x00",  // obj_defs/triggers
// level_start (0.0, 0.0)
b"\x00\x00\x00\x00",
b"\x00\x00\x00\x00",
// strings
b"\x00\x00",
// shape_defs/rects
b"\x00\x00",
// shape_defs/polygons
b"\x00\x00",
// obj_defs/graphics
b"\x00\x00",
// obj_defs/colliders
b"\x00\x00",
// obj_defs/triggers
b"\x00\x00",
]);
