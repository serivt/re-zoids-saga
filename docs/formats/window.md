# Text window

Source of knowledge: own analysis of Zoids Saga (Japan, Rev 1) running in a reference
emulator (BIOS call log, VRAM/palette dumps and the BG0 tile map while the name-entry
confirmation and the first story dialogue were on screen). Implemented in
`crates/game-core/src/window.rs`; ROM locations in `crates/extraction/src/saga.rs`.

## Where it comes from

| Data | ROM offset | Format |
|---|---|---|
| Frame tileset | `0x3B9428` | LZ77 block (see [lz77.md](lz77.md)), 2048 bytes = 64 4bpp tiles |
| Window palette | `0x6BF9F8` | 32 raw bytes, 16 BGR555 colors (the same palette the font uses) |

The game decompresses the tileset straight into VRAM with `LZ77UnCompVram` (at
`0x06002800` for the story text box, at `0x0600C000` on the name-entry screen) and
draws the window on BG0 with palette 15.

## Palette

| Index | BGR555 | Role |
|---|---|---|
| 0 | `0x7240` | Transparent on BG0 |
| 1, 11 | `0x4AC8` | Teal fill |
| 2, 3, 4 | `0x570D`, `0x6353`, `0x6FB8` | Text anti-aliasing shades |
| 9, 10, 12, 13, 14 | — | Border stripes |
| 15 | `0x7FFF` | White text |

## Tiles

Indices into the decompressed tileset:

| Tile | Role |
|---|---|
| `0x01` | Fill |
| `0x0A` `0x0B` `0x0C` `0x0D` | Corners: top-left, top-right, bottom-left, bottom-right |
| `0x0E` `0x0F` | Top and bottom edges |
| `0x10` `0x11` | Left and right edges; `0x10` also serves as the vertical divider |
| `0x20` `0x21` | Divider junctions on the top and bottom edges |

## Story dialogue layout

The box spans the bottom 8 tile rows (rows 12–19) and the full 30 columns. A divider
at column 7 separates the portrait (columns 1–6) from the text. The speaker name is
drawn at pixel (64, 104), the text lines from (64, 120) every 16 pixels.
