# Translations

Source of knowledge: the script tables the port already runs (see
[formats/script-text.md](formats/script-text.md)) and the gettext PO format Weblate
speaks. Implemented in `crates/game-core/src/translation.rs`, hooked into the script
runner (`script.rs`) and the launcher.

Translations live outside this repository. The Japanese text is copyrighted ROM
content, so nothing derived from it is committed: the translation template is
generated from a player's own ROM, uploaded to the project's Weblate, and the PO
files translators produce are downloaded by each player and handed to the launcher.

## Workflow

1. A maintainer exports the template from their ROM:

   ```bash
   launcher baserom.gba --export-template zoids-saga.pot
   ```

   By default it covers the title menu, the name entry and dialogue strings 30–41 (the
   helpers every conversation shares and the opening). Any tables and ranges can be
   named instead: `title`, `name-entry`, `pause-menu`, `dialogue` or `dialogue:30-41`.
2. The template is uploaded to Weblate as the source of a gettext component; every
   language is a PO file with the same keys.
3. A player downloads the PO file of their language and starts the game with it:

   ```bash
   launcher baserom.gba --translation es.po
   ```

   Messages the file covers are shown in place of the ROM's text; the rest stays
   Japanese, so a partial file works.

## Keys and markers

Each message is keyed by its script table, string index and the message's offset from
the start of the string, e.g. `dialogue/40/0x2e` in `msgctxt`; the Japanese text is the
`msgid`. A string can hold several messages (one per text box) and the key stays
valid however the script branches.

Inside the text:

| Marker | Meaning |
|---|---|
| `\n` | New line |
| `{name}` | The player's name |
| `{varN:D}` | Script variable N printed in D cells |
| `{window:N}` | Continue in window N (rare; leading window switches are not part of the text) |

A `{...}` that is not a marker prints as written.

## Name entry

The name entry's character pages and help line are not script messages, so the file
replaces them through reserved keys, which the template lists with the ROM's pages
as their source text:

| Key | Text |
|---|---|
| `name-entry/help` | The help line, 160 pixels |
| `name-entry/alphabet/N` | Page N: a label line (up to 72 pixels), then up to 5 lines of up to 13 characters each; a space is an empty cell |

A translation may have any number of pages; SELECT cycles them. The label window
grows to the left for longer labels. Characters the player picks are stored as
written, so the name prints and draws through the same rules as any text.

## Fonts and layout

Japanese text draws with the ROM's 8×16 font, one cell per character. Latin text
draws with this project's own pixel font (`assets/fonts/latin/re-zoids-latin.txt`,
an original asset in the repository): proportional, 3–5 pixels wide plus a pixel of
spacing, capitals 8 pixels tall, with accented letters built from the plain ones
(acute, grave, circumflex, diaeresis, tilde, cedilla) and `¿` `¡` as `?` `!` turned
around. Characters neither font has draw as the fallback glyph. In the name entry's
grid and name field every character is centered in an 8-pixel cell so it lines up
with the cursor and the slot marks.

Text wraps by pixels at the window's inner width; a menu window keeps two cells free
on each side for the cursor (see [formats/window.md](formats/window.md)). What fits
on a line, in Latin letters of average width:

| Where | Pixels per line | Roughly | Lines |
|---|---|---|---|
| Story box (dialogue) | 224, or 176 beside a portrait | 40 or 30 letters | 3 beside a portrait; a longer message waits for A when full, then turns a page: the name row stays and two fresh lines follow |
| Title menu | 40 before enlarging | 7 letters | 3 |
| Name-entry help | 160 | 28 letters | 1 |
| Name-entry question | 224 | 40 letters | 2 |
| Pause-menu list | 40 before enlarging | 7 letters | 6 |

When a translation is loaded the game walks each translated string (and the
strings it calls) to find the window every message lands in, and enlarges the windows
whose translated lines do not fit: a window grows to the widest line plus its margins
(and, for a menu, to the number of choices), keeping its center where it was and
staying on screen. A window is not enlarged when the larger one would cover another
window open at the same time that the original did not already cover. The launcher
prints one line per message that cannot fit even a screen-wide window or would cover
another; those still wrap mid-word. The story box beside a portrait is already the
full screen width, so its lines must stay within 176 pixels.

Messages the game's own code prints into fixed columns are not fitted: the status
screens place signs and values by cell, so their labels must stay within the pixels
the Japanese label takes (a stat label ends at cell 11; the level window leaves six
cells for the experience value). Item, weapon and Zi-data descriptions are printed by
code into windows the walker cannot see: keep weapon descriptions to one line of
176 pixels, the two-line Zi-data texts to 216 pixels per line and the narrow
variants to five lines of 104 pixels.

## Not modeled yet

Names, items and other strings the game prints outside scripts, messages reached only
through script jumps the layout walk does not follow, and the cedilla.
