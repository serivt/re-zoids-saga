# Dialogue portraits

Source of knowledge: own analysis of Zoids Saga (Japan, Rev 1) in a reference emulator
(OAM and VRAM dumps during the first dialogue, BIOS `CpuSet`/`LZ77UnCompWram` call
log) and of the script opcodes that precede each message. Implemented in
`crates/extraction/src/saga.rs` (`portrait`) and `crates/formats/src/tile.rs`
(`TileImage::compose`).

## Record table

468 records of 16 bytes at ROM `0x6D0A64`:

| Offset | Field |
|---|---|
| 0 | Pointer to an LZ77 block of 1152 bytes: 36 4bpp tiles |
| 4 | Pointer to an LZ77 block of 32 bytes: one BGR555 palette |
| 8, 12 | Pointers to the shared sprite layout (`0x83ED3E8`, `0x83ED3DC`), identical in every record |

Records are grouped **nine per character**, one per facial expression:
`record = character × 9 + expression`. The 468 records cover 52 characters.

## Which portrait a message shows

Before a message, the script sets two variables with opcode `0x09` and stores them with
`0x0B`, then calls the shared text-box routine:

```
09 00 <character> 00     variable 0 = character (0 = player, 1 = レジーナ, 2 = アース, 3 = ジャック …)
09 01 <expression> 00    variable 1 = expression
0B
21 1E 00                 call: open the box with a portrait
```

Messages from unnamed speakers call routine `0x1F` instead and show no portrait.

## Image layout

The game draws the 48×48 portrait as four sprites in one-dimensional tile mapping, at
screen position (8, 104), with OBJ palette 15 and palette index 0 transparent:

| Piece | Position (px) | Size | Tiles |
|---|---|---|---|
| 1 | (0, 0) | 32×32 | 0–15 |
| 2 | (0, 32) | 32×16 | 16–23 |
| 3 | (32, 0) | 16×32 | 24–31 |
| 4 | (32, 32) | 16×16 | 32–35 |

The tiles are decompressed to work RAM (`0x0200911C`) and copied to the end of OBJ
VRAM (`0x06017B80`) with `CpuSet`; the palette goes the same way to `0x050003E0`.
