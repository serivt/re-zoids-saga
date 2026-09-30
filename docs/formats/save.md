# Save memory

Source of knowledge: own analysis of Zoids Saga (Japan, Rev 1). A read of the save
routine (`0x08005EC0`), its descriptor at ROM `0x666E54`, the save and load wrappers
(`0x080593B8`, `0x08059468`), the continue routine (`0x08003C14`), the new-game routine
(`0x08003948`), the flag routines (`0x08000F88`, `0x08000FD8`) and the pause-menu panel.
In a reference emulator: saves made from the pause menu after a new game and after
walking, their SRAM compared with RAM dumps, write watchpoints on the block during a new
game and when the menu opens, and continues from saves patched to be missing, broken
in one copy, broken in both, or lacking single fields. Implemented in
`crates/formats/src/save.rs` (the memory), `crates/formats/src/progress.rs` (the
game-state fields), `crates/extraction/src/saga_save.rs` (layout and new-game block)
and `crates/game-core/src/save.rs` with `game.rs` (saving and continuing).

## Layout

The cartridge has 32 KiB of SRAM (`SRAM_F_V102`). The save routine is driven by a
descriptor: the size of one copy (`0x3F1C`), then address and size pairs ending at a
zero address. The first pair points at the header string in ROM, the others at the RAM
blocks a copy holds.

| SRAM offset | Size | Content |
|---|---|---|
| `0x0000` | 11 | Header `ZOIDS-SAGA` and a zero; without it the game reports no save |
| `0x000B` | `0x3F1C` | Copy 0 |
| `0x3F27` | `0x3F1C` | Copy 1, the one the loader falls back to |
| `0x7E43` | to the end | Never touched by the game (`0xFF` when erased) |

Each copy holds the blocks in descriptor order, each followed by the 32-bit
little-endian sum of its bytes:

| Offset in a copy | Size | RAM | Content |
|---|---|---|---|
| `0x0000` | `0x3F10` + 4 | `0x02000B5C` | The game state |
| `0x3F14` | 4 + 4 | `0x02004A6C` | Two 16-bit counters capped at 9999, raised by the cable-duel code |

The pause menu's save writes the game-state block to copy 0, then to copy 1, verifying
each write and retrying up to three times; the result is not checked by the menu.
Loading a block whose sum does not match clears it in RAM and reports a failure. The
counter block is saved by the duel code on its own; continuing does not load it.

## Game-state block

The fields this port reads and writes; everything else is kept as the save had it.

