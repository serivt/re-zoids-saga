//! What the Zoid lab's lists of units share: a unit shown in full with
//! its parts (START on a list, the lab task's states `0x500`–`0x560`), the
//! pages of a Zoid's parts, and the questions before a unit is taken
//! apart with weapons on its racks.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the lab
//! task's unit states at `0x08058E34`–`0x080592C4`, the part pages'
//! printer at `0x08055024`, and the rack checks of the development
//! (`0x08056C94`) and the sale (`0x08058458`); checked against Sand
//! Colony's lab in a reference emulator (breakpoints on the script runner
//! and the sound call, screenshots); see `docs/shop.md`.

use extraction::saga_party::{self, PART_SLOTS, PartSlot};

use super::super::parts::put_value;
use super::super::{
    CONFIRMED, EP_CELLS, HP_CELLS, LAST_ARMS_PAGE, LEAVE_SOUND, MenuState, PauseMenu, RACKS,
    SCRIPT_CLEAR_HELP, SCRIPT_CLOSE, SCRIPT_DF_LABEL, SCRIPT_DRAW_CHARACTER, SCRIPT_EP_LABEL,
    SCRIPT_FIXED_PAGES, SCRIPT_HP_LABEL, SCRIPT_PERCENT, SCRIPT_PRESENT, SCRIPT_PRESENT_ALL,
    SCRIPT_RACK_PAGES, SCRIPT_SIZES, SCRIPT_SP_LABEL, SCRIPT_TRAINING_LABEL, SCRIPT_WAIT_KEY,
    SCRIPT_YES_NO, ZOID_WINDOW,
};
use super::lab::LIST_WINDOW;
use super::{HELP_WINDOW, SCRIPT_DRAW_HELP, ShopStep};
use crate::ScriptHost;
use crate::script::ScriptError;
use crate::windows::ScriptWindows;

const SCRIPT_DETAIL_WINDOWS: usize = 268;
pub(super) const SCRIPT_ARMS_HELP: usize = 269;
const SCRIPT_ARMS_LAST_HELP: usize = 270;
/// The stock-full question the equipment screen asks too: 外そうとしている,
/// the part, then 「…は」「これ以上ストックできません。捨てますか？」.
const SCRIPT_TAKING_OFF: usize = 149;
const SCRIPT_STOCK_FULL: usize = 150;
/// The most of one part the stock takes: past it, the lab asks.
const STOCK_ROOM: u8 = 8;

/// The list a unit shown in full goes back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::menu) enum LabList {
    /// The units a development can be built from.
    Bases,
    /// The units the lab buys.
    Sale,
    /// The broken units the lab revives.
    Broken,
    /// The units a member can board.
    PilotUnits,
}

/// Why the lab takes a unit apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::menu) enum Taking {
    /// To build a development from it.
    Development,
    /// To buy it.
    Sale,
}

