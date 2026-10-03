# The web version

Source of knowledge: this project's own design. The browser's side is the public
WebAssembly and Web APIs (canvas, Web Audio, Gamepad, Web Storage, IndexedDB, service
workers, the web app manifest); no other emulator's or port's code was consulted.

The aim is the same game in a browser, with the same rule as everywhere else: the player
provides the ROM. The game's code is compiled to WebAssembly (`wasm32-unknown-unknown`),
and a backend for the browser takes the place of `platform-sdl3`, as SDL3 does on the
desktop and on Android. The plan, in phases:

1. **The core in WebAssembly** (done, below).
2. **The browser's backend and page** (done, below): the picture on a canvas, the sound
   through Web Audio, the keyboard, gamepads and an on-screen pad for touch screens, the
   saves in the browser's storage, and an HTML page to choose the ROM, a translation and
   the options, the launcher's display presets and LCD grids among them.
3. **Kept in the browser** (done, below):
   - the ROM, kept in the browser's database so it is chosen once, and never sent
     anywhere;
   - the translation, downloaded from the translations' repository or chosen as a
     file, kept the same way;
   - the saves exported and imported as the launcher does;
   - the page installable and playable offline, a progressive web app.
4. **Saves in the cloud** (done, below): optional, with an account signed in by a link
   sent by email; the saves and the achievements kept by the player's account, never
   the ROM.

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

- **Picture:** each frame's pixels are written into a canvas as large as the frame,
  which the game places in the window itself: centered, as large as it fits, in whole
  multiples for the sharp scaling and the LCD grids, and around the on-screen pad while
  it shows. The page's style draws it sharp (`image-rendering: pixelated`) or smooth.
- **LCD grid:** the launcher's LCD grids and scan lines (see
  [launcher.md](launcher.md), The LCD grid), where `screen-filters` puts their lines,
  drawn on a canvas of their own over the screen at the device's pixels: white where
  the picture stays, the lines' gray where it darkens, which the page's style multiplies
  with the picture (`mix-blend-mode: multiply`). No WebGL is needed.
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
- **On-screen pad:** on a touch screen the controls surround the game's screen as on
  Android, laid out by the same `platform::touch` (see [android.md](android.md)): the
  screen in the middle and the controls on its sides when the window is wider than
  tall, the screen on top and the controls below it otherwise, the fast forward in the
  enhanced mode alone. They are drawn on a canvas over the window in Android's style,
  translucent and brighter while held, and the fingers come from the page's pointer
  events, so several press at once and a quick tap still counts. The page then neither
  scrolls nor zooms nor opens a menu. The pad shows when the screen's pointer is coarse
  (a phone or a tablet) or once the screen is touched, and hides while a gamepad is
  connected; the options can show it always or never, and make it 60 % to 140 % as
  large and 20 % to 100 % as opaque, as Android's. Checked in a browser emulating a
  phone's touch screen: upright, the screen took the top and the pad the rest, and
  START held on it opened the title's menu; sideways, the screen sat between the
  controls, and two fingers held the cross's right and A at once.
- **Saves:** each one is kept in the browser's `localStorage` under
  `re-zoids-saga/<name>`, as Base64 text with the time it was stored: `slot-1` to
  `slot-4`, `autosave` and `achievements`. The storage answers at once, as the game's
  saving expects, and a site's few megabytes hold every save many times over.

**The game's WebAssembly** (`apps/web`) offers the page these calls:

- `check_rom(bytes)`: what the ROM is, as the launcher tells it (the supported dump or
  not), and whether the port plays it.
- `save_details(rom, bytes)` and `save_summary(rom, bytes)`: what a save holds (the
  level, the area, the money and, when the port counted it, the time played), apart or
  in a line, read as continuing reads it; `cartridge_save(rom, bytes)`: the save
  without the port's notes.
