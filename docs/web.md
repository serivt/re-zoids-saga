# The web version

Source of knowledge: this project's own design. The browser's side is the public
WebAssembly and Web APIs (canvas, Web Audio, Gamepad, IndexedDB, the Origin Private File
System); no other emulator's or port's code was consulted.

The aim is the same game in a browser, with the same rule as everywhere else: the player
provides the ROM. The game's code is compiled to WebAssembly (`wasm32-unknown-unknown`),
and a backend for the browser takes the place of `platform-sdl3`, as SDL3 does on the
desktop and on Android. The plan, in phases:

1. **The core in WebAssembly** (done, below).
2. **The browser's backend and page** (done, below): the picture on a canvas, the sound
   through Web Audio, the keyboard and gamepads, the saves in the browser's storage, and
   an HTML page to choose the ROM, a translation and the options. Still to come in the
   page: the filters that need more than the browser's scaling (the LCD grid, the scan
   lines) through WebGL, and the on-screen pad for touch screens.
3. **Kept in the browser:**
   - the ROM, cached in the Origin Private File System so it is chosen once, and never
     sent anywhere;
   - the saves, the autosave and the achievements, in IndexedDB, exported and imported
     as `.sav` as the launcher does;
   - the page installable and playable offline, a progressive web app.
4. **Saves in the cloud,** optional, with an account: the `.sav` files and the
   achievements kept by the player's account, never the ROM.

## The core in WebAssembly

The crates the game is made of build for `wasm32-unknown-unknown` as they are:
`platform`, `formats`, `extraction`, `localization`, `gba-runtime`, `screen-filters` and
`game-core`. Only `platform-sdl3` and the launcher are tied to SDL and the file system.

A browser starts no threads (without headers a page must be served with), and the core
started one: the enhanced mode's autosave, whose worker writes the queued games. Where
the platform starts none (`target_family = "wasm"`), the autosave writes each game as it
is queued instead, and reports it as the worker does (`Autosaver::inline`, see
[formats/save.md](formats/save.md), The autosave); an image is a few kilobytes. Nothing
else in the core reads the clock or starts a thread: the times the slot list compares
come from the storage.

To check that the core still builds there:

```bash
rustup target add wasm32-unknown-unknown
cargo check -p game-core --target wasm32-unknown-unknown
```

Checked by running the game itself in WebAssembly: a test program built for
`wasm32-wasip1` (no threads either), run by Node's WASI, loaded the ROM, continued a
chapter 10 save in the enhanced mode, played 2400 frames with their sound and drew a
picture each frame. It reached the map, unlocked the achievements the save had earned,
and drew the field as the desktop does. It took 0.32 seconds in all, about 0.13 ms a
frame against the 16.7 a frame lasts. The program is 2.2 MB before compression.

## The browser's backend and page

Implemented in `crates/platform-web` (the backend) and `apps/web` (the game's
WebAssembly and the page in `apps/web/static/`).

**The backend** (`platform-web`):

- **Picture:** each frame's pixels are written into a canvas as large as the frame.
  The page's style scales it to the window: sharp (`image-rendering: pixelated`) in
  whole multiples or filling it, or smooth.
- **Sound:** each frame's samples become a Web Audio buffer at the game's own rate
  (31 536 Hz), which the browser converts to the device's. Each buffer is played right
  after the one before, 50 ms ahead at first, so they follow without gaps; as many
  frames are queued at most as in the launcher.
- **Keys:** the desktop's by default, named by the key's place (`KeyboardEvent.code`,
  so they hold on any layout). A key the game takes does nothing else on the page, and
  losing the focus lets every key go. Gamepads follow the browser's standard mapping as
  the launcher's do: the right face button A, the bottom one B, the shoulders L and R,
  Back SELECT, Start START, the D-pad or the left stick the arrows, the right stick's
  click the fast forward.
- **Saves:** each one is kept in the browser's `localStorage` under
  `re-zoids-saga/<name>`, as Base64 text with the time it was stored: `slot-1` to
  `slot-4`, `autosave` and `achievements`. The storage answers at once, as the game's
  saving expects, and a site's few megabytes hold every save many times over.

**The game's WebAssembly** (`apps/web`) offers the page two calls:

- `check_rom(bytes)`: what the ROM is, as the launcher tells it (the supported dump or
  not), and whether the port plays it.
- `start(rom, settings, translation, canvas)`: plays the game.
  - The options are given as the launcher's settings lines: `mode`, `color`, `trail`,
    `upscale`, `volume`.
  - The loop runs once for each picture the screen shows (`requestAnimationFrame`) and
    plays the frames due at the hardware's pace, whatever the screen's refresh (at most
    four after a hold up, which is skipped rather than caught up).
  - M mutes, and in the enhanced mode holding Space plays the fast forward.
  - When the player leaves from the pause menu's 終了, the page hears
    `re-zoids-saga:left` and shows its menu again.

**The page** reads the ROM chosen or dropped on it in the browser, never sending it
anywhere, and plays it only when it is the supported dump. It also takes an optional PO
file and the options, which it remembers. The sound starts on its Play button, since a
browser lets a page make sound only after the player has pressed something. It links to
the source code, as the GPL asks of a program handed to the browser.

To build it and play it locally:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129   # the wasm-bindgen of Cargo.lock
tools/package/web.sh
python3 -m http.server -d dist/web
```

`dist/web` is the whole site: any static web server serves it. Checked in a browser: the
page identified the ROM, the game ran from the publisher's logo to the title and its
attract demo, continued a chapter 10 save from the slot list (which read the browser's
storage), and played on the field from the keyboard at 60 frames a second, writing the
achievements to the browser's storage. A browser slows a page it does not show (to a
few calls a second), and the game slows with it; a hidden tab pauses it.

