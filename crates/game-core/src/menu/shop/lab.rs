//! The Zoid lab (kind 2 of the shops, task `0x08055418`): its menu, the
//! revival of broken Zoids and the way out. 開発, 乗せ換え and 売る are not
//! ported yet (see `docs/shop.md`).
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the lab's
//! task at `0x08055418` (its menu at `0x08055730`, the way out at
//! `0x0805574C`, the revival at `0x080559E8`–`0x08056020` and its end at
//! `0x08059234`), the broken list (`0x080552A8`), the Zi-data list
//! (`0x0804E3A0`) and the keeper's entry (`0x080090C8`); checked against
//! Arcana's lab in a reference emulator with saves changed by hand
//! (breakpoints on the script runner and the sound call, screenshots).

use extraction::saga_party;

use super::super::parts::put_value;
use super::super::{
    CONFIRMED, EMPTY_BACK_SOUND, EMPTY_SOUND, LEAVE_SOUND, LEFT_ALIGNED, MENU_MOVE_SOUND,
    MOVED_DOWN, MOVED_UP, MenuState, PAGE_LEFT, PAGE_RIGHT, PauseMenu, SCRIPT_CLEAR_HELP,
    SCRIPT_CLEAR_MEMBERS, SCRIPT_CLOSE, SCRIPT_DRAW_MEMBERS, SCRIPT_MONEY_UNIT, SCRIPT_NOT_DONE,
    SCRIPT_PRESENT_ALL, SCRIPT_SPACE, SCRIPT_WAIT_KEY, SCRIPT_YES_NO,
};
use super::{
    HELP_WINDOW, MONEY_WINDOW, PRICE_CELLS, SCRIPT_CLEAR_MONEY, SCRIPT_DRAW_HELP,
    SCRIPT_DRAW_MONEY, ShopStep, price_shown,
};
use crate::ScriptHost;
use crate::script::ScriptError;
use crate::windows::ScriptWindows;

// The lab's scripts of the pause-menu table (257 on).
pub(super) const SCRIPT_LAB_WINDOWS: usize = 257;
pub(super) const SCRIPT_LAB_WELCOME: usize = 258;
pub(super) const SCRIPT_LAB_CHOICE: usize = 259;
const SCRIPT_NO_ZI_DATA: usize = 260;
const SCRIPT_TOO_MANY: usize = 261;
const SCRIPT_NONE_BROKEN: usize = 262;
const SCRIPT_OOPS: usize = 263;
const SCRIPT_YOU: usize = 264;
const SCRIPT_YOU_AND: usize = 265;
const SCRIPT_THE_THREE: usize = 266;
const SCRIPT_UNPILOTED: usize = 267;
const SCRIPT_SERVICE: usize = 271;
const SCRIPT_HP: usize = 300;
const SCRIPT_EP: usize = 301;
const SCRIPT_PILOT: usize = 304;
const SCRIPT_NO_PILOT: usize = 305;
const SCRIPT_REVIVAL_WINDOWS: usize = 328;
const SCRIPT_REVIVAL_HELP: usize = 329;
const SCRIPT_COST: usize = 330;
const SCRIPT_COST_QUESTION: usize = 331;
const SCRIPT_REVIVED: usize = 332;
const SCRIPT_KEPT: usize = 333;
const SCRIPT_NO_MONEY: usize = 334;
/// The lab list's menu (a `MoveMenu` of mode 6).
const SCRIPT_REVIVAL_MENU: usize = 36;
const SCRIPT_CLEAR_ZOID: usize = 2;
const SCRIPT_DRAW_ZOID: usize = 27;
const SCRIPT_PRESENT_MONEY: usize = 17;
const REVIVAL: u16 = 0;
const DEVELOPMENT: u16 = 1;
const ZOID_WINDOW: u8 = 2;
const LIST_WINDOW: u8 = 3;
const PAGE_LINES: usize = 4;
const HP_CELLS: usize = 4;
const EP_CELLS: usize = 3;
/// The four who must have a Zoid to leave: the player and the three
/// warriors.
const FIGHTERS: u8 = 4;
/// The units the party can hold.
const MOST_UNITS: u8 = 0x98;

/// Where the lab's revival stands.
#[derive(Debug, Clone, Default)]
pub(super) struct Lab {
    /// The broken units (`0x0200E7F4`).
    broken: Vec<u8>,
    page: usize,
    line: usize,
    page_shown: Option<usize>,
    shown: Option<usize>,
}

impl PauseMenu {
    fn lab_mut(&mut self) -> Option<&mut Lab> {
        self.shop.as_mut().map(|session| &mut session.lab)
    }

    fn lab(&self) -> Option<&Lab> {
        self.shop.as_ref().map(|session| &session.lab)
    }

