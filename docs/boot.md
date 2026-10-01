# Boot sequence

Source of knowledge: screenshots every 15 frames, VRAM/palette/OAM/RAM dumps and a log
of the LZ77 decompressions of Zoids Saga (Japan, Rev 1) from power-on to the first room
in a reference emulator, plus a read of the title loader (`0x0800248C`), the menu opcode
handler and the window records. Implemented in `crates/game-core/src/boot.rs` and
`game.rs`, data in `crates/extraction/src/saga.rs` (`logo`, `title`,
`name_entry_graphics`, `kana_table`). Continuing a saved game is described in
[formats/save.md](formats/save.md).

## Publisher logo (frames 0–435)

A 256-color tiled picture: 16 colors at ROM `0x5EFC0`, raw 8bpp tiles from `0x5EFE0`
(139 of them, the count following the highest tilemap entry) and a raw 30×20 tilemap at
`0x612A0`, copied to a 32-wide screen block. The game DMAs it directly (no LZ77 call is
logged before the title). Brightness measured on the captures: black until frame 30,
fade in over 45 frames, full until 375, fade out over 30 frames, black until 435.

## Title (from frame 409)

The loader decompresses two tile blocks and builds the tilemaps in code:

| Data | ROM | Where the game puts it |
|---|---|---|
| 4bpp tiles: logo glow, subtitle and, from tile 175, the sprites | LZ77 `0x61750` (296 tiles) | VRAM tile `0x102` |
| 8bpp picture, 30×16 tiles | LZ77 `0x63414` (480 tiles) | VRAM tile `0x115` (BG2) |
| Copyright sprite tiles | LZ77 `0x6310C` (64 tiles) | OBJ tile 112 |
| Palettes 0–10 | raw `0x678C0` | BG palette RAM |
| Palettes 11–14 | raw `0x63394`, `0x633B4`, `0x633D4`, `0x633F4` | BG 11–14; the last two also OBJ 0 and 1 |

Tilemaps: BG2 rows 2–17 hold the picture tile after tile (30 per row), the rest tile
`0x80`; BG1 rows 0–5 hold two 10×6 glow blocks (tiles `0x139`… and `0x175`…, palette 12)
with a mirrored strip on rows 0 and 5 between them (palette 11), scrolled by (−4, −24);
BG3 rows 0–3 hold a 12×4 block from tile `0x10D` with an 11-tile row stride (palette
11), scrolled by (−76, −32). Fifty sprites (32×8, 16×8 and 8×8) draw "ZOIDS SAGA", the
subtitle and the copyright lines; OBJ tiles 0–111 are the 4bpp block from tile 175 on,
112 on the copyright block. The title fades in over about 165 frames; START skips the
fade, and START again runs the menu script at ROM `0x6C04FE` five frames later: reset, open menu window 0
at (10, 10) 9×8 tiles, the three choices, a menu, then a switch on var1 (0 new game,
1 continue, 2 options). The script ends by storing the variables and resetting the text
system, which clears them, so the game reads the choice from the stored copy. The
second menu's options and the guides they open are described in [guide.md](guide.md).

### The title's intro (not modeled yet)

In the original the title does not simply fade in: from the loader (frame 409) its tasks
(`0x08002E49`, `0x08002A31`, `0x080027AD`) show an emblem on black that opens, the sky
brightening behind it, then "ZOIDS SAGA" with a shimmer, the subtitle and the
copyright lines, and from frame 707 (`0x08002BF8` sets bit 8 of `0x0200E8A8`) PRESS
START blinks (tasks `0x0800338D` and `0x0800331D`). The port fades the whole title in
over 166 frames instead.

### Attract demo

