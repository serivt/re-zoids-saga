# Sprites

Source of knowledge: own analysis of Zoids Saga (Japan, Rev 1) in a reference emulator
(OAM and OBJ VRAM dumps of the first room while idle and walking, matched against the ROM
image by image), a read of the entity spawn code, which indexes the table below, and of
the OAM builder, checked against Roman's sprite above Arcana's bar in OAM and OBJ VRAM.
Implemented in `crates/extraction/src/saga.rs` (`sprite_sheet`, `sprite_sheet_by_tag`).

## Record table

290 records of 32 bytes; record `id` (1-based, as map objects name sprites) is at ROM
`0x318DFC + id × 32`, so the first record starts at `0x318E1C` and a record of `0xFFFF`
ends the table:

| Offset | Field |
|---|---|
| 0 | Pointer to the palette: 32 raw bytes, 16 BGR555 colors |
| 4 | Pointer to the images: uncompressed 4bpp tiles, 32 bytes per tile |
| 8 | Pointer to the animation table: pointers to animations, ended by a null word |
| 12 | Pointer to the frame table: pointers to frame records |
| 16 | Four ASCII characters: `ch`+number for characters, `mz`+number for map Zoids, `ma`+number for furniture-like sprites |
| 20 | Image count |
| 22 | Unknown half-word (0) |
| 24 | Unknown half-word: `9` for Zoids, `8` for characters, 1–4 for furniture |
| 26 | Signed horizontal anchor: where the frames' x offsets start, from the left of the sprite's box (16 for almost every sprite) |
| 28 | Signed vertical anchor, from the top of the box (16 for almost every sprite) |
| 30 | Unknown half-word (0 for most sprites) |

The spawn routine copies the anchor to entity `+0x10` and `+0x12` (`0x08008906`), and
the OAM builder (`0x080009AE`, `0x08000A12`) adds it to the frame's offset: the
sprite's top-left is its box's top-left plus the anchor plus the frame's offset. A
flipped frame is placed with the horizontal anchor mirrored around 16. Roman sitting
above Arcana's bar, `ma22` (`0x105`), has a horizontal anchor of 20, so he sits 4
pixels right of his metatile's box. Other furniture sprites use 8, 24 or 32, and the
tallest ones negative values.

The images are the size of the first frame: 16 tiles for the 32×32 frames of
characters and most furniture, 32 for the 32×64 frames of `ma09`.

The player's map sprite is id `0x98`, `ch00`: 32 images at `0x8202C5C`, palette at
`0x8202C3C`. The chair beside the player in the first room is id `0xF6`, `ma07`, a furniture sprite;
the two characters there are `0xD0` `ch56` and `0xD1` `ch57`.

## Zoid pictures

Each 76-byte Zoid record at ROM `0x670210` carries a sprite of its own for the status
screens (`0x0804D6E8`): 64 tiles of 4bpp images at `+0x30`, a 16-color palette at
`+0x34`, and an animation table and a frame table at `+0x38` and `+0x3C` in the formats
below. The Shield Liger's has one animation of one 64×64 frame whose top-left sits 32
pixels left of and 64 above its anchor; the Zoid status screen anchors it at (40, 88).

## Animations and frames

An animation is a list of 4-byte steps, frame record index then ticks, ended by a step
whose frame is `0xFFFF` or higher. Walking sprites have eight animations, idle up, down,
left, right then walking in the same order, of four 8-tick steps; the game halves the
ticks of map sprites, so each step lasts four frames. `ch00` idle down is frames 3, 4, 3,
5 and walking right frames 21, 22, 21, 23. `ma07` has one animation of one frame.

A frame record is 24 bytes:

| Offset | Field |
|---|---|
| 0 | First tile in the sheet (image index × tiles per image) |
| 2 | Bit 0: draw the image flipped left to right; the right-facing frames of most character sheets reuse the left-facing images this way (the player's `ch00` has its own) |
| 4 | Signed x offset of the top-left corner from the sprite's anchor (−16 for 32×32) |
| 6 | Signed y offset (−16) |
| 8 | Width in pixels |
| 10 | Height in pixels |
| 12 | Unknown: `00 01 00 01 ff 00 00 00 ff ff 00 00` in every record seen |

## How the game uses them

At scene start the game copies the current frame of each visible sprite into OBJ VRAM
with `CpuSet` (the player to `0x06010000`, tiles 0–15) and the palette to an OBJ palette
bank; walking replaces the frame in place every four frames. The sprite's anchor is the
bottom center of the metatile below the one it stands on, so a 32×32 sprite standing on
metatile `(c, r)` has its top-left at `(16c − 8, 16r)`; the player starts the first room
at screen (72, 32) and the chair stands at (88, 32). The game orders OAM so nearer
sprites cover farther ones. Sprites level with each other keep the order earlier sorts
left, which puts the chair in front of the player at the start (see
[../field.md](../field.md), Drawing).
