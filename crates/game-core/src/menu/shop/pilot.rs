//! The Zoid lab's ゾイド乗せ換え (the lab task's states `0x300`–`0x320`):
//! the party's members, the unit one is to board, what the change does to
//! the unit's values, the formation and the weapons of the unit left.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the lab
//! task's pilot-change states at `0x080571BC`–`0x08058458`; checked
//! against Sand Colony's lab in a reference emulator with saves changed by
//! hand (breakpoints on the script runner and the sound call, RAM dumps,
//! screenshots); see `docs/shop.md`.

use extraction::saga_party::{self, FormationChange, PART_SLOTS};

use super::super::parts::put_value;
use super::super::{
    CONFIRMED, EMPTY_BACK_SOUND, EMPTY_SOUND, HP_CELLS, LEAVE_SOUND, MENU_MOVE_SOUND, MOVED_DOWN,
    MOVED_UP, MenuState, PAGE_LEFT, PAGE_RIGHT, PauseMenu, SCRIPT_CLEAR_HELP, SCRIPT_CLOSE,
    SCRIPT_DRAW_CHARACTER, SCRIPT_DRAW_MEMBERS, SCRIPT_MEMBER_MENU, SCRIPT_PERCENT,
    SCRIPT_PORTRAITS, SCRIPT_PRESENT_ALL, SCRIPT_SIZES, SCRIPT_SPACE, SCRIPT_STAT_LABELS,
    SCRIPT_YES_NO, SIGNED, ZOID_WINDOW,
};
use super::lab::{LIST_WINDOW, SCRIPT_REVIVAL_MENU};
use super::units::{LabList, SCRIPT_ARMS_HELP};
use super::{HELP_WINDOW, SCRIPT_DRAW_HELP, ShopStep};
use crate::ScriptHost;
use crate::script::ScriptError;
use crate::windows::ScriptWindows;

/// The member list's windows: the member's (1), the portrait's (2) and
/// the list's (3).
const SCRIPT_MEMBER_WINDOWS: usize = 70;
const SCRIPT_WHO: usize = 288;
const SCRIPT_THE_PILOT: usize = 289;
const SCRIPT_ALREADY_ON: usize = 290;
const SCRIPT_WONT_LEAVE: usize = 291;
const SCRIPT_NO_ZOID: usize = 292;
const SCRIPT_UNIT_WINDOWS: usize = 293;
const SCRIPT_WHICH_UNIT: usize = 294;
const SCRIPT_ON_BOARD: usize = 295;
const SCRIPT_PILOTED: usize = 296;
const SCRIPT_TOO_LARGE: usize = 297;
const SCRIPT_LEAVE_FORMATION: usize = 298;
const SCRIPT_FIX_FORMATION: usize = 299;
const SCRIPT_COMPARE_WINDOW: usize = 306;
const SCRIPT_COMPARE_HP: usize = 307;
const SCRIPT_COMPARE_EP: usize = 308;
const SCRIPT_COMPARE_SP: usize = 309;
const SCRIPT_COMPARE_DF: usize = 310;
const SCRIPT_COMPARE_HELP: usize = 311;
const SCRIPT_CONFIRM: usize = 312;
const SCRIPT_DONE: usize = 313;
const SCRIPT_KEEP_QUESTION: usize = 314;
const SCRIPT_KEPT: usize = 315;
const SCRIPT_TAKEN_OFF: usize = 316;
const SCRIPT_ARROW: usize = 148;
const SCRIPT_TAKING_OFF: usize = 149;
const SCRIPT_STOCK_FULL: usize = 150;
/// はい／いいえ that B does not cancel.
const SCRIPT_YES_NO_ONLY: usize = 62;
/// Waits for A, B or START (`0x80`).
const SCRIPT_COMPARE_WAIT: usize = 38;
const SCRIPT_CLEAR_MEMBER: usize = 1;
const SCRIPT_CLEAR_PORTRAIT: usize = 2;
const SCRIPT_CLEAR_LIST: usize = 3;
const PAGE_LINES: usize = 6;
const STARTED: u16 = 0x80;
const BONUS_CELLS: usize = 3;
const COMPARE_CELLS: usize = 5;
const COMPARE_GAP: usize = 6;
/// The most of one part the stock takes: past it, the lab asks.
const STOCK_ROOM: u8 = 8;

