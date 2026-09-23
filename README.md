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
original 8×16 font and text window (LZ77 tiles + palette) are extracted and drawn in
an SDL3 window. No game runs yet.
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

Identify a ROM and show a script string rendered with the game's font (any
`<table>_<index>` id; `--dump frame.ppm` writes the frame instead of opening a window):

```bash
cargo run -p launcher -- path/to/rom.gba dialogue_00003
```

Dump the game text (all tables, or one of `name`, `item`, `dialogue`, `battle`, `menu`):

```bash
cargo run -p extractor-cli -- dump-text path/to/rom.gba dialogue
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
