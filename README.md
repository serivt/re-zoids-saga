# re-zoids-saga

A FOSS native runtime for preserving the Zoids saga of Game Boy Advance games
(*Zoids Saga*, *Zoids Saga II / Legacy*, *Zoids Saga Fuzors*), written in Rust.

The games are reimplemented as native, cross-platform applications (Windows, macOS,
Linux and eventually Android) with a modern localization layer (Japanese, English,
Spanish), while preserving the original gameplay behavior.

This is **not** an emulator and **not** a ROM hack. The original ROM is never modified
or redistributed: it is used exclusively as a data source. You must provide your own
legally obtained game files.

## Status

A playable demo of *Zoids Saga*: from the title through the first three chapters, with
the towns, their shops and Zoid labs, the pause menu, saves compatible with the original
and translations. Battles open and can be retreated from, but the fighting itself is not
implemented yet. The story stops at the start of chapter 4, where the game thanks the
player and offers to save.

## Play

1. Download the package for your system from the
   [Releases](https://github.com/serivt/re-zoids-saga/releases) page: a disk image for
   macOS, a zip for Windows, an archive for Linux (x86-64, glibc 2.34 or later). Nothing
   else needs installing.
2. Start it:
   - **macOS:** drag *Re Zoids Saga* to Applications and open it. The app is not signed,
     so the first time right-click it and choose Open (on recent versions, System
     Settings › Privacy & Security › Open Anyway).
   - **Windows:** unzip the folder anywhere and run `re-zoids-saga.exe`. If SmartScreen
     warns, choose More info › Run anyway.
   - **Linux:** unpack the folder anywhere and run `./re-zoids-saga`.
3. Choose your Zoids Saga ROM (Japan, Rev 1) and, optionally, a translation (`.po`),
   then Play. Saves are kept next to the ROM, in the original's `.sav` format.

Default keys: arrows move, X = A, Z = B, Return = START, Backspace = SELECT, A = L,
S = R, Esc asks whether to quit. Gamepads work too; keys and buttons can be changed in
Options.

## Build and run

Requirements: Rust stable (see `rust-toolchain.toml`) and the SDL3 development library
(`brew install sdl3` on macOS, `libsdl3-dev` on Debian/Ubuntu, SDL3's development
package on the library path on Windows).

```bash
cargo run -p launcher                       # the launcher's screen
cargo run -p launcher -- path/to/rom.gba    # straight into the game
```

Before submitting a change:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The launcher's options, controls and command line are in
[docs/launcher.md](docs/launcher.md), the packages in [docs/packaging.md](docs/packaging.md),
and the architecture, rules and project structure in [AGENTS.md](AGENTS.md).

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
