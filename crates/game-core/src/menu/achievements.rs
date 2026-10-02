//! The achievements, a port feature: the enhanced mode's main list has a
//! line for them after the statistics', which opens their list over the
//! menu in pages of three, each achievement's mark (★ unlocked, ☆ not)
//! and name over what it asks for, left and right turning the page and B
//! going back to the list.
//!
//! Source of knowledge: this project's own design (see
//! [`crate::achievements`]); the windows and sounds are the original's.

use platform::{Button, Input};

use super::statistics::{KIND, RECT, STYLE, WINDOW, put_text};
use super::{HELP_WINDOW, LEAVE_SOUND, MENU_MOVE_SOUND, MenuState, PauseMenu, Return};
use crate::ScriptHost;
use crate::achievements::{ACHIEVEMENTS, LOCKED_MARK, UNLOCKED_MARK, Unlocked};
use crate::port_text::{ACHIEVEMENT_PAGE, STATS_KEYS, fill, port_text};
use crate::script::ScriptError;
use crate::windows::ScriptWindows;

/// The achievements a page lists, two lines each.
const PER_PAGE: usize = 3;
/// The space between an achievement's mark and its name.
const MARK_GAP: char = ' ';
/// What an achievement asks for starts a full-width space in.
const HELP_INDENT: char = '\u{3000}';

/// The pages and where the menu is in them.
#[derive(Default)]
pub(super) struct AchievementPages {
    /// The achievements unlocked when the menu opened.
    unlocked: Unlocked,
    page: usize,
    /// The buttons of the frame before, for the presses.
    previous: Input,
}

/// The pages the achievements take.
fn pages() -> usize {
    ACHIEVEMENTS.len().div_ceil(PER_PAGE)
}

impl PauseMenu {
    /// Lists `unlocked` as the achievements unlocked, in the enhanced
    /// mode's main list.
    pub fn set_achievements(&mut self, unlocked: Unlocked) {
        self.achievements.unlocked = unlocked;
    }

    /// Opens the achievements on their first page, over the menu; `held`
    /// are the buttons down now, which are not read as a press.
    pub(super) fn open_achievements(&mut self, held: Input, windows: &mut ScriptWindows<'_>) {
        windows.close_window(Some(WINDOW));
        windows.open_window(WINDOW, KIND, RECT, STYLE);
        self.achievements.page = 0;
        self.achievements.previous = held;
        self.draw_achievements(windows);
        self.state = MenuState::Achievements;
    }

    /// A frame of the achievements: left and right turn the page, B goes
    /// back to the main list with the cursor on their line.
    pub(super) fn achievements_frame(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let previous = std::mem::replace(&mut self.achievements.previous, input);
        let pressed = |button| input.is_held(button) && !previous.is_held(button);
        if pressed(Button::B) {
            windows.play_sound(LEAVE_SOUND);
            windows.close_window(Some(WINDOW));
            self.runner.hold(input);
            return self.return_to(rom, Return::Main, windows);
        }
        let turn = if pressed(Button::Right) {
            1
        } else if pressed(Button::Left) {
            pages() - 1
        } else {
            return Ok(());
        };
        self.achievements.page = (self.achievements.page + turn) % pages();
        windows.play_sound(MENU_MOVE_SOUND);
        self.draw_achievements(windows);
        Ok(())
    }

    /// Prints the page, each achievement's mark and name over what it asks
    /// for, and on the help line the page's number, how many are unlocked
    /// and the keys.
    fn draw_achievements(&self, windows: &mut ScriptWindows<'_>) {
        let extensions = windows.extensions().clone();
        let unlocked = self.achievements.unlocked;
        let first = self.achievements.page * PER_PAGE;
        windows.clear_window(WINDOW);
        for (line, (index, achievement)) in ACHIEVEMENTS
            .iter()
            .enumerate()
            .skip(first)
            .take(PER_PAGE)
            .enumerate()
        {
            if line > 0 {
                windows.line_break(WINDOW);
            }
            let mark = if unlocked.has(index) {
                UNLOCKED_MARK
            } else {
                LOCKED_MARK
            };
            windows.put_char(WINDOW, mark);
            windows.put_char(WINDOW, MARK_GAP);
            put_text(
                windows,
                WINDOW,
                &port_text(&extensions, achievement.name.key),
            );
            windows.line_break(WINDOW);
            windows.put_char(WINDOW, HELP_INDENT);
            put_text(
                windows,
                WINDOW,
                &port_text(&extensions, achievement.help.key),
            );
        }
        windows.set_cursor(WINDOW, None);
        windows.present(Some(WINDOW));
        let count = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
        let title = fill(
            &port_text(&extensions, ACHIEVEMENT_PAGE),
            &[
                ("page", count(self.achievements.page + 1)),
                ("pages", count(pages())),
                ("count", count(unlocked.count())),
                ("total", count(ACHIEVEMENTS.len())),
            ],
        );
        windows.clear_window(HELP_WINDOW);
        put_text(windows, HELP_WINDOW, &title);
        windows.line_break(HELP_WINDOW);
        put_text(windows, HELP_WINDOW, &port_text(&extensions, STATS_KEYS));
        windows.present(Some(HELP_WINDOW));
    }
}

#[cfg(test)]
mod tests {
    use super::super::statistics::MARGIN_CELLS;
    use super::*;

    #[test]
    fn the_pages_hold_every_achievement_and_the_lines_fit() {
        assert_eq!(pages(), 11);
        assert!(pages() * PER_PAGE >= ACHIEVEMENTS.len());
        let cell = crate::text::CELL_WIDTH;
        let line = (usize::from(RECT.2) - MARGIN_CELLS) * cell;
        let gap = crate::text::TextMetrics::standard().width(&MARK_GAP.to_string());
        for achievement in &ACHIEVEMENTS {
            assert!(achievement.name.pixels + cell + gap <= line);
            assert!(achievement.help.pixels + cell <= line);
        }
    }
}
