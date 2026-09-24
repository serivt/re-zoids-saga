# Shops

Source of knowledge: own reading of Zoids Saga (Japan, Rev 1) at the addresses named
below, plus breakpoints on the script runner, the sound call, the task spawner and
the warp, per-frame logs of the fade level and the wallpaper's scroll, and screenshots
of every frame in a reference emulator while buying and selling in Arcana's item shop
(map 25) and armaments shop (map 26). The port's frames were compared with the
original's on the same inputs. Implemented in `crates/game-core/src/menu/shop.rs` (the
screens, as a mode of the pause menu), `crates/extraction/src/saga_shop.rs` (goods,
prices and counts), and `crates/game-core/src/story.rs` (the keepers).

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

## Checked against the original

- Opening an item shop and leaving it matches pixel for pixel from the talk to the
  field's return. So does opening the armaments shop.
- Buying, the notices, selling and cancelling a question match as sequences of screens
  in both shops: every window and text in the same place with the same values.

## Differences

- The port draws each step of a list or a question at once and counts the frames its
  scripts would have cost, as it does in the pause menu. The original redraws over one
  to seven frames, so the port's windows change that much earlier.
- The original drops a frame here and there while it prints (one to three when a list
  or a question opens), which stands the wallpaper still. The port models only the
  welcome's, so after the first list its wallpaper runs a few pixels ahead.
- The Zoid lab (kind 2) and the other towns' shops are not implemented.