/// Where the pilot change stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::menu) enum Step {
    /// The party's members (state `0x302`).
    Members,
    /// Whether to take the chosen member off their unit.
    MemberQuestion,
    /// A notice over the members, then their list.
    MemberNotice,
    /// The units the member can board (state `0x312`).
    Units,
    /// Whether to take the chosen unit from its pilot.
    UnitQuestion,
    /// A notice over the units, then their list.
    UnitNotice,
    /// Whether to let the formation change for an L unit.
    FormationQuestion,
    /// The unit's values before and after (state `0x320`).
    Compare,
    /// Page `n` of the unit's parts as the member would use them.
    Arms(usize),
    /// これでよろしいのですか？
    Confirm,
    /// Whether the unit left keeps its weapons.
    Keep,
    /// Whether to throw away the `n`-th rack weapon the stock is full of.
    StockFull(usize),
    /// The keeper's word that the change is done.
    Done,
}

/// The pilot change's lists and choices.
#[derive(Debug, Clone, Default)]
pub(super) struct PilotChange {
    members: Vec<u8>,
    page: usize,
    line: usize,
    page_shown: Option<usize>,
    shown: Option<usize>,
    units: Vec<u8>,
    unit_page: usize,
    unit_line: usize,
    unit_page_shown: Option<usize>,
    unit_shown: Option<usize>,
    formation: Option<FormationChange>,
}

impl PilotChange {
    fn member(&self) -> Option<u8> {
        self.members
            .get(self.page * PAGE_LINES + self.line)
            .copied()
    }

    fn unit(&self) -> Option<u8> {
        self.units
            .get(self.unit_page * PAGE_LINES + self.unit_line)
            .copied()
    }
}

impl PauseMenu {
    fn pilot_change(&self) -> PilotChange {
        self.shop
            .as_ref()
            .map(|session| session.lab.pilot.clone())
            .unwrap_or_default()
    }

    fn pilot_change_mut(&mut self) -> Option<&mut PilotChange> {
        self.shop.as_mut().map(|session| &mut session.lab.pilot)
    }

    fn pilot_step(&mut self, step: Step) {
        self.state = MenuState::Shop(ShopStep::Pilot(step));
    }

    /// ゾイド乗せ換え (state `0x300`): windows 3, 2 and 1 close and script 70
    /// opens the member's window, the portrait's and the list's.
    pub(super) fn open_pilot_change(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let members = saga_party::members(&self.game_state);
        if let Some(change) = self.pilot_change_mut() {
            *change = PilotChange {
                members,
                ..PilotChange::default()
            };
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        for window in [LIST_WINDOW, 2, ZOID_WINDOW] {
            self.run_now(rom, SCRIPT_CLOSE + usize::from(window), windows)?;
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_MEMBER_WINDOWS, windows)?;
        self.show_members(rom, true, windows)
    }

    /// The members again (state `0x301`): the window over them closes and
    /// script 70 opens theirs, which print anew with the cursor kept.
    fn reopen_members(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.shown_zoid = None;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_MEMBER_WINDOWS, windows)?;
        let members = saga_party::members(&self.game_state);
        if let Some(change) = self.pilot_change_mut() {
            change.members = members;
            change.page_shown = None;
            change.shown = None;
        }
        self.show_members(rom, true, windows)
    }

