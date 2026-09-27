//! The Zoid lab's ゾイド開発 (the lab task's states `0x200`–`0x221`): the
//! Zoids whose Zi data the party holds, the Zoid a development gives and
//! its weapons, the unit it is built from, and the development itself.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the lab
//! task's development states at `0x08056090`–`0x080571A0`, the shortfall
//! test at `0x0805534C`, the base list at `0x08055198` and the unit
//! clearing at `0x08055314`; checked against Sand Colony's lab in a
//! reference emulator with saves changed by hand (breakpoints on the
//! script runner and the sound call, RAM dumps before and after a
//! development, screenshots); see `docs/shop.md`.

use extraction::saga_party::{self, PART_SLOTS, Shortfall};

use super::super::parts::put_value;
use super::super::{
    CONFIRMED, EMPTY_BACK_SOUND, EMPTY_SOUND, EP_CELLS, HP_CELLS, LEAVE_SOUND, MENU_MOVE_SOUND,
    MOVED_DOWN, MOVED_UP, MenuState, PAGE_LEFT, PAGE_RIGHT, PauseMenu, SCRIPT_CLEAR_HELP,
    SCRIPT_CLOSE, SCRIPT_DF_LABEL, SCRIPT_DRAW_CHARACTER, SCRIPT_DRAW_MEMBERS, SCRIPT_EP_LABEL,
    SCRIPT_HP_LABEL, SCRIPT_MEMBER_MENU, SCRIPT_MONEY_WINDOW, SCRIPT_PERCENT, SCRIPT_PRESENT_ALL,
    SCRIPT_SIZES, SCRIPT_SP_LABEL, SCRIPT_YES_NO, ZOID_WINDOW,
};
use super::lab::{LIST_WINDOW, SCRIPT_REVIVAL_MENU};
use super::units::{LabList, SCRIPT_ARMS_HELP, Taking};
use super::{HELP_WINDOW, SCRIPT_DRAW_HELP, ShopStep};
use crate::ScriptHost;
use crate::script::ScriptError;
use crate::windows::ScriptWindows;

/// The development's windows (the list's and the Zi data's) and its
/// help, from pause-menu script 272 on.
const SCRIPT_WINDOWS: usize = 272;
const SCRIPT_WHICH: usize = 273;
const SCRIPT_STATS_WINDOWS: usize = 274;
const SCRIPT_STATS_HELP: usize = 275;
const SCRIPT_QUESTION: usize = 276;
const SCRIPT_DONE: usize = 277;
const SCRIPT_GIVEN_UP: usize = 278;
const SCRIPT_STRIP_QUESTION: usize = 279;
const SCRIPT_NOT_STRIPPED: usize = 280;
const SCRIPT_MONEY_AND: usize = 281;
const SCRIPT_MONEY: usize = 282;
const SCRIPT_ZOID_AND: usize = 283;
const SCRIPT_ZOID: usize = 284;
const SCRIPT_ITEMS: usize = 285;
const SCRIPT_LACKING: usize = 286;
const SCRIPT_WHICH_BASE: usize = 287;
const SCRIPT_THE_PILOT: usize = 289;
const SCRIPT_WONT_LEAVE: usize = 291;
const SCRIPT_BASE_WINDOWS: usize = 293;
const SCRIPT_ON_BOARD: usize = 295;
const SCRIPT_PILOTED: usize = 296;
const SCRIPT_TOO_MANY: usize = 261;
/// Waits for A, B or START (`0x80`).
const SCRIPT_STATS_WAIT: usize = 38;
const SCRIPT_CLEAR_DETAIL: usize = 2;
const SCRIPT_DRAW_DETAIL: usize = 27;
const SCRIPT_CLEAR_BASE: usize = 1;
const SCRIPT_CLEAR_LIST: usize = 3;
const SCRIPT_MONEY_UNIT: usize = super::super::SCRIPT_MONEY_UNIT;
const SCRIPT_SPACE: usize = super::super::SCRIPT_SPACE;
/// The Zi data's window.
const DETAIL_WINDOW: u8 = 2;
const PAGE_LINES: usize = 4;
const BASE_PAGE_LINES: usize = 6;
const STARTED: u16 = 0x80;
/// The units the party can hold.
const MOST_UNITS: u8 = 0x98;

