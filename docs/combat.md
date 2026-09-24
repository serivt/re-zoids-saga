# Battles

Source of knowledge: own reading of Zoids Saga (Japan, Rev 1) at the addresses named
below, with breakpoints on the script runner (`0x0803E51C`), the task spawner, the
sound call and the battle setup, write watchpoints on the entity table and the game
state, VRAM, palette RAM and RAM dumps, and screenshots of every frame of a battle
met on the world map in a reference emulator: from the step that met the enemy to
the menu, and from the menu's 退却 to the field's return. Implemented in
`crates/game-core/src/combat.rs` (the battle), `crates/game-core/src/objects.rs`
(the roaming enemies' formations), `crates/extraction/src/saga_encounter.rs` and
`crates/extraction/src/saga_combat.rs` (data).

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
black for 14 frames and then brightens a level a frame (`0x080014A8`).

- **A beaten enemy:** its object state loses bit 15, so it is gone for good, and the
  battle count at `+0x0A` is raised.
- **After a retreat:** the enemy stands still for 180 frames (entity state 8) before
  it chases again.

## Checked against the original

The port's battle screen was compared with the original's on every frame of the
traced battle, from the start to the menu and from the menu's 退却 to the end. It
matches but for the two frames the menu appears on. The comparison lets through
differences of one unit in a single channel, as the reference emulator's darkening
rounds red up by one.

In the game, the step, the sound, the darkening, the battle's start, the song, the
retreat's sound, the end and the field's return fall on the same frames as the
original's. The enemies themselves differ, as the formations are drawn at random.

## Differences

- The menu window appears a frame early. Presenting it cost the original two frames
  (the CPU time of its five lines of text), as happens in the pause menu.
- Closing a window costs one frame in the battle's scripts. The pause menu's windows
  took the original a second frame to redraw.
- The random draws do not match the original's, so the enemies' formations and paths
  differ.
- The map's song plays again as soon as the battle hands back; the original restarts
  it once the map has reloaded.

## Not modeled yet

- The battle itself: 戦闘開始, the commands, aiming, the attacks and their scenes,
  damage, the enemies' actions, experience, money and levels. The menu's other lines
  show the menu again.
- 部隊編成, コマンド作成 and ステータス from the battle menu.
- L showing the panels' names.
- Losing a battle, and story battles (`0x08008D28`).
