# Achievements

Source of knowledge: this project's own design. The fields and flags each achievement
reads are the game's, documented in [formats/save.md](formats/save.md),
[events.md](events.md) and [combat.md](combat.md). The totals were counted on the ROM by
the same routines the port runs (`crates/extraction`), and checked by continuing a
chapter 10 save in the port. Implemented in `crates/game-core/src/achievements.rs`, with
the list in `crates/game-core/src/menu/achievements.rs`.

The enhanced mode has 31 achievements, a port feature. Each one unlocks once and for
good. They are shared by every game played from the same saves: one file beside the
`.sav`, with the ROM's name and the extension `.achievements` (`game.achievements`). The
file is the header `# re-zoids-saga achievements` and then one key per line; a key the
port does not know is left out. The classic mode neither checks nor shows them, and the
debugging mode does not count toward them.

## Kinds

- **State.** These read the game state, so a game saved before achievements existed, by
  the port or by an emulator, unlocks them as soon as it is continued.
- **Statistics.** These read the player's statistics (see [menu.md](menu.md),
  Statistics), which only saves the port wrote since that feature keep. Other saves
  start from nothing.
- **Live.** These are met only as they happen.

## When they are checked

- **The game state and the statistics:** every 30 frames while the player walks freely
  on the field. The check uses the game state with the flags, level, experience and
  money as they stand, which the block itself only takes when the game is saved.
- **A battle's end**, as its outcome is recorded.
- **A lab's close**, when it developed a Zoid while it was open.
- **The first frame of the staff roll.**

## The announcement

Each unlock is announced by a window at the field's top, while the player walks freely
and the field is bright:

- **Layout.** The window is the original's frame, centered and as wide as its text,
  with two lines:
  - `port/achievements/unlocked` (実績解除！);
  - ★ and the achievement's name, or `port/achievements/many` ({count}件の実績) when
    several were unlocked at once, as a game saved before is continued.
- **Timing.** It slides down over 8 frames, stays 150 and slides back up over 8. It
  plays sound `0x35`, the battle's spoils jingle, as it starts.
- **Queue.** Achievements unlocked while one shows wait for the next window.

## The list

The pause menu's main list in the enhanced mode goes on after 記録 with 実績
(`port/menu/achievements`), before 終了. A on it plays `0x40` and opens the list over the
menu in window 4, the statistics' window across the screen:

- **Pages.** There are 11 pages of three achievements. Each takes two lines:
  - its mark (★ unlocked, ☆ not), a space and its name;
  - a full-width space and what it asks for.
- **Help line.** It shows the page's number and how many are unlocked of all of them
  (`port/achievements/page`), then the statistics' keys.
- **Keys.** Left and right turn the page (`0x40`, round from the last to the first). B
  plays `0x3F` and goes back to the main list with the cursor on 実績.

The texts are keyed `port/achievement/<key>` (the name) and `port/achievement/<key>-help`
(what it asks for), see [translation.md](translation.md).

## The list of achievements

