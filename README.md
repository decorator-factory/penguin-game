# penguin-game

In this game, you control a penguin. Being a penguin, you cannot fly or jump. However, you can propel yourself using your rocket launcher. Execute your moves precisely and learn unusual movement techniques to get to the goal.

This game is in very early development, but you can test it out in the browser:

- URL: https://decorator-factory.github.io/penguin-game/

- Demo URL: https://decorator-factory.github.io/penguin-game/#demo (the penguin will move on its own)

The available level is very short, intended to show the gist of the game. Your goal is to get to the platform inside the tower roof.

![Penguin inside tower roof](image.png)

## Controls

The rules are simple: shoot a rocket under your feet, and the explosion blasts you away. Note that the rocket

- `A`: Go left
- `D`: Go Right
- Mouse cursor: aim the rocket launcher
- Left mouse button: shoot rocket
- `R`: speed up the game 5 times (for debugging and demo development purposes)

If you get stuck, see how the demo penguin completes the level.

## TODO list

- Improve collision physics, especially with polygons. I will probably end up using `rapier`, but who knows.

- Level editor. Unfortunately none of the existing open-source level editors are any good for this game.

- Demo recording and proper demo movie format

- Some sort of UI for selecting levels and congratulating the player when they win

- Checkpoints. This is intended to be a difficult game, but not a "rage game". So you should be able to return to save your progress and/or return to a previous point in the level if you screw up badly.

- Props, signs and other world objects to make the levels nice and intuitive

- Sounds

