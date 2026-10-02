//! The statistics, a port feature: the enhanced mode's main list has a
//! line for them after セーブ, which opens three pages, the battles
//! fought, the collection and the other records, in a window over the
//! menu, left and right turning the page and B going back to the list.
//!
//! Source of knowledge: this project's own design; the counts come from
//! the player's statistics (see [`crate::stats`]) and the collection from
//! the game state (the Zi data at `+0x33E2`, the units, the character
//! guide's bits at `+0x34A4` and the deck commands at `+0x347B`, see
//! `docs/formats/save.md`); the windows and sounds are the original's.

use platform::{Button, Input};

use super::{HELP_WINDOW, LEAVE_SOUND, MENU_MOVE_SOUND, MenuState, PauseMenu, Return};
use crate::ScriptHost;
use crate::extension::SharedExtensions;
use crate::port_text::{
    STATS_BEST_HIT, STATS_CHARACTERS, STATS_COMMANDS, STATS_DAMAGE, STATS_DESTROYED, STATS_ENEMIES,
    STATS_GOLD, STATS_HOURS, STATS_KEYS, STATS_LONGEST, STATS_LOST, STATS_MONEY, STATS_OF,
    STATS_PAGE_BATTLES, STATS_PAGE_COLLECTION, STATS_PAGE_RECORDS, STATS_RETREATED, STATS_ROUNDS,
    STATS_STORY, STATS_TIME, STATS_TIMES, STATS_UNITS, STATS_WON, STATS_ZI_DATA, STATS_ZOIDS, fill,
    port_text,
};
use crate::script::ScriptError;
use crate::stats::Stats;
use crate::text::CELL_WIDTH;
use crate::windows::ScriptWindows;
use extraction::saga_encounter::STORY_BATTLE_COUNT;
use extraction::saga_guide::CHARACTER_COUNT;
use extraction::saga_party::{self, ZI_DATA_ZOIDS};
use formats::progress::{character_known, command_learned};

/// The window the pages show in: the one the status list and the settings
/// use, across the screen above the help line.
pub(super) const WINDOW: u8 = 4;
pub(super) const KIND: u8 = 0x21;
pub(super) const STYLE: u8 = 4;
pub(super) const RECT: (u8, u8, u8, u8) = (0, 0, 30, 14);
/// The cells of a line that hold no text: the frame and the cursor's mark
/// on each side, and one between the value and the frame.
pub(super) const MARGIN_CELLS: usize = 5;
/// The deck commands there are (the records at ROM `0x683AC0`).
const DECK_COMMANDS: usize = 33;
/// The Zoid index a unit record keeps at `+6`.
const UNIT_ZOID: usize = 6;
const PAGES: usize = 3;
const MINUTES_PER_HOUR: u32 = 60;

/// The pages and where the menu is in them.
#[derive(Default)]
pub(super) struct Statistics {
    /// The player's statistics when the menu opened.
    stats: Stats,
    page: usize,
    /// The buttons of the frame before, for the presses.
    previous: Input,
}

impl PauseMenu {
    /// Shows `stats`, the player's statistics, in the enhanced mode's main
    /// list.
    pub fn set_stats(&mut self, stats: Stats) {
        self.statistics.stats = stats;
    }

    /// Opens the statistics on their first page, over the menu; `held` are
    /// the buttons down now, which are not read as a press.
    pub(super) fn open_statistics(&mut self, held: Input, windows: &mut ScriptWindows<'_>) {
        windows.close_window(Some(WINDOW));
        windows.open_window(WINDOW, KIND, RECT, STYLE);
        self.statistics.page = 0;
        self.statistics.previous = held;
        self.draw_statistics(windows);
        self.state = MenuState::Statistics;
    }

