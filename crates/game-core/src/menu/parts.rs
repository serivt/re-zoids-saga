//! The Zoid status screen's parts pages and the stocked weapons list,
//! with the routines that describe a part.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! pages in the status state machine at `0x08052724`, the rack and part
//! printers at `0x0804E04C` and `0x0804DFCC`, the descriptions at
//! `0x0804DE48` (weapons) and `0x0804D940` (support parts), the stock list
//! at `0x0804F7AC` and the number printer at `0x08001848`; see
//! `docs/menu.md`.

use extraction::saga_party::{self, Part, PartSlot};

use super::{
    CONFIRMED, COST_COLUMN, EFFECT_BITS, EMPTY_SOUND, LAST_ARMS_PAGE, LEAVE_SOUND, LEFT_ALIGNED,
    MAX_SHOWN, MENU_MOVE_SOUND, MINUS, MOST_EFFECTS_FIRST_LINE, MOVED_DOWN, MOVED_UP, MenuState,
    NUMBER_DIGITS, PAGE_LEFT, PAGE_RIGHT, PLUS, PauseMenu, RACKS, Return, SCRIPT_ACCURACY,
    SCRIPT_ARMS_HELP, SCRIPT_ARMS_LAST_HELP, SCRIPT_ATTACK, SCRIPT_CLEAR_CHARACTER,
    SCRIPT_CLEAR_HELP, SCRIPT_CLEAR_STOCK, SCRIPT_CLOSE, SCRIPT_COLON, SCRIPT_COST, SCRIPT_DOT,
    SCRIPT_DRAW_CHARACTER, SCRIPT_DRAW_HELP, SCRIPT_DRAW_STOCK, SCRIPT_EFFECTS,
    SCRIPT_EFFECTS_LABEL, SCRIPT_FIXED_KIND, SCRIPT_FIXED_PAGES, SCRIPT_MEMBER_MENU,
    SCRIPT_NO_EFFECT, SCRIPT_NO_PART, SCRIPT_NO_RACK, SCRIPT_NO_WEAPONS, SCRIPT_PART_TEXTS,
    SCRIPT_PERCENT, SCRIPT_PRESENT, SCRIPT_PRESENT_ALL, SCRIPT_RACK_KINDS, SCRIPT_RACK_PAGES,
    SCRIPT_RANGE, SCRIPT_RANGES, SCRIPT_REACHES, SCRIPT_SPACE, SCRIPT_STOCK_LABELS,
    SCRIPT_STOCK_WINDOWS, SCRIPT_SUPPORT, SCRIPT_TIMES, SCRIPT_WAIT_KEY, SIGNED, STOCK_LIST_WINDOW,
    STOCK_MASK, STOCK_NAME_CELLS, STOCK_PAGE, STOCK_WINDOW, SUPPORT_KINDS, WEAPON, ZERO_PADDED,
    ZOID_WINDOW, close_status_windows, label_len,
};
use crate::ScriptHost;
use crate::script::ScriptError;
use crate::text::CELL_WIDTH;
use crate::windows::ScriptWindows;

/// The mark a translated part name cut short ends with.
const NAME_CUT: char = '.';

impl PauseMenu {
    /// Page `page` of the parts of the Zoid the member under the cursor
    /// pilots (`0x08052724`): each rack, then each fixed weapon, in a
    /// window of its own that covers the page before, with its part's
    /// name and values; A turns the page, B goes back.
    pub(super) fn show_arms(
        &mut self,
        rom: &[u8],
        page: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let member = self.roster.members.get(self.member).copied();
        let Some(slots) = member.and_then(|member| member.parts) else {
            return self.reopen_character(rom, windows);
        };
        if page == 0 {
            self.shown_zoid = None;
            self.run_now(rom, SCRIPT_CLOSE + usize::from(ZOID_WINDOW), windows)?;
            self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
            self.run_now(rom, SCRIPT_ARMS_HELP, windows)?;
        }
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
        self.runner.select_window(window);
        let slot = slots[page];
        if page < RACKS {
            self.print_rack(rom, window, slot, windows)?;
        } else {
            self.print_part(rom, window, slot.part, windows)?;
        }
        if page > 0 {
            self.run_now(rom, SCRIPT_PRESENT + usize::from(window), windows)?;
        }
        self.runner.select_window(window);
        self.runner.start(SCRIPT_WAIT_KEY)?;
        self.state = MenuState::Arms(page);
        Ok(())
    }

