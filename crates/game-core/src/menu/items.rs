//! The main menu's アイテム: the party's items, the Zoid to use one on and
//! the effect (the pause menu's states `0x2000`–`0x2100`).
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the menu
//! task's states at `0x08050374`–`0x08050EA4`, the item list it builds
//! (`0x0804E2C4`), the hit points' printer (`0x0804E3E4`), the effects
//! (`0x08039580` with the routines at ROM `0x683AA8`) and the check of the
//! capsule (`0x08009A2C`); checked against the menu opened in a reference
//! emulator with items set by hand, breakpoints on the script runner and
//! the sound call. See `docs/menu.md`.

use extraction::saga_party;

use super::{
    CONFIRMED, EMPTY_BACK_SOUND, EMPTY_SOUND, HELP_WINDOW, LEAVE_SOUND, MENU_MOVE_SOUND,
    MOVED_DOWN, MOVED_UP, MenuState, PAGE_LEFT, PAGE_RIGHT, PauseMenu, Return,
    SCRIPT_CLEAR_CHARACTER, SCRIPT_CLEAR_HELP, SCRIPT_CLEAR_MEMBERS, SCRIPT_CLEAR_STOCK,
    SCRIPT_CLOSE, SCRIPT_DISABLED, SCRIPT_DRAW_CHARACTER, SCRIPT_DRAW_HELP, SCRIPT_DRAW_MEMBERS,
    SCRIPT_DRAW_STOCK, SCRIPT_MEMBER_MENU, SCRIPT_NO_ITEMS, SCRIPT_PERCENT, SCRIPT_PRESENT,
    SCRIPT_PRESENT_ALL, SCRIPT_SPACE, SCRIPT_TIMES, SCRIPT_WAIT_KEY, UNIT_DISABLED, ZERO_PADDED,
    label_len,
};
use crate::ScriptHost;
use crate::script::ScriptError;
use crate::windows::ScriptWindows;

/// The item counts of the game state; the menu lists items 0–6.
const ITEM_COUNTS: usize = 0x3305;
const ITEMS: u8 = 7;
/// ショックウエイブ, which works in battle only.
const SHOCK_WAVE: u8 = 3;
/// 緊急退避カプセル, back to the lab where the place allows it.
const CAPSULE: u8 = 6;
const PAGE_LINES: usize = 6;
const NAME_CELLS: usize = 8;
const COUNT_CELLS: usize = 2;
const DESCRIPTION_WINDOW: u8 = 1;
const LIST_WINDOW: u8 = 2;
const TARGET_WINDOW: u8 = 1;
const TARGET_LIST_WINDOW: u8 = 3;
/// The item names (`name` 241 on), help (`item` 63 on) and what using one
/// did (`item` 70 on).
const ITEM_NAMES: usize = 241;
const ITEM_HELP: usize = 63;
const ITEM_MESSAGES: usize = 70;
// Scripts of the pause-menu table.
const SCRIPT_ITEM_WINDOWS: usize = 50;
const SCRIPT_ITEM_QUESTION: usize = 51;
const SCRIPT_NOT_HERE: usize = 53;
const SCRIPT_BATTLE_ONLY: usize = 54;
const SCRIPT_TARGET_WINDOWS: usize = 117;
const SCRIPT_NO_ZOID: usize = 118;
const SCRIPT_HP: usize = 119;
const SCRIPT_EP: usize = 120;
const SCRIPT_SP: usize = 121;
const SCRIPT_DF: usize = 122;
const SCRIPT_WHO: usize = 123;
const SCRIPT_NOT_PILOTED: usize = 124;
const SCRIPT_USED: usize = 125;
const SCRIPT_BROKEN: usize = 126;
const SCRIPT_FULL: usize = 127;
const USE_SOUND: u8 = 0x50;
const HP_CELLS: usize = 4;
const EP_CELLS: usize = 5;
const DF_CELLS: usize = 3;
/// A plain number: blanks before it.
const PLAIN: u8 = 0;

/// Where the item screens stand.
#[derive(Debug, Clone, Default)]
pub(super) struct ItemMenu {
    /// The items the party has, by id (`0x0200E779`, counted at
    /// `0x0200E780`).
    list: Vec<u8>,
    page: usize,
    line: usize,
    /// Whether a page of the list has been printed since its windows
    /// opened, and the entry described.
    page_shown: bool,
    shown: Option<usize>,
    /// The item chosen.
    item: u8,
    target_page: usize,
    target_line: usize,
    target_page_shown: bool,
    target_shown: Option<usize>,
    /// Whether the member described had no Zoid.
    target_without_zoid: bool,
}

