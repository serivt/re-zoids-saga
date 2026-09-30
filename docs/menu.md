# Pause menu

Source of knowledge: screenshots and RAM dumps of Zoids Saga (Japan, Rev 1) in a
reference emulator while every item of the START menu was visited from the first room
(`research/build/mgba/menu/`) and, with the party formed, from the eastern labyrinth,
a read of the status screens' routines named below, a watch on the interpreter's current-window pointer
(RAM `0x02009118`) while the menu opened, and a read of the pause-menu script table
and of the menu, present, draw and message opcode handlers. For the parts pages:
breakpoints on the script runner, the rack and part printers and the sound call while
every page of the four members' Zoids was shown, screenshots frame by frame, and a read
of the printers named below. For the weapons list and the equipment screen: the same
traces while the Shield Liger's laser was taken off, moved to another rack and listed,
write watchpoints on the stock and the unit's parts, and dumps of VRAM, OAM, palettes
and the display registers on the part lists. For the formation screen: a read of its
task (`0x08037B84`) and of the routines named below, breakpoints on the script runner,
the sound call and the loaders while units were taken out, placed and the list paged,
on the party the labyrinth leaves and on one with more members, a broken unit and an
L unit; VRAM, OAM and palette dumps, and screenshots of every frame compared with the
port's. For アイテム: a read of the menu task's states `0x2000`–`0x2100`
(`0x08050374`–`0x08050EA4`) and of the routines named below, breakpoints on the script
runner and the sound call while items set by hand in a save were listed, refused and used,
and screenshots of every frame compared with the port's. For the Zi data lists: a read
of the menu task's states `0x1400` and `0x1500` (`0x0804FB48`, `0x0804FE28`) and of the
list builders named below, and screenshots of every frame compared with the port's while
Zi data and Zi-data items set by hand in a save were listed and paged. Saving is described in
[formats/save.md](formats/save.md). Implemented in
`crates/game-core/src/menu/` (`formation.rs` for the formation screen); data in
`crates/extraction/src/saga.rs` (`PAUSE_MENU_SCRIPTS`, `PART_NAME_SCRIPTS`,
`pause_wallpaper`), `crates/extraction/src/saga_party.rs` (`unit_parts`, `part`,
`join_formation`, `leave_formation`) and `crates/extraction/src/saga_formation.rs`.

The towns' shops are built from the same scripts and wallpaper; see [shop.md](shop.md).

## Scripts

The menu is not one script but a table of 698 short ones at ROM `0x75B1BC` (the
`menu` string table of the text dump is its tail) that the game's own code runs one
after another, printing the numbers itself between them. The ones this port uses:

| Script | Content |
|---|---|
| 46 | Opens the help line, window 0 at (0, 14) 30×6 tiles, and the list, window 3 at (0, 0) 9×14 (light menu), with ステータス / アイテム / 武装 / 部隊編成 / コンフィグ / セーブ |
| 64, 65–67 | The party panel, window 2 at (11, 0) 19×8, and its labels 達のレベル：, 経験値：, 次のレベルまで： |
| 44, 45 | The money box, window 1 at (18, 10) 12×4, and the Ｇ after the amount |
| 47 | 項目を選択してください in the help line, present all, menu |
| 48, 49 | The status list, window 4 at (9, 0) 14×14: 部隊 / キャラクター / 武器 / Ｚｉデータ / Ｚｉデータ用アイテム / 図鑑, then the question and its menu |
| 68, 69 | The unit list: the help line, window 1 at (0, 0) 30×16 with the ゾイドＨＰ／ゾイドＥＰ header, then 配置なし per empty slot (six slots) |
| 70–78 | The character screen: window 1 at (0, 0) 18×14, the portrait window 2 at (1, 4) 8×8 and the member list window 3 at (17, 0) 13×14; the labels 耐久／攻撃／防御／反応／命中, the help for a boarded Zoid (76, 77) or for leaving (78) |
| 35 | The member list's menu: opcode `0x36` with `0x12` (see [formats/script-text.md](formats/script-text.md)), then store the variables |
| 335 + n | Character `n`'s portrait in window 2 |
| 80–90 | The Zoid status screen: the help line and window 1 at (0, 0) 30×14 (80), the labels ＨＰ／ＥＰ／ＳＰ／ＤＦ／訓練度 (81–85), ／ (86), 戦闘不能 (87) and the sizes ［Ｓサイズ］／［Ｍサイズ］／［Ｌサイズ］ (88–90) |
| 91–93, 98–100 | The parts pages: windows 1–3 at (0, 0), (0, 3) and (0, 6), 30×8, opened with ウエポンラック１–３, or cleared and drawn again with 固定武装１–３： |
| 94–97 | 攻撃：, 　命中：, 消費：, 　射程： |
| 101, 102 | The pages' help: Ａボタン：次へ／Ｂボタン：キャラクター選択に戻る, and on the last page Ａボタン・Ｂボタン：キャラクター選択に戻る |
| 164–174 | Ranges １－１, １－２, １－３, ２－２, ２－３, ３－３ and reaches 単, 貫, 広, ブ, 全 |
| 175–205 | A support part's labels: 効果：, the effects (ＤＦ＋, 対レーザー＋ in a narrow and a wide form, 回避＋, ＳＰ＋, 命中＋, ＨＰ 回復, ダメージを無効化, 属性付加 with 水中 or 砂漠, 全能力＋５０％, ＨＰ上限＋, ＥＰ上限＋), 消費：, 目標： 味方／自分, 時間： with 戦闘終了まで, ターン, 一瞬 or 装備している間, and 制限：なし／制限：戦闘中１回のみ |
| 207–218 | 特殊効果なし, the special effects ＤＦ無視, 命中率低下, キャラ無効化, マヒ, then ラックなし, 装備なし and the rack kinds ［攻　］, ［　防］, ［攻防］, ［固定］ |
| 39, 41–43 | ％, ：, a full-width space and ・, which the game's code prints between its values |
| 0–7, 24–32 | Clear window `n`; present every window (24); draw window `n − 25` |
| 56, 57, 58, 60 | The notices for no items, weapons, Zi data or Zi-data items: clear the help line, print, present it, wait for a key, clear |
| 103–107, 206 | The weapons list: window 1 at (0, 0) 17×14 and window 2 at (16, 0) 14×14, the labels 　攻撃：／　命中：／　消費：／　射程： and 特殊効果： |
| 422 + n | Part `n`'s description, which the weapons list shows in the help line |
| 108–112 | The Zi data list: its help line and windows, 必要金額：, Ｇ with 必要ゾイド： on the next line, Ｚｉデータ用アイテム： and なし |
| 219 + n | The special Zoids a development may ask for (Zoid byte `0xFA + n`) |
| 55, 635 + n | The Zi-data items list's help line and windows, and item `n`'s text |
| 128–132 | The equipment screen: windows 1 and 3 as on the character screen, 搭乗ゾイドなし, and the indented ウエポンラック lines under the Zoid's name |
| 133–135 | 誰が搭乗しているゾイドの武装を変更しますか？, the notice that the character is not aboard, and 「…は装備を変更されたくないようです」 after the character's name |
| 136–139 | The rack list: window 4 at (1, 4) 18×8 and the numbers １–３ |
| 140, 141 | ウエポンラックが無いので選べません and 固定武装なので取り外しできません, each with its key wait |
| 142, 147 | The rack's parts: window 2 at (16, 4) 14×10 and window 1 at (16, 0) 14×4 with ウエポンラック, and 装備を外す |
| 149, 150, 61 | 外そうとしている, 「…は」「これ以上ストックできません。捨てますか？」 and the はい/いいえ window |
| 40, 217 | × before a stock count, and the blank kind of a slot without a rack |
| 151, 152, 153, 154–158 | The message-speed setting: window 4 at (9, 0) 15×4 with 戦闘メッセージ速度 and the value, the help text, window 5 at (23, 0) 7×14 with １–５ and ボタン, the menu, then the value strings |
| 160, 61, 161, 162 | セーブしますか？, the cancelable はい/いいえ window 7 at (11, 4) 8×6, セーブしました, セーブを中止しました |
| 115, 116 | The encyclopedia choice, window 5 at (23, 0) 7×6 with ゾイド / キャラ |
| 63, 37 | まだできてません and a cancelable key wait, which this port shows for the screens it lacks |

The panel prints the player's name before the level label and right-aligns the numbers
with full-width digits so the value ends in the panel's last inner cell; the money box
right-aligns the amount before Ｇ. The experience to the next level comes from the
99-entry table at ROM `0x66BB58` (entry `n` is what level `n + 1` needs: 14, then
7·n³). A new game shows level 1, 0 experience, 14 to the next level and 0 G.

## Party data

The game keeps its state in a block at RAM `0x02000B5C`, the one its save holds (see
[formats/save.md](formats/save.md)). The panel prints the party's level (`+0xCD2`),
experience (`+0xCD4`) and money (`+0xD28`), and the player's name is at `+0xD18`; the port
keeps these in `Party`, writes them into the block before a battle, which adds its
money and experience there and raises the level, and takes them back when it ends. The
status screens read the rest from the block when the menu
opens (`Roster`, built by `crates/extraction/src/saga_party.rs`):