Source of knowledge: own reading of the title controller (`0x080020E0`) and the demo task
(`0x080034AC`), checked frame by frame in a reference emulator. In its state 200 the
controller counts the frames the title waits without START once its intro is over (bit
8 of `0x0200E8A8` set and bit 2, START opening the menu, clear); past `0x257`, 600
frames, it sets bit 4, which stops the loader and the blinking (frame 1307 from power-on).
It then darkens the title with the fade task (`0x08004034`, slot 10; states 2000 and
`0x834`) and starts the demo task (state `0x898`). The demo runs the battle module
(`0x0803DC54`, slot 4) on four records in a row, `0x66413C + demo × 0xB0 + scene × 0x2C`
(the eight records before the staged scenes' table, read as those, see
[battle.md](battle.md)), with the record's `+0xC` and `+0x10` set: START then blacks the
screen at once (`0x0800196C`) and ends the scene with 2, which ends the demo. The title's
song goes on through it. When the demo ends (state `0x8FC`) the controller turns to the
other demo and loads the title again from state 0 with its song, 7 frames after the last
scene's end or 6 after START. From power-on the first demo's scenes start at frames 1345,
2000, 2651 and 3459, the title loads again at 4080 and the second demo starts at 5018.

In the port (`crates/game-core/src/attract.rs`) the title shows 26 frames after its
loader, as at the boot, so the count starts 272 frames after it shows; the stretches of
black and of scene then start and end on the original's frames, the scenes' own steps
within a frame or two, and the pictures match.

## Name entry

Windows as the original's records show (see [formats/window.md](formats/window.md)):

| Window | Tiles | Kind | Content |
|---|---|---|---|
| 0 | (0, 0) 8×8 | framed | Portrait of character 0, expression 0 |
| 1 | (8, 0) 22×4 | no frame | `ＳＴＡＲＴ：終了　ＳＥＬＥＣＴ：文字変更` |
| 4 | (24, 4) 6×4 | light | Page label |
| 5 | (8, 4) 16×4 | light | Three spaces then the name, one 8-pixel cell per character |
| 6 | (0, 8) 30×12 | light menu | Five rows of 13 characters separated by full-width spaces |

Behind window 0 sits a 128×128 8bpp picture (LZ77 `0x4142B8`, palette LZ77 `0x425D58`
into colors 64–127). Sprites: the name field arrows at (72, 40) and (168, 40) (LZ77
`0x4B6788`, palette `0x4B6834`), an 8×8 mark under each of the 8 name slots at
x = 96 + 8n, y = 50 (`0x4B6A54`, `0x4B6A68`), and the 8×16 grid cursor at
(8 + 16·column, 73 + 16·row) (`0x4B6940`, `0x4B69C0`). The character table at ROM
`0x6D4884` holds 30 rows of 13 Shift-JIS characters: katakana, hiragana, alphanumerics,
special characters, symbols, then a 10-column hiragana page for the kanji search that
is not modeled. SELECT cycles the five pages (カタカナ, ひらがな, 英数文字, 特殊文字,
記号文字), A appends the character under the cursor (up to 8), B deletes, START runs
name-entry script 1 (table `0x6D08F8`): it reprints the name, opens the question
window and the はい/いいえ menu, and ends with var0 = 0 for yes or 255 for no. The
default name is アトレー.

## Opening

After 60 black frames the first room (map 4) loads with the player on the chair at
metatile (6, 2) facing up and Regina (`ch01`, sprite `0x99`) on (6, 4). Her walk, from
the entity trace: 15 frames, three steps left, 25 frames, turn right, 31 frames, five
steps right, turn left, 61 frames, two steps left, 11 frames, face up; then dialogue
string 40 runs with the story box. When it closes the screen fades to black over 32
frames, stays black 158 frames and fades back over 32 while Regina paces: three steps
left from 4 frames after the box closed, turn right, 61 frames, five steps right, turn
left. Sixty frames after her last turn dialogue 41 runs. When it closes she leaves 6
frames later: three steps left, then down and left alternately to (2, 7), then down to
(2, 12), where she is removed; the player turns to watch her 23 frames into her first
step down. Two frames after she is gone the player steps left onto (5, 2), the field
music starts and control begins. Entering the room also sets flag `0x11F`, the mark that
the opening was seen (see [formats/save.md](formats/save.md)); what else the room's own
event code does is not modeled.
