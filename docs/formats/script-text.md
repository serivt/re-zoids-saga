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
| `dialogue` | `0x74FCF4` | 977 | Story dialogue, ~300 KB of script |
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

Argument counts were read from each handler (bytes consumed after the opcode). Opcodes
not listed take no arguments; the executor skips unknown bytes one at a time.

| Opcode | Args | Notes |
|---|---|---|
| `0x01` | 7 | |
| `0x02`–`0x05`, `0x0D`, `0x0E`, `0x30`, `0x31`, `0x36`, `0x3A`–`0x3D` | 1 | |
| `0x06`, `0x09`, `0x0F`, `0x15`–`0x18`, `0x33`, `0x35`, `0x37`, `0x39` | 3 | `0x09` sets variable `arg0 & 7` to the 16-bit value `arg1 | arg2 << 8` |
| `0x0A`, `0x19`–`0x1C`, `0x32`, `0x34`, `0x38` | 2 | `0x19`–`0x1C` are add/sub/mul/div between variables |
| `0x10`–`0x12` | 5 | |
| `0x0B`, `0x0C` | 0 | Copy the eight script variables to/from RAM `0x02007574` |
| `0x20` | message | See above |
| `0x21` | 2 | **Call** string `arg0 | arg1 << 8` of the same table, then continue |
| `0x22` | — | **End** of string |
| `0x08` | 2 | **Jump**: cursor = opcode address + signed 16-bit offset |
| `0x13`, `0x14` | 4 | **Conditional jump**: compare variables `arg0 & 7` and `arg1 & 7` (equal / greater), then jump by the signed 16-bit offset in `arg2`, `arg3` |
| `0x07` | table | **Switch**: `arg0 & 7` selects entry `n` of a table of signed 16-bit offsets that starts right after `arg0`; the game skips `2n` bytes and jumps by the offset found there. The table has no length field: it ends where its nearest target begins |

Eight 16-bit script variables live on the interpreter's stack; `0x0B`/`0x0C` exchange
them with RAM. Their semantics (speaker, portrait, flags) are not modeled yet: the codec
keeps opcodes with raw arguments.

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
