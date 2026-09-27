# Shops

Source of knowledge: own reading of Zoids Saga (Japan, Rev 1) at the addresses named
below, plus breakpoints on the script runner, the sound call, the task spawner and
the warp, per-frame logs of the fade level and the wallpaper's scroll, and screenshots
of every frame in a reference emulator while buying and selling in Arcana's item shop
(map 25) and armaments shop (map 26), and for the Zoid lab (map 27) the same with saves
changed by hand (a broken unit, damaged units, money). The port's frames were compared
with the original's on the same inputs; for the shops of areas 2 and 3 too, from saves
changed by hand, with the reference emulator's cycle counter read at the lab keeper's
rebuild and its return. For the lab's development: a read of its states named below,
breakpoints on the script runner and the sound call, RAM dumps before and after, and
screenshots of every frame compared with the port's. Implemented in
`crates/game-core/src/menu/shop/` (the screens, as a mode of the pause menu;
`develop.rs` for the development), `crates/extraction/src/saga_shop.rs` (goods,
prices and counts), `crates/extraction/src/saga_party.rs` (what a development needs
and does) and `crates/game-core/src/story.rs` (the keepers).

## Keepers

A keeper's object script is one of 65 twelve-byte stubs from `0x080090F0` on. Each
calls one of three entries with a shop number:

| Entry | Kind | Shop |
|---|---|---|
| `0x080090A0` | 0 | Item shop (`0x08053748`, task `0x0805378C`) |
| `0x080090B4` | 1 | Armaments shop (`0x080544D8`, task `0x0805451C`) |
| `0x080090C8` | 2 | Zoid lab (`0x080553E0`), after `0x08006E4C` with the area |

In Arcana, `0x080090F0` is item shop 1, `0x080090FC` armaments shop 1 and
`0x08009108` lab 1. The item shop's keeper stands behind a counter: the player speaks
across it (see [field.md](field.md), Talking).

The entries call `0x08008F58` with the kind and the number. It runs in the talk
dispatch, a frame after the talk:

1. `0x08001524` darkens the field a level a frame. Its first frame sets level 0, so
   the level reads 1 two frames after the call and 31 on the 32nd.
