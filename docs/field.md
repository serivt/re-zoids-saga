# Field movement

Source of knowledge: per-frame traces of Zoids Saga (Japan, Rev 1) in a reference
emulator while holding and tapping each direction in the first room (OAM entry of the
player, the game's scroll shadow at IWRAM `0x03004BAE`, and the `CpuSet` log of sprite
frames), plus the traces of walking through the room's exit. For doors: a read of the
movement check and the warp routines named below, and a door taken from a save placed
next to it, with breakpoints on those routines, the fade level traced per frame and
screenshots every other frame compared with the port's. Implemented in
`crates/game-core/src/field.rs` and `game.rs`.

## Measured behavior

| Aspect | Original |
|---|---|
| Grid | The player stands on a 16×16 metatile; a direction press starts a step onto the next metatile, and a started step always completes even if the button is released (a 3-frame tap moves 16 pixels) |
| Speed | 1 pixel per frame; a held direction chains steps without a pause |
| Sprite | 32×32 from sprite `ch00` (id `0x98`, the player on the map), top-left at `(16 × column − 8, 16 × row)` for the standing metatile |
| Footing | The metatile below the standing one holds the bottom-center 16×16 of the sprite; steps are blocked by the attribute of the footing's neighbour, so the sprite stopped at x = 24 against the two-metatile left wall and at y = 16 against the two-metatile top wall |
| Camera | Follows so the sprite stays at screen (104, 64), clamped to the map (768×320 pixels for the first room) |
| Start | Standing on metatile (5, 2) of map 4 after the intro: sprite at map (72, 32) |

## Animation

Animations come from the sprite's own tables (see [formats/sprite.md](formats/sprite.md)):
the idle animation is the facing direction in the order up, down, left, right, the
walking one is that plus four, and every step lasts half its listed ticks, four frames,
so a 16-pixel step is one full walking cycle. VRAM dumps while walking right showed
images 21, 22, 21, 23 of `ch00`, which is exactly its animation 7.

## Characters

The map's object list places the other characters; each starts on its metatile playing
its starting animation, blocks the metatile below it like the player's footing, and is
drawn together with the player in order of anchor y so nearer sprites cover farther
ones, in front of the player when level with it (the game re-sorts OAM the same way when
the player walks past, and the chair `ma07` covers the player's arm at the start). That
chair at (6, 2) is what stops a step right from the start; the desk above the player is
a blocked attribute.

### Wandering

Source: the wander tick (`0x0800A594`), the random direction routine (`0x0800A54C`), the
step-end handler and a 3,000-frame trace of `ch56` and `ch57` in the first room. Objects
of kind 2 walk around on their own:

| Aspect | Original |
|---|---|
| Waiting | A timer counts down one per frame; when it reaches 0 the character acts |
| Direction | The RNG is seeded with the frame counter and one value drawn: the top two bits pick up, down, left, right (`0`–`3`) |
| Step | If the metatile ahead of the footing is free (not blocked, not an exit, not another character or the player, including the cell the player is stepping into) the character faces it and walks at half the player's speed: 16 pixels in 32 frames, its metatile being the destination from the first frame |
| Blocked | The character only turns; either way the next wait is `rng & 0x7F` frames (0–127; 3–123 observed) |
| Talking | A stepping character cannot be spoken to; a conversation freezes everyone |

The step end also writes the character's metatile into the saved object state at RAM
`0x02000B5C + 0x50 + index × 16` (`+2` x, `+3` y), which the loader reads back for maps
whose record id has bit 15; that persistence is not modeled yet.

## Random numbers

Source: the routine at `0x08001080` and its seeding at `0x08001068`. A 16-bit state
`s`, a call counter `c` (RAM `0x0200607C`) and the `VBlank` frame counter `f`
(`0x03002338`):

```
c += 1
s = s × 5 + ((c >> (s & 15)) + 1) + (~f << 1)     (all mod 2¹⁶)
seed(f): s = f + ((f ^ 0xFF) << 8)
```

Because the frame counter enters every draw, the port's sequences only match the
original's when both run the same calls on the same frames; the wander logic re-seeds
before each direction, so what matters is that draws are distributed like the game's.
Implemented in `crates/game-core/src/rng.rs`.

## Talking

Pressing A while standing still with a character on the metatile ahead of the footing
speaks to it: a character of behavior 0 or 1 turns to face the player (animation =
opposite facing), and if its object names a dialogue string the field reports it and
the caller runs that script with `ScriptRunner` (see
[formats/script-text.md](formats/script-text.md)) about three frames later while the
field stays frozen. Furniture (behavior 2) neither turns nor talks. Observed on `ch56`:
entity state 3 for the player and 1 for the character during the box, both back to 0
when it closes. Objects whose script is code run the event the port transcribed for
that address, and chests (behavior 4) open; both are described in [events.md](events.md).

## Exits

When a step completes onto a footing whose attribute is `0x4000 | n`, the engine reports
exit `n`; the caller looks the warp up in the current map's table (see
[formats/map.md](formats/map.md)), loads the destination scene and stands the player on
the arrival metatile, turning it when the warp says so. The screen fades out with the
door sound before the load and back in after it, at the timings in
[events.md](events.md), and the destination map runs its own event when it has one. The
first room's lower exit lands in map 5 at metatile (8, 16): sprite (120, 256), camera
(16, 160), as in the original.

## Doors

Doors are exits the player takes by pushing against them. Each frame the player pushes
toward a cell it cannot enter, the movement check (`0x0800AE7C`) reads that cell's
attribute (`0x080084D4`). A door (`0xC000 | n`) then:

1. turns the player toward it;
2. looks up exit `n` in the map's warp table and plays its sound (`0x080083B8`);
3. warps with the table's facing (`0x08007188`), where a walked exit keeps the player's.

Traced on the door of map 30 (`mq0200`) at (14, 8), leading to map 34 at (12, 20):

- **The push:** starts on frame 0. The turn and the sound (`0x82`, the Gustav's) come on
  frame 1.
- **The fade out:** the screen darkens a level a frame from frame 3 and is black from
  frame 18.
- **The warp:** loads the map on frame 33.
- **The fade in:** the new map brightens a level a frame from frame 65, 15 black frames
  later than a room's exit, since this load takes longer.

## Pause menu

START opens the menu described in [menu.md](menu.md); the field waits underneath it.

## Not modeled yet

The black after a door for loads other than the one measured, the code-driven scripts of characters
beyond the opening chapter, the saved-state overlay of object lists, objects that show
the party's Zoid, and the diagonal input priority of the original (this engine takes the
first held direction in the order up, down, left, right).