    /// The member list's loop (`0x08057222`): six members a page in
    /// window 3; with `help` or a new member under the cursor, 288 誰を乗せ
    /// 換えたいのですか？ 搭乗ゾイド： and their Zoid (or 292 なし); the
    /// member in window 1 (name and bonuses) and their portrait; the menu.
    fn show_members(
        &mut self,
        rom: &[u8],
        help: bool,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let change = self.pilot_change();
        let start = change.page * PAGE_LINES;
        let mut line = change.line;
        if change.page_shown != Some(change.page) {
            if change.page_shown.is_some() {
                self.run_now(rom, SCRIPT_CLEAR_LIST, windows)?;
            }
            let page: Vec<u8> = change
                .members
                .iter()
                .skip(start)
                .take(PAGE_LINES)
                .copied()
                .collect();
            for (index, member) in page.iter().enumerate() {
                self.print_character_name(rom, *member, LIST_WINDOW, windows)?;
                if index + 1 < PAGE_LINES && start + index + 1 < change.members.len() {
                    windows.line_break(LIST_WINDOW);
                }
            }
            line = line.min(page.len().saturating_sub(1));
            windows.set_cursor(LIST_WINDOW, Some(line));
            windows.set_cursor(LIST_WINDOW, None);
            let more = change.members.len() > start + PAGE_LINES;
            windows.set_scroll_marks(LIST_WINDOW, (change.page > 0, more));
        }
        let selected = start + line;
        let moved = change.shown != Some(selected);
        if (help || moved)
            && let Some(member) = change.members.get(selected).copied()
        {
            self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
            self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
            self.run_now(rom, SCRIPT_WHO, windows)?;
            match saga_party::character_unit(&self.game_state, member)
                .and_then(|unit| saga_party::unit_status(&self.game_state, unit))
            {
                Some(status) => self.print_zoid_name(rom, status.zoid, HELP_WINDOW, windows)?,
                None => self.run_in(rom, HELP_WINDOW, SCRIPT_NO_ZOID, windows)?,
            }
        }
        if moved && let Some(member) = change.members.get(selected).copied() {
            if change.shown.is_some() {
                self.run_now(rom, SCRIPT_CLEAR_MEMBER, windows)?;
                self.run_now(rom, SCRIPT_CLEAR_PORTRAIT, windows)?;
            }
            self.draw_member(rom, member, windows)?;
        }
        if let Some(change) = self.pilot_change_mut() {
            change.line = line;
            change.page_shown = Some(change.page);
            change.shown = Some(selected);
        }
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.runner.start(SCRIPT_MEMBER_MENU)?;
        self.pilot_step(Step::Members);
        Ok(())
    }

    /// A member in window 1: their name and their five bonuses in percent
    /// (耐久, 攻撃, 防御, 反応, 命中), signed, and their portrait.
    fn draw_member(
        &mut self,
        rom: &[u8],
        member: u8,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let bonuses = saga_party::pilot_bonuses(rom, &self.game_state, member).unwrap_or_default();
        self.run_now(rom, SCRIPT_DRAW_CHARACTER, windows)?;
        self.print_character_name(rom, member, ZOID_WINDOW, windows)?;
        for (label, bonus) in SCRIPT_STAT_LABELS.iter().zip(bonuses) {
            windows.line_break(ZOID_WINDOW);
            self.run_in(rom, ZOID_WINDOW, *label, windows)?;
            put_value(windows, ZOID_WINDOW, bonus, BONUS_CELLS, SIGNED);
            self.run_in(rom, ZOID_WINDOW, SCRIPT_PERCENT, windows)?;
        }
        self.run_now(rom, SCRIPT_PORTRAITS + usize::from(member), windows)
    }

