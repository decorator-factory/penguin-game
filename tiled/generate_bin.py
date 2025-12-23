"""
Requires Python 3.12 or later. No other dependencies.

NB: in Python's "type annotation" system, `float` means `int | float`. Annoying.

NB2: polygon colliders are supposed to be convex for now, but we never check for that.
     if a polygon collider is concave, its convex hull is taken instead.
"""

from __future__ import annotations

import json
import struct
import sys
from collections.abc import Callable, Mapping
from datetime import UTC, datetime
from math import cos, radians, sin
from pathlib import Path
from typing import Any

if len(sys.argv) != 3:
    sys.stderr.write(
        "error: Expected exactly two arguments, the path to the Tiled level (.tmj) and path to write the binary level\n"
    )
    sys.exit(1)

level_path = Path(sys.argv[1])
out_path = Path(sys.argv[2])

with level_path.open("rb") as file:
    level = json.load(file)

layers = level["layers"]
if not layers:
    raise Exception("Must have at least one layer")

all_objects = [obj for layer in layers[::-1] for obj in layer["objects"]]

def _extract_property(obj: dict[str, Any], name: str, expected_type: str) -> Any:
    for prop in obj.get("properties", []):
        if prop["name"] == name:
            if prop.get("type") != expected_type:
                raise Exception(
                    f"Property {name!r} in object with id={obj['id']} "
                    "must be of type {expected_type!r}")
            return prop.get("value")
    raise Exception(f"Expected property {name!r} not found in object with id={obj['id']}")


STATUS_ICONS: Mapping[str, int] = {"Wrong": 0, "Nice": 1}

type GetLocById = Callable[[int], tuple[float, float] | None]

def render_trigger_kind(obj: dict[str, Any], get_loc_by_id: GetLocById, ser: Serializer) -> bytes:
    match _extract_property(obj, "action", "string"):
        case "Panic":
            return b"\x00" * 16
        case "Hello":
            return b"\x01" + b"\x00"*15
        case "ShowText":
            text: str = _extract_property(obj, "text", "string")
            try:
                bs = text.encode("utf-8")
            except ValueError:
                raise Exception(f"Object with id={obj['id']} has a string that cannot be encoded as UTF-8")
            text_id = ser.add_string(bs)
            return b"\x02" + text_id.to_bytes(2, "little") + b"\x00" * 13
        case "SetEyepatch":
            enabled: bool = _extract_property(obj, "enabled", "bool")
            return b"\x03" + bytes([enabled]) + b"\x00" * 14
        case "Goto":
            target_id = _extract_property(obj, "loc", "object")
            status_icon = _extract_property(obj, "status_icon", "string")
            assert isinstance(target_id, int)
            if target_id == 0:  # Tiled uses this as the default apparently
                raise Exception(f"Object with id={obj['id']} must have a location assigned")

            if loc := get_loc_by_id(target_id):
                x, y = loc
            else:
                raise Exception(f"Object with id={obj['id']} references object with unknown ID {target_id}")

            status_icon_num = STATUS_ICONS.get(status_icon)
            if status_icon_num is None:
                raise Exception(f"Object with id={obj['id']} defines an incorrect status icon")

            return b"\x04" + struct.pack("<ff", x, y) + bytes([status_icon_num]) + b"\x00" * 6
        case _:
            raise Exception(f"Unknown action in trigger object with id={obj['id']}")


def find_texture(obj: dict[str, Any]) -> bytes | None:
    for prop in obj.get("properties", []):
        if prop["name"] == "texture_id":
            value = prop["value"]
            assert '"' not in value and "\\" not in value
            if value in {"none", ""}:
                break
            return value.encode("ascii")
    return None


def _rotate_around_origin(x: float, y: float, rad: float) -> tuple[float, float]:
    c, s = cos(rad), sin(rad)
    return (x * c - y * s, y * c + x * s)


def rotate_polygon(poly: Poly, rad: float, cx: float, cy: float) -> Poly:
    points = (_rotate_around_origin(x, y, rad) for x, y in poly)
    return tuple([(x + cx, y + cy) for x, y in points])


