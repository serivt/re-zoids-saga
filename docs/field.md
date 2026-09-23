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
| Sprite | 32×32 from sheet `mz25` (the player on the map), top-left at `(16 × column − 8, 16 × row)` for the standing metatile |
| Footing | The metatile below the standing one holds the bottom-center 16×16 of the sprite; steps are blocked by the attribute of the footing's neighbour, so the sprite stopped at x = 24 against the two-metatile left wall and at y = 16 against the two-metatile top wall |
| Camera | Follows so the sprite stays at screen (104, 64), clamped to the map (768×320 pixels for the first room) |
| Start | Standing on metatile (5, 2) of map 4 after the intro: sprite at map (72, 32) |

## Animation

Images in the sheet are grouped by direction in the order up, down, left, right:

| State | Image indices | Timing |
|---|---|---|
| Idle | `direction × 3 + {0, 1, 0, 2}` | 4 frames per image |
| Walking | `12 + direction × 3 + {0, 1, 0, 2}` | 4 frames per image, so a 16-pixel step is one full cycle |

## Exits

When a step completes onto a footing whose attribute is `0x4000 | n`, the engine reports
exit `n`; the caller looks the warp up in the current map's table (see
[formats/map.md](formats/map.md)), loads the destination scene and stands the player on
the arrival metatile, turning it when the warp says so. The first room's lower exit lands
in map 5 at metatile (8, 16): sprite (120, 256), camera (16, 160), as in the original.

## Not modeled yet

Doors taken by pressing A (`0xC000` attributes), the fade and door sound of a warp, NPCs,
the sitting character at the desk (sheet `ma06`), and the diagonal input priority of the
original (this engine takes the first held direction in the order up, down, left, right).
