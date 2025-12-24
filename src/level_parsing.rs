/// Code for parsing the "version 0" level format.
/// See `@/docs/level-binary-format.md` for details.
use crate::{
    levels::{
        StatusIcon,
        TriggerKind,
    },
    raw_level::{
        Graphic,
        RawLevel,
        Shape,
        Trigger,
    },
    text_utils::{
        rc_str_from_utf8,
        split_from_bytes,
    },
};
use bytemuck::{
    Pod,
    Zeroable,
};
use core::mem::size_of;
use glam::{
    Vec2,
    vec2,
};
use pack1::{
    F32LE,
    U16LE,
    U32LE,
};
use std::{
    collections::{
        HashMap,
        hash_map::Entry,
    },
    rc::Rc,
};

#[derive(thiserror::Error, PartialEq, Debug)]
#[non_exhaustive]
pub enum ParseError {
    #[error("missing the penguin-level-bin-v0\\n header")]
    MissingHeader,

    #[error("file too small, expected at least {size} bytes", size = MIN_LEVEL_SRC_SIZE)]
    NotEnoughData,

    #[error("bad offset table size: {0}")]
    BadOffsetTableSize(u16),

    #[error(
        "section {name} is out of bounds: (offset={1}, length={2})",
        name = section_name(*.0))]
    SectionOutOfBounds(usize, u32, u32),

    #[error("in section {name}: {1}", name = section_name(*.0))]
    SectionGeneric(usize, &'static str),

    #[error("in section {name}, shape #{1}: {2}", name = section_name(*.0))]
    SectionShapeErr(usize, u16, ShapeError),

    #[error("in section {name}, graphic #{1}: {2}", name = section_name(*.0))]
    SectionGraphicErr(usize, u16, GraphicError),

    #[error("in section {name}, trigger #{1}: {2}", name = section_name(*.0))]
    SectionTriggerErr(usize, u16, TriggerError),

    #[error("bad string with id {0}")]
    BadString(u16),
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C, packed)]
struct SectionDesc {
    offset: U32LE,
    size: U32LE,
}

const SEC_LEVEL_START: usize = 0;
const SEC_STRINGS: usize = 1;
const SEC_GRAPHICS: usize = 2;
const SEC_COLLIDERS: usize = 3;
const SEC_TRIGGERS: usize = 4;
const SECTION_COUNT: usize = 5;

#[rustfmt::skip]
const SECTION_NAMES: [&str; SECTION_COUNT] = [
    "level_start",
    "strings",
    "obj_defs/graphics",
    "obj_defs/colliders",
    "obj_defs/triggers",
];

const fn section_name(idx: usize) -> &'static str {
    if idx < SECTION_NAMES.len() { SECTION_NAMES[idx] } else { "" }
}

pub fn parse(src: &[u8]) -> Result<RawLevel, ParseError> {
    if src.len() < MIN_LEVEL_SRC_SIZE {
        return Err(ParseError::NotEnoughData);
    }

    let base = src;

    let Some(src) = src.strip_prefix(b"penguinlevel-bin-v0\n") else {
        return Err(ParseError::MissingHeader);
    };

    let (offset_count, src) = split_from_bytes::<U16LE>(src);
    let offset_count = offset_count.get();
    if offset_count as usize != SECTION_COUNT {
        return Err(ParseError::BadOffsetTableSize(offset_count));
    }
    let (section_descs, _) = split_from_bytes::<[SectionDesc; SECTION_COUNT]>(src);

    let sections = map_sections(section_descs, base).map_err(|(idx, desc)| {
        ParseError::SectionOutOfBounds(idx, desc.offset.get(), desc.size.get())
    })?;

    let Some(start_pos) = parse_level_start(sections[SEC_LEVEL_START]) else {
        return Err(ParseError::SectionGeneric(SEC_LEVEL_START, "wrong section size"));
    };
    let strings = parse_strings(SEC_STRINGS, sections[SEC_STRINGS])?.into_boxed_slice();

    let mut polygons = PolygonPointStore { strings: &strings, cache: HashMap::with_capacity(64) };

    let colliders = parse_colliders(SEC_COLLIDERS, sections[SEC_COLLIDERS], &mut polygons)?;
    let graphics = parse_graphics(SEC_GRAPHICS, sections[SEC_GRAPHICS], &strings, &mut polygons)?;
    let triggers = parse_triggers(SEC_TRIGGERS, sections[SEC_TRIGGERS], &strings, &mut polygons)?;

    Ok(RawLevel { start_pos, graphics, colliders, triggers })
}

