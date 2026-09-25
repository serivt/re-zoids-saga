# Battles

Source of knowledge: own reading of Zoids Saga (Japan, Rev 1) at the addresses named
below, with breakpoints on the script runner (`0x0803E51C`), the task spawner, the
sound call and the battle setup, write watchpoints on the entity table and the game
state, VRAM, palette RAM and RAM dumps, and screenshots of every frame of a battle
met on the world map in a reference emulator: from the step that met the enemy to
the menu, and from the menu's 退却 to the field's return. The attack scenes were
traced the same way, with per-frame logs of the task states, the scroll shadows, the
per-scanline table, the blend registers and the entity table; the results and the
field after them from a save state taken as a fight was won, its experience also set
by hand below a level and its result to a loss.
Implemented in `crates/game-core/src/combat/` (the battle and its fight, its units and
rules, the attack scenes, their sprites, the player's aim and the results),
`crates/game-core/src/objects.rs` (the roaming enemies' formations),
`crates/game-core/src/field.rs` and `crates/game-core/src/game.rs` (the field after
the battle),
`crates/extraction/src/saga_encounter.rs`, `crates/extraction/src/saga_combat.rs` and
`crates/extraction/src/saga_battle.rs` (data).

This page covers the real battles. The scripted battle scenes of the opening are in
[battle.md](battle.md).

## Entry points

| Routine | What it starts |
|---|---|
| `0x0800C3AC` | A battle: the controller (task `0x0802AC0C`, slot 4) until it reports its end; returns 0 for a win, 1 for a loss, 2 for a retreat |
| `0x0800B9CC` | A battle against a roaming enemy (entity state 4) |
| `0x08008D28(n)` | Story battle `n` (records of 36 bytes at ROM `0x67C0F4`), which some fifty events call; the first is in map 12 (`mq0160`, handler `0x080106C4`, flag `0x12B`) |
| `0x08008BFC(n)` | The same, fading the field in afterwards |

`0x080329E4(type, group, a, b)` sets the battle up at RAM `0x02005C70`:
- the type (bit 0 for a story battle);
- the formation;
- the song (`0x17`);
- the terrain of each side.

## Roaming enemies

The Zoid maps' enemies are map objects of behavior 1 and command 4. Their formations
come with the object states the game-state block keeps (see
[formats/save.md](formats/save.md)).

Entering a map whose area differs from the last one entered rebuilds the states
(`0x08006E4C`). Continuing a game clears the area first (`0x0800C0CC`), so the first
map entered rebuilds them too. For each map Zoid of the area, in map order, the next
slot is drawn a formation (`0x080328FC`):

1. The table at ROM `0x6838C8 + (area − 1) × 0x30 + column × 4` points at twelve
   36-byte formations. The column is the object's sprite byte.
2. A draw of 0–99 picks a rarity class: below 60 class 0, below 95 class 1, class 2
   otherwise.
3. A second draw picks one of the formations of that class (the first when none is).
4. The formation is copied to RAM `0x02004A70 + slot × 36`, outside the save. The
   object shows its leader's Zoid, the first byte of its record.

A formation names its leader in its first two bytes and up to six members in the
four-byte groups from `+4`. Each is a group and a record of the 28-byte enemy records
(`0xFF` for none). The leader's record is at ROM `0x67894C`, a member's at `0x67664C`,
both `group × 0x380 + record × 0x1C`. An enemy record holds the Zoid (`+0`), the pilot
(`+0x10`), the level (`+0x14`) and the reward (`+0x18`). The leader stands in slot 4,
member `n` in slot `n`.

Command 4 (`0x0800AA98`) runs the flight routine of the shy townsfolk (see
[field.md](field.md)) toward the player within two cells, and wanders farther away.

## Meeting

The step check (`0x0800AE7C`) finds an entity on the cell ahead. The two meet when all
of these hold:
- both have behavior 1;
- the player's command is 0;
- both cells are on the same level (attribute bit `0x1000`);
- `0x02000008` bit 1 is clear.

If the player stepped, it turns toward the enemy and the enemy away. If the enemy
stepped, the player must be standing or stepping and not on an exit's cell
(attribute bit `0x4000`); the enemy then stops. The player's entity state becomes 4.

The frame after the step, state 4 (`0x0800B9CC`) plays sound `0x52` and darkens the
field (`0x08001524`). The battle starts in the frame it is black, 33 frames after the
step. Each side's terrain is the low byte of the attribute it stands on; when the
player's cell has any other bit set, the enemy's is used for both.

## The screen

The screen is mode 0 with four text backgrounds and the units as sprites.

| Layer | Content |
|---|---|
| BG3 (`0x040F`) | The grid the formation screen shows too |
| BG2 (`0x420A`) | The two grounds, 15×18 tiles each, from the 24-byte records of ROM `0x66B72C` by terrain: palette, tiles and map (entries from `+0x3C`) of the party's ground, then of the enemy's. The party's is written at tile (15, 2) with palette bank 1, the enemy's at (0, 2) with bank 2 |
| Sprites | The units, in the picture the status screens show, at the 16-byte slot records of ROM `0x66B484` (the party's six, then the enemy's six); the enemy's are mirrored |
| BG1 (`0x0004`) | A panel per filled formation slot, six tiles apart, or five when all six are filled |
| BG0 (`0x0104`) | The windows |

A panel is the 6×4 map at ROM `0x66B882`:
- **Frame:** the window frame's tiles, loaded from tile `0x56`.
- **Name:** the top two rows, eight tiles the text system writes the name into.
- **Bars:** the last two rows, hit points and energy. Each bar's middle is four entries
  of the table at ROM `0x66B8B2` by level (plus `0x13` for energy), over the tiles
  LZ77-decompressed from ROM `0x36BB40` to tile `0x30`.
- **Levels:** 0 when full, 24 when empty, `24 − value × 24 / most` otherwise; a unit
  with anything left keeps a sliver.

BG1 is scrolled 16 pixels up, so only the bars show. L slides the names in (task
`0x0802F8F8`), which the port does not model yet.

Before building the screen, the battle computes the statistics of every unit in the
formation again (`0x0802B5D0` calling `0x08036CB0`). The hangar computed them before
the pilots' bonuses were set, so a saved game can hold lower maximum hit points than
the battle shows.

## Tasks

The battle runs as tasks, one step a frame, in slot order. A script call blocks its
task until it returns; the task goes on in the frame it does.

| Slot | Task | Role |
|---|---|---|
| 4 | `0x0802AC0C` | The controller: sets the sides up, waits for the opening, starts the results, darkens the screen and ends |
| 5 | `0x0802E9CC`, then `0x08035778` | The opening and the menu, then the results |
| 9 | `0x08003F70` | The message wait, armed by `0x08004018`: 1, 15, 30, 45 or 60 frames by message speed (ROM `0x664668`), or A at the slowest |
| 10 | `0x08004034` | The fade: a level a frame from its second frame, reporting its end on its 34th |

The windows and texts are scripts of the `battle-menu` and `battle-text` tables:

| Script | Content |
|---|---|
| menu 2 | The message window, window 1 at (0, 16) 30×4 |
| menu 5, 6, 7 | Present, draw and clear window 1 |
| menu 0 | The menu, window 2 at (10, 3) 10×12: 戦闘開始, 部隊編成, コマンド作成, ステータス, 退却 |
| menu 1 | Reset the text system |
| text 1 | 敵ゾイドに接触！！ |
| text 15 | を確認しました, after a Zoid's name (name `1 + zoid`) |
| text 16 | 戦闘態勢に入ります |
| text 13 | *name*たちは退却に成功しました |

## Timeline

Frames from the battle's start, the frame the field is black:

| Frame | What happens |
|---|---|
| 5 | The opening task starts song `0x16` and builds the screen in the dark: 41 frames with four panels (9, plus 8 a panel that names its unit) |
| 47 | The fade starts; the screen shows its first brighter level on frame 66 and full on frame 81 |
| 83 | The message window and 敵ゾイドに接触！！, then the wait |
| | Each enemy slot is checked in turn: 2 frames for an empty one; a filled one prints *zoid*を確認しました and waits |
| | 戦闘態勢に入ります, the wait, then the menu |

Choosing 退却 ends the opening in the next frame. The results task starts two frames
later:
1. Sound `0x53`, then the message and the wait.
2. The controller darkens the screen a level a frame from the fade's third frame.
3. The text system is reset, and the battle ends one frame after the fade reports.

Retreating always works (`0x08033DC8` returns 1). It marks every party unit `0x3000`,
and the result routine (`0x08032684`) reads that as result 4. `0x080364DC` writes the
units' hit and energy points back into the game state.

After the battle the field reloads where everyone stood (`0x08007E4C`). It stays
black for 14 frames and then brightens a level a frame (`0x080014A8`); what follows is
in [Back on the field](#back-on-the-field).

## The fight

Choosing 戦闘開始 engages: the panels scroll down from 16 pixels up by 2 a frame and
the grounds and their units come down 12 pixels, one a frame (`0x0802E9CC`,
`0x0802F010`), then the step waits 15 frames. The controller (`0x0802AC0C`) then runs
the fight:

| State | What it does |
|---|---|
| `0x3F2` | The engage step has ended: song `0x17` |
| `0x76C` | The end when a side is beaten, the round otherwise |
| `0x1B58` | The round's menu: `battle-menu` 6 and 7, `battle-text` 0x3B, `battle-menu` 5, then the menu, `battle-menu` 3 (0x19 in story battles); 退却 always works |
| 2000, `0x7DA` | The row advance (task `0x0802F09C`), 5 frames; a side whose front row is empty would move its back row up |
| `0x8FC`, `0x906` | The effects that ran out (`0x0802FD4C`); without any it reports on its second frame |
| `0x9C4` | The turn's order, which costs a frame more |
| `0xBB8`, `0xBD6` | The next actor (`0x0802AF70`): the turn's 16 rolls, its defense dropped, its status check (`0x0802FAA8`, 2 frames a slot) |
| `0xBEA`, `0xC1C` | It acts (the action task `0x0802E814`): the others are darkened, the message *name*は, then the party's menu (`battle-menu` 4: 攻撃, 防御) or the enemy's choice |
| `0xED8`, `0xEE2` | It can't act (paralysed, `0x4000`): `battle-text` 0x20 |
| `0x1194`, `0x119E` | It defends (`0x100`): `battle-text` 0x1A; it takes half the damage until its next action |
| 4000, `0x1004` | The screen fades out |
| `0x1068`, `0x10CC` | The attack scene (below): result 1 for its end, 2 for an aim given up |
| `0x1130`, `0x113A` | The aim was given up: the screen is built again while black, 18 frames and 8 a panel, the message window 2 frames before their end, then the fade in; the actor acts again from `0xBEA` |
| `0x1388`, `0x13EC` | Back to the screen (task `0x0802BCA4`) |
| `0x1770` | The beaten units leave; the next actor, the next round once all have acted (`0x0802F5C4`: the effects count a turn down, the round's flags clear), or the end |
| `0x2328` | The battle is over |

The screen's other changes:

- **Darkening** (`0x08031D50`): the units that don't act lose 12 of each color channel
  (24 when paralysed); the panels but the actor's take palette 14. The return's
  rebuild and the round's menu light them again.
- **Names** (`0x080339C4`): a unit's is its Zoid's, with a letter (`battle-text` 88 +
  its place among them) when its side has more than one of it. The panels' names are
  printed into window 0 and taken from there: the player's (`battle-text` 0x33) for
  character 0, name 154 + the character's otherwise.
- **The return** (`0x0802BCA4`): the screen is built while black, 20 frames and 8 a
  panel, the message window 3 frames before their end; it fades in over 35 frames, and
  2 frames later come the hit display and the messages. The hit display
  (`0x0802D36C`) plays sound `0x5A`, loads for a frame, then each hit unit sparks (the
  battle screen's effect 6, 8 pixels right of and 16 above it) and shakes by −2, 0, 2
  and 0 pixels three times. The messages (`0x0802C084`) give each target's name, then
  `battle-text` 2, the damage and 3 (0x25 and 0x26 for a critical hit, 4 for a miss, 6
  after a beaten one), each with its wait; a panel's bars follow its message. The
  display ends 7 frames after its sparks, the messages 5 after their last wait, the
  return 2 after both.

## Results

Once the fight is over the controller starts the results (task `0x08035778` in slot 5).
The battle's result (`0x0200EB84 + 0x21E8`: 1 or 2 a win, 3 a loss, 4 a retreat) picks
its messages; each ends with `battle-menu` 0x14, a wait for a key, but the retreat's,
which takes the timed wait.

| State | What it does |
|---|---|
| 0 | Sound `0x33` for a win, `0x53` for a retreat |
| 1000 | 敵ゾイドを全て倒しました！！ (`battle-text` 0xB); the money of the beaten enemies, doubled by the battle's flag `2`, into the game state (`+0xD28`, at most 9,999,999) with *n*Ｇ手に入れた (0xA); then the spoils' roll and, for a spoil, sound `0x35` |
| `0x834`, `0x898`, `0x8FC`, `0xA8C` | The spoil: an item (0x27, `name` 241 + id, counted at `+0x3305`, at most 99), a Zi-data item (0x28, the `item` table, `+0x330C`), a weapon (0x29, the part table, `+0x334C`, at most 9) or a Zi data (0x2A, the Zoid's name, 0x2B; with だが、そのＺｉデータは既に持っていた・・・, 0x2C, when the flag at `+0x33E2 + id` was set already); each with 」を手に入れた (9) |
| `0xBB8` | The experience, doubled by the flag `4`, into `+0xCD4` with *n*の経験値を得た (7); the party's level (`+0xCD2`) rises while it is below 99 and the experience reaches the table's entry for it (ROM `0x66BB58`) |
| 4000, `0x1004` | For each level, sound `0x34` and *name*達のＬＥＶＥＬが*n*になりました！！ (0x33, 8, 0x2E) |
| `0x1388`, `0x1392` | The points of the levels (task `0x080360A1`, below), then the other characters' bonuses by the level (`0x080368BC`: level × the growth table at ROM `0x66BB38`, into `+0xCE8`, `+0xCF8` and `+0xD08`) and the first four characters' units computed again (`0x08036960`) |
| `0x1B58` | The loss: *name*の部隊は全滅しました (0x33, 0xC) |
| 8000, `0x1FA4` | The retreat: *name*たちは退却に成功しました (0xD) |
| `0x2328` | The write-back (`0x080364DC`), and after a loss the player's recovery; then the end |

**The spoils** (`0x0803666C`, `0x0803680C`). A roll of the formation's leader and its
members with a record picks a unit; its enemy record gives the Zi data (its first byte)
and, by a roll of 3, one of its three weapons (a part where the pair's first half-word
is not 0 and the second not `0xFFFF`). The formation gives an item (`+0x20`) and a
Zi-data item (`+0x21`). A roll of 20 in the table at ROM `0x66BB10` (`0x66BB24` in
chapter 10) picks the kind: nothing, the item, the Zi-data item, the weapon or the Zi
data; a kind without a spoil gives nothing. A battle whose flag `1` is set always
rolls 0.

**The points** (task `0x080360A1`). Ten a level, shared out among the player's five
bonuses (`+0xCDC` 耐久力, `+0xCE2` 攻撃力, `+0xCE0` 防御力, `+0xCDE` 反応値, `+0xCE4`
命中値, at most 200 each). The statistics' window (`battle-menu` 0x16, window 3) shows
each bonus as it was and as it is (`0x08036270`); the menu (0x17, window 2, run by
0xF, a `0x36` menu in mode 6) takes a point for each A on a line. The message is
上げたい能力を選んでください（残り*n*）. Once the points are out, or every bonus is at
200, 能力の割り振りを終了します (0x31) and a key end it.

**The write-back** (`0x080364DC`). Each formation slot's unit takes its battle unit's
hit and energy points; a slot whose pilot has no battle unit is left with none, marked
`0x800` and taken out of the formation (`0x08037B1C`). After a loss the player's unit
(or the first free one) comes back whole and takes the formation's second slot
(`0x08037AB4`).

The game takes the game-state block back once the battle has handed over.

## Back on the field

The field reloads with the player and the enemy on their cells, their animations
restarted (`0x0800B9CC`). Until it is bright again both stand still, the player in
entity state 4 and the enemy in state 1. The frame it is bright the outcome takes
hold:

- **A win:** the enemy's object state loses bit 15, so it is gone for good, and the
  battle count at `+0x0A` is raised. The enemy is wrecked (entity state 5) and the
  player is free again.
- **A loss:** the player is wrecked, its animation shift (`+0x38`) set to 1.
- **A retreat:** the enemy stands still for 180 frames (entity state 8) before it
  chases again.

A wrecked Zoid (`0x0800BC8C`) plays its facing's animation once from the start, with
the animation shift it had. The traced enemy was walking when met (shift 1): its
animation took 16 frames; one met standing takes twice as long. It then turns into
sprite `0xFD` facing up (`0x080089A0`), and sound `0x5A` plays. The explosion is
fourteen steps of 6 ticks and a last one of 32, an empty 8×8 frame. Once it has played
the Zoid is gone. Pressing A at a wrecked Zoid does nothing.

The objects update before the player: two tasks, `0x0800BDE0` for the objects and
`0x0800BDA4` for the player, each running an entity's state and then its animation.
So in the frame the outcome takes hold, the enemy's animation has already stepped
when it restarts, and the player's steps once more after its own.

After a loss, the party is taken to its area's return point once the player's
explosion has played:

1. The screen darkens a level a frame from the frame after next (`0x08001524`).
2. Once it is black, the map loads (`0x08006E08`, `0x08007188`). The return point is
   record *area − 1* (the game state's byte 3) of the 23 at ROM `0x328D6C`, 8 bytes
   each: a map, a column and a row, in half-words. The world map's is map 27 at
   (8, 8). The player arrives facing up.
3. The map brightens a level a frame, 13 frames after it has loaded.

Frames from the one the field is bright again:

| Frame | Win (the traced enemy) | Loss |
|---|---|---|
| 0 | The enemy's animation restarts | The player's restarts, a frame in |
| 16 | | The explosion and its sound |
| 17 | The explosion and its sound | |
| 74 | | The player is gone |
| 75 | The enemy is gone | |
| 76 | | The first darker level |
| 106 | | Black: the return point loads |
| 120 | | The first brighter level |
| 150 | | Full brightness |

## Rules

The port keeps the rules of a fight in `crates/game-core/src/combat/units.rs`,
`crates/game-core/src/combat/attack.rs` and `crates/game-core/src/combat/ai.rs`; the
fight plays them (see [The fight](#the-fight)).

### Units

A unit in battle is 0x2CC bytes at `0x0200EB8C + side × 0x10D8 + slot × 0x2CC`.

- **A party unit** is a copy of its record in the game state (`0x0802B5D0`, `0x0802B6E0`).
  Its full hit points include the pilot's 耐久 bonus. Its energy, SP and DF are computed
  again without the pilot (`0x08036CB0` with character `0xFF`); each turn adds the
  pilot's other bonuses.
- **An enemy** is built from its record (`0x0802B728`, `0x0802B978`, `0x0802B9DC`,
  `0x08036EF4`). The Zoid's record gives the traits, the size, six part slots and the
  statistics; the enemy record's three parts replace the first three slots. Its
  pilot's record is taken by area (ROM `0x67B35C`). The passive parts add hit points,
  energy, SP or DF, the pilot's 耐久 adds to the hit points, and it starts at full. It
  also brings its way of choosing (`+0x11`), its experience (`+0x14`) and its money
  (`+0x18`).
- **Weapons.** A unit's weapons are its parts by slot (`0x0802BA24`), with each part
  record's flags, accuracy, power (16.16), cost, reach and spread. A passive part
  (`0x20000000`) is no weapon: it adds its beam defense or its traits (`0x0803376C`).
  The beam defense starts at half the DF.

Each turn (`0x08032CA4`), the statistics are built in three steps:
1. The effects the unit is under change them, first by amounts, then in percent.
2. They are clamped at zero.
3. The pilot's bonuses raise them in percent:
   - 防御 raises the defense and the beam defense;
   - 反応 the speed;
   - 攻撃 each weapon's power;
   - 命中 each weapon's accuracy.

An effect can make a unit ignore its pilot. Each of the 48 effects (`0x08032B54`) holds:
- what it changes, and whether it raises or lowers it;
- the amount;
- whether it is a percent or an amount, on the unit or on its pilot;
- the turns it has left;
- the part that caused it.

### Attacks

The chance to hit (`0x08034500`) is the weapon's accuracy less the target's evasion
bonus and a hundredth of its speed, clamped to 50–99:
- a flying target is 20 harder to hit, unless the weapon is anti-air (`0x200`);
- a swimming one is 20 harder on water (terrain 6);
- some weapons and states make it sure or impossible.

The damage (`0x080345EC`) is the weapon's power less the target's defense in percent.
The defense is the beam defense against beams (`0x30`), capped at 65:
- a critical hit adds half the power and ignores the defense, as piercing weapons do;
- a defending target takes half;
- any hit deals at least the smallest amount.

`0x08033E40` applies an attack:
1. The weapon's energy cost is paid.
2. Target `k` is hit when roll `15 − k` of the turn is at most the chance to hit, or
   always when the chance is 100 (sixteen rolls of 0–99 drawn when the actor's turn
   starts, `0x08033D94`).
3. A hit is critical when its roll is also below `(bonus >> 1) + 2`, twice that and
   at least 10 for a keen weapon (`0x80`), never on a target with trait `0x200` from
   chapter 6 on.
4. A target left without hit points is beaten (`0x400`); it leaves the battle when the
   actor's turn ends, and its experience and money go to the party's reward.

### Turns and the enemies' choices

- **Order.** The units still fighting act by speed, fastest first (`0x08032410`,
  `0x08032564`, a stable insertion sort, the party's before the enemy's). Two battle
  flags make it slowest first or shuffled.
- **Reach.** Rows are 1 apart front to front, 2 between a front and a back row, 3 back
  to back (`0x08038F10`). A weapon's reach is a range of those distances
  (`0x08038F60`).
- **Spread.** A weapon takes one slot, a slot and the one behind it, a column of three,
  a square of four or the whole side (`0x08038B00`).
- **Choice.** The enemies' first way of choosing (`0x0805959C`):
  1. One time in two it looks for support worth giving. Restoring an ally that lacks
     four fifths of both its hit points and its energy comes first, then repairing one
     that lacks two thirds of its hit points, then other support an ally is not under.
  2. Otherwise it picks, at random, one of its weapons that reach the party, and one
     of its groups.
  3. With nothing to use it defends.
- **The other fifteen ways of choosing** (the table at ROM `0x75C048`) are not modeled
  yet.

The port's units, their statistics, the order and the three attacks of the traced
battle matched the original's RAM: 20 hit points on a Command Wolf, 23 and 24 on the
Gator and the Iguan, then 19 on the Iguan.

## Attack scenes

An attack plays as a scene (task `0x0802BC99` → `0x08042348` in slot 5, with the
scenery's task `0x08043FF8` in slot 6), the same module the opening's scripted scenes
run ([battle.md](battle.md)). The attacker's view comes first: its Zoid slides in, its
pilot speaks and it fires. Then each target's view follows: the shots land and its
pilot reacts.

### Records

The scene builds a 28-byte record for every unit (`0x08041AA8`, `0x080418B0`, at EWRAM
`0x0200D920`, party then enemy):

| Offset | Content |
|---|---|
| `+0` | The Zoid |
| `+2` | Its size class (the unit's `+0x35`), 0 when above 2 |
| `+3` | The scenery kind: its side's terrain by `0x080419CC` (0→1, 1→5, 2→8, 3→4, 4→2, 5→12, 6→3, 7→10, 8→6, 9→13, 10→16, 11→11, 12→14, 13→15); a flying unit takes the sky, 7, but over kinds 8, 10, 11, 15 and 16 (`0x08041A54`) |
| `+4` | The six parts, 0 for none or for a slot whose own place on the Zoid's picture is 0 |
| `+0x15` | The pilot: its record's byte 2, the portrait and the `battle` strings |
| `+0x16`, `+0x17` | The side and the slot |

A view shows scenery `kind × 3 + size`. Its image comes from the loader entries at
ROM `0x6F6934` (tiles) and `0x6F6BBC` (palette), the Zoid's from `0x6F8974` and
`0x6F9100`. On the party's side both images show as stored, the Zoid at the right; on
the enemy's side both are mirrored, the Zoid at the left. The weapons of the first
three racks show on the Zoid as sprites (`0x08045678`): racks 0 and 1 in front of it,
rack 2 behind.

### States

| State | What it does |
|---|---|
| 0 | Builds the records and the screen, and starts the scenery's task |
| `0x1050` | The attacker's view (`0x08042570`): scenery, Zoid off the screen, mounted weapons, `system` script 0xB (the pilot's window with its portrait, the message window) |
| `0x1000`, `0x1002` | Fade in: the brightness from 16 down by 2 a frame (`0x08042100`, `0x08042128`) |
| `0x1051` | The slide (`0x08044FA0`): the Zoid comes in from 176 pixels off the edge, `0x2000·t²` in 16.16, and arrives on the 39th frame; for the party outside the story's battles, the player's aim follows (`0x1052`, `0x1054`) |
| `0x1010` | Waits 30 frames (90 in story battles), clears the message window (`system` 0x12) and runs the pilot's line; the attack then applies (`0x08046918`, `0x08033E40`) and the shots spawn |
| `0x1070`, `0x1090` | The shots play until none is left |
| `0x1005` | The view holds 30 frames (after a reaction 50, 180 in story battles) |
| `0x1001`, `0x1003` | Fade out: the brightness up by 2 a frame, the shots' blend fading with it (`0x08042078`, `0x080420A0`) |
| `0x10B0` | The next target (`0x08042154`), or the end |
| `0x1060`, `0x1061` | The target's view, faded in and slid in the same way; the hit shots spawn once its Zoid has arrived |
| `0x10A0` | The target's pilot reacts, the shots' animations paused meanwhile |
| `0x2000`, `0x2010` | The end: the screen goes back to the battle's (result 1, or 2 after an aim given up) |

Holding A skips ahead. In the shots' states, holding it counts down 60 frames (35 on
a target's view) and then ends them; in the hold it counts down what is left of those
first. Story battles cannot skip the shots.

A view's load (the images, the text system's reset) stalls the main loop. On the
traced battle, the scenery's task ran on the setup's 3rd frame and from the 10th on
for the attacker, and on the 2nd and from the 9th on for the target. The port counts
those frames; the load time may differ for other images. Spawning a view's shots
costs one frame more when their sprites are heavy: it did for 4128 bytes of tiles and
not for 1920, and the port draws the line at 4096.

### The player's aim

When the party attacks outside a story battle, the attacker's view does not go on to
its line: once the Zoid has arrived (state `0x1052`, `0x080426D4`) the scene loads the
aim's sprites and starts task `0x08045BF8` in slot 8, then waits for it (`0x1054`,
`0x08042750`). The load costs the next frame. The weapon's cursor starts on the weapon
the unit aimed last, when it is still there (EWRAM `0x0200E24E`).

| Task state | What it does |
|---|---|
| 0 | Sound `0x64` |
| 9 | Once the cursor stands on the weapon, the prompt 選択して下さい。 (`system` 0xC) and the weapon's window (0x10) |
| 1 | Up and down move to the previous and next slot with a part the scene shows (`0x08047DD8`, `0x08047DAC`), sound `0x40`; the cursor flies there, and once it stands on it the window shows the weapon: cleared (0xD), its name (the part table), its power, accuracy, reach and cost (0x11), shown (0xE). A takes a weapon the unit has the energy for (else sound `0x4F` and ＥＰが足りません, 0x14); B gives up, sound `0x3F` |
| 100, `0x65` | The Zoid's fade comes back (below), 13 frames; then 2, or 7 for B |
| 2 | The weapon's side and shape. A weapon for its own user, or for everyone, takes its targets at once (sound `0x42`, state `0xE`). Otherwise the window closes (0xF), the icons of the side's units load, and the grid opens on the shape's first group with a unit (sound `0x43`, 31 frames); a weapon that reaches none refuses, sound `0x4F` and 距離が近すぎます or 距離が遠すぎます (0x13, state 200) |
| 3 | The arrows move between the shape's groups (sound `0x48`); A takes the group (sound `0x42`); B closes the grid (16 frames) and goes back to the weapons |
| 4, 5 | The figures hide; once the cursors have stopped, the chosen ones turn red, the grid closes (32 frames) and the weapon's cursor ends its animation |
| 6, 7 | The result: 1 with the weapon and the targets (`0x0200DA80`), 2 for B |

The weapon's window gives the power (the turn's, rounded, at most 999, －－－ below
1), the accuracy (at most 99, －－－ for support), the reach (`system` 21 by the
weapon's reach code `+0xF`) and the energy cost. With the scene's result 1 the
attacker speaks and fires (`0x1010`); with 2 it fades out and ends (`0x2010`).

**The grid.** A weapon's reach code and whether it is a fixed weapon (slot 3 on) give
its shape (`0x0804593C`): single cells, a cell and the one behind it, a column, a
square, the whole side, or one column only. Each shape tries its groups in the order of
ROM `0x6D43BC` and starts on the first with a unit (`0x08047310`). Up and down go round
a column, sideways to the other column's same row first, then its others from the top
(`0x08047754`); a move only takes a group with a unit (`0x08047184`). The grid is entity
23 (effect 151), whose animation lights the group (`0x080459F4`). The icons are the
units' status pictures (ROM `0x670210 + zoid × 0x4C + 0x30`) at ROM `0x6D4358`; the
party's grid shows its side mirrored (ROM `0x6D4370`).

The grid opens in a loop of its own (`0x08045358`): 15 frames in which it turns and
grows, its ratio going from 4.75 to 1 (ROM `0x6D439C`) and its angle a sixteenth of a
turn a step, then 16 in which the icons grow to 1/1.625 of their size (ROM
`0x6D437C`). It closes by the icons alone, a step every frame after B or every two
frames after A.

**The cursors.** Seven entities of effect 154: the weapon's cursor (`0x080492DC`) flies
there on a line, three pixels a frame, and tells the task once it is there. Six locking
cursors (`0x08049560`) follow it one frame behind each other. A group sends one to each
of its cells with a unit, from the seventh down (`0x080469D4`); each flies there ten
pixels a frame and stops 25 pixels above the icon, then its percent (effect 152, whose
steps are the numbers) and the unit's hit points count up: 8 percent a frame, and a
sixteenth of the hit points (at least 1). The hit points are four digits built in
memory (ROM `0x6D4724`, `0x6D4778`), `????` for a unit that can't be hit critically.

**The Zoid's fade.** The weapon behind the Zoid (slot 2) makes the Zoid's layer blend
over the scenery (`0x080481D4`): 12 frames from the whole Zoid to a quarter, the front
weapons semi-transparent. Leaving that weapon, and the 13 frames of states 100 and
`0x65`, bring it back.

**Frames the load and the text take.** The trace showed three places where the work
runs past the frame: the aim's sprites' load (the whole scene loses the next frame),
the weapon's figures (0x11: the scene's tasks lose the next two frames, the window shows
on the second), and the window's closing (0xF: the other windows' redraw; the scene
loses the second of its two frames, and the window goes at its end). The port times
those as measured.

### Lines

The pilot's line is `battle` string `pilot`, the reaction `battle` string 86 + `pilot`
(ROM `0x755E88`). Variable 3 picks among the string's lines: `(roll >> 1) % 3` from the
turn's first roll (`0x080421FC`), 4 for a missed target and 3 for a destroyed one.
Some weapons have their own lines (`0x08042218`: parts `0xF2`, `0x149`, `0x14F`, `0x161`,
`0x224`, `0x22A`, `0x236`, from ROM `0x756090`), and so do units with a special pilot
(`+0xCE`, ROM `0x755FE0`); neither is modeled yet.

When a pilot's expression changes, the portrait disappears for a frame while the
game loads its tiles, then the new one shows.

### Scenery

Each frame the scenery's task:

1. Moves BG2 by the kind: `0x1000` a frame (`0x4000` for kind 3, 3 pixels for the sky),
   to the right on the enemy's side and to the left on the party's.
2. Runs the scenery's line routine (the jump table at `0x080441B8` by scenery − 3).
   Most add an amount to bands of lines each frame, with a ramp for the ground; the
   routines of sceneries 9–11, 15–17 and 33–35 set their bands from the wave table at
   ROM `0x6D3B94` instead.
3. Writes the table the `HBlank` handler reads: BG2's scroll plus each line's offset
   (minus on the party's side), and BG2's scroll alone for lines 144–159.
4. Sets BG1's scroll: the slide's position, 112 pixels further for the party's side,
   with the shake.

The sky's Zoids bob by the wave table.

### Shots

A weapon part's 16-byte record at ROM `0x6E51BC` names the animations:

- `+0`: the attacker's view;
- `+1`: the target's (its misses from the table at `0x6E8F64`);
- `+2`, `+3`: the sky's;
- `+4`: the back rack's;
- `+6`–`+12`: offsets added on the rack.

An animation is a 0x110-byte record of ROM `0x6D718C` with up to 18 sprites. Each gives:

- a shot sprite (effects table entry 154 + id);
- a place, on the weapon's rack or on the screen;
- flags: animated, looping, semi-transparent, set off at once, a link step, spreads
  by the Zoid's size (ROM `0x6E8A88`), a screen shake, behind the Zoid;
- a behavior's value, a delay, the animation, and a sound.

The sprites are entities of the sprite system. Once set off, a sprite waits its
delay, then shows, starts its shake and plays its sound. Its behavior (the routines
at ROM `0x6D4784`) then moves it and sets off the next one:

| Behavior | What it does |
|---|---|
| 0 | Shows for its value's frames |
| 1 | Sets off the next at its link step; goes when its animation ends |
| 2 | Sets off the next when its animation ends |
| 10 | From its first frame, sets off the next on its link step and the sprite its value's low byte names on the step its high byte gives; goes when its animation ends |
| 13 | A bullet flying in at 24 pixels a frame; past its mark it restarts the sprite it names |
| 14 | A bullet that jumps a screen back and flies in to its place, then sets off the sprite it names |
| 17 | Drifts by its value until its animation ends, setting off the next at its link step |

The five screen shakes (`0x08044B34` and after, tables at ROM `0x6D4168`, `0x6D41E8`,
`0x6D4218`, `0x6D4188` and `0x6D4208`) move the Zoid's layer frame by frame. Kinds 1
and 4 set it and return it to its place; kinds 2, 3 and 5 push it back for good.

The semi-transparent sprites blend 15/16 of their color with 8/16 of the layer below
(`BLDALPHA` `0x080F`).

## Checked against the original

The port's battle screen was compared with the original's on every frame of the
traced battle, from the start to the menu and from the menu's 退却 to the end. It
matches but for the two frames the menu appears on. The comparison lets through
differences of one unit in a single channel, as the reference emulator's darkening
rounds red up by one.

The attack scene of the traced battle's first turn (the Iguan's flamethrower on a
Command Wolf) was compared on every frame, from the battle screen's fade out to the
scene's end. The task's 26 state changes and its nine sounds fall on the same frames
as the original's. 251 of the 481 frames are identical under that tolerance. The
others differ in two ways:

- a band of the scenery is one pixel off in the lines above the point the original's
  rewrite of the scroll table reached (see Differences);
- inside the semi-transparent shots, by a few units in a channel, as the reference
  emulator blends in finer steps than the hardware's 5 bits.

The fight's controller and its tasks were compared on the traced battle's first turn,
the Iguan's attack: the states, sounds and messages fall on the same frames, and the
return to the screen is identical.

The results were compared on every frame from a save state taken as the original's
fight ended, and with the party's experience set a little below the next level: the
messages, sounds and scripts fall on the same frames, and so do the points' screen,
its menu and its twenty choices, and the game state's level, experience, money,
bonuses and units the results leave. The screens are identical but for the spoil the
different rolls gave and two frames of the points' screen (below).

The player's aim was compared on every frame from the traced save state of the
party's turn, choosing the first weapon and the first group, and again giving it up
with B. The scene's and the task's states, the sounds and the window scripts fall on
the same frames as the original's through the target's view. The windows, the
prompt, the figures and the cursors are identical; what differs is the scenery's
rewrite line (below), 8 to 15 pixels of the grid while it turns, and the back rack's
laser, whose second shot the original draws a few frames earlier. Giving the aim up is
identical from the fade out to the menu, but for the panels of the units the save
state's first turn hit.

In the game, the step, the sound, the darkening, the battle's start, the song, the
retreat's sound, the end and the field's return fall on the same frames as the
original's. The enemies themselves differ, as the formations are drawn at random.

Back on the field, the port was compared with the original's saved fight, won and,
with its result set to a loss, lost. The port met an enemy on the same cells of the
world map, and fought until it won, and again with the party's hit points set to 1
until it lost. The lost battle's return is identical to the original's on every
frame, from the field's reappearance through the explosion, the darkening, the
return point's load and its brightening, but for the enemy's own sprite, another
Zoid. The enemy the port beat had been standing, so its animation ran at half the
traced one's speed: each step of its explosion is identical to the original's.

## Differences

- The menu window appears a frame early. Presenting it cost the original two frames
  (the CPU time of its five lines of text), as happens in the pause menu.
- Closing a window costs one frame in the battle's scripts. The pause menu's windows
  took the original a second frame to redraw.
- The random draws do not match the original's, so the enemies' formations and paths
  differ.
- The map's song plays again as soon as the battle hands back; the original restarts
  it once the map has reloaded.
- The points' screen: the original's text takes it about two lines of the statistics
  a frame and its first presenting a frame more, which the port times as measured; the
  menu's window and the statistics' first show a frame early. The original's `0x36`
  menu returns in the key's frame; the port's a frame later, which the screen makes up
  for by going on at once.
- The frames the aim's window work takes are the ones measured: once the player had
  looked at the weapon behind the Zoid, the window's closing took the original one
  frame rather than two, which the port does not follow.
- The grid turns with the BIOS's sine table rounded as the port computes it; a few
  pixels of its edges differ while it turns.
- The attack scenes' scenery: the original rewrites the scroll table while the frame
  is drawn, and the line the rewrite has reached by then moves with the frame's CPU
  load (lines 16 to 93 in the traced scene). The port takes line 16 throughout, so a
  band that moves a pixel in a frame can show it some lines early.

## Not modeled yet

- In the results: the battle's flags (`0x0200EB84 + 2`: no spoils, double money,
  double experience), the training a unit gains when `+0x21E0` is set, the link
  battles' results (flag `0x20`), and a new unit for a party without one after a loss.
- The row advance and the expiry of effects (their fixed frames are).
- The enemies' other fifteen ways of choosing; items and deck commands; the battle
  menu's other lines, which show the menu again.
- In the attack scenes: the shot behaviors other than 0, 1, 2, 10, 13, 14 and 17 (they
  play as 2), the palette flash of a critical hit, support weapons (their own-side
  flows and displays), the view that skips the attacker (`0x0200EB84 & 3 == 1`), the
  weapons' and the special pilots' own lines, and the story battles' scripted aim.
- In the aim: the front weapon's hiding while the Zoid fades (the mount's `+0x4A`),
  and the entities the back rack's weapon moves when it fires (`0x08042780`).
- 部隊編成, コマンド作成 and ステータス from the battle menu.
- L showing the panels' names.
- Story battles (`0x08008D28`).