    /// What a pilot change's script ended with.
    pub(super) fn pilot_change_step(
        &mut self,
        rom: &[u8],
        step: Step,
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        match step {
            Step::Members => self.pilot_member_choice(rom, code, line, windows),
            Step::MemberQuestion => {
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                if code == CONFIRMED && line == 0 {
                    return self.open_pilot_units(rom, windows);
                }
                if code != CONFIRMED {
                    windows.play_sound(LEAVE_SOUND);
                }
                self.show_members(rom, true, windows)
            }
            Step::MemberNotice => {
                windows.play_sound(EMPTY_BACK_SOUND);
                self.show_members(rom, true, windows)
            }
            Step::Units => self.unit_choice(rom, code, line, windows),
            Step::UnitQuestion => {
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                if code == CONFIRMED && line == 0 {
                    return self.check_formation(rom, windows);
                }
                if code != CONFIRMED {
                    windows.play_sound(LEAVE_SOUND);
                }
                self.show_units(rom, true, windows)
            }
            Step::UnitNotice => {
                windows.play_sound(EMPTY_BACK_SOUND);
                self.show_units(rom, true, windows)
            }
            Step::FormationQuestion => {
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                if code == CONFIRMED && line == 0 {
                    return self.open_compare(rom, windows);
                }
                if code != CONFIRMED {
                    windows.play_sound(LEAVE_SOUND);
                }
                self.show_units(rom, true, windows)
            }
            Step::Compare => {
                if code == STARTED {
                    self.shown_zoid = None;
                    self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
                    self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                    self.run_now(rom, SCRIPT_ARMS_HELP, windows)?;
                    return self.show_boarding_arms(rom, 0, windows);
                }
                windows.play_sound(EMPTY_BACK_SOUND);
                self.run_now(rom, SCRIPT_CONFIRM, windows)?;
                self.runner.start(SCRIPT_YES_NO)?;
                self.pilot_step(Step::Confirm);
                Ok(())
            }
            Step::Arms(page) => {
                if code == CONFIRMED && page + 1 < PART_SLOTS {
                    return self.show_boarding_arms(rom, page + 1, windows);
                }
                self.close_lab_arms(rom, page, code, windows)?;
                self.show_compare(rom, windows)
            }
            Step::Confirm => {
                if code == CONFIRMED && line == 0 {
                    return self.confirmed(rom, windows);
                }
                if code != CONFIRMED {
                    windows.play_sound(LEAVE_SOUND);
                }
                self.reopen_members(rom, windows)
            }
            Step::Keep => {
                if line == 0 {
                    self.pilot_answer(SCRIPT_KEPT)
                } else {
                    self.ask_keep_full(rom, 0, windows)
                }
            }
            Step::StockFull(rack) => {
                if line == 0 {
                    self.ask_keep_full(rom, rack + 1, windows)
                } else {
                    self.pilot_answer(SCRIPT_KEPT)
                }
            }
            Step::Done => {
                self.apply_pilot_change(rom);
                self.reopen_members(rom, windows)
            }
        }
    }