| Offset | Size | Field |
|---|---|---|
| `+0x00` | 2 | Options: bit `0x1000` shows the party's hit points and energy over its units in battle (L toggles it) |
| `+0x02` | 1 | Area: low byte of the current map record's id, written on entering a map |
| `+0x04` | 2 | Map record |
| `+0x06` | 2 | Metatile column of the player |
| `+0x08` | 2 | Metatile row |
| `+0x0C` | 33 × 2 | Game flags: flag `n` is bit `15 − n % 16` of half-word `n / 16`; flag `0x1E + n` marks chest `n` opened |
| `+0x0A` | 2 | Battles won against roaming enemies |
| `+0x50` | 16 each | Object states of the current area, ended by `0xFFFF` (see below) |
| `+0xCD2` | 1 | Party level (99 at most) |
| `+0xCD4` | 4 | Party experience |
| `+0xCD8` | 4 × 16 | Member records, copied from ROM `0x67AC4C` by a new game: the pilot bonuses in percent at `+4`, `+6`, `+8`, `+10`, `+12` (耐久, 反応, 防御, 攻撃, 命中); the hangar sets the three warriors' to the party level times their growth at ROM `0x66BB38` (`0x080368BC`) |
| `+0xD18` | 8 × 2 | The player's name: one Shift-JIS code per character, zero after the last |
| `+0xD28` | 4 | Money, capped at 9,999,999 |
| `+0xD2C` | 173 × 56 | Units: 153 ordinary, then 20 special (see below) |
| `+0x3304` | 1 | Ordinary units in use |
| `+0x334C` | 150 | Parts in stock, one count per part id (at most 9); the equipment screen moves parts between the stock and the units |
| `+0x33E2` | 1 each | Zoids seen, by picture id: the Zoid guide shows an entry whose byte is not zero |
| `+0x347B` | 1 each | Deck commands learned, by command number |
| `+0x349C` | 6 | The deck of deck commands, a command number a slot, `0xFF` when empty; the battle menu's コマンド作成 writes it (`0x0803B7B8`) |
| `+0x34A4` | 87 × 4 | Character table: a flag half-word (bit `0x01`: set whenever the game adds the character, `0x02`: a party member the status screens list, `0x08`: the equipment screen refuses to change the character's parts, `0x10`: in the formation, `0x20`: in the character guide), then the character's unit, `0xFF` when none |
| `+0x3600` | 6 × 4 | Formation slots: the unit, then its character, `0xFF` when empty |
| `+0x3618` | 1 | Battle message speed − 1 |
| `+0x3F0E` | 2 | Song playing when the pause menu opened |

When the pause menu opens the game copies the map, the player's metatile and the song
into the block, so a save always holds where the menu was opened. The player's facing
is not saved: a continued game faces down. Flag `0x11F` is set when a new game enters
the first room; continuing a save without it plays the opening again.

Entering a map whose area differs from the last one entered (RAM `0x0200000D`) rebuilds
the object states (`0x08006E4C`). The game goes through every record of the new area
whose id has bit 15 and adds a state for each object after the player:

| Offset | Size | Field |
|---|---|---|
| `+0x00` | 2 | The map record, with bit 15 while the object is there |
| `+0x02`, `+0x03` | 1 each | Metatile column and row, written back halfway through each step |
| `+0x04` | 2 | The sprite |
| `+0x06` | 2 | For a map Zoid, its formation slot |
| `+0x08` | 2 | The object's parameter |
| `+0x0A` | 1 | The object's command |
| `+0x0B` | 3 | Bytes the loader copies into the entity, zero when built |

A map Zoid (behavior 1) is drawn a formation, and its sprite becomes its leader's Zoid
(see [../combat.md](../combat.md)). The table holds at most 200 states; the rebuild
stops the game on an error screen past that. Continuing a game clears the area the
rebuild compares with (`0x0800C0CC`), so the first map entered always rebuilds the
table and the objects start again from their maps' lists. The formations are kept in
RAM outside the block (`0x02004A70`).

### Units

Source: the unit allocator (`0x08036A30`), its initializer (`0x08036B2C`) and the
statistics routine (`0x08036CB0`); see [../events.md](../events.md) for how the hangar
fills them.

| Offset | Size | Field |
|---|---|---|
| `+0x00` | 2 | The Zoid record's first half-word |
| `+0x02` | 2 | Flags: `1` in use, `2` in the formation, `4` piloted, `8` special |
| `+0x04` | 2 | The unit's own slot |
| `+0x06` | 2 | Zoid: index of the 76-byte records at ROM `0x670210` |
| `+0x08`, `+0x0C` | 4 each | Current values of the first two statistics |
| `+0x10` | 6 × 4 | Parts, the part id in the upper half-word (`0xFFFF` none) |
| `+0x28` | 12 | Statistics: two words and two half-words |
| `+0x34` | 1 | Training level (100 for special units) |
| `+0x35` | 1 | The Zoid record's byte 4: its size class, 0 S, 1 M, 2 L |

## New game

The new-game routine clears the block and then:

- sets the level to 1 and the message speed to 3;
- copies the four member records;
- marks every character's byte, the six two-byte pairs at `+0x3600` and the six
  bytes at `+0x349C` as `0xFF`;
- sets bits `0x03` of character 0 and bit `0x20` of the characters listed at the first
  pointer of ROM `0x66C8D0` (characters 0–3), which puts them in the character guide.

The name entry then writes the name. Entering the first room sets the area, flag
`0x11F` and the object states, and adds nothing else before the first save.

## Continuing

つづきから loads copy 0; if its sum fails it loads copy 1. It shows one of three strings
of the dialogue table in the story box and waits for a key:

| Case | Notice | Then |
|---|---|---|
| Copy 0 sound | none | The saved map |
| Copy 1 used | 12: セーブデータが壊れているため１つ前のセーブデータを復旧させます | The saved map |
| No header | 10: セーブデータがありません | The title starts over |
| Both broken | 11: セーブデータが壊れています | The title starts over |

Timings, measured frame by frame: the title starts to darken 10 frames after A and
fades out over 16 frames, the screen stays black for 73 frames and the room fades in
over 16. A notice appears 23 frames after the screen went black. After the notice for
copy 1 is dismissed the screen stays black for 33 frames before the room fades in.

## In this port

The launcher keeps the save as a 32 KiB file next to the ROM with the extension `.sav`,
as emulators do, so the same file works in both (and one such file per save slot, see
below). Saving writes the game-state block into
both copies of the existing file, keeping its other blocks, or into a fresh image when
there is none. The port models the position, flags, level, experience, money, message
speed, name, song, chests, deck commands, Zoids seen, the party the hangar forms and the
parts and stock the equipment screen changes;
the rest of the block is carried unchanged from the save that was loaded, or from the
new-game block.

A new game saved by the port matched the original's own save of the same moment byte
for byte, before the port built the object states. The states now differ where the
formations were drawn: the draws mix in the frame counter, and the port's draws do not
fall on the original's frames. The original rebuilds the table on continuing anyway.

The Shift-JIS name cannot hold every letter a translation's alphabet offers. Half-width
letters are stored as their full-width forms and others as `？`; the exact name is also
written, in UTF-8, in a note in the bytes after the copies: `RZSN`, the sum of the
game-state block it belongs to, a length byte and the name. The note is used only when
its sum matches the block loaded, so a save rewritten by the original falls back to the
block's own name.

## Save slots (a port feature)

Source of knowledge: this project's own design. The original has a single save; the port
keeps several, each a whole save memory in the format above, so every slot is a `.sav`
an emulator or a flash cart loads. The launcher offers four unless told otherwise
(`--slots n`, 1 to 9): slot 1 is the usual `.sav` (next to the ROM, or `--save`) and slot
n the same name with `.n` before the extension (`game.2.sav`). With one slot everything
behaves as the original.

With several, the pause menu's セーブ asks for a slot (see [../menu.md](../menu.md)) and
つづきから lists them over the title, in window 1 at (5, 3) 20×10 with the help in window 2
at (0, 14) 30×6: どのデータからつづけますか？, then the area and money of the slot under the
cursor. The cursor starts on the latest game: the slot stored last (the file's
modification time), the first of equal ones, or the first game when the storage keeps no
times. An empty slot is refused with sound `0x4F`; B goes back to the title's menu,
which starts over with its cursor on はじめから; a broken slot or one whose first copy
is broken goes through the loader's notices above. When no slot holds anything,
つづきから goes straight to the original's notice that there is no save. The list's
area is the one of the saved map's record rather than the block's byte, which a block
made by hand may lack. The title's guides read the latest game.

A slot saved by the port in a new game and in the first chapter continued in the
reference emulator on the saved map.

## The autosave (a port feature)

Source of knowledge: this project's own design. With the enhanced mode's autosave on
(see [../launcher.md](../launcher.md)), the game is also saved in a place of its own,
beside the slots with `.auto` before the extension (`game.auto.sav`), a save memory in
the format above like theirs. It is written each time the player walks freely on a map
other than the one they last walked freely on: once the arrival's fade, the map's
handler and any scene it starts have let go, and the conditions the pause menu opens
under hold, so the game saved is one the player could have saved by hand there. The
first map walked on after a new game or a continue is only noted. A warp that an event
makes in the middle of a scene saves nothing until the player has control again, on
the map the scene leaves them on. The classic mode never writes it.

Each autosave joins a queue (`crates/game-core/src/autosave.rs`) that a worker thread
empties in the order the saves were asked for, so the field never waits for the disk
and quick changes of map are all written, the last one last. Reading the autosave (the
title's list, continuing) first waits for the queue, and so does leaving the game.
`Event::Autosaved` reports each save once written, `Event::StorageFailed` one that
could not be.

While the queue works the field shows `port/autosave/notice` (Autosaving) with one to
three dots after it, a dot more every 12 frames, at the top left (the glyphs' corner at
(4, 0), the letters from row 4): the port's small capitals, five pixels tall, with no
plate, white at 176/256 over the picture and a shadow a pixel down and right in black
at 128/256. It fades in over 16 frames when a save is queued, stays at least 60 frames
so it can be read, and once the queue is empty fades out over 16; a save queued
meanwhile keeps it.

つづきから lists the autosave first once it holds anything, marked `port/save-slots/autosave`
(Ａ) where the slots show their number, the slots numbered from 1 after it; with it
under the cursor the help's first line is `port/save-slots/autosave-help`. The list
grows a line for it, moving up (window 1 at (5, 1) 20×12 with four slots), and the
cursor starts on the latest game among all of them, as before. The autosave counts as
a save to choose from, so with one slot and an autosave the list shows too. A game
continued from it has no slot of its own: the pause menu's セーブ starts on the first
empty slot, else the latest game. セーブ and the end of the demo never offer the
autosave, and the title's guides read the latest game, the autosave included.

The autosave was continued in the port on the map it was written on, after walking
through an exit in the third area, and four changes of map in a row were written in
their order.

## Not modeled yet

The counter block, the member records beyond carrying them, units
gained outside the opening chapter, and the title's opening animation when it starts
over after a notice.
