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
original timing (see [docs/events.md](docs/events.md)); the town's shops are not
implemented yet. The throne
room's two battle scenes play as in the original, shots included (see
[docs/battle.md](docs/battle.md)); the battle system itself is not implemented yet. The
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

Play from the publisher logo through the title, the name entry and the opening into the
first room (arrows move, holding Z while moving runs, X = A, Z = B, Return = START and opens the pause menu, Backspace = SELECT, A = L, S = R, Esc quits); the title's オプション opens the Zoid and character guides (see [docs/guide.md](docs/guide.md)); music and sound effects play through the default audio device; the pause menu's セーブ and the title's つづきから use a `.sav` file next to the ROM, in the original's format, so saves move between this port, emulators and the cartridge (`--save <file.sav>` picks another file, see [docs/formats/save.md](docs/formats/save.md)); `--translation <file.po>` shows a downloaded translation and `--export-template <file.pot>` writes the template translators start from (see [docs/translation.md](docs/translation.md));
`--room` skips straight to the first room; a `<table>_<index>` string id shows that
script in its box instead; `--dump frame.ppm` writes a frame instead of opening a window:

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