def rotate_raw_polygon(obj: list[dict[str, Any]], rad: float, cx: float, cy: float) -> Poly:
    poly = tuple((p["x"], p["y"]) for p in obj)
    return rotate_polygon(poly, rad, cx, cy)


type Poly = tuple[tuple[float, float], ...]
MAGIC_HEADER = b"penguinlevel-bin-v0\n"
SECTION_COUNT = 5

class Serializer:
    def __init__(self) -> None:
        # NOTE: we're not doing any overlap optimizations yet
        # (where "banana" and "ban" occupy the same buffer space)
        self._strings: list[bytes] = [b""]
        self._polygon_to_id: dict[Poly, int] = {}
        self._string_to_id: dict[bytes, int] = {}
        self._graphics: list[bytes] = []
        self._colliders: list[bytes] = []
        self._triggers: list[bytes] = []

    def render(self, level_start: tuple[float, float], filename: str) -> bytes:
        chunks: list[bytes] = []
        chunks.append(MAGIC_HEADER)
        chunks.append(SECTION_COUNT.to_bytes(2, "little"))

        level_start_section = struct.pack("<ff", *level_start)
        sections: list[tuple[bool, bytes]] = [  # (include_in_offset_table?, data)
            # This is a textual comment in the middle of the binary file.
            # Yep, that's fine. The offset table is to handle this kind of stuff.
            (False, self._render_comment_section(filename)),

            (True, level_start_section),
            (True, self._render_string_section()),
            (True, self._render_graphics_section()),
            (True, self._render_colliders_section()),
            (True, self._render_triggers_section()),
        ]
        offset_table_length = 2 + 5 * 8  # size + 5 sections, 2xu32 each
        pos = len(MAGIC_HEADER) + offset_table_length
        for include, section in sections:
            if include:
                chunks.append(struct.pack("<II", pos, len(section)))
            pos += len(section)
        for _, data in sections:
            chunks.append(data)
        return b"".join(chunks)

    def _render_comment_section(self, filename: str) -> bytes:
        return (
            b"author=@/tiled/generate_bin.py\n"
            b"src=%(filename)s\n"
            b"dt=%(datetime)s\n"
            % {
                b"filename": filename.encode("utf-8", "replace"),
                b"datetime": str(datetime.now(UTC)).encode()
            }
        )

    def _render_string_section(self) -> bytes:
        chunks: list[bytes] = [struct.pack("<H", len(self._strings))]
        pos = 0
        for s in self._strings:
            chunks.append(struct.pack(b"<II", pos, len(s)))
            pos += len(s)
        chunks.extend(self._strings)
        return b"".join(chunks)

    def _render_graphics_section(self) -> bytes:
        return struct.pack("<H", len(self._graphics)) + b"".join(self._graphics)

    def _render_colliders_section(self) -> bytes:
        return struct.pack("<H", len(self._colliders)) + b"".join(self._colliders)

    def _render_triggers_section(self) -> bytes:
        return struct.pack("<H", len(self._triggers)) + b"".join(self._triggers)

    def cache_polygon(self, poly: Poly) -> int:
        existing = self._polygon_to_id.get(poly)
        if existing is not None:
            return existing
        self._strings.append(b"".join([struct.pack("<ff", x, y) for (x, y) in poly]))
        new_id = len(self._strings) - 1
        self._polygon_to_id[poly] = new_id
        return new_id

    def add_string(self, s: bytes) -> int:
        existing = self._string_to_id.get(s)
        if existing is not None:
            return existing
        self._strings.append(s)
        new_id = len(self._strings) - 1
        self._string_to_id[s] = new_id
        return new_id

    def add_graphic(self, shape: bytes, texture: bytes) -> None:
        assert len(shape) == 16, len(shape)
        tex_id = self.add_string(texture)
        self._graphics.append(shape + tex_id.to_bytes(2, "little"))

    def add_collider(self, shape: bytes) -> None:
        assert len(shape) == 16, len(shape)
        self._colliders.append(shape)

    def add_trigger(self, shape: bytes, trigger: bytes) -> None:
        assert len(shape) == 16, len(shape)
        assert len(trigger) == 16, len(trigger)
        self._triggers.append(shape + trigger)


