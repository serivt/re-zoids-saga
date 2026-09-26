//! セーブ with the port's save slots, which the original lacks: the list of
//! slots (see [`crate::slots`]) before the original's はい/いいえ, with a
//! question of the port's own that names the slot.
//!
//! Source of knowledge: this project's own design; the question's window
//! and the notices after it are the original's (see `docs/menu.md`).

use super::{HELP_WINDOW, MENU_WINDOW, MenuState, MenuStep, PauseMenu, Return, SCRIPT_YES_NO};
use crate::ScriptHost;
use crate::guide::GuideError;
use crate::port_text::{SLOT_OVERWRITE, SLOT_QUESTION, fill, port_text};
use crate::slots::{self, Pick, Purpose, Slot, SlotPicker};
use crate::windows::ScriptWindows;
use platform::Input;

/// The column after the main list in the original; a translation may
/// widen the list.
const MAIN_LIST_END: u8 = 9;

/// The slots the game offers, the one chosen, and the list while it is
/// on screen.
#[derive(Default)]
pub(super) struct SaveSlots {
    slots: Vec<Slot>,
    line: usize,
    picker: Option<SlotPicker>,
}

impl PauseMenu {
    /// Offers `slots` to save into, the list starting on `line`; without
    /// this call, or with one slot, セーブ asks the original's question.
    pub fn set_save_slots(&mut self, slots: Vec<Slot>, line: usize) {
        self.save.line = line.min(slots.len().saturating_sub(1));
        self.save.slots = slots;
    }

    /// The slot to write when the menu asks to save.
    #[must_use]
    pub fn save_slot(&self) -> usize {
        self.save.line
    }

    /// Whether セーブ lists the slots first.
    pub(super) fn offers_slots(&self) -> bool {
        self.save.slots.len() > 1
    }

    /// セーブ with several slots: the list, the help line saying what to do.
    pub(super) fn open_save_slots(&mut self, windows: &mut ScriptWindows<'_>) {
        let left = windows
            .windows()
            .get(usize::from(MENU_WINDOW))
            .and_then(Option::as_ref)
            .map_or(usize::from(MAIN_LIST_END), |main| main.x + main.width);
        let left = u8::try_from(left).unwrap_or(MAIN_LIST_END);
        let mut picker = SlotPicker::new(
            self.save.slots.clone(),
            Purpose::Save,
            slots::menu_layout(left),
            self.save.line,
        );
        picker.open(self.held, windows);
        self.save.picker = Some(picker);
        self.state = MenuState::SaveSlot;
    }

    /// A frame of the list, the menu staying open: a slot chosen closes
    /// it and asks whether to save there (or over its game) with the
    /// original's はい/いいえ; B goes back to the main menu.
    pub(super) fn save_slot_frame(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<MenuStep, GuideError> {
        let Some(picker) = self.save.picker.as_mut() else {
            return Ok(MenuStep::Open);
        };
        let pick = picker.update(rom, input, windows)?;
        let Some(pick) = pick else {
            return Ok(MenuStep::Open);
        };
        picker.close(windows);
        let chosen = match pick {
            Pick::Slot(slot) => Some((slot, picker.slot(slot).is_some_and(Slot::has_game))),
            Pick::Canceled => None,
        };
        self.save.picker = None;
        self.runner.hold(input);
        let Some((slot, taken)) = chosen else {
            self.return_to(rom, Return::Main, windows)?;
            return Ok(MenuStep::Open);
        };
        self.save.line = slot;
        let key = if taken { SLOT_OVERWRITE } else { SLOT_QUESTION };
        let number = u32::try_from(slot + 1).unwrap_or(u32::MAX);
        let question = fill(&port_text(windows.extensions(), key), &[("slot", number)]);
        for ch in question.chars() {
            windows.put_char(HELP_WINDOW, ch);
        }
        self.runner.start(SCRIPT_YES_NO)?;
        self.state = MenuState::Save;
        Ok(MenuStep::Open)
    }
}