struct PolygonPointStore<'s> {
    strings: &'s [Rc<[u8]>],
    cache: HashMap<u16, Rc<[Vec2]>>,
}

impl PolygonPointStore<'_> {
    fn fetch(&mut self, string_id: u16) -> Result<Rc<[Vec2]>, ShapeError> {
        let points: &Rc<[Vec2]> = match self.cache.entry(string_id) {
            Entry::Occupied(occ) => occ.into_mut(),
            Entry::Vacant(vac) => vac.insert(parse_polygon_points(string_id, self.strings)?),
        };
        Ok(Rc::clone(points))
    }
}

fn parse_polygon_points(
    string_id: u16,
    strings: &[impl AsRef<[u8]>],
) -> Result<Rc<[Vec2]>, ShapeError> {
    let Some(s) = strings.get(string_id as usize) else {
        return Err(ShapeError::StringNotFound { string_id });
    };

    let Some(pairs): Option<&[[F32LE; 2]]> = bytemuck::try_cast_slice(s.as_ref()).ok() else {
        return Err(ShapeError::StringBadLength { string_id });
    };
    let points: Rc<[Vec2]> = pairs.iter().map(|[x, y]| vec2(x.get(), y.get())).collect();
    if points.len() < 3 || points.len() >= 500 {
        return Err(ShapeError::StringBadLength { string_id });
    }
    Ok(points)
}

fn map_sections<'b, const N: usize>(
    descs: &[SectionDesc; N],
    base: &'b [u8],
) -> Result<[&'b [u8]; N], (usize, SectionDesc)> {
    let mut rv: [&[u8]; N] = [&[]; N];
    for (i, &desc) in descs.iter().enumerate() {
        let offset = desc.offset.get() as usize;
        let size = desc.size.get() as usize;
        let Some(slice) = base.get(offset..offset + size) else {
            return Err((i, desc));
        };
        rv[i] = slice;
    }
    Ok(rv)
}

fn parse_level_start(section_data: &[u8]) -> Option<Vec2> {
    let (chunk, rest) = section_data.split_first_chunk::<8>()?;
    if !rest.is_empty() {
        return None;
    }
    let [x, y]: [F32LE; 2] = bytemuck::cast(*chunk);
    Some(vec2(x.get(), y.get()))
}

// Strings

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C, packed)]
struct StringDesc {
    offset: U32LE,
    length: U32LE,
}

fn parse_strings(section_id: usize, section_data: &[u8]) -> Result<Vec<Rc<[u8]>>, ParseError> {
    let (count, items, buffer) = parse_array::<StringDesc>(section_data)
        .map_err(|e| ParseError::SectionGeneric(section_id, e))?;

    let mut rv = Vec::with_capacity(count as usize);
    for (i, item) in items.iter().enumerate() {
        let offset = item.offset.get() as usize;
        let length = item.length.get() as usize;
        let Some(slice) = buffer.get(offset..offset + length) else {
            return Err(ParseError::BadString(i as u16));
        };
        rv.push(slice.into());
    }
    Ok(rv)
}