impl PauseMenu {
    /// START on a list's unit (state `0x500`): the list and the unit's
    /// window close, and script 268 (Ａボタン：装備武器表示 Ｂボタン：戻る)
    /// opens window 1 with its Zoid's name and size, the full hit and
    /// energy points, SP, DF and training it has, and its picture.
    pub(super) fn show_lab_unit(
        &mut self,
        rom: &[u8],
        unit: u8,
        back: LabList,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(status) = saga_party::unit_status(&self.game_state, unit) else {
            return self.back_to_lab_list(rom, back, windows);
        };
        if let Some(session) = self.shop.as_mut() {
            session.lab.unit_view = Some((unit, back));
        }
        self.shown_zoid = None;
        let closing: &[u8] = match back {
            LabList::Bases | LabList::Sale | LabList::PilotUnits => &[LIST_WINDOW, ZOID_WINDOW],
            LabList::Broken => &[LIST_WINDOW, 2, ZOID_WINDOW],
        };
        for window in closing {
            self.run_now(rom, SCRIPT_CLOSE + usize::from(*window), windows)?;
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_DETAIL_WINDOWS, windows)?;
        self.run_now(rom, SCRIPT_DRAW_CHARACTER, windows)?;
        self.print_zoid_name(rom, status.zoid, ZOID_WINDOW, windows)?;
        self.run_in(
            rom,
            ZOID_WINDOW,
            SCRIPT_SIZES + usize::from(status.size),
            windows,
        )?;
        let signed = |value: u32| i32::from_ne_bytes(value.to_ne_bytes());
        let rows = [
            (SCRIPT_HP_LABEL, signed(status.hp.1), HP_CELLS),
            (SCRIPT_EP_LABEL, signed(status.ep.1), HP_CELLS),
            (SCRIPT_SP_LABEL, i32::from(status.sp), HP_CELLS),
            (SCRIPT_DF_LABEL, i32::from(status.df), EP_CELLS),
        ];
        for (label, value, cells) in rows {
            windows.line_break(ZOID_WINDOW);
            self.run_in(rom, ZOID_WINDOW, label, windows)?;
            put_value(windows, ZOID_WINDOW, value, cells, 0);
        }
        self.run_in(rom, ZOID_WINDOW, SCRIPT_PERCENT, windows)?;
        windows.line_break(ZOID_WINDOW);
        self.run_in(rom, ZOID_WINDOW, SCRIPT_TRAINING_LABEL, windows)?;
        put_value(
            windows,
            ZOID_WINDOW,
            i32::from(status.training),
            EP_CELLS,
            0,
        );
        self.load_zoid_sprite(rom, status.zoid);
        self.shown_zoid = Some(status.zoid);
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.runner.start(SCRIPT_WAIT_KEY)?;
        self.state = MenuState::Shop(ShopStep::LabUnit(None));
        Ok(())
    }

    /// What the unit's page or one of its parts' pages ended with: A shows
    /// the parts with its pilot's bonuses (`0x08055024`) and turns their
    /// pages; B, or A past the last page, goes back to the list (sound
    /// `0x3F` for B).
    pub(super) fn lab_unit_step(
        &mut self,
        rom: &[u8],
        page: Option<usize>,
        code: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some((unit, back)) = self.shop.as_ref().and_then(|session| session.lab.unit_view)
        else {
            return Ok(());
        };
        let next = match page {
            None => Some(0),
            Some(page) => Some(page + 1).filter(|next| *next < PART_SLOTS),
        };
        if code == CONFIRMED
            && let Some(next) = next
        {
            let slots = saga_party::parts_of(rom, &self.game_state, unit);
            if let Some(slots) = slots {
                if next == 0 {
                    self.shown_zoid = None;
                    self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
                    self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                    self.run_now(rom, SCRIPT_ARMS_HELP, windows)?;
                }
                self.show_lab_arms(rom, &slots, next, windows)?;
                self.state = MenuState::Shop(ShopStep::LabUnit(Some(next)));
                return Ok(());
            }
        }
        match page {
            None => {
                if code != CONFIRMED {
                    windows.play_sound(LEAVE_SOUND);
                }
                self.shown_zoid = None;
                self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
            }
            Some(page) => self.close_lab_arms(rom, page, code, windows)?,
        }
        self.back_to_lab_list(rom, back, windows)
    }

    fn back_to_lab_list(
        &mut self,
        rom: &[u8],
        back: LabList,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        match back {
            LabList::Bases => self.reopen_bases(rom, windows),
            LabList::Sale => self.reopen_sale(rom, windows),
            LabList::Broken => self.reopen_broken(rom, windows),
            LabList::PilotUnits => self.reopen_pilot_units(rom, windows),
        }
    }

    /// Page `page` of a Zoid's parts (`0x08055024`): each rack, then each
    /// fixed weapon, in a window of its own that covers the page before,
    /// as the Zoid status screen's pages show them; the last page's help
    /// says both keys go back.
    pub(super) fn show_lab_arms(
        &mut self,
        rom: &[u8],
        slots: &[PartSlot; PART_SLOTS],
        page: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if page == LAST_ARMS_PAGE {
            self.run_now(rom, SCRIPT_ARMS_LAST_HELP, windows)?;
        }
        let (title, window) = if page < RACKS {
            (SCRIPT_RACK_PAGES + page, page + 1)
        } else {
            (SCRIPT_FIXED_PAGES + page - RACKS, page - RACKS + 1)
        };
        let window = u8::try_from(window).unwrap_or(ZOID_WINDOW);
        self.run_now(rom, title, windows)?;
        let slot = slots[page];
        if page < RACKS {
            self.print_rack(rom, window, slot, windows)?;
        } else {
            self.print_part(rom, window, slot.part, windows)?;
        }
        if page == 0 {
            self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        } else {
            self.run_now(rom, SCRIPT_PRESENT + usize::from(window), windows)?;
        }
        self.runner.select_window(window);
        self.runner.start(SCRIPT_WAIT_KEY)
    }

