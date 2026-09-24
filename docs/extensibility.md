# Extensibility

Source of knowledge: the shape the code has taken so far (the script host trait,
the translation loader, the data readers in `extraction`) and how modding communities
extend comparable games. This page sets the rules new code follows so that mods can
replace data, add scripts and, later, add logic without forking the engine. The
contracts exist in the code (see "Implemented so far"); mod packs and scripting do
not yet, and the rules are what makes them possible.

## What a mod can be

Mods fall into three layers, cheapest first. The engine is built so each layer can
arrive without redoing the previous one.

| Layer | Examples | Mechanism |
|---|---|---|
| Data | Sprites, maps, music, fonts, translations, number tables (experience, prices, stats) | A pack of files that override what the engine would read from the ROM, or patches to typed tables |
| Scripts | New or changed dialogue, menus, cutscenes | Scripts in a readable text form assembled into the game's own bytecode and run by the existing interpreter |
| Logic | New mechanics, menus, rules | Extensions reacting to engine events, first in Rust, later from an embedded scripting language through the same events |

Mods never contain data taken from the ROM: like translations, they carry their own
assets or patches and reference the ROM's content by identifier. The repository's
license (GPL-3.0-only) covers the engine; a mod is the mod author's work.

## Rules for new code

1. **Game logic never reads the ROM directly.** Everything the game needs comes from a
   data provider that resolves an identifier (map 4, sprite `0x98`, song 7, string
   `dialogue/40`, the experience table) first in mod packs, then in the ROM. The
   readers in `extraction::saga` take the ROM slice; game code calls `GameData`
   instead. A function that takes `rom: &[u8]` belongs in `extraction`, not in
   `game-core`, with two exceptions that address the image by offset: the script
   interpreter and the sound driver.
2. **Behavior flows through events and commands.** A screen raises what happened
   (`RoomEntered`, `Talk`, `MenuChoice`) and the parts that care react, instead of one
   screen calling another. Queries a mod may want to answer (which song a map plays,
   how a message reads, how large a window is) are hooks with a default answer that an
   extension can replace. `ScriptHost` is the first such contract; new contracts follow
   its style: a trait with default methods that do nothing.
3. **The engine's own features are extensions.** Translation, window fitting, the
   name-entry pages and the music per map plug into the same events and hooks a mod
   would. If the contract is not enough for us, it is not enough for anyone.
4. **State is typed data.** `Party`, flags, the field position and the settings are
   plain structures that can be serialized, inspected and patched. No game state hides
   in a screen's private fields once it outlives the screen. The save is the original's
   own format (see [formats/save.md](formats/save.md)); `formats::Progress` is its typed
   view.
5. **Identifiers are stable and documented.** Table names, script keys
   (`table/index/offset`), sprite tags, map and song numbers, event names and hook
   names are public contracts. Renaming one is a compatibility break and is called
   out in the commit and in `docs/`.
6. **Order and conflicts are explicit.** Packs load in a declared order; the last
   override wins; hooks are asked in order and the first answer wins. The launcher
   reports what each pack replaced.
7. **Portability first.** No mechanism that needs the player to compile anything,
   no native plugins (dynamic libraries): a mod is files, and later scripts, that work
   the same on desktop and Android.

## First events and hooks

The events below are the ones the current code already produces implicitly; making
them explicit is the first step. Names are the contract.

Events (notifications; every extension sees them, in order):

| Event | Payload | Raised by |
|---|---|---|
| `Frame` | frame number | the game loop, once per update |
| `TitleShown` | — | boot |
| `NameConfirmed` | the name | name entry |
| `RoomEntered` | map number, arrival cell | field load and every warp |
| `ExitTaken` | map, exit index, destination map | field |
| `Talk` | character index, dialogue string | field |
| `ScriptStarted`, `ScriptEnded` | table, string index | script runner |
| `MessageShown` | table, string index, offset, window | script runner |
| `WindowOpened`, `WindowClosed` | id, rectangle, kind | script host |
| `SoundRequested` | song number | script host, field, menus |
| `FlagChanged` | flag, value | script host |
| `MenuOpened`, `MenuChoice`, `MenuClosed` | table, string index, line | pause menu, title, name entry |
| `SaveRequested`, `LoadRequested` | — | pause menu, title |
| `StorageFailed` | why | saving and continuing, when the save cannot be read or written |