/// Where the development stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::menu) enum Step {
    /// The Zoids whose Zi data the party holds (state `0x201`).
    List,
    /// The Zoid a development gives (state `0x202`).
    Stats,
    /// Page `n` of its parts (state `0x205`).
    Arms(usize),
    /// The notice of what the development lacks.
    Lacking,
    /// The units it can be built from (state `0x212`).
    Bases,
    /// Whether to take the chosen unit from its pilot.
    BaseQuestion,
    /// The notice that the pilot will not leave the unit.
    BaseRefused,
    /// Whether to develop (state `0x220`).
    Question,
    /// The keeper's answer: done, given up or refused.
    Answer,
    /// The notice that the party cannot hold another unit.
    TooMany,
}

/// The development's lists and choices.
#[derive(Debug, Clone, Default)]
pub(super) struct Development {
    zoids: Vec<u8>,
    page: usize,
    line: usize,
    page_shown: Option<usize>,
    shown: Option<usize>,
    bases: Vec<u8>,
    base_page: usize,
    base_line: usize,
    base_page_shown: Option<usize>,
    base_shown: Option<usize>,
    base: Option<u8>,
}

impl Development {
    fn zoid(&self) -> Option<u8> {
        self.zoids.get(self.page * PAGE_LINES + self.line).copied()
    }

    fn chosen_base(&self) -> Option<u8> {
        self.bases
            .get(self.base_page * BASE_PAGE_LINES + self.base_line)
            .copied()
    }
}

impl PauseMenu {
    fn development(&self) -> Development {
        self.shop
            .as_ref()
            .map(|session| session.lab.development.clone())
            .unwrap_or_default()
    }

    fn development_mut(&mut self) -> Option<&mut Development> {
        self.shop
            .as_mut()
            .map(|session| &mut session.lab.development)
    }

    fn develop_step(&mut self, step: Step) {
        self.state = MenuState::Shop(ShopStep::Develop(step));
    }

    /// ゾイド開発 with Zi data and room for a unit (state `0x200`): the
    /// lab's list and title close, script 272 opens the Zi data's window
    /// and the list's, and 273 asks どれを開発したいのですか？
    pub(super) fn open_development(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let zoids = (0..saga_party::ZI_DATA_ZOIDS)
            .filter(|&zoid| formats::progress::zoid_seen(&self.game_state, usize::from(zoid)))
            .collect();
        if let Some(development) = self.development_mut() {
            *development = Development {
                zoids,
                ..Development::default()
            };
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(LIST_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(DETAIL_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_WINDOWS, windows)?;
        self.run_now(rom, SCRIPT_WHICH, windows)?;
        self.show_developments(rom, windows)
    }

    /// The list again after a screen that closed it (state `0x201`): the
    /// money comes back and the list is printed anew, its cursor kept.
    fn reopen_developments(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.shown_zoid = None;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
        self.reopen_money(rom, windows)?;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_WINDOWS, windows)?;
        self.run_now(rom, SCRIPT_DRAW_MEMBERS, windows)?;
        self.run_now(rom, SCRIPT_WHICH, windows)?;
        if let Some(development) = self.development_mut() {
            development.page_shown = None;
            development.shown = None;
        }
        self.show_developments(rom, windows)
    }

    /// The money window, which the Zoid's window replaced: script 44, a
    /// space, the money in seven cells and Ｇ.
    fn reopen_money(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_MONEY_WINDOW, windows)?;
        self.run_in(rom, super::MONEY_WINDOW, SCRIPT_SPACE, windows)?;
        put_value(
            windows,
            super::MONEY_WINDOW,
            super::price_shown(self.party.money),
            super::PRICE_CELLS,
            0,
        );
        self.run_in(rom, super::MONEY_WINDOW, SCRIPT_MONEY_UNIT, windows)
    }

