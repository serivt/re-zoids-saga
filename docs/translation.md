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

Text longer than a line wraps mid-word and, in a menu, breaks the choices; keep each
line within the cells above and break lines with `\n` where the original does.

The ROM font has no half-width Latin letters, so ASCII draws with its full-width
forms (`Ａ`), accented Latin letters draw as their plain ones (`é` as `e`, `ñ` as `n`,
`¿` as `?`) and other characters the font lacks draw as the fallback glyph. A modern
Latin font is future work.

## Not modeled yet

Names, items and other strings the game prints outside scripts, and a check that a
translation fits its windows.