    /// What a lab's script ended with.
    pub(super) fn lab_step(
        &mut self,
        rom: &[u8],
        step: ShopStep,
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        match step {
            ShopStep::Choice if code != CONFIRMED => self.leave_lab(rom, windows),
            ShopStep::Choice => self.lab_choice(rom, line, windows),
            ShopStep::LabNotice => {
                windows.play_sound(EMPTY_BACK_SOUND);
                self.lab_menu()
            }
            ShopStep::LabService => {
                self.state = MenuState::Closing(0);
                Ok(())
            }
            ShopStep::Broken => self.broken_choice(rom, code, line, windows),
            ShopStep::ReviveQuestion => self.revive_answer(rom, code, line, windows),
            ShopStep::ReviveNotice => {
                windows.play_sound(EMPTY_BACK_SOUND);
                self.show_broken(rom, false, windows)
            }
            ShopStep::Revived => {
                windows.play_sound(EMPTY_BACK_SOUND);
                let broken = saga_party::broken_units(&self.game_state);
                let empty = broken.is_empty();
                if let Some(lab) = self.lab_mut() {
                    lab.broken = broken;
                    lab.page_shown = None;
                    lab.shown = None;
                }
                if empty {
                    return self.all_revived(rom, windows);
                }
                self.show_broken(rom, true, windows)
            }
            ShopStep::LabRevivalEnd => {
                windows.play_sound(EMPTY_BACK_SOUND);
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                self.run_now(rom, SCRIPT_LAB_WINDOWS, windows)?;
                self.lab_menu()
            }
            _ => Ok(()),
        }
    }

    /// The lab's question and menu (script 259).
    fn lab_menu(&mut self) -> Result<(), ScriptError> {
        self.runner.start(SCRIPT_LAB_CHOICE)?;
        self.state = MenuState::Shop(ShopStep::Choice);
        Ok(())
    }

    /// A notice over the lab's menu: sound `0x4F`, then `0x41` and the
    /// menu once it is dismissed.
    fn lab_notice(
        &mut self,
        script: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.play_sound(EMPTY_SOUND);
        self.runner.start(script)?;
        self.state = MenuState::Shop(ShopStep::LabNotice);
        Ok(())
    }

    /// ゾイドの復活 opens the broken Zoids, or says there are none; ゾイド開発
    /// needs Zi data and room for another unit; the rest is not ported.
    fn lab_choice(
        &mut self,
        rom: &[u8],
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        match line {
            REVIVAL => {
                let broken = saga_party::broken_units(&self.game_state);
                if broken.is_empty() {
                    return self.lab_notice(SCRIPT_NONE_BROKEN, windows);
                }
                self.open_revival(rom, broken, windows)
            }
            DEVELOPMENT if saga_party::zi_data_count(&self.game_state) == 0 => {
                self.lab_notice(SCRIPT_NO_ZI_DATA, windows)
            }
            DEVELOPMENT if saga_party::unit_count(&self.game_state) > MOST_UNITS => {
                self.lab_notice(SCRIPT_TOO_MANY, windows)
            }
            _ => {
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                self.run_now(rom, SCRIPT_NOT_DONE, windows)?;
                self.runner.start(SCRIPT_WAIT_KEY)?;
                self.state = MenuState::Shop(ShopStep::LabNotice);
                Ok(())
            }
        }
    }

    /// B on the lab's menu (`0x0805574C`): when the player or a warrior
    /// has no Zoid the keeper objects (おっと。…); otherwise sound `0x3F`,
    /// the formation is put right, and when a Zoid is short of points the
    /// keeper says the repair is on the house before the lab closes.
    fn leave_lab(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let missing: Vec<bool> = (0..FIGHTERS)
            .map(|character| saga_party::character_unit(&self.game_state, character).is_none())
            .collect();
        if missing.iter().any(|&missing| missing) {
            windows.play_sound(EMPTY_SOUND);
            self.run_now(rom, SCRIPT_OOPS, windows)?;
            if missing[0] {
                if missing[1..].iter().any(|&missing| missing) {
                    self.run_now(rom, SCRIPT_YOU_AND, windows)?;
                    self.run_now(rom, SCRIPT_THE_THREE, windows)?;
                } else {
                    self.run_now(rom, SCRIPT_YOU, windows)?;
                }
            } else {
                self.run_now(rom, SCRIPT_THE_THREE, windows)?;
            }
            self.runner.start(SCRIPT_UNPILOTED)?;
            self.state = MenuState::Shop(ShopStep::LabNotice);
            return Ok(());
        }
        windows.play_sound(LEAVE_SOUND);
        saga_party::fix_formation(&mut self.game_state);
        if saga_party::any_damaged(&self.game_state) {
            self.runner.start(SCRIPT_SERVICE)?;
            self.state = MenuState::Shop(ShopStep::LabService);
        } else {
            self.state = MenuState::Closing(0);
        }
        Ok(())
    }