    /// Moves and pages print again; A takes the member, asking first when
    /// they pilot a unit; B goes back to the lab's menu (sound `0x3F`).
    fn pilot_member_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let change = self.pilot_change();
        let more = change.members.len() > (change.page + 1) * PAGE_LINES;
        match code {
            MOVED_UP | MOVED_DOWN => {
                if let Some(change) = self.pilot_change_mut() {
                    change.line = usize::from(line);
                }
                self.show_members(rom, false, windows)
            }
            PAGE_LEFT | PAGE_RIGHT => {
                let page = match code {
                    PAGE_LEFT if change.page > 0 => Some(change.page - 1),
                    PAGE_RIGHT if more => Some(change.page + 1),
                    _ => None,
                };
                if let Some(page) = page {
                    windows.play_sound(MENU_MOVE_SOUND);
                    if let Some(change) = self.pilot_change_mut() {
                        change.page = page;
                    }
                }
                self.show_members(rom, false, windows)
            }
            CONFIRMED => self.take_member(rom, windows),
            _ => {
                windows.play_sound(LEAVE_SOUND);
                for window in [LIST_WINDOW, 2, ZOID_WINDOW] {
                    self.run_now(rom, SCRIPT_CLOSE + usize::from(window), windows)?;
                }
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                self.reopen_lab_with_money(rom, windows)
            }
        }
    }

    /// A on a member with a unit: one who keeps it (flag `0x08`) will not
    /// leave it (…は…から降りたくないみたいですね); otherwise …は…に搭乗し
    /// てるみたいですけど、よろしいのですか？ and はい／いいえ.
    fn take_member(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(member) = self.pilot_change().member() else {
            return self.show_members(rom, false, windows);
        };
        let Some(unit) = saga_party::character_unit(&self.game_state, member) else {
            return self.open_pilot_units(rom, windows);
        };
        let zoid = saga_party::unit_status(&self.game_state, unit).map_or(0, |status| status.zoid);
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        let keeps = saga_party::keeps_equipment(&self.game_state, member);
        if keeps {
            windows.play_sound(EMPTY_SOUND);
        }
        self.print_character_name(rom, member, HELP_WINDOW, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_THE_PILOT, windows)?;
        self.print_zoid_name(rom, zoid, HELP_WINDOW, windows)?;
        self.runner.select_window(HELP_WINDOW);
        if keeps {
            self.runner.start(SCRIPT_WONT_LEAVE)?;
            self.pilot_step(Step::MemberNotice);
            return Ok(());
        }
        self.run_now(rom, SCRIPT_ALREADY_ON, windows)?;
        self.runner.start(SCRIPT_YES_NO)?;
        self.pilot_step(Step::MemberQuestion);
        Ok(())
    }

    /// The units the member can board (`0x08055120` with the member):
    /// windows 3, 2 and 1 close and script 293 opens the unit's and the
    /// list's.
    fn open_pilot_units(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(member) = self.pilot_change().member() else {
            return Ok(());
        };
        let units = saga_party::units_for(&self.game_state, member);
        if let Some(change) = self.pilot_change_mut() {
            change.units = units;
            change.unit_page = 0;
            change.unit_line = 0;
            change.unit_page_shown = None;
            change.unit_shown = None;
            change.formation = None;
        }
        for window in [LIST_WINDOW, 2, ZOID_WINDOW] {
            self.run_now(rom, SCRIPT_CLOSE + usize::from(window), windows)?;
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_UNIT_WINDOWS, windows)?;
        self.show_units(rom, true, windows)
    }

    /// The unit list again after a unit shown in full.
    pub(super) fn reopen_pilot_units(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_UNIT_WINDOWS, windows)?;
        if let Some(change) = self.pilot_change_mut() {
            change.unit_page_shown = None;
            change.unit_shown = None;
        }
        self.show_units(rom, true, windows)
    }

    /// The unit list's loop (`0x08057726`): with `help`, the member's name
    /// and 294 をどのゾイドに搭乗させたいのですか？ スタートボタン：ゾイド詳細表示;
    /// six units a page, the one under the cursor in window 1, the menu.
    fn show_units(
        &mut self,
        rom: &[u8],
        help: bool,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let change = self.pilot_change();
        if help && let Some(member) = change.member() {
            self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
            self.print_character_name(rom, member, HELP_WINDOW, windows)?;
            self.run_in(rom, HELP_WINDOW, SCRIPT_WHICH_UNIT, windows)?;
            self.run_now(rom, SCRIPT_DRAW_MEMBERS, windows)?;
        }
        let start = change.unit_page * PAGE_LINES;
        let mut line = change.unit_line;
        if change.unit_page_shown != Some(change.unit_page) {
            if change.unit_page_shown.is_some() {
                self.run_now(rom, SCRIPT_CLEAR_LIST, windows)?;
            }
            let page: Vec<u8> = change
                .units
                .iter()
                .skip(start)
                .take(PAGE_LINES)
                .copied()
                .collect();
            for (index, unit) in page.iter().enumerate() {
                if let Some(status) = saga_party::unit_status(&self.game_state, *unit) {
                    self.print_zoid_name(rom, status.zoid, LIST_WINDOW, windows)?;
                }
                if index + 1 < PAGE_LINES && start + index + 1 < change.units.len() {
                    windows.line_break(LIST_WINDOW);
                }
            }
            line = line.min(page.len().saturating_sub(1));
            windows.set_cursor(LIST_WINDOW, Some(line));
            windows.set_cursor(LIST_WINDOW, None);
            let more = change.units.len() > start + PAGE_LINES;
            windows.set_scroll_marks(LIST_WINDOW, (change.unit_page > 0, more));
        }
        let selected = start + line;
        if change.unit_shown != Some(selected)
            && let Some(unit) = change.units.get(selected).copied()
        {
            if change.unit_shown.is_some() {
                self.shown_zoid = None;
                self.run_now(rom, SCRIPT_CLEAR_MEMBER, windows)?;
            }
            if let Some(status) = saga_party::unit_status(&self.game_state, unit) {
                self.load_zoid_sprite(rom, status.zoid);
            }
            self.draw_unit(rom, unit, ZOID_WINDOW, windows)?;
        }
        if let Some(change) = self.pilot_change_mut() {
            change.unit_line = line;
            change.unit_page_shown = Some(change.unit_page);
            change.unit_shown = Some(selected);
        }
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.runner.start(SCRIPT_REVIVAL_MENU)?;
        self.pilot_step(Step::Units);
        Ok(())
    }

    /// Moves and pages print again; START shows the unit in full; A takes
    /// it; B goes back to the members (sound `0x3F`).
    fn unit_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let change = self.pilot_change();
        let more = change.units.len() > (change.unit_page + 1) * PAGE_LINES;
        match code {
            MOVED_UP | MOVED_DOWN => {
                if let Some(change) = self.pilot_change_mut() {
                    change.unit_line = usize::from(line);
                }
                self.show_units(rom, false, windows)
            }
            PAGE_LEFT | PAGE_RIGHT => {
                let page = match code {
                    PAGE_LEFT if change.unit_page > 0 => Some(change.unit_page - 1),
                    PAGE_RIGHT if more => Some(change.unit_page + 1),
                    _ => None,
                };
                if let Some(page) = page {
                    windows.play_sound(MENU_MOVE_SOUND);
                    if let Some(change) = self.pilot_change_mut() {
                        change.unit_page = page;
                    }
                }
                self.show_units(rom, false, windows)
            }
            CONFIRMED => self.take_unit(rom, windows),
            STARTED => match change.unit() {
                Some(unit) => self.show_lab_unit(rom, unit, LabList::PilotUnits, windows),
                None => self.show_units(rom, false, windows),
            },
            _ => {
                windows.play_sound(LEAVE_SOUND);
                self.shown_zoid = None;
                self.run_now(rom, SCRIPT_CLOSE + usize::from(LIST_WINDOW), windows)?;
                self.reopen_members(rom, windows)
            }
        }
    }

    /// A on a unit: its pilot, if it has one, will not leave it when they
    /// keep it, or the lab asks whether to take it; then the formation.
    fn take_unit(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(unit) = self.pilot_change().unit() else {
            return self.show_units(rom, false, windows);
        };
        let Some(pilot) = saga_party::pilot_of(&self.game_state, unit) else {
            return self.check_formation(rom, windows);
        };
        let zoid = saga_party::unit_status(&self.game_state, unit).map_or(0, |status| status.zoid);
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        if saga_party::keeps_equipment(&self.game_state, pilot) {
            windows.play_sound(EMPTY_SOUND);
            self.print_character_name(rom, pilot, HELP_WINDOW, windows)?;
            self.run_in(rom, HELP_WINDOW, SCRIPT_THE_PILOT, windows)?;
            self.print_zoid_name(rom, zoid, HELP_WINDOW, windows)?;
            self.runner.select_window(HELP_WINDOW);
            self.runner.start(SCRIPT_WONT_LEAVE)?;
            self.pilot_step(Step::UnitNotice);
            return Ok(());
        }
        self.print_zoid_name(rom, zoid, HELP_WINDOW, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_ON_BOARD, windows)?;
        self.print_character_name(rom, pilot, HELP_WINDOW, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_PILOTED, windows)?;
        self.runner.start(SCRIPT_YES_NO)?;
        self.pilot_step(Step::UnitQuestion);
        Ok(())
    }

    /// An L unit for a member in the formation: このゾイドは大きすぎるので隊列
    /// に影響がでるようです, then …を隊列から外しますか？ or 隊列が修正されますが、
    /// よろしいのですか？ with はい／いいえ; otherwise the comparison.
    fn check_formation(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let change = self.pilot_change();
        let (Some(member), Some(unit)) = (change.member(), change.unit()) else {
            return Ok(());
        };
        let formation = saga_party::formation_change(&self.game_state, member, unit);
        if let Some(change) = self.pilot_change_mut() {
            change.formation = formation;
        }
        let Some(formation) = formation else {
            return self.open_compare(rom, windows);
        };
        self.runner.select_window(HELP_WINDOW);
        self.run_now(rom, SCRIPT_TOO_LARGE, windows)?;
        match formation {
            FormationChange::Leave(_) => {
                self.print_character_name(rom, member, HELP_WINDOW, windows)?;
                self.run_in(rom, HELP_WINDOW, SCRIPT_LEAVE_FORMATION, windows)?;
            }
            FormationChange::Fix(_) => {
                self.run_in(rom, HELP_WINDOW, SCRIPT_FIX_FORMATION, windows)?;
            }
        }
        self.runner.start(SCRIPT_YES_NO)?;
        self.pilot_step(Step::FormationQuestion);
        Ok(())
    }

    /// The list and the unit's window close for the comparison.
    fn open_compare(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.shown_zoid = None;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(LIST_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.show_compare(rom, windows)
    }

    /// What boarding does (state `0x320`): script 306 opens window 1 with
    /// the Zoid's name and size, then for its hit points, SP and DF the
    /// unit's value with no pilot, the member's bonus in percent, → and the
    /// sum, and for its energy points the value alone; its picture; 311
    /// ゾイドの性能はこのように変化します スタートボタン：装備武器表示.
    fn show_compare(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let change = self.pilot_change();
        let (Some(member), Some(unit)) = (change.member(), change.unit()) else {
            return Ok(());
        };
        let Some(status) = saga_party::unit_status(&self.game_state, unit) else {
            return Ok(());
        };
        let values = if saga_party::pilot_of(&self.game_state, unit).is_some() {
            saga_party::pilotless_values(rom, &self.game_state, unit)
        } else {
            Some(saga_party::ZoidValues {
                hp: status.hp.1,
                ep: status.ep.1,
                sp: status.sp,
                df: status.df,
                size: status.size,
            })
        };
        let Some(values) = values else {
            return Ok(());
        };
        let [durability, _, defense, reaction, _] =
            saga_party::pilot_bonuses(rom, &self.game_state, member).unwrap_or_default();
        let signed = |value: u32| i32::from_ne_bytes(value.to_ne_bytes());
        let raised = |value: i32, bonus: i32| value + saga_party::percent(value, bonus);
        let window = ZOID_WINDOW;
        self.run_now(rom, SCRIPT_COMPARE_WINDOW, windows)?;
        self.print_zoid_name(rom, status.zoid, window, windows)?;
        self.run_in(
            rom,
            window,
            SCRIPT_SIZES + usize::from(status.size),
            windows,
        )?;
        windows.line_break(window);
        self.run_in(rom, window, SCRIPT_COMPARE_HP, windows)?;
        let hp = signed(values.hp);
        self.compare_row(
            rom,
            (hp, COMPARE_CELLS),
            durability,
            raised(hp, durability),
            false,
            windows,
        )?;
        windows.line_break(window);
        self.run_in(rom, window, SCRIPT_COMPARE_EP, windows)?;
        let ep = signed(values.ep);
        put_value(windows, window, ep, COMPARE_CELLS, 0);
        for _ in 0..COMPARE_GAP {
            self.run_in(rom, window, SCRIPT_SPACE, windows)?;
        }
        self.run_in(rom, window, SCRIPT_ARROW, windows)?;
        put_value(windows, window, ep, COMPARE_CELLS, 0);
        windows.line_break(window);
        self.run_in(rom, window, SCRIPT_COMPARE_SP, windows)?;
        let sp = i32::from(values.sp);
        self.compare_row(
            rom,
            (sp, COMPARE_CELLS),
            reaction,
            raised(sp, reaction),
            false,
            windows,
        )?;
        windows.line_break(window);
        self.run_in(rom, window, SCRIPT_COMPARE_DF, windows)?;
        let df = i32::from(values.df);
        self.compare_row(
            rom,
            (df, HP_CELLS),
            defense,
            raised(df, defense),
            true,
            windows,
        )?;
        self.load_zoid_sprite(rom, status.zoid);
        self.shown_zoid = Some(status.zoid);
        self.run_now(rom, SCRIPT_COMPARE_HELP, windows)?;
        self.runner.start(SCRIPT_COMPARE_WAIT)?;
        self.pilot_step(Step::Compare);
        Ok(())
    }

    /// A row of the comparison: the value in `cells` (with ％ when
    /// `percent`), the bonus in five cells and ％, →, and the sum.
    fn compare_row(
        &mut self,
        rom: &[u8],
        (value, cells): (i32, usize),
        bonus: i32,
        sum: i32,
        percent: bool,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let window = ZOID_WINDOW;
        put_value(windows, window, value, cells, 0);
        if percent {
            self.run_in(rom, window, SCRIPT_PERCENT, windows)?;
        }
        put_value(windows, window, bonus, COMPARE_CELLS, 0);
        self.run_in(rom, window, SCRIPT_PERCENT, windows)?;
        self.run_in(rom, window, SCRIPT_ARROW, windows)?;
        put_value(windows, window, sum, cells, 0);
        if percent {
            self.run_in(rom, window, SCRIPT_PERCENT, windows)?;
        }
        Ok(())
    }

    /// Page `page` of the unit's parts as the member would use them.
    fn show_boarding_arms(
        &mut self,
        rom: &[u8],
        page: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let change = self.pilot_change();
        let slots = change
            .member()
            .zip(change.unit())
            .and_then(|(member, unit)| saga_party::parts_for(rom, &self.game_state, unit, member));
        let Some(slots) = slots else {
            return Ok(());
        };
        self.show_lab_arms(rom, &slots, page, windows)?;
        self.pilot_step(Step::Arms(page));
        Ok(())
    }

    /// はい on これでよろしいのですか？: when the unit the member leaves has
    /// weapons on its racks, its window closes and 314 asks whether it
    /// keeps them (はい／いいえ that B does not end); otherwise 313
    /// 乗せ換えは完了しましたよ.
    fn confirmed(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let left = self
            .pilot_change()
            .member()
            .and_then(|member| saga_party::character_unit(&self.game_state, member));
        let armed =
            left.is_some_and(|unit| !saga_party::rack_weapons(&self.game_state, unit).is_empty());
        if !armed {
            return self.pilot_answer(SCRIPT_DONE);
        }
        self.shown_zoid = None;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_KEEP_QUESTION, windows)?;
        self.runner.start(SCRIPT_YES_NO_ONLY)?;
        self.pilot_step(Step::Keep);
        Ok(())
    }

    /// いいえ: each weapon the stock is full of is asked about in turn
    /// (いいえ keeps them all on, 315); past the last they come off, 316.
    fn ask_keep_full(
        &mut self,
        rom: &[u8],
        rack: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let left = self
            .pilot_change()
            .member()
            .and_then(|member| saga_party::character_unit(&self.game_state, member));
        let Some(unit) = left else {
            return self.pilot_answer(SCRIPT_DONE);
        };
        let weapons = saga_party::rack_weapons(&self.game_state, unit);
        let full = weapons
            .iter()
            .enumerate()
            .skip(rack)
            .find(|(_, part)| saga_party::stock(&self.game_state, **part) > STOCK_ROOM);
        let Some((index, part)) = full else {
            saga_party::strip_racks(&mut self.game_state, unit);
            return self.pilot_answer(SCRIPT_TAKEN_OFF);
        };
        let part = *part;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_TAKING_OFF, windows)?;
        self.print_part_name(rom, HELP_WINDOW, part, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_STOCK_FULL, windows)?;
        self.runner.start(SCRIPT_YES_NO_ONLY)?;
        self.pilot_step(Step::StockFull(index));
        Ok(())
    }

    fn pilot_answer(&mut self, script: usize) -> Result<(), ScriptError> {
        self.runner.select_window(HELP_WINDOW);
        self.runner.start(script)?;
        self.pilot_step(Step::Done);
        Ok(())
    }

    /// The change itself, once the keeper's word is read: the member
    /// leaves the formation when asked to, and their unit; the unit's pilot
    /// leaves it; the member boards it (`0x08036BE0`), moving to the middle
    /// of their column when the formation was put right (`0x080371AC`).
    fn apply_pilot_change(&mut self, rom: &[u8]) {
        let change = self.pilot_change();
        let (Some(member), Some(unit)) = (change.member(), change.unit()) else {
            return;
        };
        let state = &mut self.game_state;
        if let Some(FormationChange::Leave(slot) | FormationChange::Fix(slot)) = change.formation {
            saga_party::leave_formation(state, slot);
        }
        if saga_party::character_unit(state, member).is_some() {
            saga_party::leave_unit(rom, state, member);
        }
        if let Some(pilot) = saga_party::pilot_of(state, unit) {
            saga_party::leave_unit(rom, state, pilot);
        }
        saga_party::board(rom, state, member, unit);
        if let Some(FormationChange::Fix(slot)) = change.formation {
            saga_party::join_formation(state, slot - slot % 3 + 1, member);
        }
    }
}
