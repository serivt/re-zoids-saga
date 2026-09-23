# Zoid and character guides

Source of knowledge: own analysis of Zoids Saga (Japan, Rev 1). A read of the Zoid
guide's state machine (`0x080431D0`), the character guide's (`0x0804D0D0`) and the
routines they call: the known checks (`0x080431AC`, `0x0804CD84`), the silhouette
(`0x080430EC`), the backdrop and picture loaders (`0x08043EAC`, `0x08043D6C`,
`0x08044E98`), the parts (`0x08043164`, `0x0804588C`, `0x08045678`) and the character
menu's flags (`0x0804CFFC`, `0x0804CF64`, `0x0804CE3C`). In a reference emulator:
screenshots, VRAM, OAM and register dumps of both guides with a save and with one
patched to have seen every Zoid, the BIOS decompressions logged while an entry loads,
and every transition measured frame by frame. Implemented in
`crates/extraction/src/saga_guide.rs` (tables, pictures, parts),
`crates/game-core/src/guide.rs` (the screens) and `game.rs` (the title's options).

## From the title

The title's second menu (オプション) leaves in the stored var1 the option's line plus 4,
except the link-cable ones:

| var1 | Option |
|---|---|
| 4 | ゾイド図鑑, the Zoid guide |
| 5 | キャラクター図鑑, the character guide |
| 8 | 通信対戦, a battle over the link cable |
| 11 | Ｚｉデータ受け渡し, trading Zi data |

The guides read what the player has seen from the save, loaded silently as continuing
does; without a save they read a new game's state. Opening one holds the title 11
frames after A, fades it out over 16, keeps the screen black 54 frames and fades the
guide's menu in over 16. Leaving holds 10 frames, fades out over 16 and stays black 45
frames before the title starts over.

## Scripts

The guides' menus are strings of a table of scripts the game's code runs by index,
`system`, 30 pointers at ROM `0x6D0880`: string 2 is the Zoid guide's menu, 4 the
character guide's. Strings 3 and 5 are older lists with placeholder texts that the game
does not reach; string 1 is a debug menu.

| Table | ROM | Strings |
|---|---|---|
| `zoid-guide` | `0x70081C` | 0 opens the header, stats and description windows; 1 clears them; 2 the popup; 3 the unknown entry; then 4 armies × 20 types × 20 entries |
| `character-guide` | `0x7056B8` | 0 opens seven windows; 1 clears them; 2 the popup; then 5 series × 10 groups × 20 entries |
| `characters` | `0x706664` | 87 pointers, every character's entry in the order of the save's character table |

An entry prints into windows the helper opened, switching window inside its messages.
A Zoid entry sets var5 to the Zoid's picture id; a character entry shows the portrait
with the portrait opcode. The popup leaves 1 (next), 2 (previous) or 255 (back to the
menu) in var0, and 0 when B closes it.

## Zoid guide

The menu stores the army in var7 (255 to leave) and the type in var6; the row is
`4 + (army × 20 + type) × 20`. Menus longer than their window scroll: the cursor moves
to the last shown line, then the list moves one line at a time, with the frame's middle
tiles `0x32` and `0x34` marking lines hidden above and below.

An entry runs helper 0, helper 1 and the entry; if the Zoid is unseen (its byte at
`+0x33E2 + id` in the game state is 0), helper 1 and helper 3 replace the text with
question marks and every color of the Zoid and its parts is divided by eight, leaving a
silhouette. Choosing an entry fades out 10 frames after A over 16, keeps the screen
black 49 frames (34 between entries) and fades in over 8; the backdrop and windows show
alone for 13 frames, then the Zoid slides in from the right over 17 frames, `u` frames
from the end standing `u × (150 − u) / 18` pixels right of its place. A shows the popup,
L and R page directly, B goes back to the menu (black 40 frames, menu fading in over
16). Paging wraps within the row.

What the screen is made of:

| Layer | Content |
|---|---|
| BG0 | The windows |
| BG1 | The Zoid: 256 8bpp tiles from the record of `0x6F8974 + id × 12`, 64 colors from `0x6F9100 + id × 12` at colors 0–63, drawn at x 112 with the 16×16 map at `0x564548` |
| BG2 | The backdrop: tiles and 64 colors (at 64–127) from `0x6F6934` and `0x6F6BBC`, record `terrain × 3 + variant` with the terrain a byte at `0x6D3D94 + id` and the variant byte 4 of the Zoid's 76-byte record at `0x670210`; the same map, twice across |
| OBJ | Up to three parts |

The records are 12 bytes: an LZ77 source, the destination and a word the loader does
not need here.

### Parts

A Zoid's record names parts every four bytes from `+0x0A`; the guide draws the first
three that are neither 0 nor `0xFFFF` (27 of the 141 entries have one: turrets, cannons).
The second reads its 16-byte record from `0x6F77C4 + part × 16`, the others from
`0x6F6E44`: LZ77 4bpp tiles, an LZ77 palette of 16 colors, an animation table and a
frame table in the map sprites' format (see [formats/sprite.md](formats/sprite.md)),
except that a frame is a list of 20-byte pieces (tile, flags, x, y, width, height,
eight bytes) ended by a tile of `0xFFFF`, one OBJ each. The part's anchor on the picture
is in a 28-byte row per Zoid at `0x6E78EC`: seven x values then seven y values, slot
by slot, `0xFFFF` meaning the seventh. A piece's corner is at the picture's left edge
plus the anchor plus the piece's offset. The guide plays animation 0.

## Character guide

Before its menu the guide clears flags 0–29 and sets, for each series with a known
character, the series flag (0, 5, 10, 15, 25) and, for each of its ten groups that has
one, the series flag plus the group's number (1 to 10). A character is known when bit `0x20` of its half-word in the
character table (`+0x34A4 + index × 4`) is set; its index is the position of its entry
in the `characters` table. The menu shows only series and groups whose flags are set,
so the menu line is turned back into a series and a group by counting the set flags
among the candidates: series flags 0, 5, 10, 15, 25; groups 1–4, 6–9, 11–14, 16–24 and
26–29.

Entries of characters the player does not know are skipped. An entry runs helper 0 and
the entry, and shows the character's map sprite, `0x98 + index`, standing and facing
down with its 32×32 frame at (80, 16). Choosing a group fades out 10 frames after A,
stays black 51 frames (38 between entries) and fades in over 8.

## Not modeled yet

The backdrop's drift: the original scrolls the backdrop line by line from the battle
backgrounds' code, the sky by 1/16 of a pixel a frame and the nearer bands faster, with
bands that differ per backdrop; here it stays where it loads. The title's opening
animation after leaving a guide, the link-cable options (the port stays on the title),
and the parts' animations beyond the first.
