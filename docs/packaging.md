# Packaged builds

The builds players download run without a development environment: SDL3 is
compiled from source and linked into the program, and the program carries
everything else it needs (the Latin font is embedded at build time; the game's
data comes from the player's ROM). Nothing derived from a ROM is ever packaged.

Source of knowledge: this project's own tooling; SDL3's static build comes from
the `sdl3-sys` crate's `build-from-source-static` feature.

## The `packaged` feature

`cargo build --release -p launcher --features packaged` turns on:

- `platform-sdl3/static-sdl`: SDL3 built with CMake from the `sdl3-src` crate and
  linked statically, so the program needs no SDL3 installed. Everyday builds leave
  it off and link the system's SDL3, which is faster to build.
- On Windows, the GUI subsystem: no console window opens behind the game's.

Building it needs CMake and a C compiler; on Linux, also the development headers
of the audio and windowing systems SDL3 supports (see the workflow below for the
list). SDL3 loads those systems' libraries at run time, so the program runs on
whichever the player's desktop has.

## Packages

| System | Script | Package |
|---|---|---|
| macOS 11+ | `tools/package/macos.sh [version]` | `re-zoids-saga-<version>-macos.dmg`: `Re Zoids Saga.app` (universal: Apple Silicon and Intel, ad-hoc signed), `README.txt`, `LICENSE.txt` and a link to Applications |
| Windows | `tools/package/windows.ps1 [version]` | `re-zoids-saga-<version>-windows-x86_64.zip`: `re-zoids-saga.exe`, `README.txt`, `LICENSE.txt` |
| Linux (glibc 2.35+) | `tools/package/linux.sh [version]` | `re-zoids-saga-<version>-linux-x86_64.tar.gz`: `re-zoids-saga`, `README.txt`, `LICENSE.txt` |

Each script runs from the repository's root, builds with `--locked`, and writes to
`dist/` (gitignored). The version defaults to `git describe`. `README.txt` is
[tools/package/README-player.txt](../tools/package/README-player.txt), the players'
instructions.

The programs are not signed by an identified developer: macOS asks to confirm the
first opening (right-click, Open), and Windows SmartScreen asks to run anyway. The
players' instructions explain both. Removing the warnings needs an Apple Developer
ID with notarization and a Windows code-signing certificate.

## Releases

[.github/workflows/release.yml](../.github/workflows/release.yml) runs the three
scripts on GitHub's runners (macOS 14, Windows, Ubuntu 22.04). Pushing a tag `v*`
publishes the packages as a GitHub release, a prerelease when the tag has a
suffix (`v0.1.0-demo`); running the workflow by hand only keeps them as the run's
artifacts.