// Graphics

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C, packed)]
struct GraphicChunk {
    shape: [U32LE; 4],
    texture_string_id: U16LE,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum GraphicError {
    #[error("bad shape: {0}")]
    BadShape(ShapeError),

    #[error("bad string index #{0}")]
    BadStringIndex(u16),

    #[error("string #{0} is not valid utf-8")]
    StringNotUtf8(u16),
}

fn parse_graphic(
    chunk: GraphicChunk,
    strings: &[Rc<[u8]>],
    polygons: &mut PolygonPointStore,
) -> Result<Graphic, GraphicError> {
    let shape = parse_shape(chunk.shape, polygons).map_err(GraphicError::BadShape)?;
    let string_id = chunk.texture_string_id.get();
    let Some(rc_bytes) = strings.get(string_id as usize) else {
        return Err(GraphicError::BadStringIndex(string_id));
    };
    let Ok(texture) = rc_str_from_utf8(Rc::clone(rc_bytes)) else {
        return Err(GraphicError::StringNotUtf8(string_id));
    };
    Ok(Graphic { shape, texture })
}

fn parse_graphics(
    section_id: usize,
    section_data: &[u8],
    strings: &[Rc<[u8]>],
    polygons: &mut PolygonPointStore,
) -> Result<Box<[Graphic]>, ParseError> {
    let (_, chunks, _) = parse_array::<GraphicChunk>(section_data)
        .map_err(|e| ParseError::SectionGeneric(section_id, e))?;

    chunks
        .iter()
        .copied()
        .map(|chunk| parse_graphic(chunk, strings, polygons))
        .enumerate()
        .map(|(i, x)| x.map_err(|e| (i, e)))
        .collect::<Result<Box<[Graphic]>, _>>()
        .map_err(|(idx, err)| ParseError::SectionGraphicErr(section_id, idx as u16, err))
}

// Colliders

fn parse_colliders(
    section_id: usize,
    section_data: &[u8],
    polygons: &mut PolygonPointStore,
) -> Result<Box<[Shape]>, ParseError> {
    let (_, items, _) = parse_array::<[U32LE; 4]>(section_data)
        .map_err(|e| ParseError::SectionGeneric(section_id, e))?;
    items
        .iter()
        .copied()
        .map(|chunk| parse_shape(chunk, polygons))
        .enumerate()
        .map(|(i, x)| x.map_err(|e| (i, e)))
        .collect::<Result<Box<[Shape]>, _>>()
        .map_err(|(idx, err)| ParseError::SectionShapeErr(section_id, idx as u16, err))
}

// Triggers

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C, packed)]
struct TriggerChunk {
    shape: [U32LE; 4],
    trigger_kind: [u8; 16],
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum TriggerError {
    #[error("bad shape: {0}")]
    BadShape(ShapeError),

    #[error("bad string index #{0}")]
    BadStringIndex(u16),

    #[error("string #{0} is not valid utf-8")]
    StringNotUtf8(u16),

    #[error("bad status icon")]
    BadStatusIcon,

    #[error("unknown trigger")]
    UnknownTrigger,
}

fn parse_trigger(
    chunk: TriggerChunk,
    strings: &[Rc<[u8]>],
    polygons: &mut PolygonPointStore,
) -> Result<Trigger, TriggerError> {
    let shape = parse_shape(chunk.shape, polygons).map_err(TriggerError::BadShape)?;
    let kind = parse_trigger_kind(chunk.trigger_kind, strings)?;
    Ok(Trigger { shape, kind })
}

fn parse_trigger_kind(chunk: [u8; 16], strings: &[Rc<[u8]>]) -> Result<TriggerKind, TriggerError> {
    match chunk[0] {
        0x00 => Ok(TriggerKind::Panic),

        0x01 => Ok(TriggerKind::Hello),

        0x02 => {
            let string_id = u16::from_le_bytes([chunk[1], chunk[2]]);
            let Some(rc_bytes) = strings.get(string_id as usize) else {
                return Err(TriggerError::BadStringIndex(string_id));
            };
            let Ok(text) = rc_str_from_utf8(Rc::clone(rc_bytes)) else {
                return Err(TriggerError::StringNotUtf8(string_id));
            };
            Ok(TriggerKind::ShowText(text))
        }

        0x03 => {
            let enable = match chunk[1] {
                0 => false,
                1 => true,
                _ => return Err(TriggerError::UnknownTrigger),
            };
            Ok(TriggerKind::SetEyepatch(enable))
        }

        0x04 => {
            let x = bytemuck::from_bytes::<F32LE>(&chunk[1..5]).get();
            let y = bytemuck::from_bytes::<F32LE>(&chunk[5..9]).get();
            let status_icon = match chunk[9] {
                0 => StatusIcon::Wrong,
                1 => StatusIcon::Nice,
                _ => return Err(TriggerError::BadStatusIcon),
            };
            Ok(TriggerKind::Goto(vec2(x, y), status_icon))
        }

        _ => Err(TriggerError::UnknownTrigger),
    }
}

fn parse_triggers(
    section_id: usize,
    section_data: &[u8],
    strings: &[Rc<[u8]>],
    polygons: &mut PolygonPointStore,
) -> Result<Box<[Trigger]>, ParseError> {
    let (_, items, _) = parse_array::<TriggerChunk>(section_data)
        .map_err(|e| ParseError::SectionGeneric(section_id, e))?;
    items
        .iter()
        .copied()
        .map(|chunk| parse_trigger(chunk, strings, polygons))
        .enumerate()
        .map(|(i, x)| x.map_err(|e| (i, e)))
        .collect::<Result<Box<[Trigger]>, _>>()
        .map_err(|(idx, err)| ParseError::SectionTriggerErr(section_id, idx as u16, err))
}

// Shapes

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum ShapeError {
    #[error("string with id {string_id} has unsuitable length")]
    StringBadLength { string_id: u16 },
    #[error("string with id {string_id} not found")]
    StringNotFound { string_id: u16 },
}

fn parse_shape(chunk: [U32LE; 4], polygons: &mut PolygonPointStore) -> Result<Shape, ShapeError> {
    if chunk[0].get() == 0xffff_ffff {
        let string_id = chunk[1].get() as u16;
        let points = polygons.fetch(string_id)?;
        Ok(Shape::Polygon(points))
    } else {
        let [x, y, w, h] = bytemuck::cast::<_, [F32LE; 4]>(chunk).map(F32LE::get);
        Ok(Shape::Rect { pos: vec2(x, y), size: vec2(w, h) })
    }
}

/// Parses an array of at most `u16::MAX` items.
///
/// First, the length is read (as little-endian u16), then a packed array of items is read.
/// The return tuple is: number of items, iterator over items and the unconsumed bytes.
fn parse_array<T: bytemuck::AnyBitPattern>(
    section_data: &[u8],
) -> Result<(u16, &[T], &[u8]), &'static str> {
    if section_data.len() < 2 {
        return Err("section too small");
    }
    let (count, body) = split_from_bytes::<U16LE>(section_data);
    let count = count.get();
    let expected_body_size = (count as usize) * size_of::<T>();