2. The entity table and the object palettes are saved.
3. The shop's task runs until it ends.
4. The map is loaded again (`0x08007188`, the player's cell, no facing).
5. The entities and palettes are put back, the player turned as it was.
6. The field shows at full brightness at once.

## Screen

The shop is built from the pause menu's pieces (see [menu.md](menu.md)). The
wallpaper is the same four blocks, loaded from the table at ROM `0x75BCA4`. Its scroll
is a task of its own (`0x080535D8`): one pixel left and one down each frame. The
windows are scripts of the pause-menu table, with the item shops' from 223 and the
armaments shops' from 242:

| Item | Arms | Content |
|---|---|---|
| 223 | 242 | The help line, window 0 at (0, 14) 30×6. The title, window 2 at (0, 0) 10×4. 買う / 売る, window 3 at (0, 4) 10×6 |
| 224 | 243 | いらっしゃいませ / よぅ、いらっしゃい in the help line |
| 225 | 244 | どのようなご用件でしょうか？ / 何の用だい？, present all, cancelable menu |
| 226 | 245 | The notice that there is nothing to sell, with its key wait |
| 227 | 246 | The count, window 4 at (0, 10) 12×4. The goods, window 5 at (9, 0) 21×10 |
| 228, 229 | 247, 248 | 持っている数 (drawing window 4) and 個 |
| 230, 231 | | いくつお買い上げになりますか？ with the pad's help. The quantity, window 7 at (12, 5) 6×4 |
| 232, 233 | 249, 250 | Not enough money; no room for more (notices with a key wait) |
| 234, 235, 236 | 251, 252 | が, 個で, then Ｇになります　お買い上げになりますか？ / は, then Ｇになるな　買うかい？ |
| 237, 238 | 253, 254 | The keeper's thanks, or its answer to いいえ |
| 239 | 255 | The wares, window 4 at (6, 0) 24×10 / (7, 0) 23×10 |
| 240, 241 | 256 | How many to sell; Ｇになります　お売りになりますか？ / Ｇってとこか　売るかい？ |

The money box is the pause menu's (44), with a space, the money in seven cells and Ｇ.
The はい / いいえ choice is script 61.

## Goods

An item shop's goods are record `n` of the 16-byte table at ROM `0x75BCD4`: up to four
entries of a kind and an index, ended by kind `0xFF`.

| Kind | Names | Texts | Prices | Counts |
|---|---|---|---|---|
| 0, consumables | `name` 241 + id | `item` 63 + id | ROM `0x75BE24` | game state `+0x3305` |
| 1, Zoid cores | `item` id | `pause-menu` 572 + id | ROM `0x75BE40` | game state `+0x330C` |

An armaments shop's goods are record `n` of the 8-byte table at ROM `0x75BF40`: up to
four part ids, ended by `0xFFFF`. A part's price is its record's `+4` (records at ROM
`0x66C8F8`), and its stock is the byte at `+0x334C + id`. Arcana's shops:

| Shop | Goods |
|---|---|
| Items 1 | コア活性イオン小 (500 G), 緊急退避カプセル (15000 G), ショックウエイブ (2800 G) |
| Arms 1 | Parts `0x15`, `0x29`, `0x3F`, `0x11` (2300, 3200, 2800, 3600 G) |

The party carries at most 99 of an item and stocks at most 9 of a part. The money is
the game state's `+0xD28`, at most 9999999 after a sale. The shops buy back for half
the price: the sum of all the items sold, halved (rounded down), or half the part's
price.

## Flow

The task is a state machine. Its states are shown in parentheses.

- **Opening.** The windows, the money and a present-all run in the dark. The wallpaper
  task and the fade task (`0x08004034`) start five frames after the build.
  The fade level holds at 31 and falls one a frame, and the task waits for it to end.
  Then the welcome and the choice run.
- **Choice (`0x20`).** B leaves. 売る with nothing to sell plays sound `0x4F`, shows the
  notice and plays `0x41`. Otherwise the help line is cleared and the list opens:
  `0x100` to buy, `0x200` to sell.
- **Goods (`0x100`, `0x101`).** Each line is the name padded with full-width spaces to
  nine cells (`0x0804D698`), the price in seven cells and Ｇ.
  - The help shows when the cursor lands on another entry, after a notice, or when a
    window drew over it (`0x101`'s redraw flag): the count in window 4 (two cells for
    an item, one for a part), and the text in the help line. An item shows its text;
    a part shows its record's values through `0x080544A8`, which draws the help line
    and prints the part as the parts pages do (see [menu.md](menu.md)), without a
    pilot's bonus.
  - B closes windows 5 and 4, clears the help line and goes back to the choice.
  - A when the money is short, or when the count is at its limit, plays `0x4F`, shows
    the notice and plays `0x41`.
  - A otherwise asks how many of an item (`0x110`), or goes straight to the question
    for a part (`0x120`).
- **Quantity (`0x08053600`).** It opens window 7 and prints the value (from 1) after a
  space in two cells. It then reads the pressed keys each frame, with no repeat:
  - L: −10, or from 10 or less to the other end of the range;
  - R: +10, or from within 10 of the most to the other end;
  - left, right: ∓1, wrapping around;
  - A: sound `0x47`;
  - B: sound `0x3F`.
  
  Each change plays `0x40` and prints the value again. A and B close the window. The
  most is the money over the price, and at most what the party can still carry. B goes
  back to the goods.
- **Question (`0x120`).** The help line gets the name, が, the count (left-aligned), 個で,
  the total (left-aligned) and the question; a part gets は, its price and its own
  question. はい adds the goods, takes the money, prints the money and the count again,
  and the keeper thanks. いいえ gets the other answer. A key wait follows (sound `0x41`,
  and `0x41` again from the task), then the goods, redrawing the help. B at the
  question asks how many again, or for a part goes back to the goods.
- **Wares (`0x200`, `0x201`).** Entering `0x201` lists the wares again from the game
  state:
  - an item shop lists the consumables the party carries (`0x0804E2C4`);
  - an armaments shop lists the stocked weapons and support parts (`0x0804E24C`,
    mask 15).
  
  Four a page, each line is the name padded to eight cells, ×, the count (two digits
  with zeros for an item, one for a part), a space, half the price in seven cells and
  Ｇ.
  - The page is printed again when it changes, or after a sale. A page left empty
    steps back to the one before, with the cursor on its last line; the cursor never
    stays below the last line.
  - The window's side marks show whether pages lie before and after it.
  - L and R turn the page when there is one (sound `0x40`).
  - B, or a list left empty, closes the windows and goes back to the choice; an
    emptied list first shows the notice and plays `0x41`.
  - A asks how many of an item (`0x210`), or the question for a part (`0x220`).
- **Selling (`0x210`, `0x220`).** The question and the answers are as for buying, at
  half the price. はい adds the money and prints it again. Afterwards the task goes
  back to `0x201` with the cursor where it was, and after a sale prints the page
  again. B at the question asks how many again, or for a part goes back to the wares.
- **Leaving.** B on the choice plays `0x3F` and starts the fade task darkening. The
  shop ends once it is black, and the field returns as described above.

## Timing

- **From the frame the game reads A on the keeper.** The fade level is 1 on the third
  frame and 31 on the 33rd, when the shop is built.
- **From the build.**
  - The wallpaper moves from the sixth frame.
  - The fade level holds at 31 until the ninth frame, then falls a level a frame to 0
    on the 40th.
  - The welcome and the question show on the 42nd frame, or the 43rd in an item shop:
    its longer text overruns a frame, which stands the wallpaper still for it.
  - The choice's menu runs from the next frame.
- **From the sound of B on the choice.** The fade level rises a level a frame from the
  third frame, and the field shows on the 51st.

## Zoid lab

The lab's keeper (`0x080090C8` with the number, `0x08009108` for Dr. T's lab in Arcana)
first rebuilds the area's object states (`0x08006E4C` with the area, as entering an area
does; see [events.md](events.md)), then opens the lab through `0x08008F58` as the shops
open, and once it has closed and the field is back fills the hit and energy points of
every unit in use (`0x08037148`), broken ones too, which stay broken.

