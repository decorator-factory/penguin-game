# Level stuff

This is the current workflow for making a level:

1. Use Tiled 1.11.2 to author or edit a level
2. Use the `generate_code.py` script to generate Rust code for building the level
3. Move that code to the right file in `src/generated_levels`
4. Auto-format and lint the code

Note: the Tiled maps have a "size" but there's no size restrictions.
However, zooming in Tiled is a bit broken on infinite maps, so I don't
recommend using them.

## TODOs

- This is really suboptimal. You cannot iterate on levels without rebuilding the project.
    Also, this means no user-generated levels.
- Errors like typoed textures and fields can either cause the code generator to fail,
    generate code that doesn't compile, or generate code that panics. Inconsistent and bad UX.
    We should implement a file-based level system where a file is parsed into a `Result<LevelSeed, Err>`
    and a `LevelSeed` should be able to "instantiate" a level with all its BVH's and such without
    failure.
