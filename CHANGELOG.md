# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Achievements.** The enhanced mode counts 31 achievements: one for each chapter
  cleared (the last for beating Vega), optional story moments, the collection (Zi data,
  chests, characters, deck commands, Zoid cores, units), the level and money, the
  battles' records, a few met only as they happen (a flawless story battle, a first-round
  win, a Zoid developed, the ending within 15 hours) and one for all the others. Most
  read the game state, so a game saved before, here or in an emulator, unlocks them as
  soon as it is continued. Each unlock slides a window down at the field's top, and the
  pause menu has an Achievements line before Quit that lists them in pages, unlocked or
  not. They are shared by every save of the ROM, in a `.achievements` file beside them.
- **Display filters.** Options has a Display screen with new ways to show the picture:
  - **Pixel art filter.** Besides sharp and smooth, the picture can fill the window or
    the screen keeping its shape, its pixels square and sharp, blended only at their
    edges where the scale is not whole, so fullscreen no longer means black bars or a
    blurry picture.
  - **Colors.** The game can be shown with the colors of the original Game Boy Advance's
    unlit screen (darker and paler, as the games were drawn for) or of the GBA SP's lit
    one, besides the original colors.
  - **LCD grid.** A filter that scales by whole multiples and darkens the lines between
    the pixels, as the handheld's screen showed them.
  - **LCD trail.** Each picture can linger a frame under the next, as on the handheld's
    slow screen, which also blends effects the game flickers to look see-through.

## [0.5.0] - 2026-10-02

### Added

- **Mute.** M (or the key or gamepad button chosen for it in the options) turns the sound
  off and, pressed again, back on, in either mode; a small MUTE mark shows at the top
  right while it is off.
- **Quit.** In the enhanced mode the pause menu ends with a Quit line that asks, with the
  original's yes/no, whether to go back to the launcher, warning that what was not saved
  is lost; yes closes the game and shows the launcher's screen again.