- `start(rom, settings, translation, canvas, pad, menu, grid)`: plays the game, the
  on-screen pad drawn on `pad` and the LCD grid on `grid`, canvases beside the game's in
  the element over the window, which takes the fingers; the page's `menu` button is
  kept at the game screen's top left corner, wherever the screen goes.
  - The options are given as the launcher's settings lines: `mode` and the enhanced
    mode's `battle-animations`, `damage-numbers`, `auto-text`, `autosave`,
    `weapon-reach` and `fast-forward`, `color`, `trail`, `upscale`, `filter` (`lcd`,
    `lcd-soft`, `lcd-fine`, `scanlines`, or none), `volume`, `muted`
    (`1` to start without sound), `scaling` (`sharp`, `fill` or `smooth`), and the
    pad's `touch` (`auto`, `on` or `off`), `touch-size` and `touch-opacity`.
  - When the game's pause menu changes the enhanced mode's settings, the page hears
    `re-zoids-saga:enhancements`, the settings' lines as its detail, and remembers them
    for the next game, as the launcher does.
  - It gives the page the game's session: `pause()`, `resume()`, `quit()` (the page
    then hears `re-zoids-saga:left`), and `set_volume`, `set_scaling`, `set_filter`,
    `set_touch` and `set_touch_opacity`, which change those options while the game
    plays.
  - The loop runs once for each picture the screen shows (`requestAnimationFrame`) and
    plays the frames due at the hardware's pace, whatever the screen's refresh (at most
    four after a hold up, which is skipped rather than caught up).
  - M mutes, and in the enhanced mode holding Space plays the fast forward.
  - When the player leaves from the pause menu's 終了, the page hears
    `re-zoids-saga:left` and shows its menu again.

