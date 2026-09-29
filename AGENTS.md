# AGENTS.md — re-zoids-saga

Guidance for AI agents and human contributors working on this repository.

## Project Overview

**re-zoids-saga** is a FOSS native runtime for preserving the Zoids saga of Game Boy Advance games. The goal is to reimplement the games as native, cross-platform applications (Windows, macOS, Linux, and Android) with a modern localization layer (Japanese, English, Spanish), while preserving the original gameplay behavior.

This is **not** an emulator and **not** a ROM hack. The original ROM is never modified or redistributed: it is used exclusively as a **data source**. Users must provide their own legally obtained game files.

The user-facing product is a **launcher**: the user opens the application, loads their ROM, and plays. Extraction of game data from the ROM happens transparently at load time (with caching), never as a manual pre-processing step the user must run.

## Legal and Licensing Rules (Non-Negotiable)

1. **All code in this repository is written from scratch.** Never copy code from other decompilation projects, ports, emulators, or ROM hacks — not even snippets, not even "adapted" versions. The only exception is the hardware runtime, which may be *rewritten in Rust* from the GPL-compatible references listed in [Hardware Runtime References](#hardware-runtime-references), under the conditions stated there.
2. Studying the ROM, reverse-engineering its formats, and reusing its **data/assets at runtime** (extracted locally by the user) is allowed. Shipping copyrighted assets in this repository is not.
3. Referencing *architectural patterns* from other projects (e.g., static recompilation approaches) is fine conceptually; their implementation must never be reproduced unless the project is an approved reference (see below).
4. Never commit ROMs, ROM fragments, extracted assets, or any copyrighted game content. Add extraction output directories to `.gitignore`.

## Hardware Runtime References

The `gba-runtime` crate (PPU/graphics, APU and the m4a sound engine, DMA, timers, cartridge save chips and hardware I/O registers) is written from public hardware documentation and validated against existing emulators. Reference projects live as git-excluded checkouts under `ref/`, pinned to a commit.

**Primary sources (always allowed):** GBATEK, Tonc, CowBite and the pret decompilations *as documentation of behavior* (the pret repositories carry no license, so their code is never reused).

**Behavioral oracle only — never a source of code:** [gbarecomp](https://github.com/mstan/gbarecomp) is licensed **PolyForm Noncommercial 1.0.0**, which is neither FOSS nor GPL-compatible. It may be run to compare framebuffers, audio and timing against our runtime, and its hardware notes may be consulted for facts. Its source code must not be read to write ours, translated, adapted or paraphrased. A recompilation experiment built with it exists under `ref/ZoidsSagaRecomp` for that purpose only.

**GPL-compatible implementation references (rewrite allowed):** [NanoBoyAdvance](https://github.com/nba-emu/NanoBoyAdvance) (GPL-3.0) and [mGBA](https://github.com/mgba-emu/mgba) (MPL-2.0). Their logic may be rewritten in Rust when the hardware documentation is insufficient, under these conditions:

- It is a rewrite of the *behavior*, in idiomatic Rust following this file's code style, not a line-by-line transliteration.
- Each rewritten component is recorded in `docs/third-party.md` with the upstream project, file(s), commit and license, and the commit message names the source.
- No file is copied verbatim; MPL/GPL notices therefore never need to travel, but attribution is still kept.
- The specific game facts this project has established (e.g., Zoids Saga uses 32 KiB SRAM via `SRAM_F_V102`, the m4a `SoundMainRAM` mixer and a cooperative task kernel) decide *which* hardware features the runtime needs first; the runtime is not a general-purpose emulator.

## Architecture

The system is layered with strict downward-only dependencies:

```
┌─────────────────────────────────────┐
│ Game Data (extracted from ROM)      │  maps, sprites, audio, text, metadata
├─────────────────────────────────────┤
│ Localization Layer                  │  JA / EN / ES as external data
├─────────────────────────────────────┤
│ Game Logic (reimplemented)          │  battle, world, menus, scripts, saves
├─────────────────────────────────────┤
│ GBA Runtime                         │  software PPU, APU, memory, timing
├─────────────────────────────────────┤
│ Platform Layer (SDL3)               │  window, input, audio out, filesystem
└─────────────────────────────────────┘
```

Core architectural rules:

- **Data and code are fully separated.** Game content (dialogue, Zoid names, attacks, items, stats, maps) lives in data files, never hardcoded in source.
- **The platform layer is an abstraction boundary.** Game logic and the GBA runtime must never call platform APIs directly; everything goes through traits/interfaces. Android (or any future platform) is a platform implementation, not a design constraint that leaks upward.
- **Extraction is a library, invoked by the launcher at ROM load time** (`ROM → extraction → intermediate format → game database`). The launcher identifies the title (ROM header/hash), runs extraction transparently, and caches the resulting game database in the OS user-data directory keyed by ROM hash, so later launches skip straight to the game. The runtime consumes the intermediate format only; it never parses the ROM directly.
- **Shared infrastructure first.** The GBA games likely share engine code and formats. Common functionality (text systems, tile rendering, compression, script interpretation) belongs in shared crates, with per-game code kept as thin as possible.

## Localization System

Localization is a first-class subsystem, not an afterthought:

- Translations live outside this repository, in
  [re-zoids-saga-translations](https://github.com/serivt/re-zoids-saga-translations),
  as gettext PO files. Every message is keyed by a stable ID (script table, string
  index and the message's offset in the string, e.g. `dialogue/40/0x2e`); see
  [docs/translation.md](docs/translation.md).
- The translation template is generated from each translator's own ROM by the
  launcher (`--export-template`); the original Japanese text is copyrighted ROM
  content and is never committed or published, neither as a template nor inside a PO
  file (published PO files repeat the key as their `msgid`).
- Players download the PO file of their language and hand it to the launcher
  (`--translation`); messages it covers replace the ROM's text at run time, the rest
  stays Japanese.
- Internal text encoding is **UTF-8/Unicode** everywhere. Original ROM text encodings are converted at extraction time.
- Text layout (line wrapping, box fitting) is computed dynamically by the engine per language and font — never pre-baked into the strings.
- Fonts: the ROM's font for Japanese and this project's own Latin pixel font (`assets/fonts/latin/`, text-art source parsed at build time) for translations; TTF/OTF assets with fallback support are future work.
- Every localizable category (dialogue, menus, items, Zoids, attacks, characters, locations, tutorials) uses the same pipeline.
- Validation of a translation against the windows' layout constraints is future work.

## Technology Stack

- **Language:** Rust (stable toolchain, latest stable edition).
- **Platform layer:** SDL3 (window, input, audio, gamepad).
- **Rendering:** software rendering of the GBA PPU output to a native framebuffer, scaled to modern resolutions. No 3D renderer is needed.
- **Data formats:** `serde`-friendly formats (JSON/RON/TOML) for authored data; compact binary for extracted game databases where size matters.
- **Errors:** `thiserror` for library error types, `anyhow` only at binary/CLI boundaries.
- **Workspace:** Cargo workspace with focused crates (e.g., `runtime`, `platform`, `localization`, `extractor`, per-game crates).

Do not introduce new dependencies casually. The project is licensed **GPL-3.0-only**; every dependency must be FOSS-licensed and GPL-compatible (permissive licenses like MIT/Apache-2.0/BSD are fine), and justified.

## Project Structure

The repository is a Cargo workspace. Each crate maps to one architectural layer or one tool; crates never reach across layers.

```
re-zoids-saga/
├── AGENTS.md                    # single source of truth for contributors and agents
├── CLAUDE.md                    # pointer to AGENTS.md
├── README.md
├── LICENSE
├── Cargo.toml                   # workspace definition
├── rust-toolchain.toml
├── .gitignore                   # must exclude data/ and any ROM or extracted content
├── .github/workflows/           # CI: packaged builds and releases (docs/packaging.md)
│
├── crates/
│   ├── platform/                # platform abstraction traits only (window, input,
│   │                            # audio output, filesystem, timing) — zero SDL code
│   ├── platform-sdl3/           # SDL3 implementation of the platform traits
│   ├── gba-runtime/             # software GBA runtime
│   │   └── src/
│   │       ├── ppu/             # tile/sprite/background rendering to framebuffer
│   │       ├── apu/             # audio synthesis
│   │       ├── memory/          # memory map, DMA
│   │       └── timing/          # frame timing, interrupts
│   ├── formats/                 # reverse-engineered ROM format codecs: compression,
│   │                            # tilesets, palettes, text encoding, script bytecode
│   ├── extraction/              # library: ROM identification (header/hash), extraction
│   │                            # into the intermediate format, game-database caching
│   ├── localization/            # string database, font loading, dynamic text layout,
│   │                            # completeness validation
│   ├── game-core/               # game systems shared across the Zoids titles
│   │                            # (menus, battle framework, world, script interpreter,
│   │                            # save system)
│   └── games/
│       ├── saga/                # Zoids Saga-specific logic and data bindings
│       ├── legacy/              # Zoids Saga II / Legacy-specific logic and data bindings
│       └── fuzors/              # Zoids Saga III / Fuzors-specific logic and data bindings
│
├── apps/
│   ├── launcher/                # main binary (and library): ROM picker, title detection,
│   │                            # transparent extraction with progress UI, then wires the
│   │                            # matching game crate + platform-sdl3; settings, language
│   │                            # selection
│   ├── android/                 # the Android app: libmain.so runs the launcher, and its
│   │                            # Gradle project (docs/android.md)
│   └── extractor-cli/           # thin CLI over crates/extraction for development and
│                                # debugging (inspect/dump extracted data)
│
├── assets/                      # original assets created for this project (committable)
│   └── fonts/
│       ├── latin/
│       ├── japanese/
│       └── symbols/
│
├── data/                        # gitignored — local development workspace for extracted
│                                # game databases; at runtime the launcher caches extracted
│                                # data in the OS user-data directory instead
│
├── docs/
│   ├── architecture.md          # decisions and diagrams beyond this file's summary
│   └── formats/                 # reverse-engineering notes per ROM format
│
└── tools/                       # development scripts (CI helpers, validators)
    └── package/                 # scripts that build the packages players download
```

Placement rules:

- **`crates/platform` contains only traits and shared types.** Any code that touches SDL (or a future Android backend) lives in its own `platform-*` crate.
- **`crates/formats` is pure codecs**: it decodes bytes into typed structures and knows nothing about gameplay. Both the extractor and the runtime depend on it; it depends on neither.
- **`crates/games/*` crates stay thin.** Anything used by more than one title moves down into `game-core`. A game crate holds only title-specific behavior, data schemas, and wiring.
- **`apps/*` are composition roots.** Binaries wire crates together and hold no game or engine logic of their own.
- **`crates/extraction` owns the ROM-to-database pipeline as a library.** The launcher calls it at load time; `extractor-cli` is only a development wrapper around it. Extraction must be fast enough for a first-launch experience and idempotent, so cached databases can be invalidated purely by ROM hash and extractor version.
- **No translation data is committed.** PO files live in the translations repository and on players' machines, templates only on translators' machines; the repository holds only the exporter and the loader. Original Japanese text is copyrighted ROM content: it is produced by extraction on the user's machine and is never committed.
- **`data/` is always gitignored.** Nothing derived from a ROM enters version control — including test fixtures, which must be synthetic. End users never see this directory; their extracted data lives in the OS user-data directory managed by the launcher.
- **New code follows [docs/extensibility.md](docs/extensibility.md):** game logic reads data through a provider by stable identifier, behavior flows through events and hooks, and the engine's own features use the same contracts a mod would.
- Dependency direction is strictly downward: `apps → games → game-core → (gba-runtime, localization, extraction, formats) → platform`. A crate importing from a layer above it is an architecture violation.

## Code Style

### Clean Code Rules

- Follow clean code principles: small functions with a single responsibility, meaningful names, no magic numbers (use named constants), early returns over deep nesting, and no dead code.
- Prefer expressing intent through **names and structure**, not through comments.
- Keep modules cohesive and small. If a file grows past a few hundred lines, look for a missing abstraction.
- No premature abstraction either: introduce a trait or generic only when a second concrete use exists or is imminent.
- Immutability by default; mutation is localized and explicit.
- No `unwrap()`/`expect()` in library code paths; propagate errors with `?` and typed errors. `expect()` with a clear message is acceptable only in tests and at process entry points for truly unrecoverable states.
- All warnings are errors in CI (`clippy` with `-D warnings`, `rustfmt` enforced).

### Comments Policy

- **No inline comments.** Code must be self-explanatory through naming and structure. If a line needs an inline comment, rewrite the line.
- Only **high-level documentation comments** are allowed: Rust doc comments (`///`, `//!`) on modules, types, traits, and public functions, explaining *what* the item is for and any invariants or constraints the signature cannot express.
- Never write comments that narrate what the next line does, justify a change, or reference the development process.
- Exception: a comment documenting a **hardware constraint or ROM format fact** that the code cannot express (e.g., a GBA register quirk or an offset layout) is allowed, placed as a doc comment on the item that embodies it.

### Language

- **All code, identifiers, doc comments, commit messages, and repository documentation are in English.** Spanish and Japanese appear only inside localization data files as content.

## Testing

- Every non-trivial module has unit tests. Reverse-engineered format parsers must have tests with synthetic fixtures (never real ROM data committed to the repo).
- The GBA runtime components (PPU, APU, memory) are tested against known-good expected outputs from synthetic inputs.
- The translation loader and template exporter are covered by tests with synthetic data.
- Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` before considering any change complete.

## Git Conventions

- Conventional commit format (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`), imperative mood, in English.
- Small, focused commits. A commit that mixes a refactor with a behavior change is two commits.
- Never commit generated extraction output, ROMs, or user game data.

## Agent Working Rules

- When implementing reverse-engineered behavior, document the source of knowledge (own analysis, public format documentation) in the PR/commit description — never in inline code comments.
- When uncertain whether something counts as "copying code" versus "reusing a concept", treat it as copying and reimplement from a behavioral description instead.
- Prefer completing a vertical slice (one feature working end-to-end through all layers) over broad horizontal scaffolding.
- Keep `AGENTS.md` updated when architectural decisions change; this file is the single source of truth for contributors and agents.