The lab's task (`0x08055418`) builds its screen from pause-menu script 257 (the help
line, 研究所 in window 2 11×4 and the menu in window 3 11×10), the money box as the
shops', the welcome 258 and the question 259, whose menu offers ゾイドの復活, ゾイド開発,
ゾイド乗せ換え and ゾイドを売る. The welcome overruns its frame as the item shops' does.

- **ゾイドの復活** lists the broken units (`0x080552A8`: every unit slot with a Zoid whose
  first half-word has bit `0x800`). With none, sound `0x4F`, notice 262 (戦闘不能ゾイドは
  いないみたいですよ) and `0x41`. Otherwise windows 3 and 2 close and script 328 opens the
  Zoid's window (2, 16×14) and the list (3, 15×10, four Zoids a page). Each pass prints
  329 (どのゾイドを復活させたいのですか？ スタートボタン：ゾイド詳細表示) in the help line;
  when the Zoid under the cursor changes, window 2 is cleared and shows its name, 300 and
  the full hit points in four cells, 301 and the full energy points in three, 304
  搭乗者： and the pilot (or 305 なし), and its picture at (40, 88). The menu is script 36.
  A asks the price: the Zoid's name, 330 の復活には, the price left-aligned, 331 Ｇ必要ですね
  復活させますか？ and the yes/no menu. The price is the Zoid record's `+0x28` raised by
  the unit's training in percent, a tenth of it (660 for the Shield Liger untrained).
  はい with too little money gives 334 (お金が足りないみたいですね); with enough, the broken
  bit goes, the hit and energy points are full, the money is taken and printed again,
  332 (わかりました カンペキな状態にしておきますからね), `0x41`, and the list again, or with
  none left 262 and the lab's menu. いいえ gives 333 (それでバトルに支障はきたさないのですか？);
  B on the question sound `0x3F` and the list. B on the list: `0x3F`, windows 3 and 2
  close and script 257 builds the lab's screen again.
- **ゾイド開発** with no Zi data (`0x0804E3A0`: the bytes `+0x33E2` for the Zoids 0–0x98)
  gives notice 260 (Ｚｉデータを持ってないと開発できませんよ), and with more than 0x98 units
  261 (これ以上ゾイドを持てないみたいですね), each with `0x4F` and `0x41`. Otherwise it
  develops (see below).
