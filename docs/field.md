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
| Running | B held when a step starts (`0x0800B282`, the player's keys at entity `+0x56`) doubles the speed, halves the step's frames and shifts the walk animation's ticks by 2 instead of 1: 8 frames a step on foot, 16 for the Gustav's 32-pixel cells. Traced in the castle and in the labyrinth: the steps start on the original's frames |
| Sprite | 32×32 from sprite `ch00` (id `0x98`, the player on the map), top-left at `(16 × column − 8, 16 × row)` for the standing metatile |
| Footing | The metatile below the standing one holds the bottom-center 16×16 of the sprite; steps are blocked by the attribute of the footing's neighbour, so the sprite stopped at x = 24 against the two-metatile left wall and at y = 16 against the two-metatile top wall |
| Camera | Follows so the sprite stays at screen (104, 64), clamped to the map (768×320 pixels for the first room). Placing the player (`0x08008B70`) does not move it: only the player's steps do. A cutscene's pan (`0x08008324`) moves it on from there; a move past the map's edge is refused (the routine asks the tile streamer at each 8-pixel boundary), so a pan longer than the map stops at the edge for the moves left and comes straight back from it |
| Start | Standing on metatile (5, 2) of map 4 after the intro: sprite at map (72, 32) |

## Animation

Animations come from the sprite's own tables (see [formats/sprite.md](formats/sprite.md)):
the idle animation is the facing direction in the order up, down, left, right, the
walking one is that plus four, and every step lasts half its listed ticks, four frames,
so a 16-pixel step is one full walking cycle. VRAM dumps while walking right showed
images 21, 22, 21, 23 of `ch00`, which is exactly its animation 7.

The animation step (`0x08000BD8`) times a new animation's first step with the shift the
entity has at that moment (`+0x38`), and the frame that starts it already counts one.
The player's own step starts its walking animation before setting the walking shift,
so its first step keeps the standing one's timing: on foot both halve the ticks, but
the Gustav stands with whole ticks (actors other than characters do), so its first
walking step shows 7 frames and the next ones 4. Likewise a finished step starts the
standing animation before the standing shift comes back: the Gustav's first standing
step shows 3 frames. A map's actors start with their standing shift, whatever the
player had when it left the last map.

## Characters

The map's object list places the other characters; each starts on its metatile playing
its starting animation, blocks the metatile below it like the player's footing, and is
drawn together with the player so nearer sprites cover farther ones (see Drawing
below). The chair `ma07` at (6, 2) stops a step right from the start; the desk above the
player is a blocked attribute.

## Drawing

Source: the OAM builder (`0x08000A90`, `0x08000554`), its sort (`0x08000468`), the
entity reset (`0x08000D1C`), and screenshots of the original compared with the port's
frame by frame in the first room, in Arcana and above its bar.

- **Order.** The game keeps a list of entities that orders its sprites, front first.
  Each scene load resets it to object order. Before each frame's sprites, a selection
  sort goes through it in place: an entity trades places with a later one that is
  visible, not off the screen, and lower on the map (larger y). Entities level with
  each other never trade, so their order is whatever the earlier trades left. When the
  first room loads, the maid far below sends the prince to the end of the list, so the
  chair at his desk covers him. A sprite whose top-left is more than 56 pixels left of
  the screen or 32 above it, or beyond 320 and 192 from there, is left out of OAM
  (`0x080005CA`). That flag only takes effect in the next frame's sort.
- **One frame late.** The game copies its sprite table, the scroll registers, the
  text layers and the brightness at the vertical blank. A frame therefore shows
  positions, order, flips, camera, windows and fade level as the frame before left
  them. Each sprite's picture is copied straight into video memory, so it is the
  current one. A walking sprite's new step shows one frame before it moves. The port
  keeps the screen state at the start of each frame (`Field::latch`,
  `ScriptWindows::latch`) and draws it with the current pictures.
- **Sprites near the top.** The picture copy happens while the frame is being drawn,
  in the entities' update: a sprite high on the screen (the factory's door, rows 4 to
  31) has been drawn already, so it shows a new step's picture a frame after the
  step, where the port shows it at once. Sprites lower down (the Gustav near the
  middle) show it at once in both.