**The page** puts the ROM first: a first visit explains that the player brings their own
copy, and the ROM chosen or dropped anywhere on the page is read in the browser, never
sent anywhere, and played only when it is the supported dump. Play sits under it, with a
line of what the options choose; on a phone a bar keeps Play in reach once the button
scrolls away. The rest is in tabs: Options (the translation, the mode, classic at first,
with a tip on what the enhanced one adds, and, while it is the enhanced one, its
settings, the picture, folded under a line of what it chooses, with the launcher's eight
display presets (Modern first, and chosen at first), which set the screen filter, the
scaling, the upscaler, the colors and the trail together and show which one the options
match, the sound and the touch pad), Saves, Cloud (when the build has a cloud project)
and Help (the keys, the gamepad, offline and installing). The options are remembered. A
gamepad moves through the page too: the D-pad or the stick moves, A chooses, B goes
back. The sound starts on Play, since a browser lets a page make sound only after the
player has pressed something; when the pad will show, Play also asks the browser for the
whole screen, where the browser lets a page take it (not every phone's does). While the
game plays, Esc or the button at the screen's corner pauses it under a menu: resume, the
volume, the scaling, the pad's opacity and whether it shows, and the way back to the
launcher, which loses what was not saved. It links to the source code, as the GPL asks
of a program handed to the browser.

To build it and play it locally:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129   # the wasm-bindgen of Cargo.lock
tools/package/web.sh
python3 -m http.server -d dist/web
```

`dist/web` is the whole site: any static web server serves it. The script also zips it
as `dist/re-zoids-saga-<version>-web.zip`, which each release carries (see
[packaging.md](packaging.md)). Checked in a browser: the page identified the ROM, the
game ran from the publisher's logo to the title and its attract demo, continued a
chapter 10 save from the slot list (which read the browser's storage), and played on the
field from the keyboard at 60 frames a second, writing the achievements to the browser's
storage. A browser slows a page it does not show (to a few calls a second), and the game
slows with it; a hidden tab pauses it.

## Kept in the browser, and offline

Implemented in `apps/web/static/` (`store.js`, `saves.js`, `main.js`, `sw.js`,
`manifest.webmanifest`) and the game's WebAssembly's `save_summary` and `cartridge_save`.

**The ROM.** Once the page has identified it as the supported dump, it keeps the file
in the browser's database (IndexedDB, under `re-zoids-saga`), since it is too large for
the page's storage. It also asks the browser not to clear the page's data when space runs
low. The next visit finds it there and needs nothing chosen: the status line says it is
remembered, and Forget the ROM removes it (the saves stay). It is never sent anywhere.

**The translation.** The list offers the languages of the translations' repository
(`po/languages.json`, read from GitHub as the launcher does) and a PO file of the
player's own. The one chosen is kept in the same database with its name, so it needs no
connection afterwards. Offline, its line is offered again from what is kept.

**The saves.** They stay in the page's storage as the game keeps them. Once the ROM is
known, the Saves tab shows a card for each slot and the autosave with what it holds: the
level, area and money and, when the port counted it, the time played (`save_details`).
Each one exports, from a menu, under the ROM's name:

- `.sav` for an emulator or a flash cart;
- `.srm` for RetroArch, the same bytes;
- **Cartridge:** without the port's notes (`cartridge_save`, see
  [formats/save.md](formats/save.md)).

A slot imports a `.sav` or `.srm`, which must read as a save of the game. A dialog first
shows what the file holds against what the slot holds, and offers to export the slot's
before it is replaced; the slot's is also kept as `<slot>.bak`. The autosave only
exports.

**Offline and installed.** A service worker keeps the page, its scripts and style, the
manifest, the icon and the game's WebAssembly in the browser's cache. It caches them
under the build's version, which `tools/package/web.sh` writes in, and a new build
clears the caches of the builds before. The page's own files come from the cache first;
anything else (the translations' repository) from the network. With the ROM and the
translation kept, the game then plays with no connection at all. The manifest lets the
browser install the page as an app, standalone, with the project's icon. A service
worker needs a secure page: HTTPS, or `localhost` while testing.

Checked in a browser:

- the ROM chosen and Spanish downloaded from the repository were both remembered on the
  next visit;
- the Saves list read a chapter 10 save (Lv 31, area 10, 4698050 G, 87:23 played);
- its cartridge export differed from it in the 70 bytes after the copies (from
  `0x7E43`) alone, all erased, as on the desktop;
- an `.srm` imported into an empty slot after the browser's question;
- with the server stopped, the page loaded from its cache with the ROM and the
  translation remembered, and the game started.

## Saves in the cloud

Implemented in `services/cloud/supabase/` (the database, its rules and tests, the local
project's settings) and the page's `cloud.js` (the account and the requests) and
`sync.js` (the syncing). The backend is a [Supabase](https://supabase.com) project:
its Auth signs the players in, and its Postgres keeps the saves behind row-level
security. The page calls their REST APIs directly with `fetch`, with no library, so it
still works offline.

**The account.** The player gives an email address, and Auth sends a link that signs
them in on the page (a magic link: no password). The page keeps the session in its
storage and refreshes it before it runs out. Sign out ends it; Delete my account deletes
the account, the email and every save in the cloud (`delete_my_account`), the saves in
the browser staying. The page shows the section only when the build names a project
(`config.js`), and links to `privacy.html`, which tells the players what is kept and how
to delete it.

**What the cloud keeps** (`public.saves`, one row per account, ROM and save):

- the ROM's SHA-1, computed in the browser: never the ROM;
- the save, `slot-1` to `slot-4`, `autosave` or `achievements`, as the page keeps it
  (Base64, at most 64 KiB);
- what it holds, for the lists;
- when the server stored it.

Each change keeps the version before in `public.save_history`, the last ten of each
save. Every row is the signed-in player's own: the rules let them read, add, change and
remove their own alone, and the history read alone. The server refuses a slot the game
has not, or a ROM not named by a SHA-1.

**The syncing.** Each save remembers its last sync: the cloud's time and the browser's.

- A save changed in one place only since then goes to the other. One taken from the
  cloud keeps the browser's as `.bak`.
- A save changed in both, or found in both before any sync, is a conflict. The Saves
  list shows what each holds, to keep the browser's (sent to the cloud) or take the
  cloud's.
- The achievements never conflict: the two lists are joined, and each side gets the
  whole.

The page syncs when it opens and once a ROM is chosen, both ways. While the game plays,
it only sends what changed, every minute and when the page is hidden, so nothing changes
under the game.

**Testing it locally** needs Docker and the Supabase CLI. No account is needed: the
local project's mail never leaves the machine, and its web view shows the links.

```bash
supabase start --workdir services/cloud        # the database, Auth and a mail box
supabase test db --workdir services/cloud      # the rules' tests (pgTAP)
SUPABASE_URL=http://127.0.0.1:54321 \
SUPABASE_ANON_KEY=<the anon key "supabase status" prints> tools/package/web.sh
```

Serve `dist/web` on `http://localhost:8737`: that is the address the local Auth sends
its links back to (`services/cloud/supabase/config.toml`).

**Deploying it** is done on the project owner's own Supabase account:

1. Create a project, then link the folder (`supabase link --workdir services/cloud
   --project-ref <ref>`) and push the database (`supabase db push --workdir
   services/cloud`).
2. In the project's Auth settings:
   - set the site URL and the redirect URLs to the page's address (HTTPS);
   - set an SMTP server for the emails, since Supabase's own sends very few an hour.
3. Build the page with `SUPABASE_URL` and `SUPABASE_ANON_KEY` from the project's API
   settings.

The anon key is meant for browsers: the rules are what keep each player to their own
rows. The service role key must never reach the page.

