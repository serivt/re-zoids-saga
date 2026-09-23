# Sprite sheets

Source of knowledge: own analysis of Zoids Saga (Japan, Rev 1) in a reference emulator
(OAM dumps and the `CpuSet` call log of the first room) and of the table that references
the copied frames. Implemented in `crates/extraction/src/saga.rs` (`sprite_sheet`,
`sprite_sheet_by_tag`).

## Record table

248 records of 32 bytes at ROM `0x318E04`:

| Offset | Field |
|---|---|
| 0 | Pointer to an animation table (not modeled yet) |
| 4 | Pointer to a frame table (not modeled yet) |
| 8 | Four ASCII characters: `mz`+number for map Zoids (152), `ch`+number for characters (87), `ma`+number (9) |
| 12 | Frame count |
| 16 | Two 16-bit values, `0x0010` high and `9` (Zoids) or `8` (characters) low; meaning not modeled |
| 20 | Tiles per frame (16 in all but three records) |
| 24 | Pointer to the palette: 32 raw bytes, 16 BGR555 colors |
| 28 | Pointer to the frames: uncompressed 4bpp tiles, `frames × tiles per frame × 32` bytes |

Tags are not unique: `mz25` names records 150 and 151. The player's map sprite is
record 151 (`mz25`, 49 frames of 32×32 at `0x08202C5C`, palette at `0x08202C3C`);
`ch00` (record 152) is another character. Frames are single sprites in one-dimensional
mapping: 16 tiles in four rows of four. Idle images `direction × 3 + {0, 1, 0, 2}`
repeat the same picture; walking images `12 + direction × 3 + {1, 2}` are the two
stepping poses.

## How the game uses them

At scene start the game copies the current image of each visible character into OBJ
VRAM with `CpuSet` (the player to `0x06010000`, tiles 0–15) and the palette to an OBJ
palette bank. Walking replaces the image in place from the sheet every four frames.
The player is a single 32×32 OBJ, palette index 0 transparent, at screen (72, 32) when
the first room starts; the OBJ at (88, 32) next to it is the sitting character.