- **B** on the menu: when the player or one of the three warriors (characters 0–3) has no
  unit, sound `0x4F` and おっと。 (263) followed by あなたは (264) when only the player has
  none, あなたと三獣士たちは (265, 266) when the player and a warrior have none, or
  三獣士たちは (266), then 267 (戦わなくていいんですか？…) and the menu again. Otherwise sound
  `0x3F`; the formation is put right (a slot whose pilot now flies another unit takes it,
  one whose pilot has none or a broken one is emptied); when a unit not broken is short
  of its full hit or energy points the keeper says 271 (あ、ゾイドの回復はサービスでやって
  おきましたよ); and the lab closes as a shop does, black three frames longer.

### Development

The lab task's states `0x200`–`0x221` (`0x08056090` on). Windows 3 and 2 close and
script 272 opens the Zi data's window (2) and the list's (3); 273 asks
どれを開発したいのですか？.

- **The list** holds the Zoids whose Zi data the party has, in id order, four a page in
  window 3 (their names), with the arrows and L and R (sound `0x40`) as the other lists.
  Window 2 shows the Zi data of the Zoid under the cursor as the status list's Ｚｉデータ
  does (see [menu.md](menu.md)), cleared (script 2) and drawn again (27) when it changes.
  The menu is script 35. B: `0x3F`, windows 3 and 2 close, and script 257 builds the lab
  again with the cursor on ゾイド開発, the line last chosen.
- **The Zoid.** A closes windows 3, 2 and 1 and script 274 shows, in window 1, what the
  development gives: the Zoid's name and size (88 + the record's `+4`), then 81 ＨＰ：
  and 82 ＥＰ： with the record's `+0x40` and `+0x44` in four cells, 83 ＳＰ： with `+0x48`
  in four and 84 ＤＦ： with `+0x4A` in three and ％, and its picture at (40, 88); 275
  このようなゾイドが開発されます スタートボタン：装備武器表示. Script 38 waits in mode 6 (see
  [formats/script-text.md](formats/script-text.md)): B (`0x3F`) goes back to the list,
  which script 44 and the money, 272, 28 and 273 print anew with the cursor kept; START
  shows the Zoid's parts as its record gives them, on the pages the Zoid status screen
  uses (see [menu.md](menu.md)) with helps 269 (Ａボタン：次へ Ｂボタン：戻る) and 270 on
  the last, and past the last page or with B the Zoid again.
- **What it needs** (`0x0805534C`), on A (sound `0x41`): the record's money (`+0x24`,
  compared with the party's), a unit to build it from when `+0x2C` is not 0, and each
  Zi-data item of `+0x2D` and `+0x2E` that is not `0xFF`, whose count at `+0x330C` must
  not be 0. A unit serves (`0x08055198`, the unit slots 0–`0xAC` in order, read as stored)
  when its Zoid is `+0x2C`, or, from `0xFA`, one of that kind's list at ROM `0x75C018`
  (eight bytes a kind, `0xFF` after the last) other than the Zoid developed. With
  something lacking: sound `0x4F`, the help cleared and drawn, 281 お金と or 282 お金が
  for the money, 283 ゾイドと or 284 ゾイドが for the unit, 285 アイテムが for the items,
  each with と when more follows, and 286 足りないみたいですね; the list after the key.
- **The unit to build from**, when the record asks for one: window 1 closes and script
  293 opens the list (3) and the unit's window (1); 287 asks どのゾイドを使って開発するの
  ですか？ スタートボタン：ゾイド詳細表示. Six units a page by their Zoids' names; window 1
  shows the one under the cursor as the revival's list does (cleared with script 1). The
  menu is script 36. B (`0x3F`) goes back to the Zi data list. START shows the unit in full
  (state `0x500`): windows 3 and 1 close and 268 (Ａボタン：装備武器表示 Ｂボタン：戻る)
  opens window 1 with the Zoid's name and size, its full hit and energy points, SP, DF
  and 85 訓練度： in three cells, and its picture; A shows its parts with its pilot's
  bonuses (`0x08055024`) on the same pages, and B, or A past the last page, goes back to
  the list. A on a unit: with no pilot the development's question follows; a pilot whose
  flag `0x08` is set will not leave (sound `0x4F`, the pilot's name, 289 は, the Zoid's,
  291 から降りたくないみたいですね, and after the key `0x41` and the list); otherwise the
  Zoid's name, 295 には, the pilot's, 296 が搭乗してるみたいですけど、よろしいのですか？ and
  はい／いいえ, whose いいえ or B go back to the list.
