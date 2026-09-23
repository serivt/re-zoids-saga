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

The player is `ch00` (record 152): 32 frames of 32×32 pixels at `0x08205C7C`, palette at
`0x08205C5C`. Frames are single sprites in one-dimensional mapping: 16 tiles in four rows
of four.

## How the game uses them

At scene start the game copies frame 0 of each visible character into OBJ VRAM with
`CpuSet` (the player to `0x06010200`, tiles 16–31) and the palette to an OBJ palette
bank (the player to bank 1). Walking replaces the frame in place from the sheet:
frames 6–8 and 15–17 were seen while the player moved in the first room. The sprite is
a single 32×32 OBJ at screen (88, 64) in the first room, priority 2, palette index 0
transparent.
