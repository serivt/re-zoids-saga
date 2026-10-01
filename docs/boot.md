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
11), scrolled by (−76, −32). These are the maps once the intro below is over; 46
sprites (32×8, 16×8 and 8×8, priority 1) then draw "ZOIDS SAGA", the subtitle and the
copyright lines; OBJ tiles 0–111 are the 4bpp block from tile 175 on, 112 on the
copyright block. START, from the second frame after the intro, runs the menu script at
ROM `0x6C04FE` five frames later, six when PRESS START showed and its line is cleared
first: reset, open menu window 0
at (10, 10) 9×8 tiles, the three choices, a menu, then a switch on var1 (0 new game,
1 continue, 2 options). The script ends by storing the variables and resetting the text
system, which clears them, so the game reads the choice from the stored copy. The
second menu's options and the guides they open are described in [guide.md](guide.md).

### The title's intro

Source of knowledge: own reading of the title's loader (`0x0800248C`) and its tasks,
checked frame by frame in a reference emulator against the display registers, the map
copy in IWRAM (`0x0300239C`, the four screen blocks the vertical blank copies to VRAM
with the scroll values at `0x03004B9C` and the brightness level at `0x03002356`) and
screenshots. Frames below count from power-on; the loader starts at 409 and the port
shows the title from 435, when the logo is gone, with the loader's first 26 frames run.

The loader lays the plate: BG1 the left glow block, BG2 (4bpp for now) the right one
with the strip's edge, BG3 the strip's middle three columns, with scrolls that put the
halves together at the middle, and the fade task (`0x08004034`) brings it out of black,
one level a frame from 416 to 446. At 478 three tasks start:

- `0x08002E49` hands the vertical blank callbacks `0x08002DF8`, `0x08002E20` (14 times)
  and `0x08002E34`: BG3 comes on at 479 darkened 15/16 and brightens by a level a frame.
- `0x08002A31` opens the plate for 36 frames: BG0 and BG1 slide right and BG2 left by a
  pixel a frame; every eighth frame the strip in BG3 grows a column on each side (tiles
  `0x10D + column + row × 11`, the fifth time writing before the block's first cell);
  every fourth frame the pieces between the halves change, from the list at `0x663F60`
  (4-byte entries: the cell in bits 2–7 and the screen block in bits 0–1 of the first
  byte, 3 ending a step; the second byte's nibbles are the strip tiles of rows 0 and 5;
  BG2's are flipped). At 514 it copies BG2's rows 0–5 to BG1 from column 14, and at 530
  turns BG0 and BG2 off (`0x08002C50`) and starts `0x08002F39`. Then it raises BG1 and
  BG3 a pixel every fourth frame 32 times; at 688 it lays the subtitle in BG0 (rows 4–5,
  tiles `0xE205`…) and blends it in through the window at lines 64–87 (`0x08002D28`,
  `0x08002D9C`, weights (16−n, n)); at 706 it creates the sprites and turns BG0 off
  (`0x08002C3C`); at 707 the intro is over (bit 8 of `0x0200E8A8`).
- `0x08002F39` lays the picture's map in BG2 and makes it 8bpp with priority 2 at 532
  (`0x08003244`), then darkens it line by line from a horizontal-blank routine
  (`0x080032AC`, copied to `0x0200EB30`): each line's brightness level is the low nibble
  of one of two 160-entry tables at `0x0200E8B0`, swapped every frame (`0x080032D0`).
  Each entry starts at level 15 with a wait of its distance from line 80; each frame
  the table not shown takes the shown one with the wait lowered, or once it is 0 the
  level, so the picture opens from the middle out. Line n shows entry n−1, set on the
  horizontal blank before it. When the last line's level is 0 (627) the effect ends
  (`0x080032F8`); the logo goes in BG0 (rows 0–2, two 14-column halves, tiles `0xD1B1`
  on, scrolled by (−8, −36), priority 0) and blends in over 32 frames (`0x08002C64`,
  `0x08002CA4`, `0x08002CC0`): weights (n, 16) brightening, then (16, 16−n).
- `0x080027AD` reads START from 479. It stops the others, clears the effects
  (`0x0800292C`), lays the finished maps (the strip between the glow blocks from
  `0x6640D4`, nine entries per row with the flip bit) and the sprites, and fades in with
  the fade task; the frame after, the layers come back on while the loader is still
  busy, from a line that moves between 36 and 52 with the frame's work and at the level
  the emulator reads from the write-only brightness register (13), then the screen is
  black until the fade, which ends 36 frames after START.

From the frame after the intro's end the loader starts `0x0800338D`, which blinks PRESS
START in BG1 (row 9, columns 10–18, tiles `0xE221`…): PRESS, START 15 frames later,
nothing 15 later, twice, then the whole line and nothing twice, every 15 frames.
`0x0800331D` stops it and clears the line when START opens the menu or the attract demo
begins (bits 2 and 4). The port (`crates/game-core/src/title_intro.rs`) keeps the maps,
scrolls, levels and registers as these tasks and callbacks leave them and composes the
picture per pixel with the hardware's priorities, window and effects. Checked against
the reference emulator frame by frame from 435 to 830, with START at four moments of the
intro and six after it: the pictures match within one step per 8-bit channel, which the
emulator's rounding of the blends gives, except the skip's flash line.

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
loader, as at the boot, and the count starts the frame after its intro's end, 299
frames after the loader, as the controller runs before the task that marks it; the
title's darkening, with PRESS START cleared, the stretches of black and of scene then
start and end on the original's frames, the scenes' own steps within a frame or two, and
the pictures match. After the demo the title's intro plays again.

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
