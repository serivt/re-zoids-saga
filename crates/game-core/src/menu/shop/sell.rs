//! The Zoid lab's ゾイドを売る (the lab task's states `0x400`–`0x402`): the
//! party's units, what the lab pays for one, and the sale.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the lab
//! task's sale states at `0x08058458`–`0x08058DB2`, the unit list at
//! `0x08055120` and the training raise at `0x080346C0`; checked against
//! Sand Colony's lab in a reference emulator with saves changed by hand
//! (breakpoints on the script runner and the sound call, screenshots);
//! see `docs/shop.md`.

use extraction::saga_party;
use extraction::saga_shop::MONEY_LIMIT;

use super::super::parts::put_value;
use super::super::{
    CONFIRMED, EMPTY_BACK_SOUND, EMPTY_SOUND, LEAVE_SOUND, LEFT_ALIGNED, MENU_MOVE_SOUND,
    MOVED_DOWN, MOVED_UP, MenuState, PAGE_LEFT, PAGE_RIGHT, PauseMenu, SCRIPT_CLEAR_HELP,
    SCRIPT_CLOSE, SCRIPT_DRAW_MEMBERS, SCRIPT_MONEY_WINDOW, SCRIPT_PRESENT_ALL, SCRIPT_YES_NO,
    ZOID_WINDOW,
};
use super::lab::{LIST_WINDOW, SCRIPT_LAB_WINDOWS, SCRIPT_REVIVAL_MENU};
use super::units::{LabList, Taking};
use super::{HELP_WINDOW, PRICE_CELLS, SCRIPT_DRAW_HELP, ShopStep, price_shown};
use crate::ScriptHost;
use crate::script::ScriptError;
use crate::windows::ScriptWindows;

const SCRIPT_WHICH: usize = 318;
const SCRIPT_THE_ZOID: usize = 319;
const SCRIPT_PRICE_QUESTION: usize = 320;
const SCRIPT_BOUGHT: usize = 321;
const SCRIPT_KEPT: usize = 322;
const SCRIPT_FEW_ZOIDS: usize = 323;
const SCRIPT_FEW_ZOIDS_END: usize = 324;
const SCRIPT_STRIP_QUESTION: usize = 325;
const SCRIPT_NOT_STRIPPED: usize = 326;
const SCRIPT_NO_PRICE: usize = 327;
const SCRIPT_THE_PILOT: usize = 289;
const SCRIPT_WONT_LEAVE: usize = 291;
const SCRIPT_WINDOWS: usize = 293;
const SCRIPT_ON_BOARD: usize = 295;
const SCRIPT_PILOTED: usize = 296;
const SCRIPT_CLEAR_UNIT: usize = 1;
const SCRIPT_CLEAR_LIST: usize = 3;
const SCRIPT_MONEY_UNIT: usize = super::super::SCRIPT_MONEY_UNIT;
const SCRIPT_SPACE: usize = super::super::SCRIPT_SPACE;
const PAGE_LINES: usize = 6;
const STARTED: u16 = 0x80;
/// The lab buys nothing unless the party could part with this many.
const FEWEST_UNITS: usize = 5;
const COUNT_CELLS: usize = 3;

/// Where the sale stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::menu) enum Step {
    /// The party's units (state `0x401`).
    List,
    /// Whether to take the chosen unit from its pilot.
    PilotQuestion,
    /// What the lab pays, and whether to sell.
    Question,
    /// A notice, then the list; with `true` printed anew.
    Notice(bool),
    /// それでは買い取りはできませんね・・・, then the list.
    Refused,
}

/// The sale's list.
#[derive(Debug, Clone, Default)]
pub(super) struct Sale {
    units: Vec<u8>,
    page: usize,
    line: usize,
    page_shown: Option<usize>,
    shown: Option<usize>,
    /// The page and the unit are to be cleared and printed again, as a
    /// sale changed them.
    reprint: bool,
}

impl PauseMenu {
    fn sale(&self) -> Sale {
        self.shop
            .as_ref()
            .map(|session| session.lab.sale.clone())
            .unwrap_or_default()
    }

