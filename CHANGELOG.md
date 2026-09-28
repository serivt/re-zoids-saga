# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
  without SDL3 installed, published by GitHub Actions from `v*` tags.

[0.1.0]: https://github.com/serivt/re-zoids-saga/releases/tag/v0.1.0
