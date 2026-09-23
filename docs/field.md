# Field movement

Source of knowledge: per-frame traces of Zoids Saga (Japan, Rev 1) in a reference
emulator while holding each direction in the first room (OAM entry of the player,
the game's scroll shadow at IWRAM `0x03004BAE`, and the `CpuSet` log of sprite frames).
Implemented in `crates/game-core/src/field.rs`.

## Measured behavior

| Aspect | Original |
|---|---|
| Speed | 1 pixel per frame while a direction is held |
| Sprite | 32×32 from sheet `mz25` (the player on the map) |
| Collision box | The bottom-center 16×16 of the sprite (offset 8, 16), tested against the scene's metatile attributes; the sprite stopped at x = 24 against the two-metatile left wall and at y = 16 against the two-metatile top wall |
| Camera | Follows so the sprite stays at screen (104, 64), clamped to the map (768×320 pixels for the first room) |
| Start | Sprite at map (88, 32) after the intro |

## Animation

Images in the sheet are grouped by direction in the order up, down, left, right:

| State | Image indices | Timing |
|---|---|---|
| Idle | `direction × 3 + {0, 1, 0, 2}` | 4 frames per image |
| Walking | `12 + direction × 3 + {0, 1, 0, 2}` | 4 frames per image, so a 16-pixel step is one full cycle |

## Attributes

Scene attributes are one 16-bit entry per 16×16 metatile (`width / 2` per row); bit 15
blocks walking. The first room also holds a few `0x4000` and `0x0001` entries near the
exits, not modeled yet.

## Not modeled yet

Doors and scene transitions, NPCs, the sitting character at the desk (sheet `ma06`), and
the diagonal input priority of the original (this engine takes the first held direction
in the order up, down, left, right).
