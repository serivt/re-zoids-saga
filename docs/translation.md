# Translations

Source of knowledge: the script tables the port already runs (see
[formats/script-text.md](formats/script-text.md)) and the gettext PO format.
Implemented in `crates/game-core/src/translation.rs`, hooked into the script runner
(`script.rs`) and the launcher.

Translations live outside this repository, in
[re-zoids-saga-translations](https://github.com/serivt/re-zoids-saga-translations).
The Japanese text is copyrighted ROM content, so nothing derived from it is published:
the PO files there hold each message's key and translation only (their `msgid`
repeats the key), and the Japanese template is generated from each translator's own
ROM. That repository's README and tools cover correcting a line, translating with the
Japanese alongside and checking the files.

## Workflow

1. A translator exports the template from their own ROM:

   ```bash
   launcher baserom.gba --export-template zoids-saga.pot
   ```

   By default it covers the title menu, the name entry, dialogue strings 30–41 (the
   helpers every conversation shares and the opening) and the port's own messages.
   Any tables and ranges can be named instead: `title`, `name-entry`, `pause-menu`,
   `dialogue` or `dialogue:30-41`, the battles' `battle`, `battle-text`,
   `battle-menu` and `battle-label`, `item`, `name`, `part` (the parts' names), the
   guides' `system`, `zoid-guide` and `character-guide` (see [guide.md](guide.md)),
   and `port`, the port's own messages (see below).
2. The translations repository's `tools/add_source.py` puts the template's Japanese
   beside a translation to edit it with any PO editor, and `tools/strip_source.py`
   takes it out again before the file is committed; `tools/check.py` refuses Japanese
   in a published file.
3. A player downloads their language from the launcher's Translation screen, which
   lists the languages of that repository's `po/languages.json` and keeps the one chosen
   (see [launcher.md](launcher.md)), or downloads the PO file by hand and starts the game
   with it:

   ```bash
   launcher baserom.gba --translation es.po
   ```

   Messages the file covers are shown in place of the ROM's text; the rest stays
   Japanese, so a partial file works.

## Keys and markers

Each message is keyed by its script table, string index and the message's offset from
the start of the string, e.g. `dialogue/40/0x2e` in `msgctxt`. The loader reads only
`msgctxt` and `msgstr`: in a template the `msgid` is the Japanese text, in a published
translation it repeats the key. A string can hold several messages (one per text box) and the key stays
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

The name entry starts from the player's name, アトレー by default. With a translation the
default becomes its Latin form, **Atory**, so a player who keeps it reads it in their
language; a name entered or loaded from a save stays as it is.

## The port's own messages

What the port adds to the game (the save slots, see [menu.md](menu.md), and the end of
the demo, see [events.md](events.md)) has no ROM
text, so its messages are this project's own, in Japanese by default, under reserved
keys that start with `port/`. The template lists them with the scope `port` (part of
the default scopes), each with a note on its room; markers `{level}`, `{area}`,
`{money}` and `{slot}` print the value with full-width digits. The launcher reports a
translated message wider than its note allows.

| Key | Default | Where |
|---|---|---|
| `port/save-slots/level` | Ｌｖ{level} | A slot's level, from cell 11 of the list, 32 pixels |
| `port/save-slots/empty` | データなし | An empty slot, 112 pixels |
| `port/save-slots/broken` | こわれたデータ | A slot whose copies are both broken, 112 pixels |
| `port/save-slots/details` | エリア{area}　所持金{money}Ｇ | The help's second line for a slot with a game, 224 pixels |
| `port/save-slots/save-help` | どのスロットにセーブしますか？ | The help line while saving, 224 pixels |
| `port/save-slots/load-help` | どのデータからつづけますか？ | The help line while continuing, 224 pixels |
| `port/save-slots/question` | スロット{slot}にセーブしますか？ | The question for an empty slot, 224 pixels |
| `port/save-slots/overwrite` | スロット{slot}に上書きしますか？ | The question over a slot's game, 224 pixels |
| `port/demo/thanks` | あそんでくれて　ありがとう！ and two more lines | The thanks at the end of the demo, 3 lines of 224 pixels |
| `port/demo/question` | ここまでの記録を　セーブしますか？ | The question after them, over the original's はい/いいえ |
| `port/demo/saved` | セーブしました。 | The notice once saved |
| `port/launcher/...` | English | The launcher's screens, main, options, about and the keyboard's and gamepad's buttons, and the question before closing (asked in the game too): labels and values of the panel up to 208 pixels (the pad's directions up to 40), the subtitle, status and help lines up to 232 |

The launcher's screen comes before any ROM is read, so its messages are English by
default and switch to those of the translation chosen on it; the project's name,
Re:Zoids Saga, is never translated.

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
| Story box (dialogue) | 224, or 168 beside a portrait | 40 or 29 letters | 3 beside a portrait; a longer message waits for A when full, then turns a page: the name row stays and two fresh lines follow |
| Title menu | 40 before enlarging | 7 letters | 3 |
| Name-entry help | 160 | 28 letters | 1 |
| Name-entry question | 224 | 40 letters | 2 |
| Pause-menu list | 40 before enlarging | 7 letters | 6 |

When a translation is loaded the game walks each translated string (and the
strings it calls) to find the window every message lands in, and enlarges the windows
whose translated lines do not fit: a window grows to the widest line plus its margins
(and, for a menu, to the number of choices, unless the original menu already had more
lines than rows and scrolls), keeping its center where it was and
staying on screen. A window is not enlarged when the larger one would cover more of
another window open at the same time that the original kept clear of or only bordered
(the story box shares one column with the portrait beside it); a window the original
already drew over, as a menu over the menu that opened it, may be covered further.
Windows are fitted per string that opens them, so every conversation shares the story
box of the helper strings: one message too wide for it would enlarge the box for all. A
window one string opens and others print into is fitted from a list the port keeps:
the attack scenes' weapon window (`system` 0x10) grows to the widest translated part
name among the weapons and line of `system` 0x11 and 21. A line wider than a window
that is not fitted wraps, and in a window whose rows are full the text then waits for
a key: the weapon window's and the message window's lines of the aim must fit. The launcher
prints one line per message that cannot fit even a screen-wide window or would cover
another; those still wrap mid-word. The story box beside a portrait already reaches
the screen's right edge, so its lines must stay within 168 pixels.

Messages the game's own code prints into fixed columns are not fitted: the status
screens place signs and values by cell, so their labels must stay within the pixels
the Japanese label takes (a stat label ends at cell 11; the level window leaves six
cells for the experience value). A value or a mark the code places in a column after
a translated name still lands on its cell: where the name's letters leave the text
between cells, the port moves it to the cell instead of padding with spaces (the
formation list's sizes start at cell 10). Item, weapon and Zi-data descriptions are printed by
code into windows the walker cannot see: keep weapon descriptions to one line of
176 pixels, the two-line Zi-data texts to 216 pixels per line and the narrow
variants to five lines of 104 pixels.

## Not modeled yet

Names, items and other strings the game prints outside scripts, messages reached only
through script jumps the layout walk does not follow, and the cedilla.