def render_rect_shape(x: float, y: float, w: float, h: float, *, deg: float, ser: Serializer) -> bytes:
    if deg == 0:
        bs = struct.pack(b"<ffff", x, y, w, h)
        assert bs[:4] != b"\xff\xff\xff\xff"
        return bs
    elif deg % 90 == 0:
        # game only supports axis-aligned rects, so a rotated rect will be a polygon
        if deg == 90:
            x -= h
            w, h = h, w
        elif deg == 180:
            x -= w
            y -= h
        else:
            assert deg == 270
            y -= w
            w, h = h, w
        return render_rect_shape(x, y, w, h, deg=0.0, ser=ser)
    else:
        poly: Poly = tuple((w*u, h*v) for (u, v) in [(0, 0), (1, 0), (1, 1), (0,1)])
        poly = rotate_polygon(poly, radians(deg), x, y)
        return render_polygon_shape(poly, ser)


def render_polygon_shape(points: Poly, ser: Serializer) -> bytes:
    string_id = ser.cache_polygon(points)
    return b"\xff\xff\xff\xff" + string_id.to_bytes(2, "little") + b"\x00"*10


def render_obj_shape(obj: dict[str, Any], ser: Serializer) -> bytes:
    match obj:
        case {"polygon": polygon, "x": x, "y": y, "rotation": deg}:
            points = rotate_raw_polygon(polygon, radians(deg), x, y)
            return render_polygon_shape(points, ser)

        case {"x": x, "y": y, "width": w, "height": h, "rotation": deg}:
            return render_rect_shape(x, y, w, h, deg=deg, ser=ser)

        case _:
            raise Exception(f"Unknown object type with ID {obj.get('id')}")


def render_obj(obj: dict[str, Any], get_loc_by_id: GetLocById, ser: Serializer) -> None:
    # Beware: Tiled exports are kinda cursed. For example, polygons have `x` and `y`
    # fields (which represent an offset) as well as `width` and `height` (which to my
    # knowledge don't represent anything and just chill out there).
    # Also beware: rectangles can have rotation

    match obj:
        case {"ellipse": True}:
            raise NotImplementedError("Ellipses are not supported yet")

        case {"text": _}:
            raise NotImplementedError("Text is not supported yet")

        case {"type": "Location"}:
            pass

        case {"type": "Collider"}:
            shape = render_obj_shape(obj, ser)
            ser.add_collider(shape)
            if texture := find_texture(obj):
                ser.add_graphic(shape, texture)

        case {"type": "Trigger"}:
            shape = render_obj_shape(obj, ser)
            trigger = render_trigger_kind(obj, get_loc_by_id, ser)
            ser.add_trigger(shape, trigger)

        case {"type": "Graphics"}:
            shape = render_obj_shape(obj, ser)
            texture = find_texture(obj)
            assert texture, f"id={obj['id']}"
            ser.add_graphic(shape, texture)

        case _:
            raise Exception(f"Unknown object type with ID {obj.get('id')}")


locations: dict[int, tuple[float, float]] = {}

for obj in all_objects:
    if obj.get("type") == "Location":
        locations[obj["id"]] = (obj["x"], obj["y"])

ser = Serializer()
level_start: tuple[float, float] | None = None
for obj in all_objects:
    if obj.get("type") == "LevelStart":
        if level_start is not None:
            raise Exception("Duplicate LevelStart object")
        level_start = (obj["x"], obj["y"])
    else:
        render_obj(obj, locations.get, ser)

if level_start is None:
    raise Exception("Expected a level start object in the level")

filename = str(level_path.absolute().relative_to(Path(__file__).parent.absolute()))
rendered = ser.render(level_start, filename)

with out_path.open("wb") as file:
    file.write(rendered)
print("Wrote", out_path)