- **Records.** In the enhanced mode the pause menu has a Records line after Save that
  opens three pages of statistics over the menu: the battles won, lost and retreated
  from, the enemy Zoids destroyed and the party's, and the money battles brought; the Zi
  data, the kinds of Zoid, the characters and the deck commands collected, each of its
  total; and the time played (the fast forward's extra frames left out), the biggest
  blow, the longest battle and the story battles won. The game counts them from a new
  game on and keeps them inside each save, where the original never looks, so the saves
  stay the original's.
- The story of chapter 10, which follows the staff credits as in the original: the new
  king sets out again with Regina, Earth and Jack into a newly found space-time, with
  companions Earth and Jack call in at the castle; researchers hand over the Zi data of
  the Emperor's army; and Vega, thrown out of his own time, challenges the party with his
  Berserk Fury, the last story battle (41), and leaves Zi data and a Zoid core behind.
  The area's shops, lab and teacher come with it. The game can now be played to its end.

### Changed

- The game no longer stops at the start of chapter 10: the end of the demo, its thanks
  and its offer to save are gone.

### Fixed

- An exit walked onto plays the sound its warp names, or the one for walking on foot,
  as doors already did and as in the original; it always played the one for riding a
  Zoid.

## [0.4.0] - 2026-10-01

### Added

- **Autosave.** The enhanced mode saves the game each time the player arrives on a new
  map and walks freely there, into a save of its own beside the slots (`game.auto.sav`,
  in the original's format like them). Continuing lists it first, marked apart from the
  numbered slots; saving never offers it. The saves queue up and are written in order in
  the background, so quick changes of map are all kept and the game never waits for them;
  a tiny, faint "Autosaving..." shows at the top left, in the translation's words, until
  they are written. It is on by default and turned off on the game mode's screen or in the
  pause menu's Config; the classic mode never autosaves.
- **Weapon reach.** The enhanced mode draws, in the armaments shops and in the pause menu's
  weapons list and equipment screen, a small picture of the battle's grid with the cells
  the weapon under the cursor reaches and those one shot takes, from the front row or the
  back; SELECT turns from one to the other. It is on by default and turned off like the
  other conveniences.
- **The title's attract demo.** Ten seconds on the title without START play one of the
  original's two demos: four battle scenes of the anime's pilots and Zoids, one after
  another, as in the original; START stops it, and the title comes back with its song.
- **The title's intro.** The title no longer just fades in: as in the original, the
  emblem opens on black, the sky rises line by line from the middle, "ZOIDS SAGA" and
  its subtitle light up, and PRESS START blinks under them. START during the intro skips
  to the finished title, which fades in, as in the original; the intro plays again after
  the attract demo or a guide.
- **Update check.** When its screen opens, the launcher asks the project's repository for
  its release tags (`vX.Y.Z`); when one is newer than the port, an *Update available* line
  with the new version shows at the bottom, and choosing it opens the releases page in
  the web browser. Without a connection nothing shows.
- **Fast forward.** In the enhanced mode, holding Space (the gamepad's right stick click,
  or the on-screen pad's new `>>` button at the top on Android) plays the game at 2x, or
  3x or 4x as chosen on the game mode's screen or in the pause menu's Config, with a
  small `>>2X` mark at the top right. The key can be changed on the keyboard's and
  gamepad's screens.

### Fixed

- With a translation, a part name too long for the weapons list or the equipment screen's
  list no longer pushes its count onto the next line and the cursor out of step: the
  name is cut short with a full stop where the count's column starts.
- **Destroyed Zoids.** A party Zoid destroyed in a battle the party goes on to win comes
  back broken and out of the formation, as in the original, to be revived at a lab; it
  stayed in the formation with no hit points and fought again in the next battle.

## [0.3.0] - 2026-09-30

### Added

- **Game mode.** The launcher's new Game mode line chooses between Classic, which plays
  exactly as the original, and Enhanced, which adds conveniences the original never
  had, each turned on or off on the game mode's screen:
  - **Settings in the pause menu:** the pause menu's Config option opens a list of the
    settings the game can change while it plays, the battle message speed and the
    enhanced mode's conveniences, each described on the help line and changed with
    left and right. The launcher remembers the changes; the classic mode keeps the
    original's Config screen.
  - **Battle animations off:** skips the battles' attack scenes. An enemy's attack goes
    straight to its outcome, and the party still aims its attacks but fires without
    the scene, so battles take about half as long.
  - **Damage numbers:** each unit a blow lands on shows the damage it took as a number
    under it for a moment, besides the message, in the game's own orange digits.
  - **Auto text:** the text boxes go on by themselves once their text has been on
    screen long enough to read, longer the more text they hold, with an AUTO mark on
    them; SELECT turns it on or off while one shows.
- The story of chapter 7: Fran's flight through the device, the forest of the Berserk
  Fury with Alster, Blue Gem, Palty and Solid, story battle 29 at the Fury's lair, and
  Blood's raid on the village, with the area's shops, labs and teachers.
- The story of chapter 8: the Death Stinger razes New Helic City; Schwarz, the
  Ultrasaurus and Dr. D's Gravity Cannon; the six Planetal Sites and their count; Opis
  turning on Blood; the defense of the Ultrasaurus; and story battles 30 to 35 up to
  the true Death Saurer at Eve Polis, with the area's shops, labs and teachers. The
  party may cross the sea once Dr. D has shown his cannon.
- The story of chapter 9: the Emperor wakes the Zoid core and fires on Arcadia Castle;
  at the base Earth and Jack let the player pick two companions for the assault; Fran,
  Gale and Opis wait in the occupied castle (story battles 36 to 38) and the Emperor at
  its heart (39 and 40); the space-time transfer device takes everyone home, with the
  area's shops, lab and teacher.
- The staff credits that close chapter 9: the red Liger running over the plains while
  the staff's names rise, with their song, as in the original.

### Changed

- The demo ends at the start of chapter 10.
- A story warp starts the map's song and runs the map's handler once its objects are
  placed, and its fade in runs the field's hook and the objects' animations, as in the
  original; a map a handler loads again starts its song once loaded.
- The song a staged battle scene gives back is the one the story last asked for, not
  a battle's.
- With a translation, the game's numbers (money, levels, hit points, the menus' figures
  and the battle messages) show in the translation's Latin digits instead of the
  Japanese font's, each in the place the original gives it.

### Fixed

- The Gustav no longer drives over the sea (as in area 3's map 50) before the story
  lets the party cross it.
- The sea and the other backdrops drift and scroll as in the original: they move with
  the camera but keep their place through map loads, and drift a pixel left every 16
  frames.
- **Screen tearing.** The window waits for the screen's vertical blank before showing a
  picture, so the picture no longer splits along a line that drifts up or down while
  the screen scrolls (seen on Windows); the game keeps the GBA's rate whatever the
  screen's refresh.
- **Invisible enemies.** An enemy beaten on the world map or in a cave no longer comes
  back unseen when its map loads again. It went after the party and started battles
  without showing, so a visible enemy could seem to start one from afar; continuing a
  save brought every enemy back and hid the problem.
- The rotated pieces of the battle scenes' sprites no longer show a few pixels out of
  place: their matrix is cut toward zero, as the original's, instead of rounded.

## [0.2.0] - 2026-09-28

### Added

- The story of chapter 4, with its events, conversations, story battles, shops,
  teachers and Dr. T's lines outside the first area.
- The story of chapter 5: the colosseum's three tournaments with their regulations and
  arena, and the final's staged battle scenes.
- The story of chapter 6, with the hidden lab's guards, Raven's white flash and the
  staged battle scenes whose target speaks its own line.
- **Translations to download.** The launcher's Translation line opens a screen that
  lists the languages of the translations' repository and downloads the one chosen,
  keeping it for offline play (choosing it again brings the latest corrections), or
  opens a PO file as before. The launcher goes online only then, when asked.
- **Android.** An APK to install directly (it is not in any store): the same launcher
  and game on SDL3 for Android, with an on-screen pad around the game's screen in either
  orientation, taps on the launcher's lines, the whole screen, and the ROM and the
  translation copied from Android's file dialog into the app. Built with
  `tools/package/android.sh`, and by the release workflow, which starts it on an
  emulator first; see `docs/android.md`.
- **The on-screen pad's size and opacity** in Android's options; the pad hides while a
  gamepad is connected.
- **Saves to export and import** on Android: Options › Saves copies a slot's `.sav` to
  a place chosen in the system's dialog, for an emulator or a backup, or puts one
  there in a slot, checking first that it is a save of the game and keeping the one it
  replaces as `.bak`.
- **Android's back button** goes back on the launcher's screens and asks before closing,
  as Esc does; taps answer the question to quit, and the launcher's help names taps and
  the pad's buttons.
- **Taps and clicks on the launcher.** A touch or a click chooses the line under it; the
  desktop can show the on-screen pad with `--touch`.

### Changed

- The demo ends at the start of chapter 7.
- F10's debugging aid only works in a build with the `debug-mode` feature
  (`cargo run -p launcher --features debug-mode`); the packages ignore F10.

### Fixed

- Searching a chest freezes the field until its reward is announced, as in the
  original: pressing A again no longer starts the search over, which kept a player who
  pressed A repeatedly from ever getting the reward.
- Music and sound effects mix as the original's sound driver does: the sampled channels'
  buffer equals the driver's byte for byte (interpolation, volumes, wrapping, reverb),
  notes take channels by the driver's priorities, vibrato has its full depth, and the
  programmable channels follow the driver's envelopes, tables and pan at the hardware's
  level.
- Characters sharing a palette slot with another object show the colors the original
  shows.
- Weapons whose effects have large frames show their shots.

## [0.1.1] - 2026-09-28

### Fixed

- **Sound volume.** The sound played about 20 dB below its level: the mix's full
  range filled only a tenth of the output's. It now spans the whole range, as the
  hardware's output does, so the loudest songs peak just under full scale.

### Changed

- **Supported ROM.** The launcher plays only the verified dump of Zoids Saga (Japan,
  Rev 1). The first release (Rev 0), which keeps its data at other addresses and
  crashed the port, is named and refused, and so is any other dump or game; Play stays
  off and the command line stops with the same message.

## [0.1.0] - 2026-09-27

The first playable demo of *Zoids Saga* (Japan, Rev 1): the story from the title
through the end of chapter 3, running on a native reimplementation of the whole base
game engine. Every system the single-player game is built on is in place, read from the
player's own ROM and checked against the original frame by frame: the field and its
events, the battle system, the pause menu, the shops and the Zoid labs, sound, saves and
translations. The chapters still to come are mostly story on top of this engine, not new
systems; link-cable play is not implemented.

### Added

- **ROM and data.** Identification by cartridge header and SHA-1; maps, sprites,
  palettes, fonts, scripts, Zoids, parts, items, enemy formations and songs are read
  from the player's own ROM at load time. Nothing from the ROM is shipped.
- **Boot.** Publisher logo, title screen and its menus, name entry, new game and
  continue, with the original's timing.
- **Field.** Rooms, the world map and Zoid maps with the camera following the party;
  walking and running, exits, doors and space-time portals; townsfolk who wander, turn
  to talk or shy away; roaming enemy Zoids that chase the party; chests and their
  rewards; the game's own random numbers.
- **Events.** The story's cooperative task engine: cutscenes with actors walking and
  gliding, fades, dialogue with portraits and choices, flags, party changes, gifts,
  and battles started by the story. It runs the story of chapters 1 to 3; the demo
  ends at the start of chapter 4 with a thank-you and an offer to save.
- **Battles.** The whole battle system, against roaming formations and story enemies:
  the pre-battle menu (fight, formation, deck commands, status, retreat), turn order by
  speed, each unit's action (attack, defend, battle items), weapon choice and aim on
  the target grid by range, energy, hit chances, critical hits, shields and status
  effects such as paralysis, the enemies' AI, deck commands that change the rules of a
  round, attack scenes with their shots, effects and pilots' lines, retreat, and the
  results: money, spoils (items, cores, parts, Zi data), experience, levels and the
  bonus points each level brings.
- **Pause menu.** Party and character status, each Zoid's parts and their figures, the
  weapons stock and equipping them, formation (including large Zoids that take two
  places), items and their use, Zi data and Zi-data items, the Zoid and character
  guides, message speed and other options, and saving.
- **Shops and labs.** Item shops and armaments shops (buying and selling, stock
  limits), and the Zoid lab: repairs, reviving wrecked Zoids, developing new Zoids from
  Zi data, cores and base Zoids, changing pilots, and selling Zoids.
- **Sound.** Music and sound effects played by a reimplementation of the game's sound
  driver: songs, voices and samples read from the ROM, mixed in software.
- **Saves** in the original's format, compatible with emulators and the cartridge, with
  optional extra save slots.
- **Translations** from gettext PO files, with Latin text in this project's own pixel
  font laid out by pixel width, windows enlarged where a translation needs room, and a
  template exporter for translators. The Spanish and English translations are published
  in [re-zoids-saga-translations](https://github.com/serivt/re-zoids-saga-translations),
  keyed by message and without the ROM's Japanese text, where anyone can suggest
  corrections. With a translation the prince's default name is Atory, the Latin form
  of アトレー.
- **Launcher** with ROM and translation pickers, window size, fullscreen, filter,
  volume, keyboard and gamepad controls, and a quit confirmation. It shows the port's
  version, and an About screen with the license and links to the project's and the
  translations' repositories, which open in the web browser; `--version` prints the
  version. F10 turns on a debugging aid (intangible roaming enemies, an overpowered
  protagonist).
- **Packaged builds** for macOS (universal disk image), Windows and Linux that run
  without SDL3 installed, which GitHub Actions attaches to each published release.
  They carry the project's icon (the app, the Windows program and every window), and
  the Windows program its version information.