- **Streamed tiles.** The animation step (`0x08000BD8`) copies the tiles of each new
  frame into the start of the sprite's slot in video memory, while the sprite table
  still names the frame before: its shape, offset and anchor. A frame smaller than
  the one before only covers the start of the slot, so for a frame the rest of the
  old picture shows through the old shape. The explosion of a beaten Zoid ends this
  way: its last frame is a single 8×8 tile, so the smoke before it shows one more
  frame, less its first tile. The port draws each sprite with the latched frame's
  shape and anchor and the current frame's tiles over the start (checked on the
  explosion against the original).

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
| A cutscene's dialogue | Characters go on walking and animating while it shows (see [events.md](events.md)) |

A stepping entity's previous metatile (entity `+0x52`, `+0x54`) blocks other steps like
its own for the first half of the step. The stepping command (`0x0800B764`) counts the
step's frames down at `+0x4C` and, when the count reaches the halfway mark at `+0x58`
(half the step's frames less one: 7 of 16, 15 of 32), makes the previous metatile the
new one. In Arcana a soldier waiting behind the captain steps into the metatile the
captain is leaving eight frames into the captain's step, as in the original.

Halfway through each step the stepping command also writes the character's metatile
into its object state at RAM `0x02000B5C + 0x50 + index × 16` (`+2` x, `+3` y). For maps
whose record id has bit 15 the loader builds the objects from those states instead of
the map's list (`0x08007188`): the sprite, the cell, the command and the parameter come
from the state, and an object whose state lost bit 15 is not loaded. See
[formats/save.md](formats/save.md) and [combat.md](combat.md) for how the states are
built. The port writes an actor's cell back every frame, from the cell that stops
blocking halfway, which is the same value.

### Shy townsfolk

Source: command 3 (`0x0800AA60`), its flight routine (`0x0800A5E4`) and a trace of a
townsperson in Arcana with B held. Objects of kind 3, the people of Arcana's streets,
wander like kind 2 while B is up. While B is held (the keys at RAM `0x03002358`) and the
player stands within three metatiles along both axes:

| Aspect | Original |
|---|---|
| Direction | One draw from the RNG. A draw of `0xFFF` or less picks a random direction, as a wanderer does. Otherwise the character steps away from the player along the axis the player is farther on. When both are as far, bit 15 of the draw picks the vertical axis |
| Step | At the player's speed, a pixel a frame; tried every frame the character is not stepping, with no wait between |
| Blocked | The character turns and tries again the next frame |

Farther away it wanders. The trace showed a townsperson three metatiles to the right of
the player trying to step left into a wall every frame until a random draw moved it.
The RNG's calls differ between the original and the port (below), so the paths differ.

### Roaming enemies

Source: command 4 (`0x0800AA98`) and the step check (`0x0800AE7C`). The enemies of the
Zoid maps are objects of behavior 1 and command 4. They run the same flight routine
toward the player within two metatiles along both axes and wander farther away. When
an enemy steps into the player's carrier, or the carrier into an enemy, the two meet
in battle: both must be map Zoids (behavior 1) on the same level (attribute bit
`0x1000`), and the player must be in control. An enemy does not reach a player standing
on an exit's cell. Meeting and the battle are described in [combat.md](combat.md).

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

The metatile ahead is found by `0x080084D4`, which walks on past cells whose attribute
has bit 13 (`0x2000`): the counters. The player speaks to whoever stands on the first
cell after them, so a shopkeeper answers across a counter (Arcana's item shop has one,
attribute `0xA000`, which also blocks walking).

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

A walk an event gives the player takes a door the same way: when a step ends and the
next one runs into a door, the door is taken in that frame. The world map's drive to
Arcana ends so (see [events.md](events.md)); traced there, the sound comes two frames
after the Gustav reaches the cell before the door.

## Pause menu

START opens the menu described in [menu.md](menu.md); the field waits underneath it.

## Debugging aid

F10 in the launcher turns a debugging mode on and off (`Game::toggle_debug_mode`). While
it is on, on every map, the roaming enemies are intangible: the player walks through them
and none meets it in battle. In battle the protagonist's attacks always land and beat
what they hit (see [combat.md](combat.md)). It is not the original's: only the port has
it, for moving around and testing quickly.

## Not modeled yet

The black after a door for loads other than the one measured, the code-driven scripts of characters
beyond the opening chapter, and the diagonal input priority of the original (this engine takes the
first held direction in the order up, down, left, right).
