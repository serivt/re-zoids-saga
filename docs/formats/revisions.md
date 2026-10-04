# The two releases

Source of knowledge: own comparison of Zoids Saga (Japan) and Zoids Saga (Japan, Rev 1):
a word-by-word alignment of the two images, the table addresses each release's code
holds in its literal pools, and the records of every table the port reads, read from
both. Implemented in `crates/extraction/src/revision.rs`.

Zoids Saga came out twice on the cartridge, with the same game code `ATZJ`:

| Release | Header version (`0xBC`) | SHA-1 |
|---|---|---|
| First release (Rev 0) | `0` | `75d8c15ac281ea93c8ac7cc7641c490799557081` |
| Revision (Rev 1) | `1` | `70bb546a7d00126d452c1d2c1ccddafb2cb91b37` |

The port names every table by the offset Rev 1 keeps it at. The image's own header says
which release it is, so every reader of the ROM follows the release it was given; an
image without the game's header (a test's) reads as Rev 1.

## What Rev 1 changed

Both releases run the same program over the same data. Rev 1 rewrote some functions and
some text, and everything after each change moved:

- **Code.** Functions from `0x36A30` on are up to 280 bytes later in Rev 1. The code the
  objects and cutscenes name (`0x0800xxxx` up to `0x0802AB08`) sits at the same address
  in both.
- **Data.** The data after the code is 280 bytes later in Rev 1, then 420 after the
  stretch Rev 1 changed at `0x534FEC`, and so on, 520 at the end.
- **Text.** Some messages were retouched: long lines split in two, お父様 became パパ,
  a few words changed. Rev 1 also added the line telling that a corrupt save was
  restored from the one before it (`dialogue` 12, empty in the first release) and one
  script to the menus' table (163 of `pause-menu`, 48 of `menu`); the scripts after it
  are one place later.
- **Guide.** The first release lists the Sinker among the Republic's Zoids (entry 84,
  the two Hammerheads after it) and the Maccurtis at 665 among the Empire's; Rev 1
  moved the Sinker to 665 and the Maccurtis to 666.
- **Records.** Zoid 142's second part slot; the development of Zoids 9, 37 and 126,
  which needs no Zoid in the first release and Zoids 8, 36 and 16 in Rev 1; and the
  pictures of Zoid 112 and of sprites 276 to 289.

The port plays the first release with its own data: its text, its guide and its
records, as that cartridge shows them.

## Where the first release keeps each table

A Rev 1 offset moves by the shift of the stretch it falls in, each holding until the
next one:

| From (Rev 1) | Shift | After |
|---|---|---|
| `0x36A30` | −148 | the first rewritten functions |
| `0x4EAC8` | −152 | |
| `0x52190` | −180 | |
| `0x5534C` | −236 | |
| `0x570E8` | −216 | |
| `0x58D8C` | −200 | |
| `0x59528` | −280 | the end of the code |
| `0x535E6C` | −420 | |
| `0x675D10` | −424 | strings of the `battle-label` to `item` tables |
| `0x6C8BBC` | −420 | |
| `0x6FED54` | −416 | |
| `0x702CE8` | −380 | the guides' scripts |
| `0x70852C` | −452 | the dialogue |
| `0x75A1A0` | −516 | the battle and menu scripts |
| `0x75B44C` | −520 | the menus' script Rev 1 added at `0x75B448` |

A table's pointers lead to the release's own data, so only the offsets the port knows by
heart move. The entry Rev 1 added (`0x75B448`) reads as an absent string in the first
release.

## Translations

A translation names a message by its table, its string and its offset in the string
(`dialogue/301/0x977`). Where Rev 1 retouched a message, the messages after it in the
same string moved: 673 messages, in 28 stretches of 27 strings. The port gives each of
them the offset Rev 1 keeps it at, so one translation and one template serve both
releases. The messages Rev 1 retouched keep their keys, and a translation of the Rev 1
text shows for the first release's.

## Saves

The save routine's descriptor, the header it writes and the new game's state are the same
in both releases, so a save made with one loads with the other.