    fn sale_mut(&mut self) -> Option<&mut Sale> {
        self.shop.as_mut().map(|session| &mut session.lab.sale)
    }

    fn sale_step_to(&mut self, step: Step) {
        self.state = MenuState::Shop(ShopStep::Sell(step));
    }

    /// The unit under the sale list's cursor.
    pub(super) fn chosen_sale(&self) -> Option<u8> {
        let sale = self.sale();
        sale.units.get(sale.page * PAGE_LINES + sale.line).copied()
    }

    /// ゾイドを売る (state `0x400`): windows 3, 2 and 1 close, script 293
    /// opens the unit's window (1) and the list's (3).
    pub(super) fn open_sale(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if let Some(sale) = self.sale_mut() {
            *sale = Sale::default();
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        for window in [LIST_WINDOW, 2, ZOID_WINDOW] {
            self.run_now(rom, SCRIPT_CLOSE + usize::from(window), windows)?;
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_WINDOWS, windows)?;
        self.show_sale(rom, true, windows)
    }

    /// The list again after a unit shown in full (state `0x401`).
    pub(super) fn reopen_sale(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_WINDOWS, windows)?;
        if let Some(sale) = self.sale_mut() {
            sale.page_shown = None;
            sale.shown = None;
        }
        self.show_sale(rom, true, windows)
    }

