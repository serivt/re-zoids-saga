# Script text

Source of knowledge: own analysis of the Zoids Saga (Japan, Rev 1) script interpreter
(the Thumb routine at ROM `0x0803E54C`, which dispatches opcodes `0x00`–`0x3D` through a
62-entry jump table at `0x0803E574`; message rendering is opcode `0x20`'s handler at
`0x0804017E`). Implemented in `crates/formats/src/script_text.rs` and read through
`crates/extraction/src/string_table.rs`.

## String tables

Each table is an array of little-endian 32-bit ROM pointers (`0x08xxxxxx`), one per
string. Strings are read from their pointer until the end marker; consecutive pointers
usually abut but the length is never stored. Tables located in Zoids Saga
(`crates/extraction/src/saga.rs`):

| Name | Pointer array | Entries | Content |
|---|---|---|---|
| `name` | `0x67601C` | 248 | Zoid and character names (also a few multi-part strings) |
| `item` | `0x6763FC` | 148 | Items, parts and their effect text |
| `dialogue` | `0x74FC54` | 1017 | Story and character dialogue, ~300 KB of script; the game indexes this table directly (map objects name entries by index), 29 of the first 40 pointers are null |
| `battle` | `0x755D30` | 198 | Battle quotes |
| `menu` | `0x75B388` | 156 | Menu strings |

String IDs are `<table>_<index>` with a five-digit zero-padded index.

## Encoding

A string is a byte stream of **opcodes**. Text only exists inside a *message block*,
opened by opcode `0x20` and closed by `0x1D`. Inside a message:

- A byte `>= 0x40` starts a two-byte **character**. The two bytes hold a Shift-JIS code
  with the trail byte first: `5D 83` is `ゾ` (Shift-JIS `0x835D`). Shift-JIS trail bytes
  are never below `0x40`, so every byte below `0x40` is unambiguously a control code.
  The runtime composes the code as `bytes[1] << 8 | bytes[0]`.
- Control codes inside a message:

| Byte | Meaning | Args |
|---|---|---|
| `0x0D` | Line break | 0 |
| `0x1C` | Unmodeled control | 1 |
| `0x1D` | End of message | 0 |
| `0x1E` | Insert the player-name buffer (RAM `0x0200758C`, up to 8 characters) | 0 |
| `0x1F` | Print a game variable | 2 |
| `0x0E`–`0x1B`, others `< 0x40` | Ignored by the game | 0 |

## Opcodes outside messages

Semantics read from each handler of the jump table at `0x0803E574` and checked against the
first room's conversations in a reference emulator. Implemented in
`crates/formats/src/script_ops.rs` (decoding) and `crates/game-core/src/script.rs`
(execution). The interpreter keeps eight 16-bit variables, a saved copy of them, a
current text window, and runs strings recursively through calls. Jump offsets are signed
16-bit values relative to the opcode's own address.

| Opcode | Args | Meaning |
|---|---|---|
| `0x00` | 0 | Nothing |
| `0x01` | 7 | Open window `a0` of kind `a1` at tiles (`a2`, `a3`), `a4`×`a5` tiles, style `a6` (bit 0: text appears one character per frame); it becomes the current text window |
| `0x02` | 1 | Reset the text system; mode 1 also clears the name buffer; modes below `0x10` zero the variables |
| `0x03` | 1 | Close window `a0` (`0xFF` = all) and redraw the others |
| `0x04` | 1 | Present window `a0` (`0xFF` = all) |
| `0x05` | 1 | Wait for a key: low nibble 0 accepts A (sets var0 = 1, plays sound `0x41`), `0x10` in the high nibble lets B end it with var0 = 0; the prompt blinks 20 frames off, 20 on |
| `0x06` | 3 | Menu selection (not modeled) |
| `0x07` | table | Switch: `a0 & 7` selects entry `n` of a table of signed 16-bit offsets; the table has no length field and ends where its nearest target begins |
| `0x08` | 2 | Jump |
| `0x09` | 3 | var[`a0 & 7`] = `a1 | a2 << 8` |
| `0x0A` | 2 | var[`a0`] = var[`a1`] |
| `0x0B`, `0x0C` | 0 | Save / restore the eight variables (RAM `0x02007574`) |
| `0x0D` | 1 | Draw window `a0` |
| `0x0E` | 1 | Clear the text of window `a0` |
| `0x0F` | 3 | Portrait of character `a1`, expression `a2` in window `a0` |
| `0x10`–`0x12` | 5 | Jump when var[`a0`] ==, > or < `a1 | a2 << 8` |
| `0x13`, `0x14` | 4 | Jump when var[`a0`] == or > var[`a1`] |
| `0x15`–`0x18` | 3 | var[`a0`] += / −= / ×= / ÷= 16-bit value |
| `0x19`–`0x1C` | 2 | var[`a0`] += / −= / ×= / ÷= var[`a1`] |
| `0x20` | message | Message, see above; each character of a typewriter window costs a frame |
| `0x21` | 2 | Call string `a0 | a1 << 8` of the same table, then continue |
| `0x22` | 0 | End of string (return to the caller) |
| `0x30`, `0x31` | 1 | Skipped |
| `0x32` | 2 | Clear (`a1` = 0) or set (`a1` = 1) game flag var[`a0`] |
| `0x33` | 3 | Clear or set game flag `a0 | a1 << 8` |
| `0x34` | 2 | var[`a0`] = game flag var[`a1`] |
| `0x35` | 3 | var[`a0`] = game flag `a1 | a2 << 8` |
| `0x36`, `0x38`, `0x3C` | 1, 2, 1 | Not modeled |
| `0x37` | 3 | Portrait of character var[`a1`], expression var[`a2`] in window `a0` |
| `0x39` | 3 | var[`a0`] = `a1` |
| `0x3A` | 1 | Play sound effect `0x3C + a0` |
| `0x3B` | 1 | Set bit 4 of window `a0`'s flags (not modeled) |
| `0x3D` | 1 | Wait `a0` frames |

Presenting, closing or clearing a window and showing a portrait flush the display, which
costs one frame each. Inside a message, `0x1C a0` redirects the text to window `a0`, and
`0x1F a0 a1` prints var[`a0`] right-aligned in `a1 & 7` cells.

## Shared strings

Strings 30–37 of the dialogue table are helpers every conversation calls:

| String | Content |
|---|---|
| 30 | Restore variables, open the portrait window 0 (0, 12, 8×8) and the text window 1 (7, 12, 23×8, typewriter), show the portrait named by var0/var1, present all |
| 31 | Open the text window 1 at (0, 12, 30×8, typewriter) and present: the box of characters on the map |
| 32 | Wait for A, clear window 1: the pause between two messages |
| 33 | Wait for A, close windows 0 and 1 |
| 34 | Wait for A, close window 1 |
| 35 | Restore variables, redraw the portrait from var0/var1, present |
| 36 | Close windows 0 and 1 |
| 37 | The yes/no menu at (20, 4, 7×6) |

## Typical dialogue

```
09 00 02 00   set var0 = 2            (speaker?)
09 01 00 00   set var1 = 0
0B            store variables
21 1E 00      call string 0x001E      (shared "open text box" script)
20 ... 1D     message: name, line break, 「...
21 20 00      call string 0x0020      (shared "wait for button" script)
...
21 21 00      call string 0x0021      (shared "close text box" script)
22            end
```