Hooks (queries; the first extension that answers decides, the engine's default is
last):

| Hook | Question | Default today |
|---|---|---|
| `translate_message(table, index, offset)` | Text to show instead of the ROM's | the loaded PO file |
| `fit_window(table, index, id, kind, rect)` | Rectangle a window should take | the translation's layout fit |
| `music_for_map(map)` | Song to play on entering | the call found in the map's code |
| `alphabet_pages()` | Character pages of the name entry | the translation's pages, else the ROM's |
| `sound_for(event)` | Song number for a game sound (door, confirm) | the constants in `extraction` |
| `resource(id)` | Bytes or decoded asset for an identifier | mod packs, then the ROM |

Events carry data by value and cannot be vetoed; hooks return an answer or `None`.
An extension that needs to change state does so through the same commands the
screens use (start a script, play a song, warp, set a flag), never by reaching into
another extension.

## Mod packs

The intended shape of a pack, to be implemented when the data provider exists:

```
mods/
  better-balance/
    mod.toml          name, version, engine version it needs, load priority
    tables/experience.json   a full table, or a patch: {"5": 100}
    translations/es.po
    fonts/latin.txt
    sprites/ch56.png  an image in the sprite's own layout, with its palette
  new-quest/
    mod.toml
    scripts/dialogue/1020.txt   a new string in the readable script form
```

Paths mirror the identifiers of the data provider, so the same name appears in the
pack, in the engine's log and in `docs/formats/`. A pack that only carries a
translation is the workflow of [translation.md](translation.md) with a manifest.

## Later: scripting

Logic mods need a language. The candidates weighed, all reachable through the
extension contract above so the choice can wait:

- **Rhai**: pure Rust, trivial to embed and to ship on Android; less known to modders.
- **Lua** through `mlua`: the language modding communities know; needs a C toolchain.
- **WebAssembly**: a real sandbox and any source language; heavy for casual modders.

Whatever is chosen binds the same events and hooks; the work is in the API the
engine exposes (party, flags, field, windows, commands), which is why the events come
first.

## Implemented so far

- `game_core::GameData` is the data provider: every asset and table the game reads
  comes through it by identifier (map, scene, sprite, portrait, font, skin, boot
  graphics, wallpaper, experience table, map music, script tables, sound numbers, the
  save layout and the new-game state, the guides' pictures, parts and character
  entries).
  Its `bytes()` is the one deliberate escape, used by the script interpreter and the
  sound driver, which address the image directly.
- `game_core::extension` holds `Event`, `GameSound`, the `Extension` trait (every
  method defaulted) and `Extensions`, the ordered list the game and its windows
  share. `Game::extensions()` gives it to the launcher; `insert` replaces an extension
  of the same name.
- Events raised today: `Frame`, `TitleShown`, `NameConfirmed`, `RoomEntered`,
  `ExitTaken`, `Talk`, `ScriptStarted`, `ScriptEnded`, `MessageShown`,
  `WindowOpened`, `WindowClosed`, `SoundRequested`, `FlagChanged`, `MenuOpened`,
  `MenuClosed`, `ShopOpened`, `ShopClosed`, `SaveRequested`, `LoadRequested`,
  `StorageFailed`. Not yet raised: `MenuChoice`.
- Hooks asked today: `translate_message`, `fit_window`, `music_for_map`,
  `alphabet_pages`, `name_entry_help`, `sound_for`. `resource` waits for mod packs.
- The loaded translation is `TranslationExtension`, an ordinary extension installed
  by `Game::set_translation`; the engine's defaults (the ROM's music per map, the
  sound constants, the ROM's name-entry pages) are consulted only when no extension
  answers.

## Not decided yet

The concrete data-provider API, the manifest fields, how patches address entries of
each table, the readable script form and its assembler, and the scripting language.
