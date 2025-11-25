"""
Requires Python 3.12 or later. No other dependencies.
"""

import json
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

if len(sys.argv) != 2:
    sys.stderr.write(
        "error: Expected exactly one argument, the path to the Tiled level (.tmj)\n"
    )
    sys.exit(1)

level_path = Path(sys.argv[1])

with open(level_path, "rb") as file:
    level = json.load(file)

layer = next((layer for layer in level["layers"] if layer["name"] == "Objects"), None)
if layer is None:
    sys.stderr.write("error: Layer 'Objects' not found in the level\n")
    sys.exit(1)

TEMPLATE = """
#![allow(clippy::excessive_precision, clippy::pedantic)]
/// Generated from file `%(filename)s` on `%(datetime)s`
use crate::levels;
use glam::vec2;

pub fn build(mut builder: levels::LevelBuilder) -> levels::Level {
%(lines)s
    builder.level_start(%(level_start)s);
    builder.build_or_die()
}
""".strip()


def maybe_texture_expr(obj: dict[str, Any]) -> str:
    for prop in obj.get("properties", []):
        if prop["name"] == "texture_id":
            value = prop["value"]
            assert '"' not in value and "\\" not in value
            if value in {"none", ""}:
                break
            return f'Some("{value}")'
    return "None"


def texture_expr(obj: dict[str, Any]) -> str:
    for prop in obj.get("properties", []):
        if prop["name"] == "texture_id":
            value = prop["value"]
            assert '"' not in value and "\\" not in value
            if not value:
                break
            return f'"{value}"'
    raise Exception(f"Expected {obj['id']} to have a texture_id")


def polygon_to_expr(obj: list[dict[str, Any]], x: float, y: float) -> str:
    points = [(float(x + p["x"]), float(y + p["y"])) for p in obj]

    return "&[" + ", ".join([f"vec2({px}, {py})" for px, py in points]) + "]"


lines: list[str] = []
level_start: str | None = None

for obj in layer["objects"]:
    match obj:
        case {"polygon": polygon, "x": x, "y": y, "type": "Collider"}:
            texture = maybe_texture_expr(obj)
            points = polygon_to_expr(polygon, x, y)
            lines.append(f"builder.polygon({texture}, {points});")

        case {"polygon": polygon, "x": x, "y": y, "type": "Graphics"}:
            texture = texture_expr(obj)
            points = polygon_to_expr(polygon, x, y)
            lines.append(f"builder.polygon_graphics({texture}, {points});")

        case {"x": _, "y": _, "width": _, "height": _, "ellipse": True}:
            raise NotImplementedError("Ellipses are not supported yet")

        case {"x": _, "y": _, "width": _, "height": _, "text": _}:
            raise NotImplementedError("Text is not supported yet")

        case {"x": x, "y": y, "width": w, "height": h, "type": "Collider"}:
            texture = maybe_texture_expr(obj)
            x, y, w, h = map(float, (x, y, w, h))
            lines.append(f"builder.rect({texture}, vec2({x}, {y}), vec2({w}, {h}));")

        case {"x": x, "y": y, "width": w, "height": h, "type": "Graphics"}:
            texture = texture_expr(obj)
            x, y, w, h = map(float, (x, y, w, h))
            lines.append(f"builder.rect_graphics({texture}, vec2({x}, {y}), vec2({w}, {h}));")

        case {"x": x, "y": y, "type": "LevelStart"}:
            x, y = map(float, (x, y))
            if level_start is not None:
                raise Exception("Duplicate LevelStart object")
            level_start = f"vec2({x}, {y})"

        case other:
            raise Exception(f"Unknown object type with ID {obj.get('id')}")

if level_start is None:
    raise Exception("Expected a level start object in the level")

print(
    TEMPLATE
    % {
        "filename": level_path.name,
        "datetime": datetime.now(UTC),
        "level_start": level_start,
        "lines": "\n".join(["    " + line for line in lines]),
    }
)
