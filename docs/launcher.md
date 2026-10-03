# The launcher

Source of knowledge: this project's own design. Implemented in `apps/launcher`.

## Its screen

Run without arguments, the launcher shows its own screen, **Re:Zoids Saga**: choose your
ROM in the system's file dialog and, optionally, a translation, then Play (arrows move, X
chooses, Z clears the translation, Return plays; a touch or a click chooses the line
under it).

The Translation line opens a screen of its own: *From a file...* opens a PO file in the
system's dialog, and below it every language the translations' repository offers
([re-zoids-saga-translations](https://github.com/serivt/re-zoids-saga-translations),
listed in its `po/languages.json`) downloads that language's PO file, checks that it
reads, keeps it in the launcher's settings folder as `<code>.po` and chooses it; a
language already kept says *downloaded*, and choosing it again downloads the latest
corrections. It fetches the list when the screen opens and a file when a language is
chosen, over HTTPS from `raw.githubusercontent.com`, and sends nothing else. Without a
connection the screen says so and a file can still be opened.

When its screen opens the launcher also asks, once, whether a newer version is out: it
reads the project's tags from GitHub's API (`api.github.com/repos/serivt/re-zoids-saga/tags`,
over HTTPS, on a thread of its own, sending nothing but the request) and takes the highest
of those named `vX.Y.Z` by semantic versioning. When that is newer than the port's own
version, an *Update available* line with the new version shows in green after Quit (the
lines then a pixel closer); choosing it opens the releases page
(`github.com/serivt/re-zoids-saga/releases`) in the web browser. Without a connection, or
with an answer it cannot read, nothing shows (`apps/launcher/src/update.rs`). These are
the only times the launcher goes online. Only the verified dump,
Zoids Saga (Japan, Rev 1), plays: for the first release (Rev 0), another dump of the
game or another game it says so and Play stays off, and the command line refuses them
the same way. It speaks the chosen translation's language (English without one). The
port's version shows in the top right corner.

Game mode chooses how the game plays, on a screen of its own:

- **Classic** (the default) plays exactly as the original;
- **Enhanced** adds conveniences the original never had, each turned on or off on the
  same screen (they show unavailable in the classic mode, which keeps the choices for
  when the enhanced mode comes back):
  - *Battle animations* off skips the battles' attack scenes: an enemy's attack goes
    from the fade out straight to its outcome on the battle's screen, and the party's
    scene stops once the player has aimed (see [combat.md](combat.md), Without the
    attack scenes).
  - *Damage numbers* on shows, besides each hit's message, the damage every unit
    takes as a number under it for a moment (see [combat.md](combat.md), Damage
    numbers).
  - *Auto text* on lets the text boxes go on by themselves once their text has been on
    screen long enough to read, with an AUTO mark on them; SELECT turns it on or off
    while a text box shows (see [formats/script-text.md](formats/script-text.md)).
  - *Autosave* (on by default) saves the game on each change of map into a place of its
    own, `game.auto.sav`, which つづきから lists first and セーブ never offers (see
    [formats/save.md](formats/save.md), The autosave).
  - *Weapon reach* (on by default) draws, in the armaments shops and the pause menu's
    weapons and equipment screens, the cells of the battle's grid the weapon under the
    cursor reaches, from the front row or the back, SELECT turning from one to the other
    (see [menu.md](menu.md), Weapons' reach).
  - *Fast forward* (2x by default, 3x or 4x, also in the pause menu's コンフィグ): the
    speed the game plays at while the fast forward's button (Space, the gamepad's right
    stick click or the on-screen pad's `>>`) is held: that many frames for each one
    shown, with `>>2X` (or 3X, 4X) at the frame's top right in the port's small
    capitals; letting go plays it at its pace again. The button is the launcher's alone:
    the game never sees it, and it does nothing in the classic mode or while the
    question to quit shows. The sound of the
    frames the audio queue has no room for is dropped, so it keeps its pitch and skips
    ahead (`apps/launcher/src/fast.rs`).

The enhanced mode also counts 31 achievements, from the chapters cleared to the
collection and the battles' records, announced at the field's top as they unlock and
listed in the pause menu's 実績; they are shared by every save of the ROM, in
`game.achievements` beside them (see [achievements.md](achievements.md)).

The mode is set before the game starts (`Game::set_play_mode`, see
`crates/game-core/src/play_mode.rs`) and does not change while it plays; saves are the
original's in either mode, the autosave's too. In the enhanced mode the pause menu's コンフィグ lists the
enhancements too, beside the original's message speed (see [menu.md](menu.md)); what the
player changes there holds at once and the launcher remembers it.

Options holds:

- the window's size (×1 to ×6) and fullscreen;
- Display, a screen of its own, with its preset, or its filter and colors, as the line's
  value:
  - the preset (see Presets, below): one of eight, or custom;
  - the filter (left and right go through them):
    - sharp keeps whole multiples of the screen, with black bars;
    - pixel art fills the window keeping the screen's shape, each pixel a square with
      only its edges blended where the scale is not whole (SDL 3.4's pixel-art sampling,
      which the renderer does on the graphics card; linear where a renderer lacks it), so
      a fullscreen picture is sharp without the bars;
    - LCD, LCD soft and LCD fine keep whole multiples, as sharp, and darken the lines
      between the pixels as a handheld's panel showed them, marked, lighter or thin and
      light (see The LCD grid, below);
    - scan lines keep whole multiples and darken a band under each row, as a television
      showed the picture;
    - smooth fills the window blending the pixels;
  - the upscaler (see Pixel-art magnification, below): off, Scale2x or Scale3x;
  - the colors (see Colors, below): original, GBA, GBA SP AGS-001, GBA SP AGS-101, Micro
    or DS;
  - the LCD trail (see The LCD trail, below): off, mix or fade;
- the volume;
- Saves, to export and import the saves (see Saves, below);
- the buttons: Keyboard and Gamepad list the pad's ten buttons, where you choose one and
  press its new key or button (Esc cancels; a button that had it swaps with it), or take
  the defaults back.

### Colors

Source of knowledge: this project's own design, chosen by eye against the panels'
well-known look; no other emulator's values. Implemented in
`crates/screen-filters/src/color.rs`.

The Game Boy Advance's own screen showed the games darker and paler than a modern display
does, and they were drawn bright to make up for it. The colors option shows the game as
such a panel did, or as it draws it (original, the default):

- **GBA:** the original console's unlit reflective panel, darker and paler.
- **GBA SP AGS-001:** the first SP's front light, washed out and cool, its black lit.
- **GBA SP AGS-101:** the later SP's backlight, a little paler than the original colors.
- **Micro:** the Game Boy Micro's small backlit screen, vivid.
- **DS:** the Nintendo DS's screens, which play the cartridges in their own slot: bright,
  a little vivid and cool.

A profile models its panel:

1. A channel's level of 0 to 31 turns into light along the panel's curve (its gamma).
   The unlit panels' is steeper than a modern display's, darkening the middle tones.
2. The channels bleed into each other through a mix. Rows that sum to one keep greys
   grey; negative bleeds saturate.
3. White may be dimmer than a display's, and black may let some light through.
4. The light may lean warm or cool, a gain per channel.
5. The light is written back for a display of gamma 2.2.

| Profile | Gamma | Own share of each channel | White | Black | Lean |
|---|---|---|---|---|---|
| GBA | 2.8 | 80 % | 93 % | 0.4 % | none |
| GBA SP AGS-001 | 2.6 | 84 % | 95 % | 1.5 % | cool (red 97 %, blue 104 %) |
| GBA SP AGS-101 | 2.4 | 90 % | 98 % | 0.2 % | none |
| Micro | 2.2 | 106–108 % | 100 % | 0 | none |
| DS | 2.0 | 104 % | 100 % | 0.2 % | cool (red 98 %, blue 103 %) |

The picture's colors are the console's 15-bit ones, so a table of the 32,768 colors,
built when the game starts, does the whole work. The launcher applies it to each picture
the game draws, before its own marks (MUTE, the fast forward's) go over it. Its own
screen keeps the original colors.

### Presets

Source of knowledge: this project's own design, after the display modes of handhelds
that play the cartridges on their own screen, which imitate each console's.

A preset sets the display's options together, as a screen the game was played on showed
it, or as today's screens suit it:

| Preset | Filter | Upscaler | Colors | Trail |
|---|---|---|---|---|
| GBA | LCD | off | GBA | fade |
| GBA SP AGS-001 | LCD soft | off | GBA SP AGS-001 | mix |
| GBA SP AGS-101 | LCD soft | off | GBA SP AGS-101 | mix |
| Game Boy Micro | LCD fine | off | Micro | off |
| Nintendo DS | pixel art | off | DS | off |
| Game Boy Player | scan lines | off | original | off |
| modern | pixel art | off | original | off |
| smooth pixel art | pixel art | Scale3x | original | off |

The handhelds' presets leave the upscaler off, so the grid lines the game's own pixels.

The preset is not kept apart. The line reads it from the options as they stand, and shows
custom once any of them differs. Left and right go from one preset to the next, and from
custom to the first or the last.

### The LCD grid

Source of knowledge: this project's own design. Where the lines go is
`crates/screen-filters/src/grid.rs`, which the web version shares (see
[web.md](web.md)); drawing them is `crates/platform-sdl3/src/lib.rs`.

The LCD filters scale the picture by whole multiples, as sharp does, then darken a line
at the right and the bottom of every pixel of the game. The scan lines filter darkens
only the band under each row. The lines are drawn on the graphics card, over the scaled
picture, as rectangles that multiply what they cover. A line takes a share of the output
pixels a pixel spans, rounded to whole ones and at least one. When a pixel spans fewer
output pixels than a line needs, the lines are left out.

| Filter | Multiplies by | Share of a pixel | Lines | Shows from |
|---|---|---|---|---|
| LCD | `0xB0` (69 %) | 1/5 | columns and rows | 3 output pixels a pixel |
| LCD soft | `0xCC` (80 %) | 1/5 | columns and rows | 3 |
| LCD fine | `0xD8` (85 %) | 12 % | columns and rows | 3 |
| scan lines | `0xA8` (66 %) | 40 % | rows | 2 |

With the on-screen pad the picture is placed by the pad, and the lines fall on whole
output pixels of that place.

### The LCD trail

Source of knowledge: this project's own design. Implemented in
`crates/screen-filters/src/trail.rs`.

The handheld's liquid crystals took longer than a frame to change, so the picture before
lingered under the new one. The trail comes in two kinds:

- **Mix:** each picture shown is half its own and half the picture the game drew before
  it. A picture the game alternates every other frame, as some effects do to look
  see-through, then shows as the mix of the two.
- **Fade:** each picture shown is half its own and half the one shown before it. What
  moves leaves a trail that fades over two or three frames (a half, a quarter, an
  eighth), as the original GBA's slow panel did.

The trail goes after the colors and before the launcher's own marks. It mixes the
pictures shown, so with the fast forward it mixes those the player sees.

### Pixel-art magnification

Source of knowledge: the Scale2x and Scale3x algorithms (Andrea Mazzoleni, of the
AdvanceMAME project), written from the public description of their rules, not from any
implementation. Implemented in `crates/screen-filters/src/upscale.rs`.

The upscaler makes the picture two or three times larger before the filter fits it to the
window. Each new pixel copies the pixel it comes from or one of its four neighbors,
chosen by which neighbors are equal. Flat areas and straight edges stay as drawn, while a
stair of single pixels becomes a smoother line. Nothing is blurred, since no color is
mixed.

It works on the whole picture, the launcher's marks included, after the colors and the
trail. The display takes the larger picture in place of the screen's, so the filter
still decides how it reaches the window; pixel art suits it best. The LCD grid keeps
lining the game's own pixels.

The game's font is shaded, so on text the rules see some shading as diagonals and leave
small specks along the strokes.

### Saves

Source of knowledge: this project's own design; what a save holds is read as continuing
reads it (see [formats/save.md](formats/save.md)). Implemented in
`apps/launcher/src/saves.rs` and `apps/launcher/src/front.rs`.

Options › Saves lists the four slots and the enhanced mode's autosave, each with whether
it holds a save; it needs the ROM chosen, since the saves live beside it. On Android the
app's own folder is out of reach, so this is the only way in or out. Choosing a slot
offers:

- **Export .sav...:** a copy for an emulator or a flash cart. The dialog proposes the ROM's
  name with `.sav`, which mGBA, VBA-M and the like load by themselves beside the ROM.
- **Export .srm...:** the same bytes under RetroArch's extension, `<rom>.srm`.
- **Export as the cartridge...:** the save without the port's notes (the name in full and
  the records, see [formats/save.md](formats/save.md)), their bytes erased as the
  original leaves them: the memory the cartridge itself would hold. The game in it is
  the same; the name falls back to the one the block holds, and the records start over.
- **Import...:** reads a `.sav` or `.srm` chosen in the dialog. A file that is not a save
  of the game, with a game to continue, is refused. Otherwise a confirmation shows what
  it holds against what the slot holds: the level and area, the money and, when the port
  counted it, the time played (or empty). Replace the slot's save puts it in the slot,
  keeping the one it replaces beside it as `.bak`; Cancel, or Z, leaves the slot as it
  is.

The autosave offers the three exports alone: it is the game's own to write.

About shows the version and the license, and the project's pages: the port's repository
and the translations' ([re-zoids-saga-translations](https://github.com/serivt/re-zoids-saga-translations)),
which X opens in the web browser.

The game plays with these options and this mode from then on, also when given a ROM on
the command line. The choices are remembered in `launcher.cfg` under `re-zoids-saga/launcher` in the
folder where SDL keeps a program's preferences (Application Support on macOS,
`~/.local/share` on Linux, AppData on Windows).

Esc on the launcher's screen, or while the game plays, asks before closing (No is chosen
at first; Esc again stays), and the game waits while it asks. In the enhanced mode the
pause menu's last line, 終了, goes back from the game to this screen instead (see
[menu.md](menu.md), Leaving for the launcher); the window stays open and the screen
comes back as it was. A game the command line started closes instead.

