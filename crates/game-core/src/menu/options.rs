//! コンフィグ in the enhanced mode, a port feature: in place of the
//! original's list of message speeds, a list of every setting the game
//! lets the player change while it plays, the original's message speed
//! and the enhanced mode's own conveniences (see [`crate::play_mode`]),
//! each changed with left and right and described on the help line.
//!
//! Source of knowledge: this project's own design; the windows, the menu
//! and its sounds are the original's (see `docs/menu.md`).

use platform::Input;

use super::{HELP_WINDOW, LEAVE_SOUND, MENU_MOVE_SOUND, MENU_WINDOW, MenuState, PauseMenu, Return};
use crate::ScriptHost;
use crate::guide::GuideError;
use crate::menu::MenuStep;
use crate::play_mode::{Enhancement, Enhancements};
use crate::port_text::{
    OPTIONS_ANIMATIONS, OPTIONS_ANIMATIONS_HELP, OPTIONS_AUTO_TEXT, OPTIONS_AUTO_TEXT_HELP,
    OPTIONS_AUTOSAVE, OPTIONS_AUTOSAVE_HELP, OPTIONS_DAMAGE_NUMBERS, OPTIONS_DAMAGE_NUMBERS_HELP,
    OPTIONS_KEYS, OPTIONS_OFF, OPTIONS_ON, OPTIONS_SPEED, OPTIONS_SPEED_HELP, full_width,
    port_text,
};
use crate::script::{MOVED_DOWN, MOVED_LEFT, MOVED_RIGHT, MOVED_UP, ScriptRunner};
use crate::text::CELL_WIDTH;
use crate::windows::ScriptWindows;

/// The list's window, where the original's message speed shows.
const LIST_WINDOW: u8 = 4;
const LIST_KIND: u8 = 0x21;
const STYLE: u8 = 4;
/// The column after the main list in the original; a translation may
/// widen the list.
const MAIN_LIST_END: u8 = 9;
const SCREEN_COLUMNS: u8 = 30;
/// Rows a line of the list takes, and the list's frame.
const ROWS_PER_LINE: u8 = 2;
const FRAME_ROWS: u8 = 2;
/// The fewest rows the list takes: the status panel's, which it covers.
const PANEL_ROWS: u8 = 8;
/// The cell where the values start.
const VALUE_CELL: usize = 13;
/// The cells of a line's window that hold no text: the frame and the
/// cursor's mark on each side.
const MARGIN_CELLS: usize = 4;
/// A menu that reports every key the list takes: the cursor's moves, left
/// and right.
const REPORTS_SIDES: u8 = 6;
const CONFIRMED: u16 = 1;
/// The message speeds, fastest first.
const SPEEDS: std::ops::RangeInclusive<u16> = 1..=5;

/// A line of the list: the message speed, or an enhancement on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Setting {
    MessageSpeed,
    Enhancement(Enhancement),
}

const SETTINGS: [Setting; 5] = [
    Setting::MessageSpeed,
    Setting::Enhancement(Enhancement::BattleAnimations),
    Setting::Enhancement(Enhancement::DamageNumbers),
    Setting::Enhancement(Enhancement::AutoText),
    Setting::Enhancement(Enhancement::Autosave),
];

impl Setting {
    /// The line's label and its help.
    fn texts(self) -> (&'static str, &'static str) {
        match self {
            Self::MessageSpeed => (OPTIONS_SPEED, OPTIONS_SPEED_HELP),
            Self::Enhancement(Enhancement::BattleAnimations) => {
                (OPTIONS_ANIMATIONS, OPTIONS_ANIMATIONS_HELP)
            }
            Self::Enhancement(Enhancement::DamageNumbers) => {
                (OPTIONS_DAMAGE_NUMBERS, OPTIONS_DAMAGE_NUMBERS_HELP)
            }
            Self::Enhancement(Enhancement::AutoText) => (OPTIONS_AUTO_TEXT, OPTIONS_AUTO_TEXT_HELP),
            Self::Enhancement(Enhancement::Autosave) => (OPTIONS_AUTOSAVE, OPTIONS_AUTOSAVE_HELP),
        }
    }
}

/// The enhanced mode's settings the menu offers, and the list while it is
/// on screen.
#[derive(Default)]
pub(super) struct Options {
    /// The enhancements, in the enhanced mode; `None` keeps the original's
    /// コンフィグ.
    enhancements: Option<Enhancements>,
    line: usize,
    runner: Option<ScriptRunner>,
}

impl PauseMenu {
    /// Offers the enhanced mode's `enhancements` in コンフィグ, with the
    /// message speed; `None`, the classic mode, keeps the original's list.
    pub fn set_enhancements(&mut self, enhancements: Option<Enhancements>) {
        self.options.enhancements = enhancements;
    }

    /// The enhancements as the player left them.
    #[must_use]
    pub fn enhancements(&self) -> Option<Enhancements> {
        self.options.enhancements
    }

    /// Whether コンフィグ opens the port's list of settings.
    pub(super) fn offers_options(&self) -> bool {
        self.options.enhancements.is_some()
    }

    /// コンフィグ in the enhanced mode: the list beside the main one, the
    /// cursor on its first line.
    pub(super) fn open_options(&mut self, windows: &mut ScriptWindows<'_>) {
        let left = windows
            .windows()
            .get(usize::from(MENU_WINDOW))
            .and_then(Option::as_ref)
            .map_or(usize::from(MAIN_LIST_END), |main| main.x + main.width);
        let left = u8::try_from(left).unwrap_or(MAIN_LIST_END);
        let rows = (u8::try_from(SETTINGS.len()).unwrap_or(u8::MAX) * ROWS_PER_LINE + FRAME_ROWS)
            .max(PANEL_ROWS);
        let rect = (left, 0, SCREEN_COLUMNS.saturating_sub(left), rows);
        windows.open_window(LIST_WINDOW, LIST_KIND, rect, STYLE);
        self.options.line = 0;
        self.draw_options(windows);
        windows.present(Some(HELP_WINDOW));
        let mut runner = ScriptRunner::new(Vec::new());
        runner.hold(self.held);
        runner.run_reporting_menu(LIST_WINDOW, true, REPORTS_SIDES, windows);
        self.options.runner = Some(runner);
        self.state = MenuState::Options;
    }

