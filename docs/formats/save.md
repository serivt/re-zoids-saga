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
| `+0x02` | 1 | Area: low byte of the current map record's id, written on entering a map |
| `+0x04` | 2 | Map record |
| `+0x06` | 2 | Metatile column of the player |
| `+0x08` | 2 | Metatile row |
| `+0x0C` | 33 × 2 | Game flags: flag `n` is bit `15 − n % 16` of half-word `n / 16`; flag `0x1E + n` marks chest `n` opened |
| `+0x50` | 16 each | Object states of the current area, ended by `0xFFFF` (not modeled) |
| `+0xCD2` | 1 | Party level (99 at most) |
| `+0xCD4` | 4 | Party experience |
| `+0xCD8` | 4 × 16 | Member records, copied from ROM `0x67AC4C` by a new game: the pilot bonuses in percent at `+4`, `+6`, `+8`, `+10`, `+12` (耐久, 反応, 防御, 攻撃, 命中); the hangar sets the three warriors' to the party level times their growth at ROM `0x66BB38` (`0x080368BC`) |
| `+0xD18` | 8 × 2 | The player's name: one Shift-JIS code per character, zero after the last |
| `+0xD28` | 4 | Money, capped at 9,999,999 |
| `+0xD2C` | 173 × 56 | Units: 153 ordinary, then 20 special (see below) |
| `+0x3304` | 1 | Ordinary units in use |
| `+0x33E2` | 1 each | Zoids seen, by picture id: the Zoid guide shows an entry whose byte is not zero |
| `+0x347B` | 1 each | Deck commands learned, by command number |
| `+0x34A4` | 87 × 4 | Character table: a flag half-word (bit `0x01`: set whenever the game adds the character, `0x02`: a party member the status screens list, `0x10`: in the formation, `0x20`: in the character guide), then the character's unit, `0xFF` when none |
| `+0x3600` | 6 × 4 | Formation slots: the unit, then its character, `0xFF` when empty |
| `+0x3618` | 1 | Battle message speed − 1 |
| `+0x3F0E` | 2 | Song playing when the pause menu opened |

When the pause menu opens the game copies the map, the player's metatile and the song
into the block, so a save always holds where the menu was opened. The player's facing
is not saved: a continued game faces down. Flag `0x11F` is set when a new game enters
the first room; continuing a save without it plays the opening again.

Entering a map whose area differs from the one in `+0x02` rebuilds the object states
from the map data of every record of the new area whose id has bit 15. Continuing a save
restores `+0x02` as the area already entered, so a save whose area byte does not match
its map makes the game rebuild the table on loading.

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
as emulators do, so the same file works in both. Saving writes the game-state block into
both copies of the existing file, keeping its other blocks, or into a fresh image when
there is none. The port models the position, flags, level, experience, money, message
speed, name, song, chests, deck commands, Zoids seen and the party the hangar forms;
the rest of the block is carried unchanged from the save that was loaded, or from the
new-game block.

A new game saved by the port matches the original's own save of the same moment byte
for byte, except for the object states, which the port does not build. So that the
original rebuilds them, the port writes area 0, which no map uses, unless the player is
still in the area the loaded block describes. The original was checked continuing from
such a save: it stood the player on the saved metatile and rebuilt the table.

The Shift-JIS name cannot hold every letter a translation's alphabet offers. Half-width
letters are stored as their full-width forms and others as `？`; the exact name is also
written, in UTF-8, in a note in the bytes after the copies: `RZSN`, the sum of the
game-state block it belongs to, a length byte and the name. The note is used only when
its sum matches the block loaded, so a save rewritten by the original falls back to the
block's own name.

## Not modeled yet

The object states, the counter block, the member records beyond carrying them, units
gained outside the opening chapter, and the title's opening animation when it starts
over after a notice.
