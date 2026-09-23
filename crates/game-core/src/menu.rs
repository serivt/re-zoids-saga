//! The pause menu START opens on the field.
//!
//! The original assembles it from small scripts of one table (see
//! `extraction::saga::PAUSE_MENU_SCRIPTS`): script 46 opens the help line
//! and the six-item list, 64 and 44 the party panel and the money box,
//! whose values the game's code prints right-aligned, and 47 prints the
//! help text and runs the menu. Choices open the status submenu (48, 49),
//! the weapons screen (128, 129, 133), the message-speed setting (151–159)
//! or the save question (160, 61, 161, 162), or print a notice. The
//! screens this port does not have end in the table's "not done yet"
//! notice (63). Behind the windows a logo map drifts one pixel per frame
//! diagonally over a static texture.

use extraction::saga::{self, BootError, PAUSE_MENU_SCRIPTS, PauseWallpaper};
use gba_runtime::ppu::{FullPalette, draw_background_256};
use platform::{Frame, Input, Rgb};

use crate::script::{ScriptError, ScriptRunner};
use crate::windows::ScriptWindows;
use crate::{ScriptHost, TextPainter, WindowPainter};

const SCRIPT_WAIT_KEY: usize = 37;
const SCRIPT_MONEY_WINDOW: usize = 44;
const SCRIPT_MONEY_UNIT: usize = 45;
const SCRIPT_OPEN_MENU: usize = 46;
const SCRIPT_MENU: usize = 47;
const SCRIPT_STATUS_WINDOW: usize = 48;
const SCRIPT_STATUS_MENU: usize = 49;
const SCRIPT_NO_ITEMS: usize = 56;
const SCRIPT_NO_WEAPONS: usize = 57;
const SCRIPT_NO_ZI_DATA: usize = 58;
const SCRIPT_NO_ZI_ITEMS: usize = 60;
const SCRIPT_YES_NO: usize = 61;
const SCRIPT_NOT_DONE: usize = 63;
const SCRIPT_PANEL_WINDOW: usize = 64;
const SCRIPT_LEVEL_LABEL: usize = 65;
const SCRIPT_EXP_LABEL: usize = 66;
const SCRIPT_NEXT_LABEL: usize = 67;
const SCRIPT_BOOK_WINDOW: usize = 115;
const SCRIPT_BOOK_MENU: usize = 116;
const SCRIPT_WEAPONS_WINDOWS: usize = 128;
const SCRIPT_NO_ZOID: usize = 129;
const SCRIPT_WEAPONS_HELP: usize = 133;
const SCRIPT_NOT_BOARDED: usize = 134;
const SCRIPT_SPEED_WINDOW: usize = 151;
const SCRIPT_SPEED_LIST: usize = 152;
const SCRIPT_SPEED_MENU: usize = 153;
const SCRIPT_SPEED_VALUE: usize = 153;
const SCRIPT_SAVE_QUESTION: usize = 160;
const SCRIPT_SAVED: usize = 161;
const SCRIPT_SAVE_CANCELED: usize = 162;
const HELP_WINDOW: u8 = 0;
const MENU_WINDOW: u8 = 3;
const MONEY_WINDOW: u8 = 1;
const PANEL_WINDOW: u8 = 2;
const STATUS_WINDOW: u8 = 4;
const BOOK_WINDOW: u8 = 5;
const SPEED_WINDOW: u8 = 5;
const WEAPONS_ZOID_WINDOW: u8 = 1;
const WEAPONS_LIST_WINDOW: u8 = 3;
const PANEL_LABEL_CELLS: usize = 10;
const PANEL_CELLS: usize = 17;
const MONEY_CELLS: usize = 9;
const WALLPAPER_PALETTE_START: usize = 64;
const WALLPAPER_BACKDROP: u16 = 0x7240;
const MAP_PIXELS: i32 = 256;
const ITEM_STATUS: u16 = 0;
const ITEM_ITEMS: u16 = 1;
const ITEM_WEAPONS: u16 = 2;
const ITEM_CONFIG: u16 = 4;
const ITEM_SAVE: u16 = 5;
const STATUS_WEAPONS: u16 = 2;
const STATUS_ZI_DATA: u16 = 3;
const STATUS_ZI_ITEMS: u16 = 4;
const STATUS_BOOK: u16 = 5;
const SPEED_CHOICES: u16 = 5;
const MENU_CANCELABLE: bool = true;

/// What the party has; the values a new game starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Party {
    /// Level shown for the party.
    pub level: u32,
    /// Experience points.
    pub experience: u32,
    /// Points to the next level.
    pub to_next_level: u32,
    /// Money in G.
    pub money: u32,
    /// Battle message speed, 1 (fast) to 5 (slow).
    pub message_speed: u16,
}