- the members are the characters whose flag word has bit `0x02`, in character order
  (`0x0804E34C`);
- each member's bonuses are fields 4, 10, 8, 6 and 12 of its pilot record, shown as
  耐久, 攻撃, 防御, 反応 and 命中 (`0x080334F8`);
- the unit list follows the six formation slots at `+0x3600`, each the unit and its
  pilot.

Names come from the `name` table: character `n` other than the player is entry
`154 + n` (`0x08032818`), Zoid `z` is entry `1 + z` (`0x08032800`). The port runs them
as scripts of that table, so a translation covers them.

## Status screens

The unit list (`0x0804EC2C`) prints, per formation slot, the pilot's name, then from
cell 11 the unit's current hit points in four cells (or 戦闘不能 when the Zoid record's
first half-word has bit `0x800`), ／, the full value in four cells, a space, and the
energy points the same way in three cells.

The character screen (`0x0804ED5C`) shows the member under the cursor: name, the five
bonuses (sign at cell 11, the value right-aligned to cell 14, ％), portrait, and in the
help line the Zoid it pilots (76, the Zoid's name, 77) or 78 when it has none. The
member list shows six names a page. Its menu, script 35, ends on every cursor move with
the new line, and the game redraws the left side for that member; L and R turn the page
(sound `0x40`), A on a member with a Zoid opens the Zoid status screen, B leaves (sound
`0x3F`). Before either screen the game closes windows 4 to 1 and clears the help line
(`0x0804EBDC`).

The Zoid status screen (`0x08052724`) prints the Zoid's name and size class (the unit's
byte `+0x35`: script 88 + size), then hit and energy points (four cells each), SP (four
cells), DF (three cells and ％) and training (three cells), and draws the Zoid's picture
anchored at (40, 88) (see [formats/sprite.md](formats/sprite.md)). A key wait follows:
A shows the parts pages, B returns to the character screen (sound `0x3F`).

## Parts pages

A unit has six part slots: three weapon racks, then three fixed weapons. The pages show
one slot each, and A turns to the next:

- **First page:** the game closes the status window, prints the help (101) and opens
  window 1.
- **Racks:** windows 1–3 (91–93) open 3 tiles apart, so each covers all but the first
  line of the one before.
- **Fixed weapons:** windows 1–3 are cleared and drawn again over the others (98–100),
  in the same places.
- **Last page:** help 102, and A or B returns to the character screen. B returns from
  any page, with sound `0x3F`.

Each page's window is presented and a key wait prompts in it.

A rack line (`0x0804E04C`) takes the rack's kind from the low two bits of the slot's
half-word in the Zoid record (`+8 + slot × 4`):

| Kind | Printed |
|---|---|
| 1 | ［攻　］ |
| 2 | ［　防］ |
| 3 | ［攻防］ |
| 0 | ［固定］ |

A kind-0 slot whose record has no part (`0xFFFF` at `+10 + slot × 4`) prints
：ラックなし instead. After ： comes the part the unit carries: its id is the upper
half of the unit's slot word (`+0x12 + slot × 4`).

A part (`0x0804DFCC`) is 装備なし when the id is `0xFFFF`. Otherwise it prints the part's
name, which is entry `id` of the part-name table: 594 script pointers at ROM `0x6664F0`,
the table `part`. A line break and the description follow.

### Part records

Part records are 24 bytes at ROM `0x66C8F8`:

| Offset | Content |
|---|---|
| `+0x00` | Flags: bit 0 a weapon; `0x400`, `0x800`, `0x1000`, `0x2000` its special effects; for support parts, see below |
| `+0x04` | Price |
| `+0x08` | Accuracy in percent, or a support part's second value |
| `+0x0C` | A weapon's power in 16.16, or a support part's value |
| `+0x10` | Cost in energy points |
| `+0x12`, `+0x13` | A weapon's range (scripts 164 +) and reach (170 +) |
| `+0x14` | Turns a support part lasts, 0 for the whole battle |

`0x08036E74` copies the record for the pilot. A weapon's power gains the pilot's attack
bonus (member field 10) in percent, and its accuracy the accuracy bonus (field 12),
through `0x080346C0` as the unit statistics do.

### Printing a weapon

A weapon (`0x0804DE48`) prints:

1. 攻撃： with the power rounded half up, then 命中： with the accuracy and ％. Both take
   three cells and are capped at 999.
2. Up to two special effects, a space after the first, or 特殊効果なし.
3. A line break.
4. 消費： with the cost in three cells.
5. 射程： with the range and reach.
6. Any further effect, each after a space.

### Printing a support part

