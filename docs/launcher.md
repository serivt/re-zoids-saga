# The launcher

Source of knowledge: this project's own design. Implemented in `apps/launcher`.

## Its screen

Run without arguments, the launcher shows its own screen, **Re:Zoids Saga**: choose your
ROM in the system's file dialog and, optionally, a translation, then Play (arrows move, X
chooses, Z clears the translation, Return plays; a touch or a click chooses the line
under it).

The Translation line opens a screen of its own: *From a file...* opens a PO file in the
system's dialog, and below it every language the translations' repository offers
([re-zoids-saga-translations](https://github.com/serivt/re-zoids-saga-translations),
listed in its `po/languages.json`) downloads that language's PO file, checks that it
reads, keeps it in the launcher's settings folder as `<code>.po` and chooses it; a
language already kept says *downloaded*, and choosing it again downloads the latest
corrections. This is the only time the launcher goes online, and only when asked: it
fetches the list when the screen opens and a file when a language is chosen, over HTTPS
from `raw.githubusercontent.com`, and sends nothing else. Without a connection the
screen says so and a file can still be opened. Only the verified dump,
Zoids Saga (Japan, Rev 1), plays: for the first release (Rev 0), another dump of the
game or another game it says so and Play stays off, and the command line refuses them
the same way. It speaks the chosen translation's language (English without one). The
port's version shows in the top right corner.

Game mode chooses how the game plays, on a screen of its own:

- **Classic** (the default) plays exactly as the original;
- **Enhanced** adds conveniences the original never had, each turned on or off on the
  same screen (they show unavailable in the classic mode, which keeps the choices for
  when the enhanced mode comes back):
  - *Battle animations* off skips the battles' attack scenes: an enemy's attack goes
    from the fade out straight to its outcome on the battle's screen, and the party's
    scene stops once the player has aimed (see [combat.md](combat.md), Without the
    attack scenes).
  - *Damage numbers* on shows, besides each hit's message, the damage every unit
    takes as a number under it for a moment (see [combat.md](combat.md), Damage
    numbers).
  - *Auto text* on lets the text boxes go on by themselves once their text has been on
    screen long enough to read, with an AUTO mark on them; SELECT turns it on or off
    while a text box shows (see [formats/script-text.md](formats/script-text.md)).

The mode is set before the game starts (`Game::set_play_mode`, see
`crates/game-core/src/play_mode.rs`) and does not change while it plays; saves are the
original's in either mode. In the enhanced mode the pause menu's コンフィグ lists the
enhancements too, beside the original's message speed (see [menu.md](menu.md)); what the
player changes there holds at once and the launcher remembers it.

Options holds:

- the window's size (×1 to ×6) and fullscreen;
- the filter: sharp keeps whole multiples of the screen, smooth fills the window
  blending the pixels;
- the volume;
- the buttons: Keyboard and Gamepad list the pad's ten buttons, where you choose one and
  press its new key or button (Esc cancels; a button that had it swaps with it), or take
  the defaults back.

About shows the version and the license, and the project's pages: the port's repository
and the translations' ([re-zoids-saga-translations](https://github.com/serivt/re-zoids-saga-translations)),
which X opens in the web browser.

The game plays with these options and this mode from then on, also when given a ROM on
the command line. The choices are remembered in `launcher.cfg` under `re-zoids-saga/launcher` in the
folder where SDL keeps a program's preferences (Application Support on macOS,
`~/.local/share` on Linux, AppData on Windows).

Esc on the launcher's screen, or while the game plays, asks before closing (No is chosen
at first; Esc again stays), and the game waits while it asks.

On Android the launcher is the same, answering taps, with the options of the on-screen
pad and the saves' export and import in place of the window's and the keyboard's; see
[android.md](android.md).

## Controls

| Pad | Keyboard | Gamepad |
|---|---|---|
| D-pad | Arrows | D-pad or left stick |
| A | X | Right face button |
| B | Z (held while moving runs) | Bottom face button |
| START (opens the pause menu) | Return | Start |
| SELECT | Backspace | Back |
| L, R | A, S | Shoulders |

In a build with the `debug-mode` feature (`cargo run -p launcher --features debug-mode`),
F10 turns a debugging mode on and off: the roaming enemies are intangible, so the player
walks through them without battles, and the protagonist's attacks beat whatever they hit.
Other builds, the packages players download among them, ignore F10.

## Command line

```bash
cargo run -p launcher -- path/to/rom.gba
```

A ROM on the command line skips the launcher's screen and plays from the publisher logo.

| Option | Effect |
|---|---|
| `--version` | Prints the port's version and quits |
| `--save <file.sav>` | The save file; by default the ROM's name with `.sav`, next to it (see [formats/save.md](formats/save.md)) |
| `--slots <n>` | How many save slots, 1 to 9 (4 by default; 1 is the original's single save) |
| `--translation <file.po>` | Shows a translation (see [translation.md](translation.md)) |
| `--export-template <file.pot>` | Writes the template translators start from |
| `--room` | Skips straight to the first room |
| `--touch` | Shows the on-screen pad for touch screens, as Android always does (with `SDL_MOUSE_TOUCH_EVENTS=1` the mouse plays a finger) |
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