    let Some((array_bytes, rest_bytes)) = body.split_at_checked(expected_body_size) else {
        return Err("section is of the wrong size");
    };

    let items: &[T] = bytemuck::cast_slice(array_bytes);

    Ok((count, items, rest_bytes))
}

// constant definitions

#[allow(unused)]
pub(crate) use consts::EMPTY_LEVEL_SRC;
#[allow(unused)]
pub(crate) use consts::EXAMPLE_LEVEL_SRC;
pub(crate) use consts::MIN_LEVEL_SRC_SIZE;

#[cfg_attr(not(test), allow(dead_code))]
mod consts {
    use crate::text_utils::cat;

    const fn f32b(x: f32) -> [u8; 4] {
        x.to_bits().to_le_bytes()
    }

    const fn sec32(offset: u32, length: u32) -> [u8; 8] {
        let [b0, b1, b2, b3] = offset.to_le_bytes();
        let [b4, b5, b6, b7] = length.to_le_bytes();
        [b0, b1, b2, b3, b4, b5, b6, b7]
    }

    const fn polygon_shape(string_id: u16) -> [u8; 16] {
        let [low, high] = string_id.to_le_bytes();
        let mut buf = *b"\xff\xff\xff\xff\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        buf[4] = low;
        buf[5] = high;
        buf
    }

