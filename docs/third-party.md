# Third-party references

Components of `gba-runtime` whose behavior was rewritten in Rust from a GPL-compatible
reference, as required by [AGENTS.md](../AGENTS.md#hardware-runtime-references).

| Component | Upstream | File(s) | Commit | License |
|---|---|---|---|---|
| — | — | — | — | — |

Behavioral oracle (no code derived): gbarecomp, PolyForm Noncommercial 1.0.0.

The sound engine (`crates/gba-runtime/src/apu/`) was written from the public
description of the GBA's common sound driver's data formats and from the driver
settings and buffers read out of the game's own RAM; see [sound.md](sound.md). No
emulator or decompilation code was consulted for it.