    /// Closes the windows the parts' pages up to `page` opened, and the
    /// help; B plays `0x3F` first.
    pub(super) fn close_lab_arms(
        &mut self,
        rom: &[u8],
        page: usize,
        code: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if code != CONFIRMED {
            windows.play_sound(LEAVE_SOUND);
        }
        let opened = (page + 1).min(RACKS);
        for window in (1..=opened).rev() {
            self.run_now(rom, SCRIPT_CLOSE + window, windows)?;
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)
    }

    /// Asks `script` (whether to take the weapons off the unit's racks)
    /// with はい／いいえ.
    pub(super) fn ask_strip(
        &mut self,
        rom: &[u8],
        taking: Taking,
        script: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, script, windows)?;
        self.runner.start(SCRIPT_YES_NO)?;
        self.state = MenuState::Shop(ShopStep::LabStrip(taking));
        Ok(())
    }

    /// はい takes the weapons off, asking first about each the stock has
    /// no room for; いいえ or B (sound `0x3F`) refuse.
    pub(super) fn strip_answer(
        &mut self,
        rom: &[u8],
        taking: Taking,
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if code != CONFIRMED || line != 0 {
            if code != CONFIRMED {
                windows.play_sound(LEAVE_SOUND);
            }
            return self.strip_refused(taking);
        }
        self.ask_stock_full(rom, taking, 0, windows)
    }

    /// The `rack`-th rack weapon on, the first the stock is full of:
    /// 外そうとしている…は これ以上ストックできません。捨てますか？; past the
    /// last, the weapons come off (the stock keeps each while it holds
    /// fewer than 9) and the lab goes on.
    fn ask_stock_full(
        &mut self,
        rom: &[u8],
        taking: Taking,
        rack: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(unit) = self.taken_unit(taking) else {
            return Ok(());
        };
        let weapons = saga_party::rack_weapons(&self.game_state, unit);
        let full = weapons
            .iter()
            .enumerate()
            .skip(rack)
            .find(|(_, part)| saga_party::stock(&self.game_state, **part) > STOCK_ROOM);
        let Some((index, part)) = full else {
            saga_party::strip_racks(&mut self.game_state, unit);
            return self.stripped(rom, taking, windows);
        };
        let part = *part;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_TAKING_OFF, windows)?;
        self.print_part_name(rom, HELP_WINDOW, part, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_STOCK_FULL, windows)?;
        self.runner.start(SCRIPT_YES_NO)?;
        self.state = MenuState::Shop(ShopStep::LabStockFull(taking, index));
        Ok(())
    }

    /// はい throws the weapon away and asks about the next; いいえ or B
    /// (sound `0x3F`) refuse.
    pub(super) fn stock_full_answer(
        &mut self,
        rom: &[u8],
        taking: Taking,
        rack: usize,
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if code != CONFIRMED || line != 0 {
            if code != CONFIRMED {
                windows.play_sound(LEAVE_SOUND);
            }
            return self.strip_refused(taking);
        }
        self.ask_stock_full(rom, taking, rack + 1, windows)
    }

    fn taken_unit(&self, taking: Taking) -> Option<u8> {
        match taking {
            Taking::Development => self.development_base(),
            Taking::Sale => self.chosen_sale(),
        }
    }

    fn stripped(
        &mut self,
        rom: &[u8],
        taking: Taking,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        match taking {
            Taking::Development => self.complete_development(rom),
            Taking::Sale => self.complete_sale(rom, windows),
        }
    }

    fn strip_refused(&mut self, taking: Taking) -> Result<(), ScriptError> {
        match taking {
            Taking::Development => self.development_refused(),
            Taking::Sale => self.sale_refused(),
        }
    }
}