    const fn rpad<const N: usize>(slice: &[u8]) -> [u8; N] {
        let mut buf = [0u8; N];
        let mut i = 0;
        while i < slice.len() {
            buf[i] = slice[i];
            i += 1;
        }
        buf
    }

    const RECT1234: [u8; 16] = *cat!(&f32b(1.0), &f32b(2.0), &f32b(3.0), &f32b(4.0));
    const RECT5678: [u8; 16] = *cat!(&f32b(5.0), &f32b(6.0), &f32b(7.0), &f32b(8.0));

    #[rustfmt::skip]
    pub const EMPTY_LEVEL_SRC: &[u8] = cat!(
        // magic header
        b"penguinlevel-bin-v0\n",
        // offset table
        b"\x05\x00",
        &sec32(62, 8), // level_start
        &sec32(70, 2), // strings
        &sec32(72, 2), // obj_defs/graphics
        &sec32(74, 2), // obj_defs/colliders
        &sec32(76, 2), // obj_defs/triggers
        // level_start
        &f32b(0.0), &f32b(0.0),
        // strings
        b"\x00\x00",
        // obj_defs/graphics
        b"\x00\x00",
        // obj_defs/colliders
        b"\x00\x00",
        // obj_defs/triggers
        b"\x00\x00",
    );

    #[rustfmt::skip]
    pub const EXAMPLE_LEVEL_SRC: &[u8] = cat!(
        // magic header
        b"penguinlevel-bin-v0\n",
        // offset table
        b"\x05\x00",
        &sec32(62, 8), // level_start
        &sec32(70, 72), // strings
        &sec32(142, 38), // obj_defs/graphics
        &sec32(180, 50), // obj_defs/colliders
        &sec32(230, 226), // obj_defs/triggers
        // level_start
        &f32b(42.0), &f32b(-1.23),
        // strings
        b"\x04\x00",
        &sec32(0, 0),   // empty string
        &sec32(0, 32),  // a polygon
        &sec32(32, 6),  // "banana"
        &sec32(32, 3),  // "ban"
        &f32b(0.0), &f32b(0.1), &f32b(3.1), &f32b(-1.1), &f32b(4.0), &f32b(5.1), &f32b(0.99), &f32b(6.9),
        b"banana",
        // obj_defs/graphics
        b"\x02\x00",
        &RECT1234,         b"\x02\x00",
        &polygon_shape(1), b"\x03\x00",
        // obj_defs/colliders
        b"\x03\x00",
        &RECT1234,
        &RECT5678,
        &polygon_shape(1),
        // obj_defs/triggers
        b"\x07\x00",
        &rpad::<32>(cat!(&RECT1234, &[0])),  // Panic
        &rpad::<32>(cat!(&RECT5678, &[1])),  // Hello
        &rpad::<32>(cat!(&RECT1234, &[2, 2, 0])),  // ShowText("banana")
        &rpad::<32>(cat!(&polygon_shape(1), &[3, 1])),  // SetEyepatch(true)
        &rpad::<32>(cat!(&RECT1234, &[3, 0])),  // SetEyepatch(false)
        &rpad::<32>(cat!(&RECT1234, &[4], &f32b(123.0), &f32b(4.5), &[0])),  // Goto(123.0, 4.5, Wrong)
        &rpad::<32>(cat!(&RECT5678, &[4], &f32b(123.0), &f32b(4.5), &[1])),  // Goto(123.0, 4.5, Nice)
    );

    /// Smallest possible size of a level file
    pub const MIN_LEVEL_SRC_SIZE: usize = EMPTY_LEVEL_SRC.len();
}

#[cfg(test)]
mod test {
    use glam::vec2;

    use super::*;
    use crate::text_utils::cat;

