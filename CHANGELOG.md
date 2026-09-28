# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The launcher shows the port's version, and an About screen with the license and
  links to the project's and the translations' repositories, which open in the web
  browser; `--version` prints the version.

### Changed

- With a translation, the prince's default name is Atory, the Latin form of アトレー.
- Translations are published in
  [re-zoids-saga-translations](https://github.com/serivt/re-zoids-saga-translations),
  keyed by message and without the ROM's Japanese text, where anyone can suggest
  corrections.

## [0.1.0] - 2026-09-27

The first playable demo of *Zoids Saga* (Japan, Rev 1), from the title through the end
of chapter 3.

### Added

- ROM identification by cartridge header and SHA-1, with the game's data read from the
  player's own ROM at load time; nothing from the ROM is shipped.
- The boot sequence: publisher logo, title screen, name entry, new game and continue.
- The field: rooms, the world map and Zoid maps, walking and running, exits and doors,
  characters, chests, portals and cutscenes, with the original's timing.
- The story of chapters 1 to 3, with their events, conversations and story battles; the
  demo ends at the start of chapter 4 with a thank-you and an offer to save.
- Battles against roaming and story enemies: attack scenes, support parts, deck
  commands, battle items, enemy AI, retreat, and results with money, spoils,
  experience and levels.
- The pause menu: party and character status, each Zoid's parts, the weapons stock,
  equipment, formation, items, Zi data and Zi-data items, the Zoid and character
  guides, message speed and saving.
- The towns' item and armaments shops, and the Zoid lab: revival, development from Zi
  data, pilot change and sale.
- Music and sound effects through a software mixer for the game's sound driver.
- Saves in the original's format, compatible with emulators and the cartridge, with
  optional extra save slots.
- Translations from gettext PO files, with Latin text laid out by pixel width and a
  template exporter for translators.
- A launcher with ROM and translation pickers, window size, fullscreen, filter, volume,
  keyboard and gamepad controls, and a quit confirmation.
- Packaged builds for macOS (universal disk image), Windows and Linux that run without
  SDL3 installed, published by GitHub Actions from `v*` tags.

[Unreleased]: https://github.com/serivt/re-zoids-saga/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/serivt/re-zoids-saga/releases/tag/v0.1.0
