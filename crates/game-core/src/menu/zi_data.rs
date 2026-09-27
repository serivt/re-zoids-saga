//! The status list's Zi data and Zi-data items: six to a page on the
//! right, the one under the cursor on the left.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! menu task's states `0x1400` (the Zi data, from `0x0804FB48`) and
//! `0x1500` (the Zi-data items, `0x0804FE28`), the lists' builders
//! `0x0804E3A0` and `0x0804E308`, and the status list's refusals at
//! `0x0804EB90`; see `docs/menu.md`.

use extraction::saga_party::{self, NOT_NEEDED, ZI_DATA_ZOIDS};
use extraction::saga_shop::{self, Item, ItemKind};

use super::{
    CONFIRMED, LEAVE_SOUND, LEFT_ALIGNED, MENU_MOVE_SOUND, MOVED_DOWN, MOVED_UP, MenuState,
    PAGE_LEFT, PAGE_RIGHT, PauseMenu, SCRIPT_CLEAR_CHARACTER, SCRIPT_CLEAR_STOCK,
    SCRIPT_DRAW_CHARACTER, SCRIPT_DRAW_STOCK, SCRIPT_MEMBER_MENU, SCRIPT_NO_ZI_DATA,
    SCRIPT_NO_ZI_ITEMS, SCRIPT_PRESENT_ALL, SCRIPT_SPACE, SCRIPT_TIMES, STOCK_LIST_WINDOW,
    STOCK_PAGE, STOCK_WINDOW, ZERO_PADDED, close_status_windows, label_len, parts::put_value,
};
use crate::ScriptHost;
use crate::script::ScriptError;
use crate::windows::ScriptWindows;

/// The Zi data's help line and windows, and its labels: 必要金額：,
/// Ｇ and 必要ゾイド：, Ｚｉデータ用アイテム：, and なし.
const SCRIPT_ZI_DATA_WINDOWS: usize = 108;
const SCRIPT_MONEY_NEEDED: usize = 109;
const SCRIPT_ZOID_NEEDED: usize = 110;
const SCRIPT_ITEMS_NEEDED: usize = 111;
const SCRIPT_NONE: usize = 112;
/// The Zi-data items' help line and windows.
const SCRIPT_ZI_ITEM_WINDOWS: usize = 55;
/// The special Zoids a development may ask for, from `0xFA`.
const SCRIPT_SPECIAL_ZOIDS: usize = 219;
const SPECIAL_ZOID: u8 = 0xFA;
/// The Zi-data items' texts.
const SCRIPT_ZI_ITEM_TEXTS: usize = 635;
/// The Zi-data items the game state counts (`+0x330C`).
const ZI_ITEMS: u8 = 64;
const MONEY_CELLS: usize = 7;
const COUNT_CELLS: usize = 2;
const ITEM_NAME_CELLS: usize = 8;

/// Which of the two lists is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum ZiList {
    /// The Zoids whose Zi data the party holds.
    #[default]
    Data,
    /// The Zi-data items the party carries.
    Items,
}

impl PauseMenu {
    /// ステータス → Ｚｉデータ: the Zoids whose Zi data the party holds, in
    /// picture order (`0x0804E3A0`); with none, notice 58.
    pub(super) fn open_zi_data(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.zi_list = (0..ZI_DATA_ZOIDS)
            .filter(|&zoid| formats::progress::zoid_seen(&self.game_state, usize::from(zoid)))
            .collect();
        if self.zi_list.is_empty() {
            return self.empty_list(SCRIPT_NO_ZI_DATA, windows);
        }
        self.open_zi(rom, ZiList::Data, SCRIPT_ZI_DATA_WINDOWS, windows)
    }

    /// ステータス → Ｚｉデータ用アイテム: the Zi-data items the party
    /// carries, in id order (`0x0804E308`); with none, notice 60.
    pub(super) fn open_zi_items(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.zi_list = (0..ZI_ITEMS)
            .filter(|&id| saga_shop::item_count(&self.game_state, core(id)) > 0)
            .collect();
        if self.zi_list.is_empty() {
            return self.empty_list(SCRIPT_NO_ZI_ITEMS, windows);
        }
        self.open_zi(rom, ZiList::Items, SCRIPT_ZI_ITEM_WINDOWS, windows)
    }

    fn open_zi(
        &mut self,
        rom: &[u8],
        list: ZiList,
        script: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        close_status_windows(windows);
        self.run_now(rom, script, windows)?;
        self.zi_kind = list;
        self.zi_page = 0;
        self.zi_shown = None;
        self.show_zi(rom, true, 0, windows)
    }

    /// Prints the list's page when it changed and the entry on `line` when
    /// it changed, then runs the list's menu again.
    fn show_zi(
        &mut self,
        rom: &[u8],
        page_changed: bool,
        line: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let start = self.zi_page * STOCK_PAGE;
        let page: Vec<u8> = self
            .zi_list
            .iter()
            .skip(start)
            .take(STOCK_PAGE)
            .copied()
            .collect();
        let mut line = line;
        if page_changed {
            self.run_now(rom, SCRIPT_CLEAR_STOCK, windows)?;
            for (index, id) in page.iter().enumerate() {
                self.print_zi_entry(rom, *id, windows)?;
                if index + 1 < STOCK_PAGE && start + index + 1 < self.zi_list.len() {
                    windows.line_break(STOCK_LIST_WINDOW);
                }
            }
            line = line.min(page.len().saturating_sub(1));
            windows.set_cursor(STOCK_LIST_WINDOW, Some(line));
            windows.set_cursor(STOCK_LIST_WINDOW, None);
            let more = self.zi_list.len() > start + STOCK_PAGE;
            windows.set_scroll_marks(STOCK_LIST_WINDOW, (self.zi_page > 0, more));
        }
        let selected = start + line;
        if self.zi_shown != Some(selected)
            && let Some(id) = self.zi_list.get(selected).copied()
        {
            if self.zi_shown.is_some() {
                self.run_now(rom, SCRIPT_CLEAR_CHARACTER, windows)?;
            }
            self.zi_shown = Some(selected);
            self.run_now(rom, SCRIPT_DRAW_CHARACTER, windows)?;
            match self.zi_kind {
                ZiList::Data => self.describe_zi_data(rom, id, windows)?,
                ZiList::Items => self.describe_zi_item(rom, id, windows)?,
            }
        }
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.run_now(rom, SCRIPT_DRAW_STOCK, windows)?;
        self.runner.start(SCRIPT_MEMBER_MENU)?;
        self.state = MenuState::ZiList;
        Ok(())
    }