    /// A rack's kind and part (`0x0804E04C`): ［攻　］, ［　防］, ［攻防］ or
    /// ［固定］, then the part; a fixed slot the Zoid does not fit is
    /// ラックなし.
    pub(super) fn print_rack(
        &mut self,
        rom: &[u8],
        window: u8,
        slot: PartSlot,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if slot.rack == 0 && !slot.fitted {
            self.run_in(rom, window, SCRIPT_COLON, windows)?;
            return self.run_in(rom, window, SCRIPT_NO_RACK, windows);
        }
        let kind = match slot.rack {
            0 => SCRIPT_FIXED_KIND,
            rack => SCRIPT_RACK_KINDS + usize::from(rack),
        };
        self.run_in(rom, window, kind, windows)?;
        self.run_in(rom, window, SCRIPT_COLON, windows)?;
        self.print_part(rom, window, slot.part, windows)
    }

    /// A part's name and description (`0x0804DFCC`), or 装備なし.
    pub(super) fn print_part(
        &mut self,
        rom: &[u8],
        window: u8,
        part: Option<Part>,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(part) = part else {
            return self.run_in(rom, window, SCRIPT_NO_PART, windows);
        };
        self.print_part_name(rom, window, part.id, windows)?;
        windows.line_break(window);
        if part.flags & WEAPON != 0 {
            self.describe_weapon(rom, window, part, windows)
        } else {
            self.describe_support(rom, window, part, true, windows)
        }
    }

    /// A weapon's power, accuracy, up to four special effects, cost and
    /// range (`0x0804DE48`): two effects fit after the accuracy, the rest
    /// follow the range.
    pub(super) fn describe_weapon(
        &mut self,
        rom: &[u8],
        window: u8,
        part: Part,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let power = rounded(part.power);
        let power = if (0..=MAX_SHOWN).contains(&power) {
            power
        } else {
            MAX_SHOWN
        };
        let accuracy = i32::from(part.accuracy);
        let accuracy = if (0..=MAX_SHOWN).contains(&accuracy) {
            accuracy
        } else {
            MAX_SHOWN
        };
        self.run_in(rom, window, SCRIPT_ATTACK, windows)?;
        put_value(windows, window, power, 3, 0);
        self.run_in(rom, window, SCRIPT_ACCURACY, windows)?;
        put_value(windows, window, accuracy, 3, 0);
        self.run_in(rom, window, SCRIPT_PERCENT, windows)?;
        self.run_in(rom, window, SCRIPT_SPACE, windows)?;
        let mut shown = 0;
        let mut effect = 0;
        while effect < EFFECT_BITS.len() && shown < MOST_EFFECTS_FIRST_LINE {
            if self.print_effect(rom, window, part, effect, windows)? {
                if shown % 2 == 0 {
                    self.run_in(rom, window, SCRIPT_SPACE, windows)?;
                }
                shown += 1;
            }
            effect += 1;
        }
        if shown == 0 {
            self.run_in(rom, window, SCRIPT_NO_EFFECT, windows)?;
        }
        windows.line_break(window);
        self.run_in(rom, window, SCRIPT_COST, windows)?;
        put_value(windows, window, i32::from(part.cost), 3, 0);
        self.run_in(rom, window, SCRIPT_RANGE, windows)?;
        self.run_in(
            rom,
            window,
            SCRIPT_RANGES + usize::from(part.range.0),
            windows,
        )?;
        self.run_in(
            rom,
            window,
            SCRIPT_REACHES + usize::from(part.range.1),
            windows,
        )?;
        let odd = shown % 2 == 1;
        for effect in effect..EFFECT_BITS.len() {
            self.run_in(rom, window, SCRIPT_SPACE, windows)?;
            if self.print_effect(rom, window, part, effect, windows)? && !odd {
                self.run_in(rom, window, SCRIPT_SPACE, windows)?;
            }
        }
        Ok(())
    }