    /// A frame of the list: up and down move and describe the line, left
    /// and right (and A) change its value, B goes back to the main list
    /// with the settings kept.
    pub(super) fn options_frame(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<MenuStep, GuideError> {
        let Some(runner) = self.options.runner.as_mut() else {
            return Ok(MenuStep::Open);
        };
        if !runner.update(rom, input, windows)? {
            return Ok(MenuStep::Open);
        }
        let [code, line, ..] = *runner.vars();
        self.options.line = usize::from(line).min(SETTINGS.len() - 1);
        let setting = SETTINGS[self.options.line];
        let changed = match code {
            MOVED_UP | MOVED_DOWN => {
                self.describe_option(windows);
                false
            }
            MOVED_LEFT => self.change(setting, false),
            MOVED_RIGHT | CONFIRMED => self.change(setting, true),
            _ => {
                windows.play_sound(LEAVE_SOUND);
                windows.close_window(Some(LIST_WINDOW));
                windows.clear_window(HELP_WINDOW);
                self.options.runner = None;
                self.runner.hold(input);
                self.return_to(rom, Return::Main, windows)?;
                return Ok(MenuStep::Open);
            }
        };
        if changed {
            windows.play_sound(MENU_MOVE_SOUND);
            self.draw_options(windows);
        }
        if let Some(runner) = self.options.runner.as_mut() {
            runner.run_reporting_menu(LIST_WINDOW, true, REPORTS_SIDES, windows);
        }
        Ok(MenuStep::Open)
    }

    /// Changes `setting` a step up (`more`) or down; whether it changed.
    fn change(&mut self, setting: Setting, more: bool) -> bool {
        match setting {
            Setting::MessageSpeed => {
                let speed = self.party.message_speed;
                let next = if more {
                    speed + 1
                } else {
                    speed.saturating_sub(1)
                };
                let next = next.clamp(*SPEEDS.start(), *SPEEDS.end());
                self.party.message_speed = next;
                next != speed
            }
            Setting::Enhancement(enhancement) => {
                let Some(enhancements) = self.options.enhancements.as_mut() else {
                    return false;
                };
                enhancements.toggle(enhancement);
                true
            }
        }
    }

    /// Prints the list, a line per setting with its value, the cursor on
    /// the current line, and the help line.
    fn draw_options(&self, windows: &mut ScriptWindows<'_>) {
        let extensions = windows.extensions().clone();
        let value_cell = value_cell(windows);
        windows.clear_window(LIST_WINDOW);
        for (index, setting) in SETTINGS.iter().enumerate() {
            if index > 0 {
                windows.line_break(LIST_WINDOW);
            }
            let (label, _) = setting.texts();
            let value = match *setting {
                Setting::MessageSpeed => full_width(u32::from(self.party.message_speed)),
                Setting::Enhancement(enhancement) => {
                    let on = self
                        .options
                        .enhancements
                        .unwrap_or_default()
                        .is_on(enhancement);
                    port_text(&extensions, if on { OPTIONS_ON } else { OPTIONS_OFF })
                }
            };
            put_text(windows, &port_text(&extensions, label));
            windows.pad_to(LIST_WINDOW, value_cell);
            put_text(windows, &value);
        }
        windows.set_cursor(LIST_WINDOW, Some(self.options.line));
        windows.present(Some(LIST_WINDOW));
        self.describe_option(windows);
    }

    /// The help line: what the setting under the cursor does, then the
    /// keys.
    fn describe_option(&self, windows: &mut ScriptWindows<'_>) {
        let extensions = windows.extensions().clone();
        let (_, help) = SETTINGS[self.options.line].texts();
        windows.clear_window(HELP_WINDOW);
        for ch in port_text(&extensions, help).chars() {
            windows.put_char(HELP_WINDOW, ch);
        }
        windows.line_break(HELP_WINDOW);
        for ch in port_text(&extensions, OPTIONS_KEYS).chars() {
            windows.put_char(HELP_WINDOW, ch);
        }
        windows.present(Some(HELP_WINDOW));
    }
}

/// The cell where the values start: [`VALUE_CELL`], or further left when
/// the widest value, on or off or a speed, would not fit before the
/// cursor's right mark (a translation's list starts further right).
fn value_cell(windows: &ScriptWindows<'_>) -> usize {
    let extensions = windows.extensions().clone();
    let metrics = windows.metrics();
    let widest = [
        port_text(&extensions, OPTIONS_ON),
        port_text(&extensions, OPTIONS_OFF),
        full_width(u32::from(*SPEEDS.end())),
    ]
    .iter()
    .map(|value| metrics.width(value).div_ceil(CELL_WIDTH))
    .max()
    .unwrap_or(0);
    let cells = windows
        .windows()
        .get(usize::from(LIST_WINDOW))
        .and_then(Option::as_ref)
        .map_or(VALUE_CELL + widest, |list| {
            list.width.saturating_sub(MARGIN_CELLS)
        });
    VALUE_CELL.min(cells.saturating_sub(widest))
}

/// Prints `text` on the list's current line.
fn put_text(windows: &mut ScriptWindows<'_>, text: &str) {
    for ch in text.chars() {
        windows.put_char(LIST_WINDOW, ch);
    }
}