impl Default for Party {
    fn default() -> Self {
        Self {
            level: 1,
            experience: 0,
            to_next_level: 14,
            money: 0,
            message_speed: 3,
        }
    }
}

/// Where a notice returns to when dismissed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Return {
    Main,
    Status,
    Weapons,
}

/// Which list or notice the runner is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuState {
    Main,
    Status,
    Book,
    Weapons,
    Speed,
    Save,
    Notice(Return),
    Closed,
}

/// The pause menu with its wallpaper.
pub struct PauseMenu {
    wallpaper: PauseWallpaper,
    palette: FullPalette,
    runner: ScriptRunner,
    state: MenuState,
    scroll: i32,
    party: Party,
    held: Input,
    main_line: usize,
}

impl PauseMenu {
    /// Reads the wallpaper and scripts from `rom`.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when a block cannot be read.
    pub fn new(rom: &[u8], party: Party) -> Result<Self, BootError> {
        let wallpaper = saga::pause_wallpaper(rom)?;
        let mut palette = FullPalette::from_bgr555(&[WALLPAPER_BACKDROP]);
        palette.write(WALLPAPER_PALETTE_START, &wallpaper.palette);
        let scripts = PAUSE_MENU_SCRIPTS.offsets(rom).unwrap_or_default();
        Ok(Self {
            wallpaper,
            palette,
            runner: ScriptRunner::new(scripts),
            state: MenuState::Closed,
            scroll: 0,
            party,
            held: Input::default(),
            main_line: 0,
        })
    }

    /// Opens the menu on `windows`: the windows, the party panel and the
    /// money box, then the help line and the choice.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a script cannot run.
    pub fn open(&mut self, rom: &[u8], windows: &mut ScriptWindows<'_>) -> Result<(), ScriptError> {
        self.held = Input::default();
        self.main_line = 0;
        self.build(rom, windows)?;
        self.runner.start(SCRIPT_MENU)?;
        self.state = MenuState::Main;
        self.scroll = 0;
        Ok(())
    }

    fn build(&mut self, rom: &[u8], windows: &mut ScriptWindows<'_>) -> Result<(), ScriptError> {
        windows.close_window(None);
        self.run_now(rom, SCRIPT_OPEN_MENU, windows)?;
        self.run_now(rom, SCRIPT_PANEL_WINDOW, windows)?;
        self.print_panel(rom, windows)?;
        self.run_now(rom, SCRIPT_MONEY_WINDOW, windows)?;
        self.print_money(rom, windows)?;
        windows.set_cursor(MENU_WINDOW, Some(self.main_line));
        windows.set_cursor(MENU_WINDOW, None);
        Ok(())
    }

    /// The party as the menu leaves it.
    #[must_use]
    pub fn party(&self) -> Party {
        self.party
    }

