# The launcher

Source of knowledge: this project's own design. Implemented in `apps/launcher`.

## Its screen

Run without arguments, the launcher shows its own screen, **Re:Zoids Saga**: choose your
ROM and, optionally, a translation (`.po`) in the system's file dialog, then Play (arrows
move, X chooses, Z clears the translation, Return plays). It tells whether the ROM is a
verified dump and speaks the chosen translation's language (English without one).

Options holds:

- the window's size (×1 to ×6) and fullscreen;
- the filter: sharp keeps whole multiples of the screen, smooth fills the window
  blending the pixels;
- the volume;
- the buttons: Keyboard and Gamepad list the pad's ten buttons, where you choose one and
  press its new key or button (Esc cancels; a button that had it swaps with it), or take
  the defaults back.

The game plays with these options from then on, also when given a ROM on the command
line. The choices are remembered in `launcher.cfg` under `re-zoids-saga/launcher` in the
folder where SDL keeps a program's preferences (Application Support on macOS,
`~/.local/share` on Linux, AppData on Windows).

Esc on the launcher's screen, or while the game plays, asks before closing (No is chosen
at first; Esc again stays), and the game waits while it asks.

## Controls

| Pad | Keyboard | Gamepad |
|---|---|---|
| D-pad | Arrows | D-pad or left stick |
| A | X | Right face button |
| B | Z (held while moving runs) | Bottom face button |
| START (opens the pause menu) | Return | Start |
| SELECT | Backspace | Back |
| L, R | A, S | Shoulders |

F10 turns a debugging mode on and off: the roaming enemies are intangible, so the player
walks through them without battles, and the protagonist's attacks beat whatever they hit.

## Command line

```bash
cargo run -p launcher -- path/to/rom.gba
```

A ROM on the command line skips the launcher's screen and plays from the publisher logo.

| Option | Effect |
|---|---|
| `--save <file.sav>` | The save file; by default the ROM's name with `.sav`, next to it (see [formats/save.md](formats/save.md)) |
| `--slots <n>` | How many save slots, 1 to 9 (4 by default; 1 is the original's single save) |
| `--translation <file.po>` | Shows a translation (see [translation.md](translation.md)) |
| `--export-template <file.pot>` | Writes the template translators start from |
| `--room` | Skips straight to the first room |
| `<table>_<index>` | Shows that script in its box instead, e.g. `dialogue_00043` |
| `--dump <frame.ppm>` | Writes a frame instead of opening a window (without a ROM, the launcher's screen) |

Saves use the original's format, so they move between this port, emulators and the
cartridge; slot 1 is the `.sav` and slot n the same name with `.n` before the extension
(`game.2.sav`).

## The extractor

`extractor-cli` dumps the game text (all tables, or one of `name`, `item`, `dialogue`,
`battle`, `menu`), or reports the messages that overflow the dialogue box after wrapping:

```bash
cargo run -p extractor-cli -- dump-text path/to/rom.gba dialogue
cargo run -p extractor-cli -- check-layout path/to/rom.gba dialogue
```
