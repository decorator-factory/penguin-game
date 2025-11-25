# Level stuff

This is the current workflow for making a level:

1. Use Tiled 1.11.2 to author or edit a level
2. Use the `generate_code.py` script to generate Rust code for building the level
3. Move that code to the right file in `src/generated_levels`
4. Auto-format and lint the code

## TODOs

- This is really suboptimal. You cannot iterate on levels without rebuilding the project.
    Also, this means no user-generated levels.
- Errors like missing textures cause panics. This is definitely unacceptable for
    user-generated levels.