    /// Prints special effect `effect` when the part has it (`0x0804DDBC`):
    /// ＤＦ無視, 命中率低下, キャラ無効化 or マヒ.
    pub(super) fn print_effect(
        &mut self,
        rom: &[u8],
        window: u8,
        part: Part,
        effect: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<bool, ScriptError> {
        if part.flags & EFFECT_BITS[effect] == 0 {
            return Ok(false);
        }
        self.run_in(rom, window, SCRIPT_EFFECTS + effect, windows)?;
        Ok(true)
    }

    /// A support part's effect, cost, target, duration and limit
    /// (`0x0804D940`), on two lines when `wide` or on four in a narrow
    /// window.
    #[allow(clippy::too_many_lines)]
    pub(super) fn describe_support(
        &mut self,
        rom: &[u8],
        window: u8,
        part: Part,
        wide: bool,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let support = |index: usize| SCRIPT_SUPPORT + index;
        let (flags, value) = (part.flags, part.power);
        let second = i32::from(part.accuracy);
        if !wide {
            self.run_in(rom, window, SCRIPT_SPACE, windows)?;
        }
        self.run_in(rom, window, support(0), windows)?;
        let single_line =
            if let Some(first) = [0x4000, 0x8000].iter().position(|bit| flags & bit != 0) {
                let labels = if first == 0 { [1, 2, 3] } else { [4, 5, 6] };
                self.run_in(rom, window, support(labels[0]), windows)?;
                put_value(windows, window, value, 3, LEFT_ALIGNED);
                self.run_in(rom, window, SCRIPT_PERCENT, windows)?;
                if !wide {
                    windows.line_break(window);
                }
                if second != 0 {
                    let label = if wide { labels[2] } else { labels[1] };
                    self.run_in(rom, window, support(label), windows)?;
                    put_value(windows, window, second, 3, LEFT_ALIGNED);
                    self.run_in(rom, window, SCRIPT_PERCENT, windows)?;
                }
                false
            } else if flags & 0x1_0000 != 0 || flags & 0x2_0000 != 0 {
                let label = if flags & 0x1_0000 != 0 { 7 } else { 8 };
                self.run_in(rom, window, support(label), windows)?;
                put_value(windows, window, value, 4, LEFT_ALIGNED);
                true
            } else if flags & 0x4_0000 != 0 {
                self.run_in(rom, window, support(9), windows)?;
                put_value(windows, window, value, 3, LEFT_ALIGNED);
                self.run_in(rom, window, SCRIPT_PERCENT, windows)?;
                true
            } else if flags & 0x8_0000 != 0 {
                self.run_in(rom, window, support(10), windows)?;
                put_value(windows, window, value, 3, LEFT_ALIGNED);
                self.run_in(rom, window, support(11), windows)?;
                true
            } else if flags & 0x10_0000 != 0 {
                self.run_in(rom, window, support(12), windows)?;
                true
            } else if flags & 0x20_0000 != 0 {
                match value {
                    4 => self.run_in(rom, window, support(14), windows)?,
                    8 => self.run_in(rom, window, support(15), windows)?,
                    _ => {}
                }
                self.run_in(rom, window, support(13), windows)?;
                true
            } else if flags & 0x40_0000 != 0 {
                let label = if wide { 17 } else { 16 };
                self.run_in(rom, window, support(label), windows)?;
                false
            } else {
                let (hp, ep) = (flags & 0x80_0000 != 0, flags & 0x100_0000 != 0);
                if hp {
                    self.run_in(rom, window, support(18), windows)?;
                    put_value(windows, window, value, 4, LEFT_ALIGNED);
                    if !wide {
                        windows.line_break(window);
                    }
                }
                if ep {
                    if hp && wide {
                        self.run_in(rom, window, SCRIPT_DOT, windows)?;
                    } else if hp {
                        for _ in 0..4 {
                            self.run_in(rom, window, SCRIPT_SPACE, windows)?;
                        }
                    }
                    self.run_in(rom, window, support(19), windows)?;
                    put_value(windows, window, value, 3, LEFT_ALIGNED);
                    if !wide && !hp {
                        windows.line_break(window);
                    }
                }
                false
            };
        if wide {
            windows.pad_to(window, COST_COLUMN);
        } else {
            if single_line {
                windows.line_break(window);
            }
            windows.line_break(window);
            self.run_in(rom, window, SCRIPT_SPACE, windows)?;
        }
        self.run_in(rom, window, support(20), windows)?;
        put_value(windows, window, i32::from(part.cost), 3, 0);
        if wide {
            windows.line_break(window);
        } else {
            for _ in 0..3 {
                self.run_in(rom, window, SCRIPT_SPACE, windows)?;
            }
        }
        self.run_in(rom, window, support(21), windows)?;
        if flags & 2 != 0 {
            self.run_in(rom, window, support(22), windows)?;
        } else if flags & 4 != 0 {
            self.run_in(rom, window, support(23), windows)?;
        }
        if !wide {
            windows.line_break(window);
        }
        self.run_in(rom, window, SCRIPT_SPACE, windows)?;
        self.run_in(rom, window, support(24), windows)?;
        let (label, width) = if flags & 0x8_0000 != 0 {
            (27, 9)
        } else if flags & 0x2000_0000 != 0 {
            (28, 9)
        } else if part.turns == 0 {
            (25, 9)
        } else {
            put_value(windows, window, i32::from(part.turns), 1, 0);
            (26, 8)
        };
        self.run_in(rom, window, support(label), windows)?;
        if wide {
            let used = label_len(rom, self.script_offset(support(label)));
            for _ in used..width {
                self.run_in(rom, window, SCRIPT_SPACE, windows)?;
            }
        } else {
            windows.line_break(window);
            self.run_in(rom, window, SCRIPT_SPACE, windows)?;
        }
        let limit = if flags & 0x8000_0000 != 0 { 30 } else { 29 };
        self.run_in(rom, window, support(limit), windows)
    }

    /// Runs `script` with `window` current, as the game's code prints its
    /// labels into the window it is filling.
    pub(super) fn run_in(
        &mut self,
        rom: &[u8],
        window: u8,
        script: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.runner.select_window(window);
        self.run_now(rom, script, windows)
    }

    /// A status list with nothing in it (`0x0804EB5A`): sound `0x4F`,
    /// the notice, and `0x41` when it is dismissed.
    pub(super) fn empty_list(
        &mut self,
        script: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.play_sound(EMPTY_SOUND);
        self.notice(script, Return::EmptyList)
    }

    /// The stocked weapons and support parts (`0x0804E24C` with mask 15,
    /// then `0x0804F7AC`): six to a page on the right, the one under the
    /// cursor described on the left and in the help line.
    pub(super) fn open_stock(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.stock = saga_party::stocked_parts(rom, &self.game_state, STOCK_MASK);
        if self.stock.is_empty() {
            return self.empty_list(SCRIPT_NO_WEAPONS, windows);
        }
        close_status_windows(windows);
        self.run_now(rom, SCRIPT_STOCK_WINDOWS, windows)?;
        self.stock_page = 0;
        self.stock_shown = None;
        self.show_stock(rom, true, 0, windows)
    }

    /// Prints the stock's page when it changed and the part on `line`
    /// when it changed, then runs the list's menu again.
    pub(super) fn show_stock(
        &mut self,
        rom: &[u8],
        page_changed: bool,
        line: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let start = self.stock_page * STOCK_PAGE;
        let page: Vec<u16> = self
            .stock
            .iter()
            .skip(start)
            .take(STOCK_PAGE)
            .copied()
            .collect();
        let mut line = line;
        if page_changed {
            self.run_now(rom, SCRIPT_CLEAR_STOCK, windows)?;
            for (index, id) in page.iter().enumerate() {
                self.print_part_name(rom, STOCK_LIST_WINDOW, *id, windows)?;
                self.pad_part_name(rom, STOCK_LIST_WINDOW, *id, windows)?;
                self.run_in(rom, STOCK_LIST_WINDOW, SCRIPT_TIMES, windows)?;
                let count = saga_party::stock(&self.game_state, *id);
                put_value(windows, STOCK_LIST_WINDOW, i32::from(count), 1, ZERO_PADDED);
                if index + 1 < STOCK_PAGE && start + index + 1 < self.stock.len() {
                    windows.line_break(STOCK_LIST_WINDOW);
                }
            }
            line = line.min(page.len().saturating_sub(1));
            windows.set_cursor(STOCK_LIST_WINDOW, Some(line));
            windows.set_cursor(STOCK_LIST_WINDOW, None);
            let more = self.stock.len() > start + STOCK_PAGE;
            windows.set_scroll_marks(STOCK_LIST_WINDOW, (self.stock_page > 0, more));
        }
        let selected = start + line;
        if self.stock_shown != Some(selected)
            && let Some(id) = self.stock.get(selected).copied()
        {
            self.stock_shown = Some(selected);
            self.describe_stocked(rom, id, windows)?;
        }
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.run_now(rom, SCRIPT_DRAW_STOCK, windows)?;
        self.runner.start(SCRIPT_MEMBER_MENU)?;
        self.state = MenuState::Stock;
        Ok(())
    }

    /// The part under the stock list's cursor: its text in the help line,
    /// then on the left its name and its record's values, which no pilot
    /// raises here.
    pub(super) fn describe_stocked(
        &mut self,
        rom: &[u8],
        id: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(part) = saga_party::part_record(rom, id) else {
            return Ok(());
        };
        let window = STOCK_WINDOW;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_CLEAR_CHARACTER, windows)?;
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        self.run_now(rom, SCRIPT_PART_TEXTS + usize::from(id), windows)?;
        self.run_now(rom, SCRIPT_DRAW_CHARACTER, windows)?;
        self.print_part_name(rom, window, id, windows)?;
        windows.line_break(window);
        if part.flags & WEAPON != 0 {
            let labels = SCRIPT_STOCK_LABELS;
            self.run_in(rom, window, labels, windows)?;
            put_value(windows, window, whole(part.power), 3, 0);
            self.run_in(rom, window, labels + 1, windows)?;
            put_value(windows, window, i32::from(part.accuracy), 3, 0);
            self.run_in(rom, window, SCRIPT_PERCENT, windows)?;
            windows.line_break(window);
            self.run_in(rom, window, labels + 2, windows)?;
            put_value(windows, window, i32::from(part.cost), 3, 0);
            self.run_in(rom, window, labels + 3, windows)?;
            self.run_in(
                rom,
                window,
                SCRIPT_RANGES + usize::from(part.range.0),
                windows,
            )?;
            self.run_in(
                rom,
                window,
                SCRIPT_REACHES + usize::from(part.range.1),
                windows,
            )?;
            windows.line_break(window);
            self.run_in(rom, window, SCRIPT_EFFECTS_LABEL, windows)?;
            windows.line_break(window);
            self.run_in(rom, window, SCRIPT_SPACE, windows)?;
            let mut shown = 0;
            for effect in 0..EFFECT_BITS.len() {
                if self.print_effect(rom, window, part, effect, windows)? {
                    if shown % 2 == 1 {
                        windows.line_break(window);
                    }
                    self.run_in(rom, window, SCRIPT_SPACE, windows)?;
                    shown += 1;
                }
            }
            if shown == 0 {
                self.run_in(rom, window, SCRIPT_NO_EFFECT, windows)?;
            }
        } else if part.flags & SUPPORT_KINDS != 0 {
            self.describe_support(rom, window, part, false, windows)?;
        }
        Ok(())
    }