impl PauseMenu {
    /// The items the party has, in id order (`0x0804E2C4`).
    fn party_items(&self) -> Vec<u8> {
        (0..ITEMS)
            .filter(|&item| {
                self.game_state
                    .get(ITEM_COUNTS + usize::from(item))
                    .is_some_and(|&count| count > 0)
            })
            .collect()
    }

    /// アイテム on the main list: with no items sound `0x4F` and notice 56,
    /// otherwise the list (state `0x2000`): the menu's windows close and
    /// script 50 opens the description and the list.
    pub(super) fn open_items(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.item_menu.list = self.party_items();
        if self.item_menu.list.is_empty() {
            windows.play_sound(EMPTY_SOUND);
            return self.notice(SCRIPT_NO_ITEMS, Return::EmptyMain);
        }
        for id in (1..=3).rev() {
            self.run_now(rom, SCRIPT_CLOSE + id, windows)?;
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.item_menu.page = 0;
        self.item_menu.line = 0;
        self.enter_items(rom, windows)
    }

    /// State `0x2001`: script 50, then the list where it was, all of it
    /// printed again.
    fn enter_items(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_ITEM_WINDOWS, windows)?;
        self.item_menu.page_shown = false;
        self.item_menu.shown = None;
        let line = self.item_menu.line;
        self.show_items(rom, true, line, windows)
    }

    /// The list's loop: its page when it changed (six items, each name
    /// padded to eight cells, × and the count), the item under the cursor
    /// described on the left when it changed, then the menu.
    fn show_items(
        &mut self,
        rom: &[u8],
        page_changed: bool,
        line: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let mut line = line;
        if page_changed {
            while self.item_menu.page > 0
                && self.item_menu.page * PAGE_LINES >= self.item_menu.list.len()
            {
                self.item_menu.page -= 1;
            }
            if self.item_menu.page_shown {
                self.run_now(rom, SCRIPT_CLEAR_STOCK, windows)?;
            }
            self.item_menu.page_shown = true;
            let start = self.item_menu.page * PAGE_LINES;
            let page: Vec<u8> = self
                .item_menu
                .list
                .iter()
                .skip(start)
                .take(PAGE_LINES)
                .copied()
                .collect();
            for (index, item) in page.iter().enumerate() {
                let name = ITEM_NAMES + usize::from(*item);
                self.print_name(rom, name, LIST_WINDOW, windows)?;
                let offset = self.names.string_offset(name).unwrap_or(0);
                for _ in label_len(rom, offset)..NAME_CELLS {
                    self.run_in(rom, LIST_WINDOW, SCRIPT_SPACE, windows)?;
                }
                self.run_in(rom, LIST_WINDOW, SCRIPT_TIMES, windows)?;
                let count = self.item_count(*item);
                super::parts::put_value(
                    windows,
                    LIST_WINDOW,
                    i32::from(count),
                    COUNT_CELLS,
                    ZERO_PADDED,
                );
                if index + 1 < PAGE_LINES && start + index + 1 < self.item_menu.list.len() {
                    windows.line_break(LIST_WINDOW);
                }
            }
            line = line.min(page.len().saturating_sub(1));
            windows.set_cursor(LIST_WINDOW, Some(line));
            windows.set_cursor(LIST_WINDOW, None);
            let more = self.item_menu.list.len() > start + PAGE_LINES;
            windows.set_scroll_marks(LIST_WINDOW, (self.item_menu.page > 0, more));
        }
        self.item_menu.line = line;
        let selected = self.item_menu.page * PAGE_LINES + line;
        if self.item_menu.shown != Some(selected)
            && let Some(item) = self.item_menu.list.get(selected).copied()
        {
            if self.item_menu.shown.is_some() {
                self.run_now(rom, SCRIPT_CLEAR_CHARACTER, windows)?;
            }
            self.item_menu.shown = Some(selected);
            self.run_now(rom, SCRIPT_DRAW_CHARACTER, windows)?;
            self.print_name(
                rom,
                ITEM_NAMES + usize::from(item),
                DESCRIPTION_WINDOW,
                windows,
            )?;
            windows.line_break(DESCRIPTION_WINDOW);
            self.run_item_script(
                rom,
                ITEM_HELP + usize::from(item),
                DESCRIPTION_WINDOW,
                windows,
            )?;
            self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
            self.run_now(rom, SCRIPT_DRAW_STOCK, windows)?;
        }
        self.runner.start(SCRIPT_MEMBER_MENU)?;
        self.state = MenuState::Items;
        Ok(())
    }

