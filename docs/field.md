# Field movement

Source of knowledge: per-frame traces of Zoids Saga (Japan, Rev 1) in a reference
emulator while holding and tapping each direction in the first room (OAM entry of the
player, the game's scroll shadow at IWRAM `0x03004BAE`, and the `CpuSet` log of sprite
frames), plus the traces of walking through the room's exit. Implemented in
`crates/game-core/src/field.rs`.

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

The map's object list places the other characters; each stands still on its metatile
playing its starting animation, blocks the metatile below it like the player's footing,
and is drawn together with the player in order of anchor y so nearer sprites cover
farther ones, in front of the player when level with it (the game re-sorts OAM the same
way when the player walks past, and the chair `ma07` covers the player's arm at the
start). That chair at (6, 2) is what stops a step right from the start; the desk above
the player is a blocked attribute.

## Talking

Pressing A while standing still with a character on the metatile ahead of the footing
speaks to it: a character of behavior 0 or 1 turns to face the player (animation =
opposite facing), and if its object names a dialogue string the field reports it and
the caller opens a `TalkBox` on that script while the field stays frozen. Furniture
(behavior 2) neither turns nor talks, and objects whose script is code are silent for
now. Observed on `ch56`: entity state 3 for the player and 1 for the character during
the box, both back to 0 when it closes.

## Exits

When a step completes onto a footing whose attribute is `0x4000 | n`, the engine reports
exit `n`; the caller looks the warp up in the current map's table (see
[formats/map.md](formats/map.md)), loads the destination scene and stands the player on
the arrival metatile, turning it when the warp says so. The first room's lower exit lands
in map 5 at metatile (8, 16): sprite (120, 256), camera (16, 160), as in the original.

## Not modeled yet

Doors taken by pressing A (`0xC000` attributes), the fade and door sound of a warp,
code-driven character scripts, characters that wander, the saved-state overlay of object
lists, objects that show the party's Zoid, dialogue opcodes beyond plain messages, and
the diagonal input priority of the original (this engine takes the first held direction
in the order up, down, left, right).