    #[test]
    fn parse_fail_missing_header() {
        let err = parse(cat!(b"penguinlevel-bin-v1\n", &[0; MIN_LEVEL_SRC_SIZE])).unwrap_err();
        assert_eq!(err, ParseError::MissingHeader);
    }

    #[test]
    fn parse_fail_empty_string() {
        let err = parse(b"").unwrap_err();
        assert_eq!(err, ParseError::NotEnoughData);
    }

    #[test]
    fn parse_ok_empty_level() {
        let level = parse(EMPTY_LEVEL_SRC).unwrap();
        assert_eq!(level.start_pos, vec2(0.0, 0.0));
        assert_eq!(level.graphics.len(), 0);
        assert_eq!(level.colliders.len(), 0);
        assert_eq!(level.triggers.len(), 0);
    }

    #[test]
    fn parse_ok_example_level() {
        let level = parse(EXAMPLE_LEVEL_SRC).unwrap();
        assert_eq!(level.start_pos, vec2(42.0, -1.23));

        let polygon = Shape::Polygon(
            [vec2(0.0, 0.1), vec2(3.1, -1.1), vec2(4.0, 5.1), vec2(0.99, 6.9)].into(),
        );
        let rect1234 = Shape::Rect { pos: vec2(1.0, 2.0), size: vec2(3.0, 4.0) };
        let rect5678 = Shape::Rect { pos: vec2(5.0, 6.0), size: vec2(7.0, 8.0) };

        assert_eq!(level.colliders.as_ref(), &[
            Shape::Rect { pos: vec2(1.0, 2.0), size: vec2(3.0, 4.0) },
            Shape::Rect { pos: vec2(5.0, 6.0), size: vec2(7.0, 8.0) },
            polygon.clone(),
        ]);

        assert_eq!(level.graphics.as_ref(), &[
            Graphic {
                shape: Shape::Rect { pos: vec2(1.0, 2.0), size: vec2(3.0, 4.0) },
                texture: "banana".into()
            },
            Graphic { shape: polygon.clone(), texture: "ban".into() },
        ]);

        assert_eq!(level.triggers.as_ref(), [
            Trigger { shape: rect1234.clone(), kind: TriggerKind::Panic },
            Trigger { shape: rect5678.clone(), kind: TriggerKind::Hello },
            Trigger { shape: rect1234.clone(), kind: TriggerKind::ShowText("banana".into()) },
            Trigger { shape: polygon.clone(), kind: TriggerKind::SetEyepatch(true) },
            Trigger { shape: rect1234.clone(), kind: TriggerKind::SetEyepatch(false) },
            Trigger {
                shape: rect1234.clone(),
                kind: TriggerKind::Goto(vec2(123.0, 4.5), StatusIcon::Wrong)
            },
            Trigger {
                shape: rect5678.clone(),
                kind: TriggerKind::Goto(vec2(123.0, 4.5), StatusIcon::Nice)
            }
        ]);
    }

    #[test]
    fn parse_generated_empty_level() {
        const SRC: &[u8] = include_bytes!("./samples/v0_empty_level.bin");
        let level = parse(SRC).unwrap();
        assert_eq!(level, RawLevel {
            start_pos: vec2(42.0, -1.23),
            graphics: [].into(),
            colliders: [].into(),
            triggers: [].into(),
        });
    }

    #[test]
    fn parse_generated_new_level() {
        const SRC: &[u8] = include_bytes!("./samples/v0_new_level.bin");
        let level = parse(SRC).unwrap();

        assert_eq!(level.start_pos, vec2(1524.0, 7260.0));
        assert_eq!(level.graphics.len(), 184);
        assert_eq!(level.colliders.len(), 125);
        assert_eq!(level.triggers.len(), 47);
        assert_eq!(level.triggers.last().unwrap(), &Trigger {
            shape: Shape::Rect { pos: vec2(9912.0, 5844.0,), size: vec2(96.0, 24.0,) },
            kind: TriggerKind::ShowText("to be continued".into()),
        });
    }
}