    fn item_count(&self, item: u8) -> u8 {
        self.game_state
            .get(ITEM_COUNTS + usize::from(item))
            .copied()
            .unwrap_or(0)
    }

    /// What the list's menu ended with: a move describes that item, L and
    /// R turn the page (sound `0x40`); A on ショックウエイブ or on the
    /// capsule where it cannot go says so, on another item asks for the
    /// Zoid; B goes back to the main menu with sound `0x3F`.
    pub(super) fn item_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let current = self.item_menu.line;
        let more = self.item_menu.list.len() > (self.item_menu.page + 1) * PAGE_LINES;
        match code {
            MOVED_UP | MOVED_DOWN => self.show_items(rom, false, usize::from(line), windows),
            PAGE_LEFT if self.item_menu.page > 0 => {
                windows.play_sound(MENU_MOVE_SOUND);
                self.item_menu.page -= 1;
                self.show_items(rom, true, current, windows)
            }
            PAGE_RIGHT if more => {
                windows.play_sound(MENU_MOVE_SOUND);
                self.item_menu.page += 1;
                self.show_items(rom, true, current, windows)
            }
            PAGE_LEFT | PAGE_RIGHT => self.show_items(rom, false, current, windows),
            CONFIRMED => {
                let selected = self.item_menu.page * PAGE_LINES + current;
                let Some(item) = self.item_menu.list.get(selected).copied() else {
                    return self.show_items(rom, false, current, windows);
                };
                let refusal = match item {
                    CAPSULE => Some(SCRIPT_NOT_HERE),
                    SHOCK_WAVE => Some(SCRIPT_BATTLE_ONLY),
                    _ => None,
                };
                if let Some(script) = refusal {
                    windows.play_sound(EMPTY_SOUND);
                    return self.notice(script, Return::ItemList);
                }
                self.item_menu.item = item;
                self.close_item_list(rom, windows)?;
                self.open_targets(rom, windows)
            }
            _ => {
                self.close_item_list(rom, windows)?;
                self.delayed_sound = Some((self.busy.saturating_sub(1), LEAVE_SOUND));
                self.build(rom, windows)?;
                self.return_to(rom, Return::Main, windows)
            }
        }
    }

    /// Scripts 10, 9 and 0: the list and the description close, the help
    /// line clears.
    fn close_item_list(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.set_scroll_marks(LIST_WINDOW, (false, false));
        self.run_now(rom, SCRIPT_CLOSE + usize::from(LIST_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(DESCRIPTION_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)
    }

    /// Back to the list after a refusal: sound `0x41`, the question again
    /// and the list's menu.
    pub(super) fn items_again(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.play_sound(EMPTY_BACK_SOUND);
        self.run_now(rom, SCRIPT_ITEM_QUESTION, windows)?;
        self.run_now(rom, SCRIPT_DRAW_STOCK, windows)?;
        let line = self.item_menu.line;
        self.show_items(rom, false, line, windows)
    }

    /// The Zoid to use the item on (state `0x2100`): script 117 opens the
    /// Zoid's window and the members', the first member under the cursor.
    fn open_targets(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_TARGET_WINDOWS, windows)?;
        self.item_menu.target_page = 0;
        self.item_menu.target_line = 0;
        self.item_menu.target_page_shown = false;
        self.item_menu.target_shown = None;
        self.item_menu.target_without_zoid = false;
        self.target_question(rom, windows)?;
        self.show_targets(rom, true, 0, windows)
    }

    /// The help line: the item's name and を誰が搭乗しているゾイドに使いますか？,
    /// then window 3 is drawn.
    fn target_question(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        let item = self.item_menu.item;
        self.print_name(rom, ITEM_NAMES + usize::from(item), HELP_WINDOW, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_WHO, windows)?;
        self.run_now(rom, SCRIPT_DRAW_MEMBERS, windows)
    }

    /// The members' loop: the page of names when it changed, the Zoid of
    /// the one under the cursor when it changed (or 搭乗ゾイドなし), then
    /// the menu.
    fn show_targets(
        &mut self,
        rom: &[u8],
        page_changed: bool,
        line: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let count = self.roster.members.len();
        let start = self.item_menu.target_page * PAGE_LINES;
        let mut line = line;
        if page_changed {
            if self.item_menu.target_page_shown {
                self.run_now(rom, SCRIPT_CLEAR_MEMBERS, windows)?;
            }
            self.item_menu.target_page_shown = true;
            let characters: Vec<u8> = self
                .roster
                .members
                .iter()
                .skip(start)
                .take(PAGE_LINES)
                .map(|member| member.character)
                .collect();
            for (index, character) in characters.iter().enumerate() {
                self.print_character_name(rom, *character, TARGET_LIST_WINDOW, windows)?;
                if index + 1 < PAGE_LINES && start + index + 1 < count {
                    windows.line_break(TARGET_LIST_WINDOW);
                }
            }
            line = line.min(characters.len().saturating_sub(1));
            windows.set_cursor(TARGET_LIST_WINDOW, Some(line));
            windows.set_cursor(TARGET_LIST_WINDOW, None);
            let more = count > start + PAGE_LINES;
            windows.set_scroll_marks(TARGET_LIST_WINDOW, (self.item_menu.target_page > 0, more));
        }
        self.item_menu.target_line = line;
        let selected = start + line;
        if self.item_menu.target_shown == Some(selected) {
            self.run_now(rom, SCRIPT_PRESENT + usize::from(HELP_WINDOW), windows)?;
        } else {
            self.item_menu.target_shown = Some(selected);
            self.member = selected;
            let unit = self.roster.members.get(selected).and_then(|m| m.unit);
            match unit {
                Some(_) => self.draw_target(rom, windows)?,
                None if !self.item_menu.target_without_zoid => {
                    self.shown_zoid = None;
                    self.run_now(rom, SCRIPT_CLEAR_CHARACTER, windows)?;
                    self.run_now(rom, SCRIPT_DRAW_CHARACTER, windows)?;
                    self.run_in(rom, TARGET_WINDOW, SCRIPT_NO_ZOID, windows)?;
                    self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
                }
                None => {}
            }
            self.item_menu.target_without_zoid = unit.is_none();
        }
        self.run_now(rom, SCRIPT_DRAW_MEMBERS, windows)?;
        self.runner.start(SCRIPT_MEMBER_MENU)?;
        self.state = MenuState::ItemTarget;
        Ok(())
    }

    /// Window 1 for the member under the cursor: the Zoid's name, its hit
    /// points (or 戦闘不能) and full, energy points and full, SP, DF and its
    /// picture.
    fn draw_target(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(unit) = self.roster.members.get(self.member).and_then(|m| m.unit) else {
            return Ok(());
        };
        let window = TARGET_WINDOW;
        let signed = |value: u32| i32::from_ne_bytes(value.to_ne_bytes());
        self.shown_zoid = None;
        self.run_now(rom, SCRIPT_CLEAR_CHARACTER, windows)?;
        self.run_now(rom, SCRIPT_DRAW_CHARACTER, windows)?;
        self.print_zoid_name(rom, unit.zoid, window, windows)?;
        windows.line_break(window);
        self.run_in(rom, window, SCRIPT_HP, windows)?;
        self.run_in(rom, window, SCRIPT_SPACE, windows)?;
        if unit.flags & UNIT_DISABLED == 0 {
            super::parts::put_value(windows, window, signed(unit.hp.0), HP_CELLS, PLAIN);
        } else {
            self.run_in(rom, window, SCRIPT_DISABLED, windows)?;
        }
        windows.line_break(window);
        self.run_in(rom, window, SCRIPT_FULL, windows)?;
        super::parts::put_value(windows, window, signed(unit.hp.1), HP_CELLS, PLAIN);
        windows.line_break(window);
        self.run_in(rom, window, SCRIPT_EP, windows)?;
        super::parts::put_value(windows, window, signed(unit.ep.0), EP_CELLS, PLAIN);
        windows.line_break(window);
        self.run_in(rom, window, SCRIPT_FULL, windows)?;
        super::parts::put_value(windows, window, signed(unit.ep.1), HP_CELLS, PLAIN);
        windows.line_break(window);
        self.run_in(rom, window, SCRIPT_SP, windows)?;
        super::parts::put_value(windows, window, i32::from(unit.sp), HP_CELLS, PLAIN);
        self.run_in(rom, window, SCRIPT_DF, windows)?;
        super::parts::put_value(windows, window, i32::from(unit.df), DF_CELLS, PLAIN);
        self.run_in(rom, window, SCRIPT_PERCENT, windows)?;
        self.shown_zoid = Some(unit.zoid);
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)
    }

    /// What the members' menu ended with: moves and pages as the list's;
    /// A on a member without a Zoid or with a broken one says so (sound
    /// `0x4F`), otherwise uses the item (sound `0x50`), shows the Zoid
    /// again and what it did; B goes back to the list with sound `0x3F`.
    pub(super) fn target_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let current = self.item_menu.target_line;
        let count = self.roster.members.len();
        let more = count > (self.item_menu.target_page + 1) * PAGE_LINES;
        match code {
            MOVED_UP | MOVED_DOWN => self.show_targets(rom, false, usize::from(line), windows),
            PAGE_LEFT if self.item_menu.target_page > 0 => {
                windows.play_sound(MENU_MOVE_SOUND);
                self.item_menu.target_page -= 1;
                self.show_targets(rom, true, current, windows)
            }
            PAGE_RIGHT if more => {
                windows.play_sound(MENU_MOVE_SOUND);
                self.item_menu.target_page += 1;
                self.show_targets(rom, true, current, windows)
            }
            PAGE_LEFT | PAGE_RIGHT => self.show_targets(rom, false, current, windows),
            CONFIRMED => self.use_on_target(rom, windows),
            _ => {
                windows.play_sound(LEAVE_SOUND);
                self.leave_targets(rom, windows)
            }
        }
    }

    fn use_on_target(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(member) = self.roster.members.get(self.member).copied() else {
            return self.leave_targets(rom, windows);
        };
        let unit = saga_party::character_unit(&self.game_state, member.character);
        let refusal = match (unit, member.unit) {
            (Some(_), Some(status)) if status.flags & UNIT_DISABLED == 0 => None,
            (Some(_), Some(_)) => Some(SCRIPT_BROKEN),
            _ => Some(SCRIPT_NOT_PILOTED),
        };
        if let Some(script) = refusal {
            windows.play_sound(EMPTY_SOUND);
            self.run_now(rom, script, windows)?;
            return self.notice(SCRIPT_WAIT_KEY, Return::ItemTarget);
        }
        let (Some(unit), Some(status)) = (unit, member.unit) else {
            return self.leave_targets(rom, windows);
        };
        let item = self.item_menu.item;
        windows.play_sound(USE_SOUND);
        saga_party::use_item(&mut self.game_state, unit, item);
        if let Some(count) = self.game_state.get_mut(ITEM_COUNTS + usize::from(item)) {
            *count = count.saturating_sub(1);
        }
        self.roster = crate::GameData::new(rom).roster(&self.game_state);
        self.draw_target(rom, windows)?;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        self.print_name(rom, ITEM_NAMES + usize::from(item), HELP_WINDOW, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_USED, windows)?;
        windows.line_break(HELP_WINDOW);
        self.print_zoid_name(rom, status.zoid, HELP_WINDOW, windows)?;
        self.run_item_script(rom, ITEM_MESSAGES + usize::from(item), HELP_WINDOW, windows)?;
        self.run_now(rom, SCRIPT_PRESENT + usize::from(HELP_WINDOW), windows)?;
        self.notice(SCRIPT_WAIT_KEY, Return::ItemUsed)
    }

    /// Back to the members after a refusal: sound `0x41`, the help line
    /// cleared and asked again.
    pub(super) fn targets_again(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.play_sound(EMPTY_BACK_SOUND);
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.target_question(rom, windows)?;
        let line = self.item_menu.target_line;
        self.show_targets(rom, false, line, windows)
    }

    /// After an item's message: sound `0x41`, the list counted again, and
    /// the members' screen closes.
    pub(super) fn item_used(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.play_sound(EMPTY_BACK_SOUND);
        self.item_menu.list = self.party_items();
        self.leave_targets(rom, windows)
    }

    /// The members' screen closes (scripts 11 and 9, the picture hidden):
    /// back to the list where it was, or with no items left notice 56 and
    /// the main menu.
    fn leave_targets(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.set_scroll_marks(TARGET_LIST_WINDOW, (false, false));
        self.run_now(rom, SCRIPT_CLOSE + usize::from(TARGET_LIST_WINDOW), windows)?;
        self.shown_zoid = None;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(TARGET_WINDOW), windows)?;
        if self.item_menu.list.is_empty() {
            return self.notice(SCRIPT_NO_ITEMS, Return::MainRebuilt);
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.enter_items(rom, windows)
    }
}
