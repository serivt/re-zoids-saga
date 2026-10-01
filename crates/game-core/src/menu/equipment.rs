//! The 武装 screen: which Zoid's weapons to change, its three racks, and
//! the stocked parts one of them can take.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! pause menu's state machine at `0x0804E58C`–`0x08052232` (states
//! `0x3000`–`0x3200` and `0x3FFF`), the rack printer at `0x0804E13C`, the
//! part list at `0x0804E24C` and the Zoid picture at `0x08044E98`, traced
//! in a reference emulator while the Shield Liger's laser was taken off
//! and put back; see `docs/menu.md`.

use extraction::saga_party::{self, PartSlot};

use super::parts::put_value;
use super::{
    CONFIRMED, EMPTY_BACK_SOUND, EMPTY_SOUND, HELP_WINDOW, LEAVE_SOUND, MEMBERS_PER_PAGE,
    MENU_MOVE_SOUND, MOVED_DOWN, MOVED_UP, MenuState, PAGE_LEFT, PAGE_RIGHT, PauseMenu, RACKS,
    Return, SCRIPT_CLEAR_CHARACTER, SCRIPT_CLEAR_HELP, SCRIPT_CLEAR_MEMBERS, SCRIPT_CLOSE,
    SCRIPT_COLON, SCRIPT_DRAW_CHARACTER, SCRIPT_DRAW_HELP, SCRIPT_DRAW_MEMBERS, SCRIPT_DRAW_STOCK,
    SCRIPT_MEMBER_MENU, SCRIPT_NO_PART, SCRIPT_NO_RACK, SCRIPT_PRESENT, SCRIPT_PRESENT_ALL,
    SCRIPT_RACK_KINDS, SCRIPT_TIMES, SCRIPT_WAIT_KEY, WEAPON, ZERO_PADDED,
};
use crate::ScriptHost;
use crate::script::ScriptError;
use crate::windows::ScriptWindows;

const SCRIPT_WINDOWS: usize = 128;
const SCRIPT_NO_ZOID: usize = 129;
const SCRIPT_RACK_HEADER: usize = 130;
const SCRIPT_RACK_INDENTS: [usize; 2] = [131, 132];
const SCRIPT_HELP: usize = 133;
const SCRIPT_NOT_BOARDED: usize = 134;
const SCRIPT_KEEPS: usize = 135;
const SCRIPT_RACK_WINDOW: usize = 136;
const SCRIPT_RACK_NUMBERS: usize = 137;
const SCRIPT_NO_RACK_NOTICE: usize = 140;
const SCRIPT_FIXED_NOTICE: usize = 141;
const SCRIPT_EQUIP_WINDOWS: usize = 142;
const SCRIPT_TAKE_OFF: usize = 147;
const SCRIPT_BLANK_KIND: usize = 217;
const SCRIPT_FIXED_KIND: usize = 218;
const SCRIPT_RACK_MENU: usize = 34;
const SCRIPT_DRAW_RACKS: usize = 29;
const SCRIPT_TRYING: usize = 149;
const SCRIPT_STOCK_FULL: usize = 150;
const SCRIPT_YES_NO: usize = 61;
const ZOID_WINDOW: u8 = 1;
const MEMBER_WINDOW: u8 = 3;
const RACK_WINDOW: u8 = 4;
const TITLE_WINDOW: u8 = 1;
const LIST_WINDOW: u8 = 2;
const EQUIP_PAGE: usize = 4;
const EQUIPPED_SOUND: u8 = 0x4E;
const BLINK_FRAMES: i32 = 16;

/// Where the 武装 screen is.
#[derive(Debug, Clone, Default)]
pub(super) struct Equipment {
    page: usize,
    member: Option<usize>,
    rack: usize,
    rack_shown: Option<usize>,
    list: Vec<u16>,
    list_page: usize,
    /// The entry described: 0 is 装備を外す, `n` the `n`-th stocked part.
    entry: Option<usize>,
    zoid: Option<u16>,
    /// The menu frame the rack's own part started blinking on, while
    /// 装備を外す is under the cursor (task `0x0804D8F0`).
    blink_from: Option<i32>,
}

/// The part the rack's list describes: the rack's own on 装備を外す, else
/// the stocked part under the cursor.
pub(super) fn shown_part(menu: &PauseMenu) -> Option<u16> {
    match menu.equipment.entry? {
        0 => menu.rack_part(),
        _ => menu.chosen_part(),
    }
}

