# Binary format description for RJP levels

Version 0 (in development)

## 0. Overall structure

```
magic_header.
offset_table.
level_start.
strings.
obj_defs/graphics.
obj_defs/colliders.
obj_defs/triggers.
```

All the numbers are stored in little-endian order.

## 1. `magic_header`

Every level file starts with the header `penguinlevel-bin-v0` and exactly 1 newline byte (`0x0A`).

## 2. `offset_table`

The offset table defines where each of the following sections starts and how long it is.
Every entry is a `u32` decsribing an offset, and a `u32` descirbing the size, in bytes since the start of the file.

Structure:
```
u16      # the number of sections (always 5)

# offset, size
u32 u32  # entry for `level_start`  (size always 8)
u32 u32  # entry for `strings`
u32 u32  # entry for `obj_defs/graphics`
u32 u32  # entry for `obj_defs/colliders`
u32 u32  # entry for `obj_defs/triggers`
```

## 3. `level_start`

The position of penguin where the game starts. Always 8 bytes long.

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
u32 u32  # entry 0
u32 u32  # entry 1
u32 u32  # entry 2
...

u8...  # the buffer, starts right after the last entry
```

## 6. `obj_defs/`

Objects define a shape that is either a rectangle or a polygon. A shape is 16 bytes long.

- Rectangle: `x(f32) y(f32) w(f32) h(f32)`
- Polygon: `0xFFFF_FFFF(u32) string(u16) <padding>`

A polygon referes to a string, whose length must be divisible by 4 (since a `f32` is 4 bytes long).
Polygons must not exceed 500 points, you will probably never need as much. Polygons should be convex,
and it's not specified what's going to happen if they're concave.

`0xffff_ffff` happens to be a NaN, which isn't a useful `x` value anyway.

This shape will be referred to as `Shp`.

## 6.1. `obj_defs/graphics`

Structure:
```
u16  # number of entries

# each entry is:
Shp u16  # shape, texture (as string ID)
...
```

## 6.2. `obj_defs/colliders`

Structure:
```
u16  # number of entries

# each entry is:
Shp  # just the shape
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

A trigger kind is represented as a 16-byte structure in the following way:

- `Panic`: `0x00 ?...`
- `Hello`: `0x01 ?...`
- `ShowText`: `0x02 string_id(u16) ?...`
- `SetEyepatch`: `0x03 enable(u8) ?...` (must be either 0 or 1)
- `Goto`: `0x04 x(f32) y(f32) status(u8) ?...` (status is either 0 (warning sign) or 1 (checkmark))

Structure:
```
u16  # number of entries

# each entry (32 bytes) is:
Shp TriggerKind
...
```


## Example: empty level (hex)

```
70656e6775696e6c6576656c2d62696e2d76300a05003e00000008000000460
000000200000048000000020000004a000000020000004c0000000200000000
000000000000000000000000000000
```

## Example: non-empty level (hex)

This level contains:

- 4 strings (including a polygon, an empty string and overlapping strings)
- 2 graphics (1 rectangle and 1 polygon)
- 3 colliders (2 rectangles and 1 polygon)
- 7 triggers (1 Panic, 1 Hello, 1 ShowText, 2 SetEyepatch, 2 Goto)

```
70656e6775696e6c6576656c2d62696e2d76300a05003e000000080000004600
0000480000008e00000026000000b400000032000000e6000000e20000000000
2842a4709dbf0400000000000000000000000000200000002000000006000000
200000000300000000000000cdcccc3d66664640cdcc8cbf000080403333a340
a4707d3fcdccdc4062616e616e6102000000803f000000400000404000008040
0200ffffffff010000000000000000000000030003000000803f000000400000
4040000080400000a0400000c0400000e04000000041ffffffff010000000000
00000000000007000000803f0000004000004040000080400000000000000000
00000000000000000000a0400000c0400000e040000000410100000000000000
00000000000000000000803f0000004000004040000080400202000000000000
0000000000000000ffffffff0100000000000000000000000301000000000000
00000000000000000000803f0000004000004040000080400300000000000000
00000000000000000000803f000000400000404000008040040000f642000090
40000000000000000000a0400000c0400000e04000000041040000f642000090
4001000000000000
```