The game runs at the GBA's rate, a frame every 16.743 ms (280 896 cycles of its clock),
about 59.73 a second, whatever the screen's refresh. The window waits for the screen's
vertical blank before it shows a picture (vsync, SDL's `SDL_RENDER_VSYNC`), so a picture
never shows half drawn: without it a screen tears, the top of the picture showing one
frame and the bottom the one before, along a line that drifts up or down as the game's
rate and the screen's differ. Each time around, the launcher runs the frames due since
the last (usually one, none now and then on a 60 Hz screen, which shows a picture twice
about every 4 seconds, as before) and waits for the next only when none is due, which
paces a screen without vsync too; a hold up longer than 4 frames is skipped rather than
caught up (`apps/launcher/src/pacing.rs`). The sound takes each frame's samples as
before, so it keeps the game's rate. Source of knowledge: this project's own design.

On Android the launcher is the same, answering taps, with the options of the on-screen
pad and the saves' export and import in place of the window's and the keyboard's; see
[android.md](android.md).

## Controls

| Pad | Keyboard | Gamepad |
|---|---|---|
| D-pad | Arrows | D-pad or left stick |
| A | X | Right face button |
| B | Z (held while moving runs) | Bottom face button |
| START (opens the pause menu) | Return | Start |
| SELECT | Backspace | Back |
| L, R | A, S | Shoulders |
| Fast forward (`>>`, enhanced mode) | Space | Right stick click |
| Mute (`MUTE`) | M | None |

The binding screens list the fast forward as `>>` and the mute as `MUTE` after the
console's buttons, in two columns of six.

The mute, a port feature, works in either mode: a press turns the sound off and the
next turns it on again, at the volume chosen in the options. While it is off, `MUTE`
shows in the port's small capitals at the frame's top right, white over a black shadow,
four pixels from the edge (the fast forward's mark then moves to its left). The game
never sees the button and goes on making its sound, which is only not heard, so its
timing does not change. It lasts until the launcher closes. Implemented in
`apps/launcher/src/mute.rs`.

In a build with the `debug-mode` feature (`cargo run -p launcher --features debug-mode`),
F10 turns a debugging mode on and off: the roaming enemies are intangible, so the player
walks through them without battles, and the protagonist's attacks beat whatever they hit.
Other builds, the packages players download among them, ignore F10.

## Command line

```bash
cargo run -p launcher -- path/to/rom.gba
```

A ROM on the command line skips the launcher's screen and plays from the publisher logo.

| Option | Effect |
|---|---|
| `--version` | Prints the port's version and quits |
| `--save <file.sav>` | The save file; by default the ROM's name with `.sav`, next to it (see [formats/save.md](formats/save.md)) |
| `--slots <n>` | How many save slots, 1 to 9 (4 by default; 1 is the original's single save) |
| `--translation <file.po>` | Shows a translation (see [translation.md](translation.md)) |
| `--export-template <file.pot>` | Writes the template translators start from |
| `--room` | Skips straight to the first room |
| `--touch` | Shows the on-screen pad for touch screens, as Android always does (with `SDL_MOUSE_TOUCH_EVENTS=1` the mouse plays a finger) |
| `<table>_<index>` | Shows that script in its box instead, e.g. `dialogue_00043` |
| `--dump <frame.ppm>` | Writes a frame instead of opening a window (without a ROM, the launcher's screen) |

Saves use the original's format, so they move between this port, emulators and the
cartridge; slot 1 is the `.sav` and slot n the same name with `.n` before the extension
(`game.2.sav`). The enhanced mode's autosave is the same name with `.auto`
(`game.auto.sav`), and its achievements the same name with `.achievements`
(`game.achievements`).

## The extractor

`extractor-cli` dumps the game text (all tables, or one of `name`, `item`, `dialogue`,
`battle`, `menu`), or reports the messages that overflow the dialogue box after wrapping:

```bash
cargo run -p extractor-cli -- dump-text path/to/rom.gba dialogue
cargo run -p extractor-cli -- check-layout path/to/rom.gba dialogue
```