- **The question**: with a unit taken, windows 3 and 1 close and the Zoid is shown again.
  25 and 276 開発しますが、よろしいのですか？ and はい／いいえ. B (`0x3F`) goes back to the list
  it came from, the money printed again. いいえ gives 278 (あれ？やめてしまうのですか？).
  はい, when the unit has weapons on its racks (its first three slots with a part and
  kind bit 1 or 2), asks 279 (元になったゾイドが武器を装備したままだと開発できません
  装備をすべて外しますか？); いいえ or B give 280 (それでは開発できませんね・・・); はい asks, for
  each of those weapons whose stock is already over 8, 149 外そうとしている, its name and
  150 (…これ以上ストックできません。捨てますか？), where いいえ or B give 280 too; then each
  weapon goes to the stock while it holds fewer than 9, and its slot is emptied.
- **The development** (`0x08057020`): the money is taken; the unit built from loses its
  pilot (`0x08036C2C`, which works its values out again) and is cleared, 56 zero bytes,
  with the unit count less one (`0x08055314`); each item used goes down by one; a unit of
  the new Zoid is made in the first free slot (`0x08036A30`) and its values worked out
  with no pilot (`0x08036CB0`); 277 はい。すぐにできますからね. After the answer's key the
  keeper's `0x41`, window 1 closes and the list comes back as after B, or, with more than
  0x98 units, 261 and the lab's menu (not seen in the original).

## Checked against the original

- Sand Colony's lab (map 33) with Zi data, cores, units and money set by hand in a save:
  a development with nothing lacking, one built from a piloted Command Wolf with a
  weapon on its rack (and with that weapon's stock full), one refused by a pilot who
  keeps his Zoid, the money lacking, いいえ on the question, the parts' pages of the Zoid
  and of a unit, and every way back: the same scripts in the same order, the sounds on
  the same frames but for B and a page turn a frame apart, and the same screens but for
  the transitions and the wallpaper. The original's state after a development was read
  from RAM: the unit built from is replaced by the new one in its slot, its weapon in the
  stock and its pilot without a unit.
- Dr. T's lab: every frame from the talk to the welcome, through a revival refused for
  money, one paid for, the keeper's word on the repair and the way out, matches once
  aligned, but for the transitions and the wallpaper visible between the windows.

- Opening an item shop and leaving it matches pixel for pixel from the talk to the
  field's return. So does opening the armaments shop.
- Buying, the notices, selling and cancelling a question match as sequences of screens
  in both shops: every window and text in the same place with the same values.
- The shops of areas 2 and 3 (with saves changed by hand to stand the player before
  each keeper): Sand Colony's lab (map 33), area 3's item shop 3 (map 52), armaments
  shop 3 (map 53) and labs 3 (map 62) and 4 (map 90) open on the same frame as the
  original's, but for the labs of area 3 (below), and show the same goods, prices and
  menus.

## Differences

- The port draws each step of a list or a question at once and counts the frames its
  scripts would have cost, as it does in the pause menu. The original redraws over one
  to eight frames, so the port's windows change that much earlier.
- In area 3 the lab keeper's rebuild of the object states (`0x08006E4C`) runs past its
  frame: about 296,000 cycles (a frame is 280,896) to draw formations for the area's 88
  map Zoids, against about 69,000 in areas 1 and 2 (23 and 27). The original's lab
  there opens, and every frame after, one frame later than the port's, which does not
  count the time.
- The original drops a frame here and there while it prints (one to three when a list
  or a question opens), which stands the wallpaper still. The port models only the
  welcome's, so after the first list its wallpaper runs a few pixels ahead.
- The Zoid lab's ゾイド乗せ換え and ゾイドを売る, and START on the revival's list (the
  Zoid's details), are not implemented: the port answers with the pause menu's
  まだできてません.