    /// The list's loop (`0x08056090`): four Zoids a page in window 3, the
    /// Zi data of the one under the cursor in window 2, then the menu.
    fn show_developments(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let development = self.development();
        let start = development.page * PAGE_LINES;
        let mut line = development.line;
        if development.page_shown != Some(development.page) {
            if development.page_shown.is_some() {
                self.run_now(rom, SCRIPT_CLEAR_LIST, windows)?;
            }
            let page: Vec<u8> = development
                .zoids
                .iter()
                .skip(start)
                .take(PAGE_LINES)
                .copied()
                .collect();
            for (index, zoid) in page.iter().enumerate() {
                self.print_zoid_name(rom, u16::from(*zoid), LIST_WINDOW, windows)?;
                if index + 1 < PAGE_LINES && start + index + 1 < development.zoids.len() {
                    windows.line_break(LIST_WINDOW);
                }
            }
            line = line.min(page.len().saturating_sub(1));
            windows.set_cursor(LIST_WINDOW, Some(line));
            windows.set_cursor(LIST_WINDOW, None);
            let more = development.zoids.len() > start + PAGE_LINES;
            windows.set_scroll_marks(LIST_WINDOW, (development.page > 0, more));
        }
        let selected = start + line;
        if development.shown != Some(selected)
            && let Some(zoid) = development.zoids.get(selected).copied()
        {
            if development.shown.is_some() {
                self.run_now(rom, SCRIPT_CLEAR_DETAIL, windows)?;
            }
            self.run_now(rom, SCRIPT_DRAW_DETAIL, windows)?;
            self.describe_zi_data(rom, zoid, DETAIL_WINDOW, windows)?;
        }
        if let Some(development) = self.development_mut() {
            development.line = line;
            development.page_shown = Some(development.page);
            development.shown = Some(selected);
        }
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.runner.start(SCRIPT_MEMBER_MENU)?;
        self.develop_step(Step::List);
        Ok(())
    }

    /// What a development's script ended with.
    pub(super) fn development_step(
        &mut self,
        rom: &[u8],
        step: Step,
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        match step {
            Step::List => self.development_choice(rom, code, line, windows),
            Step::Stats => self.stats_choice(rom, code, windows),
            Step::Arms(page) => self.arms_choice(rom, page, code, windows),
            Step::Lacking | Step::Answer => {
                windows.play_sound(EMPTY_BACK_SOUND);
                self.after_development(rom, windows)
            }
            Step::Bases => self.base_choice(rom, code, line, windows),
            Step::BaseQuestion => self.base_answer(rom, code, line, windows),
            Step::BaseRefused => {
                windows.play_sound(EMPTY_BACK_SOUND);
                self.show_bases(rom, true, windows)
            }
            Step::Question => self.development_answer(rom, code, line, windows),
            Step::TooMany => {
                windows.play_sound(EMPTY_BACK_SOUND);
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                self.run_now(rom, SCRIPT_CLOSE + usize::from(LIST_WINDOW), windows)?;
                self.run_now(rom, SCRIPT_CLOSE + usize::from(DETAIL_WINDOW), windows)?;
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                self.reopen_lab_menu(rom, windows)
            }
        }
    }

