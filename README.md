# re-zoids-saga

A FOSS native runtime for preserving the Zoids saga of Game Boy Advance games
(*Zoids Saga*, *Zoids Saga II / Legacy*, *Zoids Saga Fuzors*), written in Rust.

The games are reimplemented as native, cross-platform applications (Windows, macOS,
Linux and eventually Android) with a modern localization layer (Japanese, English,
Spanish), while preserving the original gameplay behavior.

This is **not** an emulator and **not** a ROM hack. The original ROM is never modified
or redistributed: it is used exclusively as a data source. You must provide your own
legally obtained game files; the launcher extracts what it needs on first load and
caches it in your user-data directory.

## Status

Early. The workspace and layer boundaries are in place: the launcher identifies a ROM
(cartridge header + SHA-1 against validated dumps), the script text codec decodes every
string table of Zoids Saga (names, items, dialogue, battle quotes, menus), and the
original 8×16 font, the text window (LZ77 tiles + palette) and the character
portraits are extracted and drawn in an SDL3 window, with text wrapped the way the
game does it. The opening chapter plays from the title into the town of Arcana: the castle
and its conversations, the throne-room cutscene, the gate, the Zoid choice in the hangar
(which forms the party), the eastern labyrinth with its chests and the Trinity Liger
event, the drive across the world map, and Arcana's arrival (the soldiers, Roman and the
room above the bar), its townsfolk, Dr. T and the teachers of deck commands, with the
original timing (see [docs/events.md](docs/events.md)). The town's item and armaments
shops buy and sell (see [docs/shop.md](docs/shop.md)); Dr. T's Zoid lab is not
implemented yet. The throne
room's two battle scenes play as in the original, shots included (see
[docs/battle.md](docs/battle.md)). The world map's enemies roam and chase the party;
meeting one opens the battle screen with its messages and menu, from which the party
can retreat (see [docs/combat.md](docs/combat.md)); the fighting itself is not
implemented yet. The
pause menu shows the party, each Zoid's status and parts, the stocked weapons, and
changes the parts on a Zoid's racks and who stands in the formation (see
[docs/menu.md](docs/menu.md)).
See [AGENTS.md](AGENTS.md) for the architecture, rules and project structure.

## Requirements

- Rust stable (see `rust-toolchain.toml`; the workspace pins the edition and MSRV)
- SDL3 development library
  - macOS: `brew install sdl3`
  - Debian/Ubuntu: `libsdl3-dev`
  - Windows: SDL3 development package on the library path

## Building

```bash
cargo build
```

Run the launcher without arguments to get its own screen, **Re:Zoids Saga**: choose your
ROM and, optionally, a translation (`.po`) in the system's file dialog, then Play (arrows
move, X chooses, Z clears the translation, Return plays). Options holds the window's size
(×1 to ×6), fullscreen, the filter (sharp keeps whole multiples of the screen, smooth
fills the window blending the pixels), the volume, and the buttons: Keyboard and Gamepad
open the keys or gamepad buttons of the pad's ten buttons, where you choose one and press
its new key or button (Esc cancels; one another button had swaps with it), or take the
defaults back. Any gamepad connected plays: its D-pad or left stick moves, its right face
button is A, the bottom one B, Start is START, Back is SELECT and the shoulders L and R,
unless chosen otherwise. The game plays with these options from then on, also when given a
ROM on the command line. The
launcher tells whether the ROM is a verified dump, speaks the chosen translation's
language (English without one), and remembers its choices in the user's settings folder (`launcher.cfg` under
`re-zoids-saga/launcher`, where SDL keeps a program's preferences: Application Support
on macOS, `~/.local/share` on Linux, AppData on Windows). Giving a ROM on the command
line skips the screen, as below. Esc on the launcher's screen, or while the game plays,
asks before closing (No is chosen at first; Esc again stays), and the game waits while it
asks.

```bash
cargo run -p launcher
```

Play from the publisher logo through the title, the name entry and the opening into the
first room (arrows move, holding Z while moving runs, X = A, Z = B, Return = START and opens the pause menu, Backspace = SELECT, A = L, S = R, Esc asks whether to quit; F10 turns a debugging mode on and off: the roaming enemies are intangible, so the player walks through them without battles, and the protagonist's attacks beat whatever they hit); the title's オプション opens the Zoid and character guides (see [docs/guide.md](docs/guide.md)); music and sound effects play through the default audio device; the pause menu's セーブ and the title's つづきから use a `.sav` file next to the ROM, in the original's format, so saves move between this port, emulators and the cartridge (`--save <file.sav>` picks another file, see [docs/formats/save.md](docs/formats/save.md)); unlike the original, the port keeps four save slots, which both ask for: slot 1 is that `.sav` and slot n the same name with `.n` before the extension (`game.2.sav`), each a save an emulator loads (`--slots <n>` sets how many, 1 to 9; `--slots 1` is the original's single save); `--translation <file.po>` shows a downloaded translation and `--export-template <file.pot>` writes the template translators start from (see [docs/translation.md](docs/translation.md));
the port's story ends with chapter 3: on reaching chapter 4's first map the game thanks the player, offers to save and goes back to the title (see [docs/events.md](docs/events.md));
`--room` skips straight to the first room; a `<table>_<index>` string id shows that
script in its box instead; `--dump frame.ppm` writes a frame instead of opening a window
(without a ROM, the launcher's screen):

```bash
cargo run -p launcher -- path/to/rom.gba
cargo run -p launcher -- path/to/rom.gba --room
cargo run -p launcher -- path/to/rom.gba dialogue_00043
```

Dump the game text (all tables, or one of `name`, `item`, `dialogue`, `battle`, `menu`),
or report the messages that overflow the dialogue box after wrapping:

```bash
cargo run -p extractor-cli -- dump-text path/to/rom.gba dialogue
cargo run -p extractor-cli -- check-layout path/to/rom.gba dialogue
```

The packages players download (a macOS disk image, a Windows zip and a Linux archive,
each running without SDL3 installed) are built by `tools/package/` and published by
GitHub Actions when a `v*` tag is pushed (see [docs/packaging.md](docs/packaging.md)):

```bash
tools/package/macos.sh v0.1.0
```

Before submitting a change:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Supported titles

| Title | Region | Game code | SHA-1 |
|---|---|---|---|
| Zoids Saga (Rev 1) | Japan | `ATZJ` | `70bb546a7d00126d452c1d2c1ccddafb2cb91b37` |
| Zoids Saga Fuzors | Japan | `BZFJ` | `f5269aba2e5f587aa2f851f35e96c1cbc5e1a78a` |
| Zoids Legacy (Saga II) | USA | — | pending |

## License

GPL-3.0-only. See [LICENSE](LICENSE). Game ROMs, BIOS images, saves and any content
extracted from them remain the property of their respective owners and are never part
of this repository.
