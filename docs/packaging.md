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

The Windows script also links the C runtime in (`-C target-feature=+crt-static`, which
the CMake build of SDL3 follows), so the program needs no Visual C++ Redistributable.

Building it needs CMake and a C compiler; on Linux, also the development headers
of the audio and windowing systems SDL3 supports (see the workflow below for the
list). SDL3 loads those systems' libraries at run time, so the program runs on
whichever the player's desktop has.

## Packages

| System | Script | Package |
|---|---|---|
| macOS 11+ | `tools/package/macos.sh [version]` | `re-zoids-saga-<version>-macos.dmg`: `Re Zoids Saga.app` (universal: Apple Silicon and Intel, ad-hoc signed), `README.txt`, `LICENSE.txt` and a link to Applications |
| Windows | `tools/package/windows.ps1 [version]` | `re-zoids-saga-<version>-windows-x86_64.zip`: `re-zoids-saga.exe`, `README.txt`, `LICENSE.txt` |
| Linux (glibc 2.34+) | `tools/package/linux.sh [version]` | `re-zoids-saga-<version>-linux-x86_64.tar.gz`: `re-zoids-saga`, `README.txt`, `LICENSE.txt` |
| Android 5.0+ | `tools/package/android.sh [version] [debug\|release]` | `re-zoids-saga-<version>-android.apk`, for `arm64-v8a` and `x86_64` (see [android.md](android.md)) |
| Web browsers | `tools/package/web.sh [version]` | `dist/web/`, the whole site for any static web server: the page, the game's WebAssembly and its JavaScript glue, the manifest and the service worker that make it installable and playable offline (see [web.md](web.md)); and the same folder as `re-zoids-saga-<version>-web.zip` |

Each script runs from the repository's root, builds with `--locked`, and writes to
`dist/` (gitignored). The version defaults to `git describe`. `README.txt` is
[tools/package/README-player.txt](../tools/package/README-player.txt), the players'
instructions.

The programs are not signed by an identified developer: macOS asks to confirm the
first opening (right-click, Open), and Windows SmartScreen asks to run anyway. The
players' instructions explain both. Removing the warnings needs an Apple Developer
ID with notarization and a Windows code-signing certificate. Unsigned programs with
little reputation can also be flagged by antivirus heuristics (Microsoft Defender's
`...!ml` detections): such a false positive is reported to Microsoft through its file
submission page, and the version information (see below) makes it less likely.

## The icon

The project's icon is `assets/icons/re-zoids-saga.png` (original art made for this
project, committed like the Latin font). `python3 tools/package/icons.py` builds the
rest from it: `re-zoids-saga.ico` (16 to 256 pixels), `re-zoids-saga.icns` (16 to 512
pixels, with macOS's `iconutil`; the master is not enlarged) and
`re-zoids-saga-128.rgba`, the raw pixels every window of the launcher gets at run time
(`Sdl3Display::set_icon`). The Windows program embeds the `.ico` together with its
version information (product, description, version, license and original file name,
the file's Details tab) through `apps/launcher/build.rs`, which does nothing for other
targets; the macOS app carries the `.icns` in its bundle.

## Releases

[.github/workflows/release.yml](../.github/workflows/release.yml) runs the five scripts
on GitHub's runners (macOS 14, Windows, Ubuntu 22.04, and Ubuntu 24.04 for Android and
the web). The web job installs the WebAssembly target and the wasm-bindgen-cli of the
`wasm-bindgen` in `Cargo.lock`; its zip has no cloud project, so its page offers no
cloud saves. A release also publishes the web version's site on Cloudflare, with the
cloud project the repository's variables name (see [web.md](web.md), Published). The
Android job signs the app with the keystore in the repository's secrets (a release fails
without it; a run by hand then uses the debug key) and starts it on an emulator with
`tools/package/android-smoke.sh`, without a ROM, before keeping it. Publishing a GitHub
release, with its tag (`v0.1.0`), title, notes and prerelease mark written by hand,
builds the packages from the release's tag and attaches them to it; the workflow runs as
it is at that tag's commit. Running the workflow by hand only keeps the packages as the
run's artifacts. Every package goes with a `<package>.sha256` file, its SHA-256 checksum
in the format `shasum -a 256 -c` and `sha256sum -c` check, among the artifacts and the
release's files alike.
