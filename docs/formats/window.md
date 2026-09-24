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

## Window kinds

The kind byte a script passes to opcode `0x01` selects the border and the text
margin: high nibble `0x10` is the striped border of dialogue boxes, `0x20` the thin
border of menus and fields (tiles `0x12`–`0x19`: corners, top, bottom, left, right),
`0x40` no border at all; low nibble 1 makes a menu, whose text starts two cells in so
the cursor brackets (tiles `0x3B`/`0x3C` left, `0x3D`/`0x3E` right, two rows tall) fit
on either side of the selected line.

## Window records

Source: the window creation routine (`0x080415D4`) and the message handler. The game
keeps eight window records of 852 bytes at RAM `0x020075A0`; a script opens one with
its tile rectangle (border included), a kind and a style. Text starts one tile in from
the top-left corner, spans `width − 2` cells and `(height − 2) / 2` lines of 16 pixels;
a line break past the last line scrolls the text up one line. A portrait is drawn one
tile in from the corner. The "more" prompt is tile `0x1C` on the bottom border, two tiles
in from the right corner. A menu keeps every line it was given and shows the ones that
fit: the cursor moves to the last shown line, then the list scrolls one line at a time
without wrapping, and tiles `0x32` and `0x34` in the middle of the top and bottom borders
mark lines hidden above and below (observed on the Zoid guide's type lists). A menu
also shows its window when the script did not present it. Implemented in
`crates/game-core/src/windows.rs`.

## Story dialogue layout

Shared string 30 opens window 0 at (0, 12) 8×8 tiles for the portrait and window 1 at
(7, 12) 23×8 tiles for the text, so the text window's left border sits on the portrait
window's right border; the game draws the junction tiles there, which is the divider at
column 7. Light-framed windows (kind `0x2x`, the status screens') that meet the same way
keep their own borders, the later one drawn over the earlier. The speaker's name is drawn at pixel (64, 104), the text lines from (64, 120),
21 cells per line and three lines before scrolling.

## Character talk layout

Speaking to a character on the map runs its dialogue string, which calls shared string
31: the same skin without a portrait, window 1 at (0, 12) 30×8 tiles, text from cell
(1, 13) with the speaker's name on the first line and up to two more lines of 28 cells,
so pixel (8, 104), (8, 120) and (8, 136). Characters appear one per frame starting about
four frames after A is pressed (39 characters were complete 43 frames after the press).
About 22 frames after the last character the prompt starts blinking, 20 frames on and
20 off. A then closes the box (or turns the page of a longer script). Measured on the
first room's `ch56` with `tools/mgba_talk.lua`, which steers the player next to an entity
and presses A.

## Text wrapping

Source: the window callback's put-character case (ROM `0x0803E330`). Each window keeps a
cursor column and row; before drawing a glyph, if the column has reached the window's
text width the cursor moves to column 0 of the next row. Wrapping is therefore
**character-level at the cell width, with no kinsoku rules**; rows clamp at 32. An
explicit line break (`0x0D`, handler at `0x08040256`) moves to the next row and, when
that row is past the visible ones, asks the window to scroll one line first.

The story dialogue box gives the text 21 cells per row and 3 rows including the
speaker's name; `game_core::DIALOGUE_TEXT_AREA` keeps the 22×2 estimate used by
`extractor-cli check-layout` before the window records were read.