    /// What the stock list's menu ended with: a cursor move shows that
    /// part, L and R turn the page (sound `0x40`), A or B go back to the
    /// status list, B with sound `0x3F`.
    pub(super) fn stock_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let line = usize::from(line);
        let current = self.stock_shown.map_or(0, |shown| shown % STOCK_PAGE);
        match code {
            MOVED_UP | MOVED_DOWN => self.show_stock(rom, false, line, windows),
            PAGE_LEFT if self.stock_page > 0 => {
                windows.play_sound(MENU_MOVE_SOUND);
                self.stock_page -= 1;
                self.show_stock(rom, true, current, windows)
            }
            PAGE_RIGHT if self.stock.len() > (self.stock_page + 1) * STOCK_PAGE => {
                windows.play_sound(MENU_MOVE_SOUND);
                self.stock_page += 1;
                self.show_stock(rom, true, current, windows)
            }
            PAGE_LEFT | PAGE_RIGHT => self.show_stock(rom, false, current, windows),
            _ => {
                if code != CONFIRMED {
                    windows.play_sound(LEAVE_SOUND);
                }
                self.rebuild_status(rom, windows)
            }
        }
    }

    /// Prints part `id`'s name in `window`.
    /// Pads a list line after part `id`'s name to cell 8, as the lists
    /// do: a full-width space per cell the ROM's name leaves short. A
    /// translated name, whose letters need not take a cell each, is padded
    /// to the column instead, and one wider than the column is cut short
    /// with a full stop, so the count after it neither strays nor wraps to
    /// the next line.
    pub(super) fn pad_part_name(
        &mut self,
        rom: &[u8],
        window: u8,
        id: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let name = self.parts.string_offset(usize::from(id)).unwrap_or(0);
        let length = label_len(rom, name);
        let used = windows
            .windows()
            .get(usize::from(window))
            .and_then(Option::as_ref)
            .and_then(|shown| shown.widths.last().copied())
            .unwrap_or(0);
        if used == length * CELL_WIDTH {
            for _ in length..STOCK_NAME_CELLS {
                self.run_in(rom, window, SCRIPT_SPACE, windows)?;
            }
        } else {
            windows.clip_line(window, STOCK_NAME_CELLS * CELL_WIDTH, NAME_CUT);
            windows.pad_to(window, STOCK_NAME_CELLS);
        }
        Ok(())
    }

    pub(super) fn print_part_name(
        &mut self,
        rom: &[u8],
        window: u8,
        id: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.parts.select_window(window);
        self.parts.start(usize::from(id))?;
        while !self.parts.update(rom, self.held, windows)? {
            if self.parts.is_waiting_for_key() {
                break;
            }
        }
        Ok(())
    }
}