    /// Whether the menu has closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state == MenuState::Closed
    }

    fn run_now(
        &mut self,
        rom: &[u8],
        script: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.runner.start(script)?;
        while !self.runner.update(rom, self.held, windows)? {}
        Ok(())
    }

    fn print_panel(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let name = windows.player_name();
        for ch in name.chars() {
            windows.put_char(PANEL_WINDOW, ch);
        }
        self.run_now(rom, SCRIPT_LEVEL_LABEL, windows)?;
        let used = name.chars().count() + label_len(rom, self.script_offset(SCRIPT_LEVEL_LABEL));
        print_number(
            windows,
            PANEL_WINDOW,
            self.party.level,
            PANEL_CELLS.saturating_sub(used),
        );
        windows.line_break(PANEL_WINDOW);
        self.run_now(rom, SCRIPT_EXP_LABEL, windows)?;
        print_number(
            windows,
            PANEL_WINDOW,
            self.party.experience,
            PANEL_CELLS - PANEL_LABEL_CELLS,
        );
        windows.line_break(PANEL_WINDOW);
        self.run_now(rom, SCRIPT_NEXT_LABEL, windows)?;
        print_number(
            windows,
            PANEL_WINDOW,
            self.party.to_next_level,
            PANEL_CELLS - PANEL_LABEL_CELLS,
        );
        Ok(())
    }

    fn print_money(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        print_number(windows, MONEY_WINDOW, self.party.money, MONEY_CELLS - 1);
        self.run_now(rom, SCRIPT_MONEY_UNIT, windows)
    }

    fn script_offset(&self, script: usize) -> usize {
        self.runner.string_offset(script).unwrap_or(0)
    }

    /// Advances one frame; returns `true` when the menu closed.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a script cannot run.
    pub fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<bool, ScriptError> {
        self.scroll += 1;
        self.held = input;
        if self.state == MenuState::Closed {
            return Ok(true);
        }
        if !self.runner.update(rom, input, windows)? {
            return Ok(false);
        }
        let [confirmed, choice, ..] = *self.runner.vars();
        let confirmed = confirmed != 0;
        match self.state {
            MenuState::Main => self.main_choice(rom, confirmed, choice, windows)?,
            MenuState::Status => self.status_choice(rom, confirmed, choice, windows)?,
            MenuState::Book => {
                windows.close_window(Some(BOOK_WINDOW));
                if confirmed {
                    self.placeholder(rom, Return::Status, windows)?;
                } else {
                    self.return_to(rom, Return::Status, windows)?;
                }
            }
            MenuState::Weapons => {
                if confirmed {
                    self.run_now(rom, SCRIPT_NOT_BOARDED, windows)?;
                    self.notice(SCRIPT_WAIT_KEY, Return::Weapons)?;
                } else {
                    self.build(rom, windows)?;
                    self.return_to(rom, Return::Main, windows)?;
                }
            }
            MenuState::Speed => {
                if confirmed && choice < SPEED_CHOICES {
                    self.party.message_speed = choice + 1;
                }
                if confirmed && choice >= SPEED_CHOICES {
                    self.placeholder(rom, Return::Main, windows)?;
                } else {
                    self.return_to(rom, Return::Main, windows)?;
                }
            }
            MenuState::Save => {
                let script = if confirmed && choice == 0 {
                    SCRIPT_SAVED
                } else {
                    SCRIPT_SAVE_CANCELED
                };
                self.notice(script, Return::Main)?;
            }
            MenuState::Notice(back) => self.return_to(rom, back, windows)?,
            MenuState::Closed => {}
        }
        Ok(self.state == MenuState::Closed)
    }

    fn main_choice(
        &mut self,
        rom: &[u8],
        confirmed: bool,
        choice: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if !confirmed {
            windows.close_window(None);
            self.state = MenuState::Closed;
            return Ok(());
        }
        self.main_line = usize::from(choice);
        match choice {
            ITEM_STATUS => {
                self.run_now(rom, SCRIPT_STATUS_WINDOW, windows)?;
                self.return_to(rom, Return::Status, windows)
            }
            ITEM_ITEMS => self.notice(SCRIPT_NO_ITEMS, Return::Main),
            ITEM_WEAPONS => {
                self.run_now(rom, SCRIPT_WEAPONS_WINDOWS, windows)?;
                self.runner.select_window(WEAPONS_ZOID_WINDOW);
                self.run_now(rom, SCRIPT_NO_ZOID, windows)?;
                for ch in windows.player_name().chars() {
                    windows.put_char(WEAPONS_LIST_WINDOW, ch);
                }
                windows.present(None);
                self.return_to(rom, Return::Weapons, windows)
            }
            ITEM_CONFIG => {
                windows.clear_window(HELP_WINDOW);
                self.run_now(rom, SCRIPT_SPEED_WINDOW, windows)?;
                let value = SCRIPT_SPEED_VALUE + usize::from(self.party.message_speed);
                self.run_now(rom, value, windows)?;
                self.run_now(rom, SCRIPT_SPEED_LIST, windows)?;
                windows.present(None);
                let line = usize::from(self.party.message_speed.saturating_sub(1));
                windows.set_cursor(SPEED_WINDOW, Some(line));
                self.runner.start(SCRIPT_SPEED_MENU)?;
                self.state = MenuState::Speed;
                Ok(())
            }
            ITEM_SAVE => {
                windows.clear_window(HELP_WINDOW);
                self.run_now(rom, SCRIPT_SAVE_QUESTION, windows)?;
                self.runner.start(SCRIPT_YES_NO)?;
                self.state = MenuState::Save;
                Ok(())
            }
            _ => self.placeholder(rom, Return::Main, windows),
        }
    }

    fn status_choice(
        &mut self,
        rom: &[u8],
        confirmed: bool,
        choice: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if !confirmed {
            windows.close_window(Some(STATUS_WINDOW));
            return self.return_to(rom, Return::Main, windows);
        }
        match choice {
            STATUS_WEAPONS => self.notice(SCRIPT_NO_WEAPONS, Return::Status),
            STATUS_ZI_DATA => self.notice(SCRIPT_NO_ZI_DATA, Return::Status),
            STATUS_ZI_ITEMS => self.notice(SCRIPT_NO_ZI_ITEMS, Return::Status),
            STATUS_BOOK => {
                self.run_now(rom, SCRIPT_BOOK_WINDOW, windows)?;
                windows.clear_window(HELP_WINDOW);
                self.runner.start(SCRIPT_BOOK_MENU)?;
                self.state = MenuState::Book;
                Ok(())
            }
            _ => self.placeholder(rom, Return::Status, windows),
        }
    }

    fn notice(&mut self, script: usize, back: Return) -> Result<(), ScriptError> {
        self.runner.start(script)?;
        self.state = MenuState::Notice(back);
        Ok(())
    }

    fn placeholder(
        &mut self,
        rom: &[u8],
        back: Return,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.clear_window(HELP_WINDOW);
        self.run_now(rom, SCRIPT_NOT_DONE, windows)?;
        self.notice(SCRIPT_WAIT_KEY, back)
    }

    fn return_to(
        &mut self,
        rom: &[u8],
        back: Return,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.clear_window(HELP_WINDOW);
        match back {
            Return::Main => {
                self.runner.start(SCRIPT_MENU)?;
                self.state = MenuState::Main;
            }
            Return::Status => {
                self.runner.start(SCRIPT_STATUS_MENU)?;
                self.state = MenuState::Status;
            }
            Return::Weapons => {
                self.run_now(rom, SCRIPT_WEAPONS_HELP, windows)?;
                self.runner
                    .run_menu(WEAPONS_LIST_WINDOW, MENU_CANCELABLE, windows);
                self.state = MenuState::Weapons;
            }
        }
        Ok(())
    }

    /// Draws the wallpaper and the windows.
    pub fn draw(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
    ) {
        frame.fill(Rgb::default());
        draw_background_256(
            frame,
            |x, y| self.wallpaper.texture.wrapping(x, y),
            |index| self.wallpaper.tiles.tile(index),
            &self.palette,
            (0, 0),
            false,
        );
        let scroll_x =
            usize::try_from((MAP_PIXELS - self.scroll % MAP_PIXELS) % MAP_PIXELS).unwrap_or(0);
        let scroll_y = usize::try_from(self.scroll % MAP_PIXELS).unwrap_or(0);
        draw_background_256(
            frame,
            |x, y| self.wallpaper.logo.wrapping(x, y),
            |index| self.wallpaper.tiles.tile(index),
            &self.palette,
            (scroll_x, scroll_y),
            true,
        );
        windows.draw(frame, skin, painter);
    }
}

