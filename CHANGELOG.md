# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
  `tools/package/android.sh`.
- **Taps and clicks on the launcher.** A touch or a click chooses the line under it; the
  desktop can show the on-screen pad with `--touch`.

### Changed

- The demo ends at the start of chapter 7.

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