    /// The broken Zoids (`0x080559E8`): the lab's list and title close and
    /// script 328 opens the Zoid's window and the list's.
    fn open_revival(
        &mut self,
        rom: &[u8],
        broken: Vec<u8>,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(LIST_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_REVIVAL_WINDOWS, windows)?;
        for unit in &broken {
            if let Some(zoid) = saga_party::unit_status(&self.game_state, *unit).map(|u| u.zoid) {
                self.load_zoid_sprite(rom, zoid);
            }
        }
        if let Some(lab) = self.lab_mut() {
            *lab = Lab {
                broken,
                ..Lab::default()
            };
        }
        self.show_broken(rom, true, windows)
    }

    /// The revival's loop: the help, the page of names when it changed,
    /// the Zoid under the cursor when it changed, then the list's menu.
    fn show_broken(
        &mut self,
        rom: &[u8],
        page_changed: bool,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(lab) = self.lab().cloned() else {
            return Ok(());
        };
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_REVIVAL_HELP, windows)?;
        self.run_now(rom, SCRIPT_DRAW_MEMBERS, windows)?;
        let start = lab.page * PAGE_LINES;
        let mut line = lab.line;
        if page_changed || lab.page_shown != Some(lab.page) {
            if lab.page_shown.is_some() {
                self.run_now(rom, SCRIPT_CLEAR_MEMBERS, windows)?;
            }
            let page: Vec<u8> = lab
                .broken
                .iter()
                .skip(start)
                .take(PAGE_LINES)
                .copied()
                .collect();
            for (index, unit) in page.iter().enumerate() {
                if let Some(status) = saga_party::unit_status(&self.game_state, *unit) {
                    self.print_zoid_name(rom, status.zoid, LIST_WINDOW, windows)?;
                }
                if index + 1 < PAGE_LINES && start + index + 1 < lab.broken.len() {
                    windows.line_break(LIST_WINDOW);
                }
            }
            line = line.min(page.len().saturating_sub(1));
            windows.set_cursor(LIST_WINDOW, Some(line));
            windows.set_cursor(LIST_WINDOW, None);
            let more = lab.broken.len() > start + PAGE_LINES;
            windows.set_scroll_marks(LIST_WINDOW, (lab.page > 0, more));
        }
        let selected = start + line;
        if lab.shown != Some(selected)
            && let Some(unit) = lab.broken.get(selected).copied()
        {
            if lab.shown.is_some() {
                self.shown_zoid = None;
                self.run_now(rom, SCRIPT_CLEAR_ZOID, windows)?;
            }
            self.draw_broken(rom, unit, windows)?;
        }
        if let Some(lab) = self.lab_mut() {
            lab.line = line;
            lab.page_shown = Some(lab.page);
            lab.shown = Some(selected);
        }
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.runner.start(SCRIPT_REVIVAL_MENU)?;
        self.state = MenuState::Shop(ShopStep::Broken);
        Ok(())
    }

    /// Window 2 for a broken unit: the Zoid's name, its full hit and
    /// energy points, its pilot (or なし) and its picture.
    fn draw_broken(
        &mut self,
        rom: &[u8],
        unit: u8,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(status) = saga_party::unit_status(&self.game_state, unit) else {
            return Ok(());
        };
        let signed = |value: u32| i32::from_ne_bytes(value.to_ne_bytes());
        self.run_now(rom, SCRIPT_DRAW_ZOID, windows)?;
        self.print_zoid_name(rom, status.zoid, ZOID_WINDOW, windows)?;
        windows.line_break(ZOID_WINDOW);
        self.run_in(rom, ZOID_WINDOW, SCRIPT_HP, windows)?;
        put_value(windows, ZOID_WINDOW, signed(status.hp.1), HP_CELLS, 0);
        windows.line_break(ZOID_WINDOW);
        self.run_in(rom, ZOID_WINDOW, SCRIPT_EP, windows)?;
        put_value(windows, ZOID_WINDOW, signed(status.ep.1), EP_CELLS, 0);
        windows.line_break(ZOID_WINDOW);
        self.run_in(rom, ZOID_WINDOW, SCRIPT_PILOT, windows)?;
        match saga_party::pilot_of(&self.game_state, unit) {
            Some(character) => self.print_character_name(rom, character, ZOID_WINDOW, windows)?,
            None => self.run_in(rom, ZOID_WINDOW, SCRIPT_NO_PILOT, windows)?,
        }
        self.shown_zoid = Some(status.zoid);
        Ok(())
    }

    /// What the list's menu ended with: moves and pages redraw, A asks the
    /// price, B goes back to the lab's menu (sound `0x3F`).
    fn broken_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(lab) = self.lab().cloned() else {
            return Ok(());
        };
        let more = lab.broken.len() > (lab.page + 1) * PAGE_LINES;
        match code {
            MOVED_UP | MOVED_DOWN => {
                if let Some(lab) = self.lab_mut() {
                    lab.line = usize::from(line);
                }
                self.show_broken(rom, false, windows)
            }
            PAGE_LEFT | PAGE_RIGHT => {
                let page = match code {
                    PAGE_LEFT if lab.page > 0 => Some(lab.page - 1),
                    PAGE_RIGHT if more => Some(lab.page + 1),
                    _ => None,
                };
                if let Some(page) = page {
                    windows.play_sound(MENU_MOVE_SOUND);
                    if let Some(lab) = self.lab_mut() {
                        lab.page = page;
                    }
                }
                self.show_broken(rom, false, windows)
            }
            CONFIRMED => self.ask_revival(rom, windows),
            0 => {
                windows.play_sound(LEAVE_SOUND);
                self.shown_zoid = None;
                self.run_now(rom, SCRIPT_CLOSE + usize::from(LIST_WINDOW), windows)?;
                self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                self.run_now(rom, SCRIPT_LAB_WINDOWS, windows)?;
                self.lab_menu()
            }
            _ => self.show_broken(rom, false, windows),
        }
    }

    fn chosen_broken(&self) -> Option<u8> {
        let lab = self.lab()?;
        lab.broken.get(lab.page * PAGE_LINES + lab.line).copied()
    }

    /// The price and the question: 〜の復活には…Ｇ必要ですね 復活させますか？
    fn ask_revival(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(unit) = self.chosen_broken() else {
            return self.show_broken(rom, false, windows);
        };
        let zoid = saga_party::unit_status(&self.game_state, unit).map_or(0, |status| status.zoid);
        let price = saga_party::revival_price(rom, &self.game_state, unit).unwrap_or(0);
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        self.print_zoid_name(rom, zoid, HELP_WINDOW, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_COST, windows)?;
        put_value(
            windows,
            HELP_WINDOW,
            price_shown(price),
            PRICE_CELLS,
            LEFT_ALIGNED,
        );
        self.run_in(rom, HELP_WINDOW, SCRIPT_COST_QUESTION, windows)?;
        self.runner.start(SCRIPT_YES_NO)?;
        self.state = MenuState::Shop(ShopStep::ReviveQuestion);
        Ok(())
    }

    /// はい revives the Zoid when the money is there (わかりました…), or says it
    /// is not (お金が足りない…); いいえ gets それでバトルに支障は…; B goes back to
    /// the list (sound `0x3F`).
    fn revive_answer(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        if code != CONFIRMED {
            windows.play_sound(LEAVE_SOUND);
            return self.show_broken(rom, false, windows);
        }
        if line != 0 {
            self.runner.start(SCRIPT_KEPT)?;
            self.state = MenuState::Shop(ShopStep::ReviveNotice);
            return Ok(());
        }
        let Some(unit) = self.chosen_broken() else {
            return self.show_broken(rom, false, windows);
        };
        let price = saga_party::revival_price(rom, &self.game_state, unit).unwrap_or(0);
        if price > self.party.money {
            self.runner.start(SCRIPT_NO_MONEY)?;
            self.state = MenuState::Shop(ShopStep::ReviveNotice);
            return Ok(());
        }
        saga_party::revive(&mut self.game_state, unit);
        self.party.money -= price;
        self.run_now(rom, SCRIPT_CLEAR_MONEY, windows)?;
        self.run_now(rom, SCRIPT_DRAW_MONEY, windows)?;
        self.run_in(rom, MONEY_WINDOW, SCRIPT_SPACE, windows)?;
        put_value(
            windows,
            MONEY_WINDOW,
            price_shown(self.party.money),
            PRICE_CELLS,
            0,
        );
        self.run_in(rom, MONEY_WINDOW, SCRIPT_MONEY_UNIT, windows)?;
        self.run_now(rom, SCRIPT_PRESENT_MONEY, windows)?;
        self.runner.start(SCRIPT_REVIVED)?;
        self.state = MenuState::Shop(ShopStep::Revived);
        Ok(())
    }

    /// The last broken Zoid revived (`0x08059234`): the list and the
    /// Zoid's window close and the keeper says none is left.
    fn all_revived(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_CLOSE + usize::from(LIST_WINDOW), windows)?;
        self.shown_zoid = None;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
        self.runner.start(SCRIPT_NONE_BROKEN)?;
        self.state = MenuState::Shop(ShopStep::LabRevivalEnd);
        Ok(())
    }
}
