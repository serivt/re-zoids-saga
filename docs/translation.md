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

## Layout

The engine wraps at the window's inner width, one character per 8-pixel cell, and a
menu window keeps two cells free on each side for the cursor (see
[formats/window.md](formats/window.md)):

| Where | Cells per line | Lines |
|---|---|---|
| Story box (dialogue) | 28, or 22 beside a portrait | 2 per page, longer messages scroll |
| Title menu | 5 | 3 |
| Name-entry question | 28 | 2 |
| Pause-menu list | 5 | 6 |

When a translation is loaded the launcher walks each translated string (and the
strings it calls) to find the window every message lands in, and enlarges the windows
whose translated lines do not fit: a window grows to the widest line plus its margins
(and, for a menu, to the number of choices), keeping its center where it was and
staying on screen.
The title menu, for instance, grows from 5 to 13 cells for "Nueva partida". The
launcher prints how many windows it enlarged and one line per message that cannot fit
even a screen-wide window; those still wrap mid-word. The story box beside a
portrait is already the full screen width, so its lines must stay within 22 cells.

The ROM font has no half-width Latin letters, so ASCII draws with its full-width
forms (`Ａ`). Accented letters are built from the plain letter with the mark drawn
above it (acute, grave, circumflex, diaeresis, tilde), and `¿` and `¡` are `？` and
`！` turned around; other characters the font lacks draw as the fallback glyph. A
modern Latin font is future work.

## Not modeled yet

Names, items and other strings the game prints outside scripts, messages reached only
through script jumps the layout walk does not follow, and the cedilla.