    /// A frame of the statistics: left and right turn the page, B goes
    /// back to the main list with the cursor on their line.
    pub(super) fn statistics_frame(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let previous = std::mem::replace(&mut self.statistics.previous, input);
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
            PAGES - 1
        } else {
            return Ok(());
        };
        self.statistics.page = (self.statistics.page + turn) % PAGES;
        windows.play_sound(MENU_MOVE_SOUND);
        self.draw_statistics(windows);
        Ok(())
    }

    /// Prints the page, a line per statistic with its value at the right,
    /// and its name and the keys on the help line.
    fn draw_statistics(&self, windows: &mut ScriptWindows<'_>) {
        let extensions = windows.extensions().clone();
        let lines = self.page_lines(&extensions);
        let text_cells = usize::from(RECT.2).saturating_sub(MARGIN_CELLS);
        windows.clear_window(WINDOW);
        for (index, (label, value)) in lines.iter().enumerate() {
            if index > 0 {
                windows.line_break(WINDOW);
            }
            put_text(windows, WINDOW, &port_text(&extensions, label));
            let cells = windows.metrics().width(value).div_ceil(CELL_WIDTH);
            windows.pad_to(WINDOW, text_cells.saturating_sub(cells));
            put_text(windows, WINDOW, value);
        }
        windows.set_cursor(WINDOW, None);
        windows.present(Some(WINDOW));
        let title = [
            STATS_PAGE_BATTLES,
            STATS_PAGE_COLLECTION,
            STATS_PAGE_RECORDS,
        ][self.statistics.page.min(PAGES - 1)];
        let page = u32::try_from(self.statistics.page + 1).unwrap_or(1);
        let pages = u32::try_from(PAGES).unwrap_or(1);
        windows.clear_window(HELP_WINDOW);
        put_text(
            windows,
            HELP_WINDOW,
            &fill(
                &port_text(&extensions, title),
                &[("page", page), ("pages", pages)],
            ),
        );
        windows.line_break(HELP_WINDOW);
        put_text(windows, HELP_WINDOW, &port_text(&extensions, STATS_KEYS));
        windows.present(Some(HELP_WINDOW));
    }

    /// The page's lines: each statistic's label and its value.
    fn page_lines(&self, extensions: &SharedExtensions) -> Vec<(&'static str, String)> {
        let counts = &self.statistics.stats;
        let value = |key: &str, values: &[(&str, u32)]| fill(&port_text(extensions, key), values);
        let times = |count: u32| value(STATS_TIMES, &[("count", count)]);
        let of = |count: usize, total: usize| {
            value(
                STATS_OF,
                &[
                    ("count", u32::try_from(count).unwrap_or(u32::MAX)),
                    ("total", u32::try_from(total).unwrap_or(u32::MAX)),
                ],
            )
        };
        match self.statistics.page {
            0 => vec![
                (STATS_WON, times(counts.battles_won)),
                (STATS_LOST, times(counts.battles_lost)),
                (STATS_RETREATED, times(counts.battles_retreated)),
                (
                    STATS_ENEMIES,
                    value(STATS_UNITS, &[("count", counts.enemies_destroyed)]),
                ),
                (
                    STATS_DESTROYED,
                    value(STATS_UNITS, &[("count", counts.party_destroyed)]),
                ),
                (
                    STATS_MONEY,
                    value(STATS_GOLD, &[("money", counts.money_earned)]),
                ),
            ],
            1 => {
                let state = &self.game_state;
                let zoids = usize::from(ZI_DATA_ZOIDS);
                vec![
                    (STATS_ZI_DATA, of(saga_party::zi_data_count(state), zoids)),
                    (STATS_ZOIDS, of(zoid_kinds(state), zoids)),
                    (
                        STATS_CHARACTERS,
                        of(
                            (0..CHARACTER_COUNT)
                                .filter(|&index| character_known(state, index))
                                .count(),
                            CHARACTER_COUNT,
                        ),
                    ),
                    (
                        STATS_COMMANDS,
                        of(
                            (0..DECK_COMMANDS)
                                .filter(|&command| command_learned(state, command))
                                .count(),
                            DECK_COMMANDS,
                        ),
                    ),
                ]
            }
            _ => {
                let (hours, minutes, _) = counts.play_time();
                vec![
                    (
                        STATS_TIME,
                        value(
                            STATS_HOURS,
                            &[("hours", hours), ("minutes", minutes % MINUTES_PER_HOUR)],
                        ),
                    ),
                    (
                        STATS_BEST_HIT,
                        value(STATS_DAMAGE, &[("count", counts.best_hit)]),
                    ),
                    (
                        STATS_LONGEST,
                        value(STATS_ROUNDS, &[("count", counts.longest_battle)]),
                    ),
                    (
                        STATS_STORY,
                        of(
                            usize::try_from(counts.story_battles_won()).unwrap_or(0),
                            STORY_BATTLE_COUNT,
                        ),
                    ),
                ]
            }
        }
    }
}

/// The kinds of Zoid the party has units of, among those with Zi data.
fn zoid_kinds(state: &[u8]) -> usize {
    let mut kinds: Vec<u8> = saga_party::zoid_units(state)
        .into_iter()
        .filter_map(|unit| saga_party::unit_record(state, unit))
        .map(|record| record[UNIT_ZOID])
        .filter(|&zoid| zoid != 0 && zoid < ZI_DATA_ZOIDS)
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    kinds.len()
}

/// Prints `text` in window `id`.
pub(super) fn put_text(windows: &mut ScriptWindows<'_>, id: u8, text: &str) {
    for ch in text.chars() {
        windows.put_char(id, ch);
    }
}