    /// Moves and pages print again; A shows the Zoid; B goes back to the
    /// lab's menu (sound `0x3F`).
    fn development_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let development = self.development();
        let more = development.zoids.len() > (development.page + 1) * PAGE_LINES;
        match code {
            MOVED_UP | MOVED_DOWN => {
                if let Some(development) = self.development_mut() {
                    development.line = usize::from(line);
                }
                self.show_developments(rom, windows)
            }
            PAGE_LEFT | PAGE_RIGHT => {
                let page = match code {
                    PAGE_LEFT if development.page > 0 => Some(development.page - 1),
                    PAGE_RIGHT if more => Some(development.page + 1),
                    _ => None,
                };
                if let Some(page) = page {
                    windows.play_sound(MENU_MOVE_SOUND);
                    if let Some(development) = self.development_mut() {
                        development.page = page;
                    }
                }
                self.show_developments(rom, windows)
            }
            CONFIRMED => {
                self.close_list_windows(rom, windows)?;
                self.show_development_stats(rom, windows)?;
                self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
                self.run_now(rom, SCRIPT_STATS_HELP, windows)?;
                self.runner.start(SCRIPT_STATS_WAIT)?;
                self.develop_step(Step::Stats);
                Ok(())
            }
            0 => {
                windows.play_sound(LEAVE_SOUND);
                self.run_now(rom, SCRIPT_CLOSE + usize::from(LIST_WINDOW), windows)?;
                self.run_now(rom, SCRIPT_CLOSE + usize::from(DETAIL_WINDOW), windows)?;
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                self.reopen_lab_menu(rom, windows)
            }
            _ => self.show_developments(rom, windows),
        }
    }

    /// Windows 3, 2 and 1 close and the help clears.
    fn close_list_windows(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.shown_zoid = None;
        for window in [LIST_WINDOW, DETAIL_WINDOW, ZOID_WINDOW] {
            self.run_now(rom, SCRIPT_CLOSE + usize::from(window), windows)?;
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)
    }

    /// The Zoid the development gives, in window 1 (script 274): its name
    /// and size, the hit, energy and SP points and DF its record gives,
    /// and its picture.
    fn show_development_stats(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(zoid) = self.development().zoid() else {
            return Ok(());
        };
        let zoid = u16::from(zoid);
        let Some(values) = saga_party::zoid_values(rom, zoid) else {
            return Ok(());
        };
        self.run_now(rom, SCRIPT_STATS_WINDOWS, windows)?;
        self.run_now(rom, SCRIPT_DRAW_CHARACTER, windows)?;
        self.print_zoid_name(rom, zoid, ZOID_WINDOW, windows)?;
        self.run_in(
            rom,
            ZOID_WINDOW,
            SCRIPT_SIZES + usize::from(values.size),
            windows,
        )?;
        let rows = [
            (SCRIPT_HP_LABEL, i64::from(values.hp), HP_CELLS),
            (SCRIPT_EP_LABEL, i64::from(values.ep), HP_CELLS),
            (SCRIPT_SP_LABEL, i64::from(values.sp), HP_CELLS),
            (SCRIPT_DF_LABEL, i64::from(values.df), EP_CELLS),
        ];
        for (label, value, cells) in rows {
            windows.line_break(ZOID_WINDOW);
            self.run_in(rom, ZOID_WINDOW, label, windows)?;
            let value = i32::try_from(value).unwrap_or(i32::MAX);
            put_value(windows, ZOID_WINDOW, value, cells, 0);
        }
        self.run_in(rom, ZOID_WINDOW, SCRIPT_PERCENT, windows)?;
        self.load_zoid_sprite(rom, zoid);
        self.shown_zoid = Some(zoid);
        Ok(())
    }

    /// B goes back to the list (sound `0x3F`); START shows the Zoid's
    /// parts; A checks what the development needs.
    fn stats_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        match code {
            STARTED => {
                self.shown_zoid = None;
                self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                self.run_now(rom, SCRIPT_ARMS_HELP, windows)?;
                self.show_development_arms(rom, 0, windows)
            }
            CONFIRMED => self.check_development(rom, windows),
            _ => {
                windows.play_sound(LEAVE_SOUND);
                self.reopen_developments(rom, windows)
            }
        }
    }

    /// Page `page` of the Zoid's parts as its record gives them.
    fn show_development_arms(
        &mut self,
        rom: &[u8],
        page: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(zoid) = self.development().zoid() else {
            return Ok(());
        };
        let Some(slots) = saga_party::record_parts(rom, u16::from(zoid)) else {
            return Ok(());
        };
        self.show_lab_arms(rom, &slots, page, windows)?;
        self.develop_step(Step::Arms(page));
        Ok(())
    }

    /// A turns the page, and past the last shows the Zoid again; B shows
    /// it at once (sound `0x3F`).
    fn arms_choice(
        &mut self,
        rom: &[u8],
        page: usize,
        code: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if code == CONFIRMED && page + 1 < PART_SLOTS {
            return self.show_development_arms(rom, page + 1, windows);
        }
        self.close_lab_arms(rom, page, code, windows)?;
        self.show_development_stats(rom, windows)?;
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.run_now(rom, SCRIPT_STATS_HELP, windows)?;
        self.runner.start(SCRIPT_STATS_WAIT)?;
        self.develop_step(Step::Stats);
        Ok(())
    }

    /// The base list again (script 293), printed anew with its cursor.
    pub(super) fn reopen_bases(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_BASE_WINDOWS, windows)?;
        if let Some(development) = self.development_mut() {
            development.base_page_shown = None;
            development.base_shown = None;
        }
        self.show_bases(rom, true, windows)
    }

    /// A on the Zoid (`0x0805534C`): with something lacking, sound `0x4F`
    /// and お金と／お金が, ゾイドと／ゾイドが, アイテムが and 足りないみたいですね;
    /// otherwise the question, or first the units to build it from.
    fn check_development(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.play_sound(EMPTY_BACK_SOUND);
        let Some(zoid) = self.development().zoid() else {
            return Ok(());
        };
        let lacking =
            saga_party::development_shortfall(rom, &self.game_state, self.party.money, zoid);
        if !lacking.is_met() {
            windows.play_sound(EMPTY_SOUND);
            self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
            self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
            for script in lacking_scripts(lacking) {
                self.run_now(rom, script, windows)?;
            }
            self.runner.start(SCRIPT_LACKING)?;
            self.develop_step(Step::Lacking);
            return Ok(());
        }
        let needs_base = saga_party::development(rom, zoid).is_some_and(|needed| needed.zoid != 0);
        if !needs_base {
            if let Some(development) = self.development_mut() {
                development.base = None;
            }
            return self.ask_development(rom, windows);
        }
        self.shown_zoid = None;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        let bases = saga_party::development_bases(rom, &self.game_state, zoid);
        for unit in &bases {
            if let Some(status) = saga_party::unit_status(&self.game_state, *unit) {
                self.load_zoid_sprite(rom, status.zoid);
            }
        }
        if let Some(development) = self.development_mut() {
            development.bases = bases;
            development.base_page = 0;
            development.base_line = 0;
            development.base_page_shown = None;
            development.base_shown = None;
        }
        self.run_now(rom, SCRIPT_BASE_WINDOWS, windows)?;
        self.show_bases(rom, true, windows)
    }

    /// 開発しますが、よろしいのですか？ and はい／いいえ over the Zoid.
    fn ask_development(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        self.run_now(rom, SCRIPT_QUESTION, windows)?;
        self.runner.start(SCRIPT_YES_NO)?;
        self.develop_step(Step::Question);
        Ok(())
    }

    /// The base list's loop (`0x08056716`): the help, six units a page in
    /// window 3 by their Zoids' names, the one under the cursor in window 1
    /// (its Zoid, full hit and energy points, pilot and picture), then the
    /// menu.
    fn show_bases(
        &mut self,
        rom: &[u8],
        help: bool,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if help {
            self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
            self.run_now(rom, SCRIPT_WHICH_BASE, windows)?;
            self.run_now(rom, SCRIPT_DRAW_MEMBERS, windows)?;
        }
        let development = self.development();
        let start = development.base_page * BASE_PAGE_LINES;
        let mut line = development.base_line;
        if development.base_page_shown != Some(development.base_page) {
            if development.base_page_shown.is_some() {
                self.run_now(rom, SCRIPT_CLEAR_LIST, windows)?;
            }
            let page: Vec<u8> = development
                .bases
                .iter()
                .skip(start)
                .take(BASE_PAGE_LINES)
                .copied()
                .collect();
            for (index, unit) in page.iter().enumerate() {
                if let Some(status) = saga_party::unit_status(&self.game_state, *unit) {
                    self.print_zoid_name(rom, status.zoid, LIST_WINDOW, windows)?;
                }
                if index + 1 < BASE_PAGE_LINES && start + index + 1 < development.bases.len() {
                    windows.line_break(LIST_WINDOW);
                }
            }
            line = line.min(page.len().saturating_sub(1));
            windows.set_cursor(LIST_WINDOW, Some(line));
            windows.set_cursor(LIST_WINDOW, None);
            let more = development.bases.len() > start + BASE_PAGE_LINES;
            windows.set_scroll_marks(LIST_WINDOW, (development.base_page > 0, more));
        }
        let selected = start + line;
        if development.base_shown != Some(selected)
            && let Some(unit) = development.bases.get(selected).copied()
        {
            if development.base_shown.is_some() {
                self.shown_zoid = None;
                self.run_now(rom, SCRIPT_CLEAR_BASE, windows)?;
            }
            self.draw_unit(rom, unit, ZOID_WINDOW, windows)?;
        }
        if let Some(development) = self.development_mut() {
            development.base_line = line;
            development.base_page_shown = Some(development.base_page);
            development.base_shown = Some(selected);
        }
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.runner.start(SCRIPT_REVIVAL_MENU)?;
        self.develop_step(Step::Bases);
        Ok(())
    }

    /// Moves and pages print again; A takes the unit, asking first when
    /// someone pilots it; B goes back to the Zi data list (sound `0x3F`).
    fn base_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let development = self.development();
        let more = development.bases.len() > (development.base_page + 1) * BASE_PAGE_LINES;
        match code {
            MOVED_UP | MOVED_DOWN => {
                if let Some(development) = self.development_mut() {
                    development.base_line = usize::from(line);
                }
                self.show_bases(rom, false, windows)
            }
            PAGE_LEFT | PAGE_RIGHT => {
                let page = match code {
                    PAGE_LEFT if development.base_page > 0 => Some(development.base_page - 1),
                    PAGE_RIGHT if more => Some(development.base_page + 1),
                    _ => None,
                };
                if let Some(page) = page {
                    windows.play_sound(MENU_MOVE_SOUND);
                    if let Some(development) = self.development_mut() {
                        development.base_page = page;
                    }
                }
                self.show_bases(rom, false, windows)
            }
            CONFIRMED => self.take_base(rom, windows),
            STARTED => match self.development().chosen_base() {
                Some(unit) => self.show_lab_unit(rom, unit, LabList::Bases, windows),
                None => self.show_bases(rom, false, windows),
            },
            0 => {
                windows.play_sound(LEAVE_SOUND);
                self.shown_zoid = None;
                self.run_now(rom, SCRIPT_CLOSE + usize::from(LIST_WINDOW), windows)?;
                self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
                self.reopen_after_close(rom, windows)
            }
            _ => self.show_bases(rom, false, windows),
        }
    }

    /// The Zi data list after the windows over it closed: the money, then
    /// the list as [`PauseMenu::reopen_developments`] prints it.
    fn reopen_after_close(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.reopen_money(rom, windows)?;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_WINDOWS, windows)?;
        self.run_now(rom, SCRIPT_DRAW_MEMBERS, windows)?;
        self.run_now(rom, SCRIPT_WHICH, windows)?;
        if let Some(development) = self.development_mut() {
            development.page_shown = None;
            development.shown = None;
        }
        self.show_developments(rom, windows)
    }

    /// A on a unit: its pilot, when one keeps it (flag `0x08`), will not
    /// leave (…は…から降りたくないみたいですね); otherwise the lab asks
    /// whether to take it (…には…が搭乗してるみたいですけど、よろしいのですか？),
    /// or with no pilot goes on to the question.
    fn take_base(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(unit) = self.development().chosen_base() else {
            return self.show_bases(rom, false, windows);
        };
        if let Some(development) = self.development_mut() {
            development.base = Some(unit);
        }
        let zoid = saga_party::unit_status(&self.game_state, unit).map_or(0, |status| status.zoid);
        let Some(pilot) = saga_party::pilot_of(&self.game_state, unit) else {
            return self.base_taken(rom, windows);
        };
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        if saga_party::keeps_equipment(&self.game_state, pilot) {
            windows.play_sound(EMPTY_SOUND);
            self.print_character_name(rom, pilot, HELP_WINDOW, windows)?;
            self.run_in(rom, HELP_WINDOW, SCRIPT_THE_PILOT, windows)?;
            self.print_zoid_name(rom, zoid, HELP_WINDOW, windows)?;
            self.runner.select_window(HELP_WINDOW);
            self.runner.start(SCRIPT_WONT_LEAVE)?;
            self.develop_step(Step::BaseRefused);
            return Ok(());
        }
        self.print_zoid_name(rom, zoid, HELP_WINDOW, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_ON_BOARD, windows)?;
        self.print_character_name(rom, pilot, HELP_WINDOW, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_PILOTED, windows)?;
        self.runner.start(SCRIPT_YES_NO)?;
        self.develop_step(Step::BaseQuestion);
        Ok(())
    }

    /// The unit taken: the list and the unit's window close, and the Zoid
    /// the development gives is shown again under the question (state
    /// `0x220`).
    fn base_taken(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.shown_zoid = None;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(LIST_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.show_development_stats(rom, windows)?;
        self.ask_development(rom, windows)
    }

    /// はい takes the unit from its pilot; いいえ or B keep to the list.
    fn base_answer(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if code == CONFIRMED && line == 0 {
            self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
            return self.base_taken(rom, windows);
        }
        if code != CONFIRMED {
            windows.play_sound(LEAVE_SOUND);
        }
        self.show_bases(rom, true, windows)
    }

    /// はい develops, asking first to take the base's weapons off; いいえ
    /// gets あれ？やめてしまうのですか？; B goes back to the list the Zoid or
    /// its base was chosen from (sound `0x3F`).
    fn development_answer(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if code != CONFIRMED {
            windows.play_sound(LEAVE_SOUND);
            let base = self.development().base;
            self.shown_zoid = None;
            self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
            if base.is_some() {
                self.reopen_money(rom, windows)?;
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                return self.reopen_bases(rom, windows);
            }
            return self.reopen_after_close(rom, windows);
        }
        if line != 0 {
            return self.answer(SCRIPT_GIVEN_UP);
        }
        let base = self.development().base;
        if let Some(base) = base
            && !saga_party::rack_weapons(&self.game_state, base).is_empty()
        {
            return self.ask_strip(rom, Taking::Development, SCRIPT_STRIP_QUESTION, windows);
        }
        self.complete_development(rom)
    }

    /// The development (`0x08057020`): the money is taken, the base taken
    /// apart, the items used and the new unit made; はい。すぐにできますからね.
    pub(super) fn complete_development(&mut self, rom: &[u8]) -> Result<(), ScriptError> {
        let development = self.development();
        let Some(zoid) = development.zoid() else {
            return Ok(());
        };
        let price = saga_party::development(rom, zoid).map_or(0, |needed| needed.money);
        self.party.money = self.party.money.saturating_sub(price);
        saga_party::develop(rom, &mut self.game_state, zoid, development.base);
        self.answer(SCRIPT_DONE)
    }

    /// The unit a development is built from.
    pub(super) fn development_base(&self) -> Option<u8> {
        self.development().base
    }

    /// それでは開発できませんね・・・, when the base's weapons stay on.
    pub(super) fn development_refused(&mut self) -> Result<(), ScriptError> {
        self.answer(SCRIPT_NOT_STRIPPED)
    }

    /// The keeper's answer in the help line, then a key.
    fn answer(&mut self, script: usize) -> Result<(), ScriptError> {
        self.runner.select_window(HELP_WINDOW);
        self.runner.start(script)?;
        self.develop_step(Step::Answer);
        Ok(())
    }

    /// After the keeper's answer (sound `0x41`): the Zoid's window closes
    /// and the list comes back, or with the party full of units the lab
    /// says so and goes back to its menu.
    fn after_development(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if saga_party::unit_count(&self.game_state) > MOST_UNITS {
            self.shown_zoid = None;
            self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
            self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
            self.runner.start(SCRIPT_TOO_MANY)?;
            self.develop_step(Step::TooMany);
            return Ok(());
        }
        self.reopen_developments(rom, windows)
    }
}