/// Length in characters of the message a label script prints.
fn label_len(rom: &[u8], offset: usize) -> usize {
    use formats::script_ops::{Instruction, MessageStep, decode_instruction, decode_message_step};
    let Ok((Instruction::Message, mut at)) = decode_instruction(rom, offset) else {
        return 0;
    };
    let mut count = 0;
    while let Ok((step, next)) = decode_message_step(rom, at) {
        at = next;
        match step {
            MessageStep::Character(_) => count += 1,
            MessageStep::End => break,
            _ => {}
        }
    }
    count
}

/// Prints `value` right-aligned in `cells` cells with full-width digits.
fn print_number(windows: &mut ScriptWindows<'_>, window: u8, value: u32, cells: usize) {
    let digits: String = value
        .to_string()
        .chars()
        .map(|digit| char::from_u32(0xFF10 + u32::from(digit as u8 - b'0')).unwrap_or(digit))
        .collect();
    let padding = cells.saturating_sub(digits.chars().count());
    for _ in 0..padding {
        windows.put_char(window, '\u{3000}');
    }
    for ch in digits.chars() {
        windows.put_char(window, ch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_right_aligned_with_full_width_digits() {
        let mut windows = ScriptWindows::new(&[], "X");
        windows.open_window(2, 0x20, (11, 0, 19, 8), 4);
        print_number(&mut windows, 2, 14, 4);
        assert_eq!(
            windows.windows()[2].as_ref().map(|w| w.lines.clone()),
            Some(vec!["\u{3000}\u{3000}１４".to_owned()])
        );
        let (bytes, _) = ([0x20u8, 0x41, 0x83, 0x0D, 0x1D, 0x22], 0);
        assert_eq!(label_len(&bytes, 0), 1);
        assert_eq!(label_len(&[0x22], 0), 0);
    }

    #[test]
    fn a_new_party_starts_at_level_one_without_money() {
        let party = Party::default();
        assert_eq!(
            (
                party.level,
                party.experience,
                party.to_next_level,
                party.money
            ),
            (1, 0, 14, 0)
        );
    }
}
