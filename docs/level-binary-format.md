# Binary format description for penguin-game levels

Version 0 (in development)

## 0. Overall structure

```
magic_header.
offset_table.
level_start.
strings.
shape_defs/
    rects.
    polygons.
obj_defs/
    graphics.
    colliders.
    triggers.
```

All the numbers are stored in little-endian order.

## 1. `magic_header`

Every level file starts with the header `penguinlevel-bin-v0` and exactly 1 newline byte (`0x0A`).

## 2. `offset_table`

The offset table defines where each of the following sections starts and how long it is.
Every entry is a `u32` decsribing an offset, and a `u32` descirbing the size, in bytes since the start of the file.

Structure:
```
u16      # the number of sections (always 7)

# offset, size
u32 u32  # entry for `level_start`  (size always 8)
u32 u32  # entry for `strings`
u32 u32  # entry for `shape_defs/rects`
u32 u32  # entry for `shape_defs/polygons`
u32 u32  # entry for `obj_defs/graphics`
u32 u32  # entry for `obj_defs/colliders`
u32 u32  # entry for `obj_defs/triggers`
```

## 3. `level_start`

The position of penguin where the game starts

Structure:
```
f32 f32  # x, y
```

## 4. `strings`

This section stores variable-length and potentially large sequences of bytes used in the level.
Strings might overlap, coincide, or it could be that one string is entirely contained in another.
Strings don't have to end with a null byte, and might contain null bytes.
Empty strings must still have a valid offset (offset 0 always works).

Structure:
```
u16  # number of entries
# offset, length; -- offset is calculated relative to the start of the buffer
u32 u16 <0u16>  # entry 0
u32 u16 <0u16>  # entry 1
u32 u16 <0u16>  # entry 2
...

u8...  # the buffer, starts right after the last entry
```

## 5. `shape_defs/`

The `shape_defs/rects` and `shape_defs/polygons` sections define the abstract shapes that can be used by objects.

### 5.1. `shape_defs/rects`

Structure:
```
u16  # number of rects (at most 32768)

f32 f32 f32 f32  # x0 y0 w0 h0
f32 f32 f32 f32  # x1 y1 w1 h1
...
```

### 5.2. `shape_defs/polygons`

Since polygons can have a variable number of points, this is conceptually similar to the `strings` section.
However, the offset and the length are specified as the number of 8-byte points

Structure:
```
u16  # number of polygons (at most 32768)
# offset, length; -- offset is calculated relative to the start of the buffer
u32 u8 <0u24>  # entry 0
u32 u8 <0u24>  # entry 1
u32 u8 <0u24>  # entry 2
...

# the buffer, starts right after the last entry:
f32 f32  # (x0, y0)
f32 f32  # (x1, y1)
...
```

## 6. `obj_defs/`

Objects will refer to a previously defined shape using a `u16` identifier. If the top bit of the identifier
is `0`, then it is a rect, otherwise it is a polygon (with the lower 15 bits used to index it). This format
will be referred to as `S16`.

Objects

## 6.1. `obj_defs/graphics`

Structure:
```
u16  # number of entries

# each entry is:
S16 u16  # shape, texture (as string ID)
...
```

## 6.2. `obj_defs/colliders`

Structure:
```
u16  # number of entries

# each entry is:
S16  # just the shape
...
```

## 6.3. `obj_defs/triggers`

Triggers are more complex because they accept a "trigger kind" with varying parameters:
```
# Trigger kinds:
- Panic
- Hello
- ShowText(string)
- SetEyepatch(bool)
- Goto((x: f32, y: f32), StatusIcon),
```

A trigger kind is represented as a 15-byte structure in the following way:

- `Panic`: `0x00 ?...`
- `Hello`: `0x01 ?...`
- `ShowText`: `0x02 string_id(u16) ?...`
- `SetEyepatch`: `0x03 enable(u16) ?...` (must be either 0 or 1)
- `Goto`: `0x04 x(f32) y(f32) status(u8) ?...` (status is either 0 (warning sign) or 1 (checkmark))

Structure:
```
u16  # number of entries

# each entry (16 bytes) is:
S16 TriggerKind
...
```

## Example 0. Smallest possible level

```
# magic_header
70 65 6e 67 75 69 6e 6c 65 76 65 6c 2d 62 69 6e 2d 76 30 0a
# offset_table
07 00
32 00  08 00  # entry for `level_start`
3a 00  02 00  # entry for `strings`
3c 00  02 00  # entry for `shape_defs/rects`
3e 00  02 00  # entry for `shape_defs/polygons`
40 00  02 00  # entry for `obj_defs/graphics`
42 00  02 00  # entry for `obj_defs/colliders`
44 00  02 00  # entry for `obj_defs/triggers`
# level_start
00 00 00 00  # start x = 0.0
00 00 00 00  # start y = 0.0
# strings
00 00
# shape_defs/rects
00 00
# shape_defs/polygons
00 00
# obj_defs/graphics
00 00
# obj_defs/colliders
00 00
# obj_defs/triggers
00 00
```