# Level stuff

This is the current workflow for making a level:

1. Use Tiled 1.11.2 to author or edit a level
2. Use the `generate_bin.py` script to generate a binary file representing the level

Usage: `python3 tiled/generate_bin.py tiled/my_level.tmj src/levels/my_level.bin`

Alternatively, you can run the `build-levels.xonsh` script found in the root directory
if you have `xonsh` installed. This lets you very easily save a level and then press
Ctrl+R in the game.

Note: the Tiled maps have a "size" but there's no size restrictions.
However, zooming in Tiled is a bit broken on infinite maps, so I don't
recommend using them.

## TODOs

- Handle missing textures better
- Add layers for proper rendering of overlapping things