| Key | Name | Asks for | Kind | Reads |
|---|---|---|---|---|
| `chapter-1` | 赤い川の彼方へ | Chapter 1 cleared | State | flag `0x144`, set as the party lands in Sand Colony |
| `chapter-2` | 砂と反逆 | Chapter 2 cleared | State | flag `0x158`, chapter 3's opening |
| `chapter-3` | 目覚めるクレーター | Chapter 3 cleared | State | flag `0x16D`, chapter 4's opening |
| `chapter-4` | 鋼と鋼 | Chapter 4 cleared | State | flag `0x178`, chapter 5's opening |
| `chapter-5` | ドームの覇者 | Chapter 5 cleared | State | flag `0x194`, chapter 6's opening |
| `chapter-6` | 風が止むとき | Chapter 6 cleared | State | flag `0x1AB`, chapter 7's opening |
| `chapter-7` | ブルージェムとの約束 | Chapter 7 cleared | State | flag `0x1B2`, chapter 8's opening |
| `chapter-8` | 逃げ場なし | Chapter 8 cleared | State | flag `0x12F`, chapter 9's opening |
| `chapter-9` | 帝国の落日 | Chapter 9 cleared, the ending seen | State | flag `0x137`, chapter 10's opening after the staff roll |
| `chapter-10` | 解き放たれた怒り | Vega's Berserk Fury beaten | State | flag `0x13C` |
| `old-rivals` | かつての好敵手 | Rosso and Viola taken along in the ruins | State | flag `0x1A4` |
| `defiance` | 不遜 | The Ultrasaurus challenged and beaten (story battle 25, which only a defiant answer starts) | Statistics | bit 25 of the story battles won |
| `lost-data` | 失われたデータ | The 16 Zi data of chapter 10's researchers | State | flags `0x13A` and `0x13B` |
| `researcher` | 研究者 | 50 Zi data | State | the bytes at `+0x33E2` |
| `complete-archive` | 完全なる記録 | Every Zi data the game gives (140) | State | the bytes at `+0x33E2` |
| `treasure-hunter` | トレジャーハンター | Every chest on the maps opened (247) | State | flags `0x1E` + the chest |
| `whos-who` | 人物名鑑 | Every character the story meets in the character guide (63) | State | bit `0x20` at `+0x34A4` |
| `tactics-master` | 戦術の達人 | Every deck command learned (33) | State | the bytes at `+0x347B` |
| `lion-reborn` | よみがえる獅子 | A unit of Trinity Liger BA (Zoid `0x90`) | State | the units, as Dr. T asks (`0x0802AACC`) |
| `core-forger` | コアの鍛冶師 | 10 kinds of Zoid core held at once | State | the counts at `+0x330C` |
| `full-hangar` | 満員の格納庫 | 50 units | State | `+0x3304` |
| `engineer` | 技術者 | A Zoid developed in a lab | Live | the development (`0x08057020`) |
| `summit` | 頂点 | Level 99 | State | `+0xCD2` |
| `millionaire` | 大富豪 | 1,000,000 G held at once | State | `+0xD28` |
| `veteran` | 歴戦の勇士 | 300 battles won | Statistics | battles won |
| `devastating-blow` | 会心の一撃 | A blow of 3000 damage or more | Statistics | the biggest blow |
| `war-of-attrition` | 消耗戦 | A battle of 20 rounds or more | Statistics | the longest battle |
| `flawless-victory` | 完全勝利 | A story battle won without losing a Zoid | Live | the battle's tally |
| `lightning-strike` | 電光石火 | A battle won in its first round | Live | the battle's tally |
| `against-the-clock` | 時間との戦い | The staff roll reached within 15 hours of play | Live | the time played as the roll starts |
| `legend-of-zi` | Ｚｉの伝説 | Every other achievement | — | the others |

## The totals

The collection asks for what the ROM can give, counted when the game is first checked:

- **Zi data: 140 of the 153** the list has room for (`0x0804E3A0`). They are the union
  of:
  - the chests' Zi data;
  - the story's gifts: Irvine's, Rosso's, Blue Gem's four, Vega's and the researchers'
    sixteen;
  - the Zi data of every formation a map Zoid can stand for. That means every map Zoid
    of the maps that keep their objects, the twelve formations of its column in its
    area that a rarity class can pick (`0x080328FC`), and the leader's and members'
    enemy records (`0x0803666C`).

  The other thirteen are never given. Among them are three that only story battles'
  enemies carry, which leave no spoils.
- **Chests: 247.** They are the numbers of the objects of behavior 4 on every map
  (chest 27 is on none). Map 0 repeats chests 0 to 5, and maps 194 and 216 share chests
  144 to 147.
- **Characters: 63 of 87.** They are the ones the story's ten groups put in the
  character guide (lists 0–9 at ROM `0x66C8D0`, met by `0x080099E4` and the like). The
  other 24 are never added.
- **Deck commands: all 33.** The teachers of areas 1 to 10 and the story's own lessons
  give each one: 王子のはげまし (19) from the king, and 0, 22 and 26 in area 1.

When the ROM gives none of a kind, that achievement never unlocks.