/// What the lacking notice says before 足りないみたいですね: the money, the
/// Zoid and the items, each joined to the next with と.
fn lacking_scripts(lacking: Shortfall) -> Vec<usize> {
    let mut scripts = Vec::new();
    if lacking.money {
        scripts.push(if lacking.zoid || lacking.items {
            SCRIPT_MONEY_AND
        } else {
            SCRIPT_MONEY
        });
    }
    if lacking.zoid {
        scripts.push(if lacking.items {
            SCRIPT_ZOID_AND
        } else {
            SCRIPT_ZOID
        });
    }
    if lacking.items {
        scripts.push(SCRIPT_ITEMS);
    }
    scripts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lacking_notice_joins_what_is_missing() {
        let all = Shortfall {
            money: true,
            zoid: true,
            items: true,
        };
        assert_eq!(
            lacking_scripts(all),
            [SCRIPT_MONEY_AND, SCRIPT_ZOID_AND, SCRIPT_ITEMS]
        );
        let money = Shortfall {
            money: true,
            ..Shortfall::default()
        };
        assert_eq!(lacking_scripts(money), [SCRIPT_MONEY]);
        let zoid = Shortfall {
            zoid: true,
            ..Shortfall::default()
        };
        assert_eq!(lacking_scripts(zoid), [SCRIPT_ZOID]);
    }
}