/// The member whose Zoid the screen changes.
pub(super) fn character(menu: &PauseMenu) -> u8 {
    menu.member_character()
}

/// A weapon drawn on the rack list's Zoid picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Mounted {
    /// The part.
    pub part: u16,
    /// The rack it sits on: 0 in front of the Zoid, 1 and 2 behind it.
    pub rack: usize,
    /// Whether it shows this frame.
    pub visible: bool,
    /// Whether it is drawn into the Zoid's picture rather than as a
    /// sprite: the first two racks' parts, when another rack is being
    /// changed (`0x08053204`).
    pub baked: bool,
}

impl PauseMenu {
    /// The member list (state `0x3000`): windows 1 and 3, the help, then
    /// the members and the Zoid of the one under the cursor.
    pub(super) fn open_equipment(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        for id in (1..=3).rev() {
            self.run_now(rom, SCRIPT_CLOSE + id, windows)?;
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_WINDOWS, windows)?;
        self.equipment = Equipment::default();
        windows.set_cursor(MEMBER_WINDOW, Some(0));
        windows.set_cursor(MEMBER_WINDOW, None);
        self.enter_equipment_members(rom, true, 0, windows)
    }

    /// State `0x3001`: the help line and window 3, then the list.
    fn enter_equipment_members(
        &mut self,
        rom: &[u8],
        page_changed: bool,
        line: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_HELP, windows)?;
        self.run_now(rom, SCRIPT_DRAW_MEMBERS, windows)?;
        self.show_equipment_members(rom, page_changed, line, windows)
    }

    /// The member loop: prints the page of names when it changed and
    /// window 1 when the member under the cursor changed, then runs the
    /// member list's menu.
    fn show_equipment_members(
        &mut self,
        rom: &[u8],
        page_changed: bool,
        line: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let start = self.equipment.page * MEMBERS_PER_PAGE;
        let count = self.roster.members.len();
        let mut line = line;
        if page_changed {
            self.run_now(rom, SCRIPT_CLEAR_MEMBERS, windows)?;
            let characters: Vec<u8> = self
                .roster
                .members
                .iter()
                .skip(start)
                .take(MEMBERS_PER_PAGE)
                .map(|member| member.character)
                .collect();
            for (index, character) in characters.iter().enumerate() {
                self.print_character_name(rom, *character, MEMBER_WINDOW, windows)?;
                if index + 1 < MEMBERS_PER_PAGE && start + index + 1 < count {
                    windows.line_break(MEMBER_WINDOW);
                }
            }
            line = line.min(characters.len().saturating_sub(1));
            windows.set_cursor(MEMBER_WINDOW, Some(line));
            windows.set_cursor(MEMBER_WINDOW, None);
            let more = count > start + MEMBERS_PER_PAGE;
            windows.set_scroll_marks(MEMBER_WINDOW, (self.equipment.page > 0, more));
        }
        let member = start + line;
        if self.equipment.member == Some(member) {
            self.run_now(rom, SCRIPT_PRESENT + usize::from(HELP_WINDOW), windows)?;
        } else {
            self.equipment.member = Some(member);
            self.member = member;
            self.draw_equipment_zoid(rom, true, windows)?;
        }
        self.run_now(rom, SCRIPT_DRAW_MEMBERS, windows)?;
        self.runner.start(SCRIPT_MEMBER_MENU)?;
        self.state = MenuState::Weapons;
        Ok(())
    }

    /// Window 1 for the member under the cursor: its Zoid's name, the
    /// three racks' parts and the Zoid's picture, or 搭乗ゾイドなし.
    fn draw_equipment_zoid(
        &mut self,
        rom: &[u8],
        clear: bool,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let member = self.roster.members.get(self.member).copied();
        let unit = member.and_then(|member| member.unit.zip(member.parts));
        self.shown_zoid = None;
        if clear {
            self.run_now(rom, SCRIPT_CLEAR_CHARACTER, windows)?;
        }
        self.run_now(rom, SCRIPT_DRAW_CHARACTER, windows)?;
        match unit {
            Some((unit, slots)) => {
                self.print_zoid_name(rom, unit.zoid, ZOID_WINDOW, windows)?;
                windows.line_break(ZOID_WINDOW);
                self.run_in(rom, ZOID_WINDOW, SCRIPT_RACK_HEADER, windows)?;
                for (rack, slot) in slots.iter().take(RACKS).enumerate() {
                    if rack > 0 {
                        windows.line_break(ZOID_WINDOW);
                        self.run_in(rom, ZOID_WINDOW, SCRIPT_RACK_INDENTS[rack - 1], windows)?;
                    }
                    self.print_slot_part(rom, ZOID_WINDOW, *slot, windows)?;
                }
                self.shown_zoid = Some(unit.zoid);
            }
            None => self.run_in(rom, ZOID_WINDOW, SCRIPT_NO_ZOID, windows)?,
        }
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)
    }

    /// A rack's part as the member list shows it: ラックなし for a fixed
    /// slot the Zoid does not fit, 装備なし, or the part's name.
    fn print_slot_part(
        &mut self,
        rom: &[u8],
        window: u8,
        slot: PartSlot,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if slot.rack == 0 && !slot.fitted {
            return self.run_in(rom, window, SCRIPT_NO_RACK, windows);
        }
        match slot.part {
            Some(part) => self.print_part_name(rom, window, part.id, windows),
            None => self.run_in(rom, window, SCRIPT_NO_PART, windows),
        }
    }

    /// What the member list's menu ended with (state `0x3001`): moves and
    /// page turns redraw, B leaves to the main menu, A goes on to the
    /// member's racks unless it has no Zoid or keeps its equipment.
    pub(super) fn equipment_member_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let line = usize::from(line);
        let current = self.member % MEMBERS_PER_PAGE;
        let count = self.roster.members.len();
        match code {
            MOVED_UP | MOVED_DOWN => self.show_equipment_members(rom, false, line, windows),
            PAGE_LEFT if self.equipment.page > 0 => {
                windows.play_sound(MENU_MOVE_SOUND);
                self.equipment.page -= 1;
                self.show_equipment_members(rom, true, current, windows)
            }
            PAGE_RIGHT if count > (self.equipment.page + 1) * MEMBERS_PER_PAGE => {
                windows.play_sound(MENU_MOVE_SOUND);
                self.equipment.page += 1;
                self.show_equipment_members(rom, true, current, windows)
            }
            PAGE_LEFT | PAGE_RIGHT => self.show_equipment_members(rom, false, current, windows),
            CONFIRMED => {
                let member = self.roster.members.get(self.member).copied();
                match member {
                    Some(member) if member.unit.is_none() => {
                        windows.play_sound(EMPTY_SOUND);
                        self.run_now(rom, SCRIPT_NOT_BOARDED, windows)?;
                        self.notice(SCRIPT_WAIT_KEY, Return::Weapons)
                    }
                    Some(member) if member.keeps_equipment => {
                        windows.play_sound(EMPTY_SOUND);
                        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
                        self.print_character_name(rom, member.character, HELP_WINDOW, windows)?;
                        self.run_in(rom, HELP_WINDOW, SCRIPT_KEEPS, windows)?;
                        self.notice(SCRIPT_WAIT_KEY, Return::Weapons)
                    }
                    _ => {
                        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                        self.run_now(rom, SCRIPT_RACK_WINDOW, windows)?;
                        self.equipment.rack = 0;
                        self.open_racks(rom, windows)
                    }
                }
            }
            _ => {
                windows.play_sound(LEAVE_SOUND);
                self.shown_zoid = None;
                windows.set_scroll_marks(MEMBER_WINDOW, (false, false));
                self.build(rom, windows)?;
                self.return_to(rom, Return::Main, windows)
            }
        }
    }

    /// Back to the member list after a notice (`0x41`, script 0).
    pub(super) fn equipment_members_again(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.play_sound(EMPTY_BACK_SOUND);
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        let line = self.member % MEMBERS_PER_PAGE;
        self.enter_equipment_members(rom, false, line, windows)
    }

    /// The racks of the member's Zoid in window 4 (state `0x3102`), each
    /// its number, kind and part (`0x0804E13C`).
    fn open_racks(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(slots) = self.member_slots() else {
            return self.enter_equipment_members(rom, false, 0, windows);
        };
        self.equipment.rack_shown = None;
        self.run_now(rom, SCRIPT_DRAW_RACKS, windows)?;
        for (rack, slot) in slots.iter().take(RACKS).enumerate() {
            if rack > 0 {
                windows.line_break(RACK_WINDOW);
            }
            self.run_in(rom, RACK_WINDOW, SCRIPT_RACK_NUMBERS + rack, windows)?;
            self.print_rack_line(rom, RACK_WINDOW, *slot, windows)?;
        }
        windows.set_cursor(RACK_WINDOW, Some(self.equipment.rack));
        windows.set_cursor(RACK_WINDOW, None);
        self.show_racks(rom, windows)
    }

    /// The rack loop: the help line describes the part on the rack under
    /// the cursor, then the rack list's menu runs.
    fn show_racks(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.describe_rack(rom, windows)?;
        self.runner.select_window(RACK_WINDOW);
        self.runner.start(SCRIPT_RACK_MENU)?;
        self.state = MenuState::Racks;
        Ok(())
    }

    /// Prints a rack's kind and part as window 4 lists them (`0x0804E13C`).
    fn print_rack_line(
        &mut self,
        rom: &[u8],
        window: u8,
        slot: PartSlot,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let kind = match (slot.rack, slot.fitted) {
            (0, false) => SCRIPT_BLANK_KIND,
            (0, true) => SCRIPT_FIXED_KIND,
            (rack, _) => SCRIPT_RACK_KINDS + usize::from(rack),
        };
        self.run_in(rom, window, kind, windows)?;
        self.run_in(rom, window, SCRIPT_COLON, windows)?;
        self.print_slot_part(rom, window, slot, windows)
    }

    /// The help line for the rack under the cursor, when it changed: the
    /// part's description with its pilot's values, 装備なし or ラックなし.
    fn describe_rack(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if self.equipment.rack_shown == Some(self.equipment.rack) {
            return Ok(());
        }
        let Some(slot) = self.member_slots().map(|slots| slots[self.equipment.rack]) else {
            return Ok(());
        };
        if self.equipment.rack_shown.is_some() {
            self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        }
        self.equipment.rack_shown = Some(self.equipment.rack);
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        if slot.rack == 0 && !slot.fitted {
            self.run_in(rom, HELP_WINDOW, SCRIPT_NO_RACK, windows)?;
        } else {
            self.describe_equipped(rom, slot.part.map(|part| part.id), windows)?;
        }
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)
    }

    /// A part's description in the help line with the values the member
    /// would use it with, or 装備なし.
    fn describe_equipped(
        &mut self,
        rom: &[u8],
        id: Option<u16>,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let character = self.member_character();
        let part = id.and_then(|id| saga_party::part(rom, &self.game_state, character, id));
        match part {
            Some(part) if part.flags & WEAPON != 0 => {
                self.describe_weapon(rom, HELP_WINDOW, part, windows)
            }
            Some(part) => self.describe_support(rom, HELP_WINDOW, part, true, windows),
            None => self.run_in(rom, HELP_WINDOW, SCRIPT_NO_PART, windows),
        }
    }

    /// What the rack list's menu ended with (state `0x3102`): a move
    /// describes that rack, A opens a rack's parts or explains why not,
    /// B goes back to the members.
    pub(super) fn rack_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        match code {
            MOVED_UP | MOVED_DOWN => {
                self.equipment.rack = usize::from(line).min(RACKS - 1);
                self.show_racks(rom, windows)
            }
            CONFIRMED => {
                let Some(slot) = self.member_slots().map(|slots| slots[self.equipment.rack]) else {
                    return self.show_racks(rom, windows);
                };
                if slot.rack != 0 {
                    return self.open_rack(rom, windows);
                }
                windows.play_sound(EMPTY_SOUND);
                let notice = if slot.fitted {
                    SCRIPT_FIXED_NOTICE
                } else {
                    SCRIPT_NO_RACK_NOTICE
                };
                self.equipment.rack_shown = None;
                self.notice(notice, Return::Racks)
            }
            _ => {
                windows.play_sound(LEAVE_SOUND);
                self.run_now(rom, SCRIPT_CLOSE + usize::from(RACK_WINDOW), windows)?;
                self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                let line = self.member % MEMBERS_PER_PAGE;
                self.enter_equipment_members(rom, false, line, windows)
            }
        }
    }

    /// Back to the rack list after its notices (`0x41`).
    pub(super) fn racks_again(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.play_sound(EMPTY_BACK_SOUND);
        self.show_racks(rom, windows)
    }

    /// The parts a rack can take (state `0x3200`): the Zoid's picture on
    /// the left, the rack's number and kind, then 装備を外す and the
    /// stocked parts its kind accepts, four lines a page.
    fn open_rack(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(slot) = self.member_slots().map(|slots| slots[self.equipment.rack]) else {
            return self.show_racks(rom, windows);
        };
        for script in [
            SCRIPT_CLOSE + usize::from(RACK_WINDOW),
            SCRIPT_CLOSE + usize::from(MEMBER_WINDOW),
        ] {
            self.run_now(rom, script, windows)?;
        }
        self.shown_zoid = None;
        self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        windows.set_scroll_marks(MEMBER_WINDOW, (false, false));
        self.equipment.zoid = self.member_zoid();
        self.run_now(rom, SCRIPT_EQUIP_WINDOWS, windows)?;
        let number = i32::try_from(self.equipment.rack + 1).unwrap_or(1);
        put_value(windows, TITLE_WINDOW, number, 1, 0);
        let mask = match slot.flags {
            1 => Some((SCRIPT_RACK_KINDS + 1, 1)),
            2 => Some((SCRIPT_RACK_KINDS + 2, 0xE)),
            3 => Some((SCRIPT_RACK_KINDS + 3, 0xF)),
            _ => None,
        };
        if let Some((kind, mask)) = mask {
            self.run_in(rom, TITLE_WINDOW, kind, windows)?;
            self.equipment.list = saga_party::stocked_parts(rom, &self.game_state, mask);
        }
        self.equipment.list_page = 0;
        self.equipment.entry = None;
        windows.set_cursor(LIST_WINDOW, Some(0));
        windows.set_cursor(LIST_WINDOW, None);
        self.show_rack_parts(rom, true, 0, windows)
    }

    /// Prints the page of the rack's parts when it changed and describes
    /// the entry under the cursor when it changed, then runs the menu.
    fn show_rack_parts(
        &mut self,
        rom: &[u8],
        page_changed: bool,
        line: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let entries = self.equipment.list.len() + 1;
        let start = self.equipment.list_page * EQUIP_PAGE;
        let mut line = line;
        if page_changed {
            self.run_now(rom, SCRIPT_DRAW_STOCK, windows)?;
            windows.clear_window(LIST_WINDOW);
            let shown = entries.saturating_sub(start).min(EQUIP_PAGE);
            for index in 0..shown {
                if index > 0 {
                    windows.line_break(LIST_WINDOW);
                }
                match (start + index).checked_sub(1) {
                    None => self.run_in(rom, LIST_WINDOW, SCRIPT_TAKE_OFF, windows)?,
                    Some(entry) => self.print_stocked_line(rom, entry, windows)?,
                }
            }
            line = line.min(shown.saturating_sub(1));
            windows.set_cursor(LIST_WINDOW, Some(line));
            windows.set_cursor(LIST_WINDOW, None);
            let more = entries > start + EQUIP_PAGE;
            windows.set_scroll_marks(LIST_WINDOW, (self.equipment.list_page > 0, more));
        }
        let entry = start + line;
        if self.equipment.entry != Some(entry) {
            self.equipment.entry = Some(entry);
            self.equipment.blink_from =
                (entry == 0 && self.rack_part().is_some()).then_some(self.scroll);
            self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
            self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
            let id = match entry.checked_sub(1) {
                None => self.rack_part(),
                Some(stocked) => self.equipment.list.get(stocked).copied(),
            };
            self.describe_equipped(rom, id, windows)?;
        }
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.run_now(rom, SCRIPT_DRAW_STOCK, windows)?;
        self.runner.start(SCRIPT_MEMBER_MENU)?;
        self.state = MenuState::Equip;
        Ok(())
    }

    /// One stocked part in the rack's list: its name padded to eight
    /// cells, ×, and how many are in stock.
    fn print_stocked_line(
        &mut self,
        rom: &[u8],
        entry: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(id) = self.equipment.list.get(entry).copied() else {
            return Ok(());
        };
        self.print_part_name(rom, LIST_WINDOW, id, windows)?;
        self.pad_part_name(rom, LIST_WINDOW, id, windows)?;
        self.run_in(rom, LIST_WINDOW, SCRIPT_TIMES, windows)?;
        let count = saga_party::stock(&self.game_state, id);
        put_value(windows, LIST_WINDOW, i32::from(count), 1, ZERO_PADDED);
        Ok(())
    }

    /// What the rack's list ended with (state `0x3200`): moves describe
    /// the entry, L and R turn the page, A puts the part on or takes the
    /// rack's part off, B goes back to the racks.
    pub(super) fn rack_part_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let entries = self.equipment.list.len() + 1;
        let current = self.equipment.entry.unwrap_or(0) % EQUIP_PAGE;
        match code {
            MOVED_UP | MOVED_DOWN => self.show_rack_parts(rom, false, usize::from(line), windows),
            PAGE_LEFT if self.equipment.list_page > 0 => {
                windows.play_sound(MENU_MOVE_SOUND);
                self.equipment.list_page -= 1;
                self.show_rack_parts(rom, true, current, windows)
            }
            PAGE_RIGHT if entries > (self.equipment.list_page + 1) * EQUIP_PAGE => {
                windows.play_sound(MENU_MOVE_SOUND);
                self.equipment.list_page += 1;
                self.show_rack_parts(rom, true, current, windows)
            }
            PAGE_LEFT | PAGE_RIGHT => self.show_rack_parts(rom, false, current, windows),
            CONFIRMED => {
                let chosen = self.chosen_part();
                if let Some(old) = self.rack_part()
                    && saga_party::stock(&self.game_state, old) >= saga_party::STOCK_LIMIT
                    && chosen != Some(old)
                {
                    return self.ask_discard(rom, old, windows);
                }
                self.change_part(rom, chosen, false, windows)
            }
            _ => {
                windows.play_sound(LEAVE_SOUND);
                self.leave_rack(rom, windows)
            }
        }
    }

    /// The stock of the rack's part is full: 外そうとしている…は
    /// これ以上ストックできません。捨てますか？ and a yes/no choice.
    fn ask_discard(
        &mut self,
        rom: &[u8],
        old: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_TRYING, windows)?;
        self.print_part_name(rom, HELP_WINDOW, old, windows)?;
        self.run_in(rom, HELP_WINDOW, SCRIPT_STOCK_FULL, windows)?;
        self.runner.start(SCRIPT_YES_NO)?;
        self.state = MenuState::Discard;
        Ok(())
    }

    /// The answer to the discard question: yes throws the rack's part
    /// away and fits the chosen one, anything else goes back to the list.
    pub(super) fn discard_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if code == CONFIRMED && line == 0 {
            let chosen = self.chosen_part();
            return self.change_part(rom, chosen, true, windows);
        }
        if code != CONFIRMED {
            windows.play_sound(LEAVE_SOUND);
        }
        let line = self.equipment.entry.unwrap_or(0) % EQUIP_PAGE;
        self.equipment.entry = None;
        self.show_rack_parts(rom, false, line, windows)
    }

    /// Fits `part` on the rack, or takes its part off, updates the stock
    /// and the unit's statistics (sound `0x4E`), and goes back.
    fn change_part(
        &mut self,
        rom: &[u8],
        part: Option<u16>,
        discard: bool,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let character = self.member_character();
        let rack = self.equipment.rack;
        saga_party::equip(rom, &mut self.game_state, character, rack, part, discard);
        windows.play_sound(EQUIPPED_SOUND);
        self.roster = crate::data::GameData::new(rom).roster(&self.game_state);
        self.leave_rack(rom, windows)
    }

    /// Leaves the rack's list (`0x08051CCC`, then state `0x3FFF`): the
    /// member list and window 1 again, then the racks with the cursor
    /// where it was.
    fn leave_rack(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.equipment.zoid = None;
        self.equipment.entry = None;
        self.equipment.blink_from = None;
        windows.set_scroll_marks(LIST_WINDOW, (false, false));
        for id in [LIST_WINDOW, TITLE_WINDOW] {
            self.run_now(rom, SCRIPT_CLOSE + usize::from(id), windows)?;
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_WINDOWS, windows)?;
        let start = self.equipment.page * MEMBERS_PER_PAGE;
        let characters: Vec<u8> = self
            .roster
            .members
            .iter()
            .skip(start)
            .take(MEMBERS_PER_PAGE)
            .map(|member| member.character)
            .collect();
        let count = self.roster.members.len();
        for (index, character) in characters.iter().enumerate() {
            self.print_character_name(rom, *character, MEMBER_WINDOW, windows)?;
            if index + 1 < MEMBERS_PER_PAGE && start + index + 1 < count {
                windows.line_break(MEMBER_WINDOW);
            }
        }
        let more = count > start + MEMBERS_PER_PAGE;
        windows.set_scroll_marks(MEMBER_WINDOW, (self.equipment.page > 0, more));
        windows.set_cursor(MEMBER_WINDOW, Some(self.member % MEMBERS_PER_PAGE));
        windows.set_cursor(MEMBER_WINDOW, None);
        self.draw_equipment_zoid(rom, false, windows)?;
        self.run_now(rom, SCRIPT_DRAW_MEMBERS, windows)?;
        self.run_now(rom, SCRIPT_RACK_WINDOW, windows)?;
        self.open_racks(rom, windows)
    }

    /// The stocked part under the list's cursor, `None` on 装備を外す.
    fn chosen_part(&self) -> Option<u16> {
        let stocked = self.equipment.entry?.checked_sub(1)?;
        self.equipment.list.get(stocked).copied()
    }

    /// The part on the rack being changed.
    fn rack_part(&self) -> Option<u16> {
        self.member_slots()
            .and_then(|slots| slots[self.equipment.rack].part)
            .map(|part| part.id)
    }

    fn member_slots(&self) -> Option<[PartSlot; saga_party::PART_SLOTS]> {
        self.roster
            .members
            .get(self.member)
            .and_then(|member| member.parts)
    }

    fn member_character(&self) -> u8 {
        self.roster
            .members
            .get(self.member)
            .map_or(0, |member| member.character)
    }

    fn member_zoid(&self) -> Option<u16> {
        self.roster
            .members
            .get(self.member)
            .and_then(|member| member.unit)
            .map(|unit| unit.zoid)
    }

    /// The weapons on the rack list's picture: the other racks' parts,
    /// and on the rack being changed the entry under the cursor, its own
    /// part blinking every 16 frames while 装備を外す is chosen.
    pub(super) fn mounted_weapons(&self) -> Vec<Mounted> {
        if self.equipment.zoid.is_none() {
            return Vec::new();
        }
        let Some(slots) = self.member_slots() else {
            return Vec::new();
        };
        (0..RACKS)
            .filter_map(|rack| {
                if rack != self.equipment.rack {
                    let part = slots[rack].part?.id;
                    return Some(Mounted {
                        part,
                        rack,
                        visible: true,
                        baked: rack < RACKS - 1,
                    });
                }
                let entry = self.equipment.entry?;
                let part = match entry.checked_sub(1) {
                    None => self.rack_part()?,
                    Some(stocked) => *self.equipment.list.get(stocked)?,
                };
                let visible = self
                    .equipment
                    .blink_from
                    .is_none_or(|from| (self.scroll - from) / BLINK_FRAMES % 2 == 1);
                Some(Mounted {
                    part,
                    rack,
                    visible,
                    baked: false,
                })
            })
            .collect()
    }

    /// The Zoid whose picture the rack's list shows.
    pub(super) fn equipment_picture(&self) -> Option<u16> {
        self.equipment.zoid
    }

    /// Whether the picture is drawn half over the wallpaper, as the
    /// original blends BG1 for the third rack.
    pub(super) fn equipment_blends(&self) -> bool {
        self.equipment.rack == RACKS - 1
    }

    /// Whether window 4 hides the status sprite, as the original's window
    /// 0 masks sprites under the rack list.
    pub(super) fn racks_mask_sprites(&self) -> bool {
        matches!(self.state, MenuState::Racks)
            || matches!(self.state, MenuState::Notice(Return::Racks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_screen_starts_on_the_first_page() {
        let equipment = Equipment::default();
        assert_eq!(
            (equipment.page, equipment.rack, equipment.member),
            (0, 0, None)
        );
        assert!(equipment.list.is_empty());
    }

    #[test]
    fn the_blank_kind_is_the_one_before_the_fixed_kind() {
        assert_eq!(SCRIPT_BLANK_KIND + 1, SCRIPT_FIXED_KIND);
        assert_eq!(SCRIPT_DRAW_RACKS - 25, usize::from(RACK_WINDOW));
    }
}