/// A 16.16 value to the nearest whole, halves up, as `0x0804DE48` rounds
/// a weapon's power.
pub(super) fn rounded(value: i32) -> i32 {
    whole(value) + i32::from(value.to_le_bytes()[1] >> 7)
}

/// A 16.16 value's whole part, toward zero, as the stock list prints a
/// weapon's power.
pub(super) fn whole(value: i32) -> i32 {
    if value < 0 {
        (value + 0xFFFF) >> 16
    } else {
        value >> 16
    }
}

/// Prints `value` the way `0x08001848` does: its last `cells` digits
/// (up to seven, 9999999 at most), after a sign when it is negative or
/// when `mode` has [`SIGNED`]; leading zeros are left out with
/// [`LEFT_ALIGNED`], printed with [`ZERO_PADDED`] and blank otherwise.
pub(super) fn put_value(
    windows: &mut ScriptWindows<'_>,
    window: u8,
    value: i32,
    cells: usize,
    mode: u8,
) {
    for ch in format_value(value, cells, mode).chars() {
        windows.put_char(window, ch);
    }
}

pub(super) fn format_value(value: i32, cells: usize, mode: u8) -> String {
    let cells = cells.min(NUMBER_DIGITS);
    let magnitude = value.unsigned_abs().min(9_999_999);
    let digits = format!("{magnitude:07}");
    let mut out = String::new();
    if value < 0 {
        out.push(MINUS);
    } else if mode & SIGNED != 0 {
        out.push(PLUS);
    }
    let mut seen = false;
    for (position, digit) in digits.bytes().enumerate().skip(NUMBER_DIGITS - cells) {
        let last = position == NUMBER_DIGITS - 1;
        if digit == b'0' && !seen && !last {
            if mode & LEFT_ALIGNED != 0 {
                continue;
            }
            if mode & ZERO_PADDED == 0 {
                out.push('\u{3000}');
                continue;
            }
        } else {
            seen = true;
        }
        out.push(char::from_u32(0xFF10 + u32::from(digit - b'0')).unwrap_or('０'));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub(super) fn values_print_their_last_digits_after_the_sign() {
        assert_eq!(format_value(25, 3, 0), "\u{3000}２５");
        assert_eq!(format_value(0, 3, 0), "\u{3000}\u{3000}０");
        assert_eq!(format_value(20, 3, LEFT_ALIGNED), "２０");
        assert_eq!(format_value(7, 3, SIGNED | LEFT_ALIGNED), "＋７");
        assert_eq!(format_value(-5, 3, 0), "－\u{3000}\u{3000}５");
        assert_eq!(format_value(5, 3, ZERO_PADDED), "００５");
        assert_eq!(format_value(1234, 3, 0), "２３４");
        assert_eq!(format_value(12, 1, 0), "２");
    }

    #[test]
    pub(super) fn a_weapons_power_rounds_halves_up() {
        assert_eq!(rounded(0x19_8000), 26);
        assert_eq!(rounded(0x19_7FFF), 25);
        assert_eq!(rounded(46 << 16), 46);
    }
}
