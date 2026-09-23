# Architecture notes

Decisions and diagrams that go beyond the summary in [AGENTS.md](../AGENTS.md).

## Crate dependency graph

Dependencies point strictly downward. A crate importing from a layer above it is an
architecture violation.

```
apps/launcher ──▶ games/{saga,legacy,fuzors} ──▶ game-core ──▶ gba-runtime ──▶ platform
apps/extractor-cli ──▶ extraction ──▶ formats           │            └────▶ localization
                                                        └──▶ extraction ──▶ formats
platform-sdl3 ──▶ platform
```

`platform-sdl3` is wired in only by the composition roots under `apps/`.

## Decisions

### D-001: Reimplementation in Rust instead of decompilation or static recompilation

Both a C decompilation (pret-style, agbcc) and a static recompilation (translating
the ROM's ARM/Thumb code to native code around a hardware runtime) were evaluated as
prototypes before this workspace existed. Both produce code derived from the ROM and
neither yields readable, localizable game logic. The project instead reimplements the
game logic in Rust from behavioral analysis of the original, and uses the ROM only as a
data source at runtime.

### D-002: Hardware runtime references

gbarecomp was evaluated as a hardware-runtime reference but is licensed PolyForm
Noncommercial 1.0.0, incompatible with GPL-3.0. It is kept as a behavioral oracle only.
NanoBoyAdvance (GPL-3.0) and mGBA (MPL-2.0) are the implementation references when
hardware documentation is not enough; every rewrite is logged in
[third-party.md](third-party.md).

## Facts about the original games that shape the runtime

These were established by analysis of the original ROMs and are recorded here because
the code cannot express them.

- Zoids Saga (ATZJ) was built with Nintendo's `agbcc` toolchain, uses the m4a
  (MusicPlayer2000) sound driver and saves to 32 KiB SRAM.
- The game runs a small cooperative multitasking kernel: an ARM routine copied to IWRAM
  saves and restores full register contexts (r0–r12, sp, lr, SPSR) in a task list and
  switches stacks between tasks. The interrupt dispatcher lives in the same IWRAM copy.
- The VBlank callback and part of the sound mixer also execute from RAM (EWRAM and
  IWRAM respectively), copied from ROM at startup.