    /// A line of the list: the Zoid's name, or the item's name padded to
    /// eight cells, × and its count.
    fn print_zi_entry(
        &mut self,
        rom: &[u8],
        id: u8,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let window = STOCK_LIST_WINDOW;
        match self.zi_kind {
            ZiList::Data => self.print_zoid_name(rom, u16::from(id), window, windows),
            ZiList::Items => {
                self.run_item_script(rom, usize::from(id), window, windows)?;
                let name = self.items.string_offset(usize::from(id)).unwrap_or(0);
                for _ in label_len(rom, name)..ITEM_NAME_CELLS {
                    self.run_in(rom, window, SCRIPT_SPACE, windows)?;
                }
                self.run_in(rom, window, SCRIPT_TIMES, windows)?;
                let count = saga_shop::item_count(&self.game_state, core(id));
                put_value(windows, window, i32::from(count), COUNT_CELLS, ZERO_PADDED);
                Ok(())
            }
        }
    }

    /// What developing the Zoid under the cursor asks for: the money, the
    /// Zoid (none, one of the Zoids, or a special kind) and the two
    /// Zi-data items (none, or the one or two needed).
    fn describe_zi_data(
        &mut self,
        rom: &[u8],
        zoid: u8,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(development) = saga_party::development(rom, zoid) else {
            return Ok(());
        };
        let window = STOCK_WINDOW;
        self.run_in(rom, window, SCRIPT_MONEY_NEEDED, windows)?;
        let money = i32::try_from(development.money).unwrap_or(i32::MAX);
        put_value(windows, window, money, MONEY_CELLS, LEFT_ALIGNED);
        self.run_in(rom, window, SCRIPT_ZOID_NEEDED, windows)?;
        self.run_in(rom, window, SCRIPT_SPACE, windows)?;
        match development.zoid {
            0 => self.run_in(rom, window, SCRIPT_NONE, windows)?,
            special @ SPECIAL_ZOID.. => {
                let script = SCRIPT_SPECIAL_ZOIDS + usize::from(special - SPECIAL_ZOID);
                self.run_in(rom, window, script, windows)?;
            }
            other => self.print_zoid_name(rom, u16::from(other), window, windows)?,
        }
        windows.line_break(window);
        self.run_in(rom, window, SCRIPT_ITEMS_NEEDED, windows)?;
        self.run_in(rom, window, SCRIPT_SPACE, windows)?;
        match development.items {
            [NOT_NEEDED, NOT_NEEDED] => self.run_in(rom, window, SCRIPT_NONE, windows),
            [NOT_NEEDED, second] => self.run_item_script(rom, usize::from(second), window, windows),
            [first, second] => {
                self.run_item_script(rom, usize::from(first), window, windows)?;
                windows.line_break(window);
                if second != NOT_NEEDED {
                    self.run_in(rom, window, SCRIPT_SPACE, windows)?;
                    self.run_item_script(rom, usize::from(second), window, windows)?;
                }
                Ok(())
            }
        }
    }

    /// The item under the cursor: its name and its text.
    fn describe_zi_item(
        &mut self,
        rom: &[u8],
        id: u8,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let window = STOCK_WINDOW;
        self.run_item_script(rom, usize::from(id), window, windows)?;
        windows.line_break(window);
        self.run_in(rom, window, SCRIPT_ZI_ITEM_TEXTS + usize::from(id), windows)
    }

    /// What the list's menu ended with: a cursor move shows that entry, L
    /// and R turn the page (sound `0x40`), A or B go back to the status
    /// list, B with sound `0x3F`.
    pub(super) fn zi_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let line = usize::from(line);
        let current = self.zi_shown.map_or(0, |shown| shown % STOCK_PAGE);
        match code {
            MOVED_UP | MOVED_DOWN => self.show_zi(rom, false, line, windows),
            PAGE_LEFT if self.zi_page > 0 => {
                windows.play_sound(MENU_MOVE_SOUND);
                self.zi_page -= 1;
                self.show_zi(rom, true, current, windows)
            }
            PAGE_RIGHT if self.zi_list.len() > (self.zi_page + 1) * STOCK_PAGE => {
                windows.play_sound(MENU_MOVE_SOUND);
                self.zi_page += 1;
                self.show_zi(rom, true, current, windows)
            }
            PAGE_LEFT | PAGE_RIGHT => self.show_zi(rom, false, current, windows),
            _ => {
                if code != CONFIRMED {
                    windows.play_sound(LEAVE_SOUND);
                }
                self.rebuild_status(rom, windows)
            }
        }
    }
}

/// Zi-data item `id`, which the game state counts with the lab's cores.
const fn core(id: u8) -> Item {
    Item {
        kind: ItemKind::Core,
        id,
    }
}
