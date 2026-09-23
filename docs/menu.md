# Pause menu

Source of knowledge: screenshots and RAM dumps of Zoids Saga (Japan, Rev 1) in a
reference emulator while every item of the START menu was visited from the first room
(`research/build/mgba/menu/`), a watch on the interpreter's current-window pointer
(RAM `0x02009118`) while the menu opened, and a read of the pause-menu script table
and of the menu, present and message opcode handlers. Implemented in
`crates/game-core/src/menu.rs`; data in `crates/extraction/src/saga.rs`
(`PAUSE_MENU_SCRIPTS`, `pause_wallpaper`).

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
| 56, 57, 58, 60 | The notices for no items, weapons, Zi data or Zi-data items: clear the help line, print, present it, wait for a key, clear |
| 128, 129, 133, 134 | The weapons screen: window 1 at (0, 0) 18×14 with 搭乗ゾイドなし and window 3 at (17, 0) 13×14 with the party's names as a menu; 133 asks whose Zoid to change, 134 answers that the character is not aboard |
| 151, 152, 153, 154–158 | The message-speed setting: window 4 at (9, 0) 15×4 with 戦闘メッセージ速度 and the value, the help text, window 5 at (23, 0) 7×14 with １–５ and ボタン, the menu, then the value strings |
| 160, 61, 161, 162 | セーブしますか？, the cancelable はい/いいえ window 7 at (11, 4) 8×6, セーブしました, セーブを中止しました |
| 115, 116 | The encyclopedia choice, window 5 at (23, 0) 7×6 with ゾイド / キャラ |
| 63, 37 | まだできてません and a cancelable key wait, which this port shows for the screens it lacks |

The panel prints the player's name before the level label and right-aligns the numbers
with full-width digits so the value ends in the panel's last inner cell; the money box
right-aligns the amount before Ｇ. A new game shows level 1, 0 experience, 14 to the
next level and 0 G.

## Windows and menus

Three rules of the original, taken from its handlers, make the scripts work:

- The interpreter keeps one current window. Opening a window makes it current; a message
  starts in it, and `0x1C` inside the message only redirects that message's text. The
  menu and key-wait opcodes act on the current window.
- Presenting a window makes it current; presenting all of them (`0xFF`) redraws each open
  window in id order and leaves the highest one current. That is how script 47 prints its
  help in window 0 yet runs its menu in window 3, and how a notice presented alone moves
  the key prompt to the help line.
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

## Flows

START on the field opens the menu; B on the main list closes it. ステータス opens the
status list; its 武器, Ｚｉデータ and Ｚｉデータ用アイテム items print their notices and
図鑑 asks ゾイド or キャラ. 武装 shows the weapons screen; choosing the character prints
that no Zoid is boarded. コンフィグ shows the message speed (3 on a new game) with the
cursor on it; picking a number stores it in the party. セーブ asks, and answers
セーブしました or セーブを中止しました (B cancels); nothing is written anywhere yet.

## Not modeled yet

The 部隊 and キャラクター status screens, the encyclopedia itself, 部隊編成 (a separate
screen with its own wallpaper), the ボタン page of the config, and where the original
goes after a speed is chosen (this port returns to the main list) all end in the
まだできてません notice or in an assumption. Saving writes no data. The button and
cursor sounds are requested but not played until the sound engine exists.
