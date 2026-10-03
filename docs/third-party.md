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

## Algorithms

Written from their public descriptions, with no code consulted:

| Component | Algorithm | Where |
|---|---|---|
| The launcher's upscaler | Scale2x and Scale3x, by Andrea Mazzoleni (AdvanceMAME) | `crates/screen-filters/src/upscale.rs` |

## Bundled as is

| Component | Upstream | Where | Version | License |
|---|---|---|---|---|
| SDL's Android Java layer (`org.libsdl.app`) and Gradle wrapper | [SDL](https://github.com/libsdl-org/SDL) `android-project` | `apps/android/project/app/src/main/java/org/libsdl/app/`, `apps/android/project/gradlew*`, `apps/android/project/gradle/wrapper/` | 3.4.16 (the `sdl3-src` crate the build uses) | zlib (`LICENSE.txt` beside the sources); the Gradle wrapper, Apache-2.0 |

The Java files are unchanged copies; the app's own activity
(`io.github.serivt.rezoidssaga.MainActivity`) only extends SDL's. They must stay at the
version of the SDL the native build compiles, since the two talk through JNI.
