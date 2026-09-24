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
and the display registers on the part lists. Saving is described in
[formats/save.md](formats/save.md). Implemented in
`crates/game-core/src/menu.rs`; data in `crates/extraction/src/saga.rs`
(`PAUSE_MENU_SCRIPTS`, `PART_NAME_SCRIPTS`, `pause_wallpaper`) and
`crates/extraction/src/saga_party.rs` (`unit_parts`, `part`).

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
keeps these in `Party`. The status screens read the rest from the block when the menu
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
  wallpaper still moving, darkens a level a frame from the fourth frame. On the 34th
  frame the map is loaded again, so the actors' animations start over. The field stays
  black 14 frames, then brightens a level a frame, and the player moves once it is
  bright.

## Flows

START on the field opens the menu; B on the main list closes it (see above). ステータス opens the
status list: 部隊 shows the unit list, which A or B leaves; キャラクター the character
screen; 武器 the weapons list; Ｚｉデータ and Ｚｉデータ用アイテム print their notices;
図鑑 asks ゾイド or キャラ and opens that guide (see [guide.md](guide.md)). 武装 shows the
equipment screen. コンフィグ shows the message speed (3 on a new game) with the
cursor on it; picking a number stores it in the party and returns to the main list
with the cursor on コンフィグ, as the original does. セーブ asks; はい writes the save
and answers セーブしました, or セーブを中止しました when it could not be written, and
いいえ or B answer セーブを中止しました.

## Not modeled yet

部隊編成 (a separate screen with its own wallpaper) and
the ボタン page of the config end in the まだできてません notice.

The original spends a frame on each window it opens, clears or presents, and its game
stands still meanwhile: the wallpaper and the blinking stop, and keys go unread. The
port draws a screen at once, then counts the frames its scripts would have cost and
holds the menu for that long. The key waits and the blinking then start within a frame
of the original's.

The menu's wallpaper in the screens after the main list still drifts from the
original's: the frames each transition stops it for are only approximated.

The discard question and the characters that keep their equipment (flag `0x08`, which
the game sets at runtime) follow the code but were not seen in the original.