    /// The sale's loop (`0x080584CE`): the list is taken again (every
    /// unit slot with a Zoid, `0x08055120`); with `help` the help asks
    /// どのゾイドを売りたいのですか？; six units a page in window 3 by their
    /// Zoids' names, stepping back a page when the cursor's is empty; the
    /// unit under the cursor in window 1; then the menu.
    fn show_sale(
        &mut self,
        rom: &[u8],
        help: bool,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let units = saga_party::zoid_units(&self.game_state);
        if let Some(sale) = self.sale_mut() {
            sale.units = units;
            while sale.page > 0 && sale.page * PAGE_LINES >= sale.units.len() {
                sale.page -= 1;
                sale.line = PAGE_LINES - 1;
            }
        }
        if help {
            self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
            self.run_now(rom, SCRIPT_WHICH, windows)?;
            self.run_now(rom, SCRIPT_DRAW_MEMBERS, windows)?;
        }
        let sale = self.sale();
        let start = sale.page * PAGE_LINES;
        let mut line = sale.line;
        if sale.reprint || sale.page_shown != Some(sale.page) {
            if sale.page_shown.is_some() {
                self.run_now(rom, SCRIPT_CLEAR_LIST, windows)?;
            }
            let page: Vec<u8> = sale
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
                if index + 1 < PAGE_LINES && start + index + 1 < sale.units.len() {
                    windows.line_break(LIST_WINDOW);
                }
            }
            line = line.min(page.len().saturating_sub(1));
            windows.set_cursor(LIST_WINDOW, Some(line));
            windows.set_cursor(LIST_WINDOW, None);
            let more = sale.units.len() > start + PAGE_LINES;
            windows.set_scroll_marks(LIST_WINDOW, (sale.page > 0, more));
        }
        let selected = start + line;
        if (sale.reprint || sale.shown != Some(selected))
            && let Some(unit) = sale.units.get(selected).copied()
        {
            if sale.shown.is_some() {
                self.shown_zoid = None;
                self.run_now(rom, SCRIPT_CLEAR_UNIT, windows)?;
            }
            if let Some(status) = saga_party::unit_status(&self.game_state, unit) {
                self.load_zoid_sprite(rom, status.zoid);
            }
            self.draw_unit(rom, unit, ZOID_WINDOW, windows)?;
        }
        if let Some(sale) = self.sale_mut() {
            sale.line = line;
            sale.page_shown = Some(sale.page);
            sale.shown = Some(selected);
            sale.reprint = false;
        }
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.runner.start(SCRIPT_REVIVAL_MENU)?;
        self.sale_step_to(Step::List);
        Ok(())
    }

    /// What a sale's script ended with.
    pub(super) fn sale_step(
        &mut self,
        rom: &[u8],
        step: Step,
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        match step {
            Step::List => self.sale_choice(rom, code, line, windows),
            Step::PilotQuestion => {
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                if code == CONFIRMED && line == 0 {
                    return self.ask_sale(rom, windows);
                }
                if code != CONFIRMED {
                    windows.play_sound(LEAVE_SOUND);
                }
                self.show_sale(rom, true, windows)
            }
            Step::Question => self.sale_answer(rom, code, line, windows),
            Step::Notice(redraw) => {
                windows.play_sound(EMPTY_BACK_SOUND);
                if redraw && let Some(sale) = self.sale_mut() {
                    sale.reprint = true;
                }
                self.show_sale(rom, true, windows)
            }
            Step::Refused => self.show_sale(rom, true, windows),
        }
    }

    /// Moves and pages print again; START shows the unit in full; A asks
    /// to sell it; B goes back to the lab's menu (sound `0x3F`).
    fn sale_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let sale = self.sale();
        let more = sale.units.len() > (sale.page + 1) * PAGE_LINES;
        match code {
            MOVED_UP | MOVED_DOWN => {
                if let Some(sale) = self.sale_mut() {
                    sale.line = usize::from(line);
                }
                self.show_sale(rom, false, windows)
            }
            PAGE_LEFT | PAGE_RIGHT => {
                let page = match code {
                    PAGE_LEFT if sale.page > 0 => Some(sale.page - 1),
                    PAGE_RIGHT if more => Some(sale.page + 1),
                    _ => None,
                };
                if let Some(page) = page {
                    windows.play_sound(MENU_MOVE_SOUND);
                    if let Some(sale) = self.sale_mut() {
                        sale.page = page;
                    }
                }
                self.show_sale(rom, false, windows)
            }
            CONFIRMED => self.choose_sale(rom, windows),
            STARTED => match self.chosen_sale() {
                Some(unit) => self.show_lab_unit(rom, unit, LabList::Sale, windows),
                None => self.show_sale(rom, false, windows),
            },
            _ => {
                windows.play_sound(LEAVE_SOUND);
                self.shown_zoid = None;
                self.run_now(rom, SCRIPT_CLOSE + usize::from(LIST_WINDOW), windows)?;
                self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                self.run_now(rom, SCRIPT_LAB_WINDOWS, windows)?;
                self.run_now(rom, SCRIPT_MONEY_WINDOW, windows)?;
                self.run_in(rom, super::MONEY_WINDOW, SCRIPT_SPACE, windows)?;
                put_value(
                    windows,
                    super::MONEY_WINDOW,
                    price_shown(self.party.money),
                    PRICE_CELLS,
                    0,
                );
                self.run_in(rom, super::MONEY_WINDOW, SCRIPT_MONEY_UNIT, windows)?;
                self.reopen_lab_cursor(windows);
                self.lab_menu()
            }
        }
    }

    /// A on a unit (`0x08058458`): with fewer than five units the party
    /// could part with, ゾイドが…体以上いないとバトルがつらくなるのではないですか？
    /// (with the list's length); a Zoid the record gives no price, そのゾイドは
    /// ちょっと値がつけられませんね…; a pilot who keeps the unit will not leave
    /// it; otherwise the lab asks whether to take it from its pilot, then
    /// the price.
    fn choose_sale(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(unit) = self.chosen_sale() else {
            return self.show_sale(rom, false, windows);
        };
        if saga_party::sellable_count(&self.game_state) < FEWEST_UNITS {
            windows.play_sound(EMPTY_SOUND);
            let listed = self.sale().units.len();
            self.run_in(rom, HELP_WINDOW, SCRIPT_FEW_ZOIDS, windows)?;
            let listed = i32::try_from(listed).unwrap_or(i32::MAX);
            put_value(windows, HELP_WINDOW, listed, COUNT_CELLS, LEFT_ALIGNED);
            return self.sale_notice(SCRIPT_FEW_ZOIDS_END, false);
        }
        if saga_party::sale_price(rom, &self.game_state, unit) == Some(0) {
            windows.play_sound(EMPTY_SOUND);
            self.runner.select_window(HELP_WINDOW);
            return self.sale_notice(SCRIPT_NO_PRICE, false);
        }
        let Some(pilot) = saga_party::pilot_of(&self.game_state, unit) else {
            return self.ask_sale(rom, windows);
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
            return self.sale_notice(SCRIPT_WONT_LEAVE, false);
        }
        self.print_zoid_name(rom, zoid, HELP_WINDOW, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_ON_BOARD, windows)?;
        self.print_character_name(rom, pilot, HELP_WINDOW, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_PILOTED, windows)?;
        self.runner.start(SCRIPT_YES_NO)?;
        self.sale_step_to(Step::PilotQuestion);
        Ok(())
    }

    fn sale_notice(&mut self, script: usize, redraw: bool) -> Result<(), ScriptError> {
        self.runner.start(script)?;
        self.sale_step_to(Step::Notice(redraw));
        Ok(())
    }

    /// The Zoid's name, 319 は, the price left-aligned, 320 Ｇですね 本当に
    /// よろしいのですね？ and はい／いいえ.
    fn ask_sale(&mut self, rom: &[u8], windows: &mut ScriptWindows<'_>) -> Result<(), ScriptError> {
        let Some(unit) = self.chosen_sale() else {
            return self.show_sale(rom, false, windows);
        };
        let zoid = saga_party::unit_status(&self.game_state, unit).map_or(0, |status| status.zoid);
        let price = saga_party::sale_price(rom, &self.game_state, unit).unwrap_or(0);
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        self.print_zoid_name(rom, zoid, HELP_WINDOW, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_THE_ZOID, windows)?;
        put_value(
            windows,
            HELP_WINDOW,
            price_shown(price),
            PRICE_CELLS,
            LEFT_ALIGNED,
        );
        self.run_in(rom, HELP_WINDOW, SCRIPT_PRICE_QUESTION, windows)?;
        self.runner.start(SCRIPT_YES_NO)?;
        self.sale_step_to(Step::Question);
        Ok(())
    }

    /// はい sells, asking first to take the unit's weapons off; いいえ gets
    /// そうですか・・・; B goes back to the list (sound `0x3F`).
    fn sale_answer(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        if code != CONFIRMED {
            windows.play_sound(LEAVE_SOUND);
            return self.show_sale(rom, true, windows);
        }
        if line != 0 {
            self.runner.select_window(HELP_WINDOW);
            return self.sale_notice(SCRIPT_KEPT, false);
        }
        let Some(unit) = self.chosen_sale() else {
            return self.show_sale(rom, true, windows);
        };
        if !saga_party::rack_weapons(&self.game_state, unit).is_empty() {
            return self.ask_strip(rom, Taking::Sale, SCRIPT_STRIP_QUESTION, windows);
        }
        self.complete_sale(rom, windows)
    }

    /// The sale: the unit is taken apart (its pilot leaves it and it is
    /// cleared), the price paid, at most 9,999,999 in hand; わかりました。
    /// 買い取りましょう.
    pub(super) fn complete_sale(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(unit) = self.chosen_sale() else {
            return self.show_sale(rom, true, windows);
        };
        let price = saga_party::sale_price(rom, &self.game_state, unit).unwrap_or(0);
        saga_party::take_apart(rom, &mut self.game_state, unit);
        self.party.money = self.party.money.saturating_add(price).min(MONEY_LIMIT);
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.runner.select_window(HELP_WINDOW);
        self.sale_notice(SCRIPT_BOUGHT, true)
    }

    /// それでは買い取りはできませんね・・・, when the unit's weapons stay on.
    pub(super) fn sale_refused(&mut self) -> Result<(), ScriptError> {
        self.runner.select_window(HELP_WINDOW);
        self.runner.start(SCRIPT_NOT_STRIPPED)?;
        self.sale_step_to(Step::Refused);
        Ok(())
    }
}