A support part (`0x0804D940`, called with its layout: wide on these pages) prints 効果：
and the first effect its flags name. The effect flags:

| Flag | Effect |
|---|---|
| `0x4000`, `0x8000` | ＤＦ＋value％, then 対レーザー＋second value％ when there is one |
| `0x10000` | 回避＋value |
| `0x20000` | ＳＰ＋value |
| `0x40000` | 命中＋value％ |
| `0x80000` | ＨＰ value 回復 |
| `0x100000` | ダメージを無効化 |
| `0x200000` | 属性付加, after 水中 (value 4) or 砂漠 (8) |
| `0x400000` | 全能力＋５０％／ＨＰ・ＥＰ全回復 |
| `0x800000`, `0x1000000` | ＨＰ上限＋value, ＥＰ上限＋value |

Then:

1. 消費： with the cost, from cell 22.
2. On the next line, 目標：, then 味方 (flag 2) or 自分 (flag 4).
3. 時間：, then one of:
   - 一瞬 with the HP recovery;
   - 装備している間 with flag `0x20000000`;
   - 戦闘終了まで for 0 turns;
   - the turns and ターン.

   Each is padded with spaces to nine cells (eight after the turns).
4. 制限：戦闘中１回のみ when bit 31 is set, 制限：なし otherwise.

The narrow layout, for windows half the screen wide, puts a space before 効果： and
spreads the same over four lines. It uses each script's other form (177, 180, 191).

Numbers go through `0x08001848`:

- the last *n* digits of the value, up to seven, capped at 9999999;
- a － for a negative value, or a ＋ when asked, before the digits;
- leading zeros printed as spaces (right-aligned), left out (left-aligned) or as ０.

The effect values are printed left-aligned.

## Weapons list

ステータス → 武器 lists the stocked weapons and support parts (`0x0804F7AC`). The game
state keeps one stock byte per part at `+0x334C + id`, for the parts 0–149, at most 9.
The list (`0x0804E24C` with mask 15) takes, in id order, every part whose count is not 0
and whose record's flags share a bit with the mask.

When the list is empty the game plays sound `0x4F`, shows notice 57, and plays `0x41`
when the notice is dismissed. The Zi-data notices do the same, and アイテム plays `0x4F`
before its notice.

Otherwise the list shows six parts a page in window 2:

- each line is the name, padded with spaces to eight cells, then × and the count in one
  cell;
- L and R turn the page (sound `0x40`), and the window's arrows show whether more lie
  before or after;
- A or B return to the status list, B with sound `0x3F`.

The part under the cursor is described twice:

- **Help line:** its text, script 422 + id.
- **Window 1:** its name, then its record's values with no pilot's bonus (the power's
  whole part, not rounded) and 特殊効果： over its effects, or the narrow form of a
  support part's description.

## Zi data lists

ステータス → Ｚｉデータ lists the Zoids whose Zi data the party holds: every Zoid 0–0x98
whose byte at game state `+0x33E2 + id` is not 0, in id order (`0x0804E3A0`).
ステータス → Ｚｉデータ用アイテム lists the Zi-data items the party carries: every item
0–63 whose count at `+0x330C + id` is not 0 (`0x0804E308`). With none, the lists play
sound `0x4F` and show notice 58 or 60, as the weapons list does.

Otherwise the status windows close, script 108 (or 55) opens the help line
(Ａボタン・Ｂボタン：抜ける) and the two windows, and the list shows six entries a page in
window 2, as the weapons list does:

- a Zi data line is the Zoid's name; an item line is the item's name padded to eight
  cells, × and the count in two digits, zero-padded;
- L and R turn the page (sound `0x40`) and keep the cursor's line; the arrows show more
  before or after;
- A or B return to the status list, B with sound `0x3F`.

Window 1 describes the entry under the cursor, cleared (script 1) and drawn again when it
changes:

- **Zi data:** the Zoid's development record (ROM `0x670210 + id × 0x4C`): 必要金額： and
  the money at `+0x24`, left-aligned, with Ｇ; 必要ゾイド： and on the next line the Zoid
  at `+0x2C` (0 is なし, below `0xFA` a Zoid's name, from `0xFA` script 219 + (z − `0xFA`));
  Ｚｉデータ用アイテム： and the items at `+0x2D` and `+0x2E` (`0xFF` is none), one a line,
  or なし when neither is set.
- **Zi-data items:** the item's name, then its text (script 635 + id).

## Items

アイテム on the main list builds the list of items 0–6 whose count (game state
`+0x3305 + id`) is not 0 (`0x0804E2C4`). With none, sound `0x4F`, notice 56
(アイテムがありません) and `0x41` when it is dismissed. Otherwise windows 3, 2 and 1 close,
the help line clears and script 50 asks どのアイテムを使いますか？ and opens the description
(window 1) and the list (window 2), 15×14 each.

- **The list** shows six items a page: the name (`name` 241 + id) padded with spaces to
  eight cells, × and the count in two digits, zero-padded. L and R turn the page (sound
  `0x40`) and the arrows show more before or after; a page emptied by the last use goes
  back one.
- **The description** of the item under the cursor, printed again when it changes: its
  name and on the next line its help (`item` 63 + id).
- **A** on ショックウエイブ (3): sound `0x4F`, notice 54 (戦闘中以外では使用できません), then
  `0x41`, the question again (51) and the list. On 緊急退避カプセル (6) where the place
  does not allow it (`0x08009A2C`: the byte at RAM `0x0200756A` is not 1 or flag
  `0x199` is set) the same with notice 53 (この場所では使用できません). On another item
  the list and the description close and the members' screen opens. **B** closes them
  too, then sound `0x3F` and the main menu is built again.
- **The members' screen** (script 117: window 1 18×14 and window 3 13×14): the help line
  gives the item's name and を誰が搭乗しているゾイドに使いますか？. Window 3 lists the members six a
  page; window 1 shows the Zoid of the one under the cursor: its name, then
  `ＨＰ：` a space and the hit points in four cells (or 戦闘不能, script 87, for a broken
  unit, flag `0x800`), `／` and the full, `ＥＰ：` and five cells, `／` and the full,
  `ＳＰ：` four cells, `ＤＦ：` three cells and ％, and the Zoid's picture at (40, 88); or
  搭乗ゾイドなし, not printed again while the cursor stays on members without one.
- **A** on a member without a Zoid or with a broken one: sound `0x4F`, 124
  (キャラクターがゾイドに搭乗していないので使えません) or 126 (ゾイドが戦闘不能状態なので使えません), a
  key, `0x41`, and the question again. Otherwise sound `0x50`, the effect on the unit
  (`0x08039580`: the routines at ROM `0x683AA8`, as in battle), one fewer of the item,
  window 1 printed again, and in the help line the item's name, を使いました, and on the
  next line the Zoid's name and `item` 70 + id (のＨＰが３００回復した…). After a key,
  `0x41`, the list is counted again, windows 3 and 1 close and the list comes back where
  it was, printed again whole; with no items left, notice 56 and the main menu. **B**:
  sound `0x3F` and the same way back.

The effects work on the unit's record: hit points at `+8`, full at `+0x28`, paralysis bit
`0x4000` of the first half-word. Items 0–2 give back 300, 150 and 50 points and 5 half
the full, never past the full; 3 clears the paralysis; 4 clears it and fills the hit
points. The items are used whatever the Zoid's hit points.

## Equipment screen

武装 (states `0x3000`–`0x3200` and `0x3FFF` of the menu's state machine) changes the
parts on a Zoid's racks.

### Choosing the Zoid

Window 3 lists the members six a page, with L and R turning the page. Window 1 shows the
Zoid of the member under the cursor:

- its name, then ウエポンラック and the three racks' parts, each ラックなし, 装備なし or a
  part's name;
- its status picture at (40, 88);
- 搭乗ゾイドなし for a member without a Zoid.

A on a member without a Zoid plays `0x4F` and shows notice 134. A on a member whose
character flags have bit `0x08` shows 135 instead. After either, a key wait, then `0x41`.
B returns to the main menu (sound `0x3F`).

### Choosing the rack

A on a member opens window 4 with its three racks. Each line (`0x0804E13C`) is the
rack's number, its kind (as on the parts pages, or blank for a slot without a rack), ：
and its part.

The help line describes the part on the rack under the cursor, with its pilot's values.
The game's window 0 hides sprites under window 4, so the status picture disappears there.

A on a fixed slot plays `0x4F` and shows notice 141, or 140 when the Zoid has no rack
there; `0x41` follows. B returns to the members.

### Choosing the part

A on a rack closes windows 4, 3 and 1:

1. **The Zoid's picture.** The battle picture (`0x08044E98`) is drawn on BG1 from the
   top-left corner. For the third rack BG1 is blended half over what lies below
   (`BLDCNT` `0x1C42`, `BLDALPHA` `0x0808`).
2. **The title.** Window 1 shows ウエポンラック, the rack's number and its kind.
3. **The list.** Window 2 lists 装備を外す, then the stocked parts the rack takes: mask 1
   for a rack of flags 1, `0xE` for 2, `0xF` for 3. The list shows four lines a page,
   each laid out like the weapons list's.

The help line describes the entry under the cursor with the member's values. For
装備を外す it describes the rack's own part.

The weapons show on the picture (`0x0804D768`). Each is the first frame of a sprite from
ROM `0x6F77C4` for the second rack and `0x6F6E44` for the others, by part. It stands at
the Zoid's mount for that rack: the 28-byte record at ROM `0x6E78EC` holds six x, a
default x, six y and a default y, where `0xFFFF` takes the default. OBJ priorities:

| Rack | Priority | Result |
|---|---|---|
| First | 1 | In front of the Zoid |
| Second | 2 | Behind the Zoid |
| Third | 3 | Behind the Zoid |

- **The rack being changed:** shows the entry under the cursor. On 装備を外す its own part
  blinks: task `0x0804D8F0` hides it for 16 frames, then shows it for 16.
- **The other two first racks:** `0x08053204` draws their parts into BG1's tiles, with
  their palettes copied to BG palettes 10–15. A priority-1 sprite covers the Zoid; the
  others only fill the pixels the Zoid leaves clear.
- **The third rack, when it is not the one being changed:** stays a sprite.

### Changing the part

A (`0x08051B26`):

1. The rack's part goes back to stock. When that stock is already 9 and the choice is
   another part, the game asks: 外そうとしている, the name, 150 and はい/いいえ. はい
   throws the part away; anything else returns to the list (B with sound `0x3F`).
2. The chosen part leaves the stock, or 装備を外す leaves the rack empty.
3. The unit's statistics are computed again (`0x08036CB0`), and its current hit and
   energy points are kept within the new maxima.
4. Sound `0x4E`.

A, or B (sound `0x3F`), then rebuilds the members' windows and returns to the rack
list, with the cursor where it was.

## Windows and menus

Four rules of the original, taken from its handlers, make the scripts work:

- The interpreter keeps one current window. Opening a window makes it current; a message
  starts in it, and `0x1C` inside the message only redirects that message's text. The
  menu and key-wait opcodes act on the current window.
- Presenting a window makes it current; presenting all of them (`0xFF`) redraws each open
  window in id order and leaves the highest one current. That is how script 47 prints its
  help in window 0 yet runs its menu in window 3, and how a notice presented alone moves
  the key prompt to the help line.
- The windows share one tile map, so the last window drawn covers the others. Drawing a
  window (`0x0D`) or presenting it draws it again on top; presenting all of them draws
  them in id order. The parts pages rely on this.
- The menus of modes 2 to 6 (opcodes `0x36` and `0x3C`) draw page marks on the
  window's sides when they start (`0x08040DA0`, `0x08040BA8`): ◂L on the left border
  and R▸ on the right, each two tiles high around the middle row (window tiles
  `0x36`/`0x37` and `0x38`/`0x39`, the lower one flipped vertically), as the game's
  code sets bits 0 and 1 of the window's byte `+0xF` (`0x080339F8`). The up and down
  marks of a scrolled list sit in the middle of the top and bottom borders.
- A window remembers the line its last menu ended on and the next menu starts there,
  so the cursor returns to the item that was chosen. The cursor tiles stay until the
  window is redrawn: a notice leaves them on the list, opening a submenu (which presents
  everything) removes them.

The game's code sometimes prints into a window a script did not open last (搭乗ゾイドなし
goes to window 1 after window 3 was opened) and runs menus over lists it filled itself
(the names on the weapons screen); the port models both with explicit calls.

## Wallpaper

Behind the windows sits a 256-color texture on BG3 and, on BG2, a map of the game's
logo that scrolls one pixel left and one down per frame:

| Data | ROM | Notes |
|---|---|---|
| Tiles | LZ77 `0x564748` | 8bpp; the maps use the first 256 |
| Palette | LZ77 `0x565E64` | 96 colors loaded at index 64 |
| Texture map | LZ77 `0x565F40` | 32×32 entries, static |
| Logo map | LZ77 `0x5660C4` | 32×32 entries, scrolled |

The backdrop color is `0x7240`.

## Opening and closing

The game keeps a fade level (IWRAM `0x03002356`), 0 to 31, that the screen shows from
16 down. It was traced per frame together with the wallpaper's scroll registers.

- **Opening**, counted from START:
  - the field goes on (its actors keep moving) while the level climbs one a frame from
    the third frame, so the screen is black from the 18th;
  - on the 33rd frame the menu is built. The wallpaper holds for five frames, then
    scrolls; the menu's scripts stop it for two frames;
  - the level falls one a frame from the ninth frame after the build, but for those two
    frames, so the menu brightens over the last 16;
  - the main list's menu runs on the 43rd frame after the build.
- **Closing**, counted from B on the main list: sound `0x3F`, then the menu, its
  wallpaper still moving, darkens a level a frame from the fifth frame. On the 34th
  frame the map is loaded again, so the actors' animations start over. The field stays
  black 16 frames, then brightens a level a frame, and the player moves once it is
  bright.

## Formation screen

部隊編成 hands over to a task of its own (`0x08037B84`); the menu's code waits for it
(`0x0805203C`). The battle's menu opens the same task, and its ステータス the character
screen; their frames there are in [combat.md](combat.md#the-menus-other-screens). Its windows and texts are scripts of the `battle-menu` and
`battle-text` tables (see [formats/script-text.md](formats/script-text.md)):

| Script | Content |
|---|---|
| menu 1 | Reset the text system |
| menu 2, 8 | The help line, window 1 at (0, 16) 30×4; the list, window 2 at (15, 4) 15×12, and the header, window 3 at (15, 0) 15×4 |
| menu 5, 9, 10; 6, 13, 14; 7, 11, 12 | Present, draw and clear windows 1, 2 and 3 |
| menu 15 | Draw window 2 and run its menu: opcode `0x3C` in mode 6, cancelable |
| text 0x17, 0x18, 0x19 | The helps: Ａボタンで選択　Ｂボタンで終了です, 場所を選び、Ａボタンで部隊に編成できます, 選択した場所にいるゾイドを　Ａボタンで部隊から外します |
| text 0x11, 0x16, 0x32, 106 | The marks before a name: ★ in the formation, × without a unit, 壊 for a broken unit (bit `0x800`), a full-width space otherwise |
| text 0x12–0x14, 0x1C | The sizes Ｓ, Ｍ, Ｌ, and － without a unit |
| text 0x15 | 搭乗していません, the header for a member without a unit |

The screen, from back to front, is the battle field's grid (BG3) and platform (BG2)
(see `crates/extraction/src/saga_formation.rs`), the units' pictures, the windows
(BG0) and the slot cursor. Each unit stands at its slot's position of the battle's
player side, 120 pixels further left, in the picture the status screen shows; the
nearer ones (lower on the screen) cover the others. Slots 0–2 are the left column
from top to bottom, 3–5 the right one. The cursor (entity 4, OBJ priority 0) sits 8
pixels right of and 16 above the slot's position, in the first frame of its animation:
the screen does not run the entities' animations.

- **List**: the members, five a page (`0x080380E8`), each with its mark, name and, from
  cell 10, its size. The header shows the Zoid of the member under the cursor, and is
  printed again only when that Zoid changes (`0x08038314`). L and R turn the page when
  there is one, with the page marks, and put the cursor on the first line.
- **A**: sound `0x47`; on a member in the formation the cursor goes to its slot to take
  the unit out; on one with a unit that is not broken, to slot 1 to place it; nothing
  otherwise. **Left** puts the cursor on the first occupied slot (slot 0 when none) to
  take a unit out. **B** or **START** leaves.
- **On the field** (`0x080383C0`): up and down move within a column, left and right
  between the columns' matching slots, with sound `0x40`; an L unit being placed only
  moves between the middles (1 and 4). A places (sound `0x51`) or takes out (`0x3E`),
  B goes back (`0x3F`); then the cursor goes, the page is printed again and the list's
  menu runs.
- **Placing** (`0x080371AC`) empties the slot first, or the middle of its column when an
  L unit fills it; an L unit empties its whole column and stands in the middle. Taking
  out (`0x08037258`) empties the slot, or the column's middle for an L unit. The game
  does not check that the slot holds anyone: on an empty slot it clears the bits of the
  entries of unit and character `0xFF`, the character's inside the game-state block.
- **Entering**, counted from the frame the choice is made: the menu's wallpaper stops
  and the menu darkens a level a frame from the fifth frame; the screen is built in the
  dark on the 36th frame, brightens a level a frame from the 60th, its help line is
  printed on the 95th and the list's menu runs.
- **Leaving**, counted from B or START: no sound; the screen darkens a level a frame from
  the fifth frame, and on the 44th the text system is reset and the main menu is built
  again with the cursor on 部隊編成. Its wallpaper starts over from its first position
  and moves a pixel a frame but for the 6th and 7th frames; the fade level holds at 31
  for three frames, falls one a frame but for those two, and the main list's menu runs
  on the 38th frame, which stops the wallpaper once more.

## Flows

START on the field opens the menu; B on the main list closes it (see above). アイテム shows
the items (see above). ステータス opens the
status list: 部隊 shows the unit list, which A or B leaves; キャラクター the character
screen; 武器 the weapons list; Ｚｉデータ and Ｚｉデータ用アイテム their lists (see above);
図鑑 asks ゾイド or キャラ and opens that guide (see [guide.md](guide.md)). 武装 shows the
equipment screen. 部隊編成 shows the formation screen (see above). コンフィグ shows the message speed (3 on a new game) with the
cursor on it; picking a number stores it in the party and returns to the main list
with the cursor on コンフィグ, as the original does. セーブ asks; はい writes the save
and answers セーブしました, or セーブを中止しました when it could not be written, and
いいえ or B answer セーブを中止しました.

### Settings in the enhanced mode (a port feature)

Source of knowledge: this project's own design; the windows, the menu and its sounds are
the original's. In the classic mode コンフィグ is the original's (above). In the enhanced
mode (see [launcher.md](launcher.md)) it lists every setting the game lets the player
change while it plays: window 4, a light menu like the save slots', takes the columns
from the main list's right edge to the screen's, 8 rows high to cover the status panel,
a line per setting with its value from cell 13: the message speed (１ to ５, the original's, stored in the party as its list does), the
battle animations (ＯＮ or ＯＦＦ, see [combat.md](combat.md), Without the attack scenes)
and the damage numbers (ＯＮ or ＯＦＦ, see [combat.md](combat.md), Damage numbers). The
help line says what
the setting under the cursor does, and below it the keys. Up and down move (`0x40`);
left and right, or A, change the value (`0x40`), the speed stopping at its ends and the
enhancements switching; B plays `0x3F` and goes back to the main list with the cursor on
コンフィグ. The settings hold from then on; the launcher remembers the enhancements for
the next game. The port's messages are keyed `port/options/...` (see
[translation.md](translation.md)). Implemented in `crates/game-core/src/menu/options.rs`.

### Save slots (a port feature)

Source of knowledge: this project's own design; the original has one save. With more than
one save slot (see [formats/save.md](formats/save.md)), セーブ first lists them: the help
line reads どのスロットにセーブしますか？ and, for a slot with a game, its area and money;
window 4, a light menu, takes the columns from the main list's right edge (column 9, or
further when a translation widened the list) to the screen's, 10 rows high, one line per
slot: its number, then the player's name and from cell 11 the level (Ｌｖ), or データなし,
or こわれたデータ when both copies are broken. More slots than rows scroll, with the scroll
marks. The cursor starts on the slot the game was continued from or last saved to, else
the first empty one, else the latest game. Moving plays `0x40` and describes the slot;
B plays `0x3F` and goes back to the main list; A plays `0x47`, closes the list and asks
スロットｎにセーブしますか？ (or スロットｎに上書きしますか？ over a game) in the help
line with the original's はい/いいえ window (script 61); from there it goes on as the
original's question. The port's messages are keyed `port/save-slots/...` in a
translation (see [translation.md](translation.md)). Implemented in
`crates/game-core/src/slots.rs` and `menu/save_slots.rs`.

## Not modeled yet

緊急退避カプセル: what sets the byte at RAM `0x0200756A` that allows it is not traced, so
the port always refuses it (notice 53); the original's yes/no question (52, 61), the
count taken and the menu's exit to the lab (state `0xFFFE`) are not ported.

The item screens were compared on every frame with items set by hand in a save from the
factory (map 21): the screens, the cursor, the pages, the refusals and the uses match;
the transitions show their last picture at once (see below), the key wait after an
item's message starts up to three frames early (the original's printing runs past its
frames), and the wallpaper drifts after the way back to the main menu.

The Zi data lists were compared on every frame with ten Zoids' Zi data and four items set
by hand in a save: the lists, the pages, the cursor and the descriptions match; the
description's window shows its last picture at once where the original shows it cleared
for three frames, and the way back shows the status list at once.

The ボタン page of the config ends in the まだできてません notice.

The formation screen tints a unit's picture darker (every channel less 24,
`0x08031E90`) when its battle record has bit `0x4000`; a broken unit (bit `0x800`) did
not show it, and what sets that bit is not known yet, so the port draws every unit as
it is.

The original spends a frame on each window it opens, clears or presents, and its game
stands still meanwhile: the wallpaper and the blinking stop, and keys go unread. The
port draws a screen at once, then counts the frames its scripts would have cost and
holds the menu for that long. The key waits and the blinking then start within a frame
of the original's. The steps of a transition in between do not show (on the formation
screen, the header cleared before the new name, the help line cleared before its new
text), and a menu's cursor answers a key two frames before the original's.

The menu's wallpaper in the screens after the main list still drifts from the
original's: the frames each transition stops it for are only approximated.

The discard question and the characters that keep their equipment (flag `0x08`, which
the game sets at runtime) follow the code but were not seen in the original.
