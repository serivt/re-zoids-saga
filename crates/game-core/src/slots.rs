//! Save slots, which the original lacks: the port keeps several saves,
//! each a whole save memory of the original's format (see
//! `docs/formats/save.md`), and lets the player choose one when saving
//! and continuing. With the enhanced mode's autosave, continuing also
//! lists the game saved on the last change of map, first and marked
//! apart from the numbered slots; saving never offers it.
//!
//! Source of knowledge: this project's own design, drawn with the game's
//! windows: a light menu lists the slots (number, name and level, or
//! whether the slot is empty or broken) and the help window says what to
//! do, then the area and money of the slot under the cursor. The menu
//! moves, confirms and cancels like the game's own lists, with their
//! sounds.

use std::time::SystemTime;

use platform::Input;

use crate::extension::Rect;
use crate::port_text::{
    SLOT_AUTOSAVE, SLOT_AUTOSAVE_HELP, SLOT_BROKEN, SLOT_DETAILS, SLOT_EMPTY, SLOT_LEVEL,
    SLOT_LOAD_HELP, SLOT_SAVE_HELP, fill, full_width, port_text,
};
use crate::save::Found;
use crate::script::{MOVED_DOWN, MOVED_UP, ScriptError, ScriptHost, ScriptRunner};
use crate::windows::ScriptWindows;

/// Slots the launcher offers unless told otherwise.
pub const DEFAULT_SLOTS: usize = 4;
/// The windows' kinds and style, as the pause menu's: a light menu and a
/// light text window.
const LIST_KIND: u8 = 0x21;
const HELP_KIND: u8 = 0x20;
const STYLE: u8 = 4;
/// The cell a slot's level starts at, after its number and an 8-cell
/// name.
const LEVEL_CELL: usize = 11;
const SCREEN_COLUMNS: u8 = 30;
/// A menu that reports moves and no other key.
const MOVES_ONLY: u8 = 1;
const CONFIRMED: u16 = 1;
const FULL_WIDTH_SPACE: char = '\u{3000}';
/// Sound of a slot that cannot be chosen, the menus' refusal.
const REFUSED_SOUND: u8 = 0x4F;
/// Sound of leaving a list with B.
const LEAVE_SOUND: u8 = 0x3F;

/// What a slot holds, as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slot {
    /// Nothing was saved there.
    Empty,
    /// Both copies are broken.
    Broken,
    /// A game.
    Game(SlotSummary),
}

/// The fields of a saved game the list and the help line show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotSummary {
    /// The player's name.
    pub name: String,
    /// The party level.
    pub level: u8,
    /// The area of the map saved on, 1 to 10.
    pub area: u8,
    /// The party's money.
    pub money: u32,
}

impl Slot {
    /// What continuing found in the slot, as the list shows it.
    #[must_use]
    pub fn from_found(found: &Found) -> Self {
        match found {
            Found::Missing => Self::Empty,
            Found::Corrupt => Self::Broken,
            Found::Saved(game) | Found::Restored(game) => {
                game.progress().map_or(Self::Broken, |progress| {
                    Self::Game(SlotSummary {
                        name: game.player_name.clone(),
                        level: progress.level,
                        area: progress.area,
                        money: progress.money,
                    })
                })
            }
        }
    }

    /// Whether the slot holds a game.
    #[must_use]
    pub fn has_game(&self) -> bool {
        matches!(self, Self::Game(_))
    }
}

/// The slot with the game saved last, given when each slot was stored:
/// the latest time among the slots with a game, the first of equal ones,
/// or the first game when no time is known.
#[must_use]
pub fn latest(times: &[Option<SystemTime>], contents: &[Slot]) -> Option<usize> {
    let games = || {
        contents
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.has_game())
            .map(|(index, _)| index)
    };
    games()
        .filter_map(|index| Some((times.get(index).copied().flatten()?, index)))
        .max_by(|(left, left_index), (right, right_index)| {
            left.cmp(right).then(right_index.cmp(left_index))
        })
        .map(|(_, index)| index)
        .or_else(|| games().next())
}

/// Why the slots are listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Saving from the pause menu: any slot may be chosen.
    Save,
    /// Continuing from the title: an empty slot is refused.
    Load,
}

/// Where the list and the help line go: the list window's id and
/// rectangle, the help window's id, and the rectangle to open it at, or
/// `None` when it is already open (the pause menu's help line).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotLayout {
    /// The list window.
    pub list: (u8, Rect),
    /// The help window, and where to open it if it is not open.
    pub help: (u8, Option<Rect>),
}

/// The pause menu's layout with its main list ending before column
/// `left`: the list from there to the screen's right edge, where the
/// status list goes, and the menu's own help line.
#[must_use]
pub fn menu_layout(left: u8) -> SlotLayout {
    SlotLayout {
        list: (4, (left, 0, SCREEN_COLUMNS.saturating_sub(left), 10)),
        help: (0, None),
    }
}

/// The title's layout: the list over the picture and a help window where
/// the pause menu has its help line.
pub const TITLE_LAYOUT: SlotLayout = SlotLayout {
    list: (1, (5, 3, 20, 10)),
    help: (2, Some((0, 14, 30, 6))),
};

/// What the player did with the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// Chose the list's line `n`, from 0.
    Slot(usize),
    /// Left it with B.
    Canceled,
}

/// The list of slots on screen, and its menu.
pub struct SlotPicker {
    slots: Vec<Slot>,
    purpose: Purpose,
    layout: SlotLayout,
    line: usize,
    /// Whether the first line is the autosave.
    autosave: bool,
    runner: ScriptRunner,
}

impl SlotPicker {
    /// Lists `slots` for `purpose` in `layout`, the cursor on `line`.
    #[must_use]
    pub fn new(slots: Vec<Slot>, purpose: Purpose, layout: SlotLayout, line: usize) -> Self {
        let line = line.min(slots.len().saturating_sub(1));
        Self {
            slots,
            purpose,
            layout,
            line,
            autosave: false,
            runner: ScriptRunner::new(Vec::new()),
        }
    }

    /// Takes the first slot listed for the autosave: it is marked instead
    /// of numbered, the numbers start again from 1 on the next line, and
    /// the help line names it.
    #[must_use]
    pub fn with_autosave(mut self) -> Self {
        self.autosave = !self.slots.is_empty();
        self
    }

    /// The slot under the cursor.
    #[must_use]
    pub fn line(&self) -> usize {
        self.line
    }

    /// What slot `n` holds.
    #[must_use]
    pub fn slot(&self, n: usize) -> Option<&Slot> {
        self.slots.get(n)
    }

    /// Opens the windows and starts the menu; `down` are the buttons held
    /// on this frame, which are not read as presses.
    pub fn open(&mut self, down: Input, windows: &mut ScriptWindows<'_>) {
        let (help, help_rect) = self.layout.help;
        if let Some(rect) = help_rect {
            windows.open_window(help, HELP_KIND, rect, STYLE);
        }
        let (list, rect) = self.layout.list;
        windows.open_window(list, LIST_KIND, fit(rect, self.slots.len()), STYLE);
        for (index, slot) in self.slots.iter().enumerate() {
            if index > 0 {
                windows.line_break(list);
            }
            let extensions = windows.extensions().clone();
            match index.checked_sub(usize::from(self.autosave)) {
                Some(numbered) => {
                    let number = u32::try_from(numbered + 1).unwrap_or(u32::MAX);
                    put_text(windows, list, &full_width(number));
                }
                None => put_text(windows, list, &port_text(&extensions, SLOT_AUTOSAVE)),
            }
            windows.put_char(list, FULL_WIDTH_SPACE);
            match slot {
                Slot::Empty => put_text(windows, list, &port_text(&extensions, SLOT_EMPTY)),
                Slot::Broken => put_text(windows, list, &port_text(&extensions, SLOT_BROKEN)),
                Slot::Game(summary) => {
                    put_text(windows, list, &summary.name);
                    windows.pad_to(list, LEVEL_CELL);
                    let level = port_text(&extensions, SLOT_LEVEL);
                    put_text(
                        windows,
                        list,
                        &fill(&level, &[("level", u32::from(summary.level))]),
                    );
                }
            }
        }
        windows.set_cursor(list, Some(self.line));
        windows.set_cursor(list, None);
        windows.present(Some(help));
        windows.present(Some(list));
        self.describe(windows);
        self.runner.hold(down);
        self.runner
            .run_reporting_menu(list, true, MOVES_ONLY, windows);
    }

    /// Advances the menu a frame; returns what the player did once they
    /// chose or left. A move describes the new slot; choosing an empty
    /// slot to continue is refused with the menus' sound.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when the menu fails.
    pub fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<Option<Pick>, ScriptError> {
        if !self.runner.update(rom, input, windows)? {
            return Ok(None);
        }
        let [code, line, ..] = *self.runner.vars();
        self.line = usize::from(line).min(self.slots.len().saturating_sub(1));
        let (list, _) = self.layout.list;
        let pick = match code {
            MOVED_UP | MOVED_DOWN => {
                self.describe(windows);
                None
            }
            CONFIRMED
                if self.purpose == Purpose::Load
                    && self.slots.get(self.line) == Some(&Slot::Empty) =>
            {
                windows.play_sound(REFUSED_SOUND);
                None
            }
            CONFIRMED => Some(Pick::Slot(self.line)),
            _ => {
                windows.play_sound(LEAVE_SOUND);
                Some(Pick::Canceled)
            }
        };
        if pick.is_none() {
            self.runner
                .run_reporting_menu(list, true, MOVES_ONLY, windows);
        } else {
            windows.set_cursor(list, None);
        }
        Ok(pick)
    }

    /// Closes the list, and the help window if the list opened it; the
    /// pause menu's help line is only cleared.
    pub fn close(&self, windows: &mut ScriptWindows<'_>) {
        let (list, _) = self.layout.list;
        windows.close_window(Some(list));
        match self.layout.help {
            (help, Some(_)) => windows.close_window(Some(help)),
            (help, None) => windows.clear_window(help),
        }
    }

    /// The help line: what to do, then the slot under the cursor.
    fn describe(&self, windows: &mut ScriptWindows<'_>) {
        let (help, _) = self.layout.help;
        let extensions = windows.extensions().clone();
        let key = match self.purpose {
            _ if self.autosave && self.line == 0 => SLOT_AUTOSAVE_HELP,
            Purpose::Save => SLOT_SAVE_HELP,
            Purpose::Load => SLOT_LOAD_HELP,
        };
        windows.clear_window(help);
        put_text(windows, help, &port_text(&extensions, key));
        if let Some(Slot::Game(summary)) = self.slots.get(self.line) {
            windows.line_break(help);
            let details = port_text(&extensions, SLOT_DETAILS);
            let details = fill(
                &details,
                &[("area", u32::from(summary.area)), ("money", summary.money)],
            );
            put_text(windows, help, &details);
        }
    }
}

/// `rect` grown to hold `lines` text lines, two rows each within its
/// border, moving up as far as the screen lets it: the title's list is
/// sized for the four slots and takes a fifth line for the autosave.
fn fit(rect: Rect, lines: usize) -> Rect {
    let (x, y, width, height) = rect;
    let needed = u8::try_from(lines * 2 + 2).unwrap_or(u8::MAX);
    let extra = needed.saturating_sub(height);
    (x, y.saturating_sub(extra), width, height.max(needed))
}

/// Prints `text` into window `id`, a new line at each line break.
fn put_text(windows: &mut ScriptWindows<'_>, id: u8, text: &str) {
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            windows.line_break(id);
        }
        for ch in line.chars() {
            windows.put_char(id, ch);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::save::SavedGame;
    use formats::Progress;
    use formats::progress::STATE_LEN;

    fn game(name: &str, level: u8) -> Slot {
        let mut state = vec![0; STATE_LEN];
        let mut progress = Progress::read(&state).expect("block");
        progress.level = level;
        progress.area = 1;
        progress.money = 305;
        progress.write(&mut state).expect("block");
        Slot::from_found(&Found::Saved(SavedGame {
            state,
            player_name: name.to_owned(),
            stats: crate::stats::Stats::default(),
        }))
    }

    fn press(button: platform::Button) -> Input {
        Input::default().with(button)
    }

    fn step(
        picker: &mut SlotPicker,
        windows: &mut ScriptWindows<'_>,
        input: Input,
    ) -> Option<Pick> {
        let pick = picker.update(&[], input, windows).expect("menu");
        picker
            .update(&[], Input::default(), windows)
            .expect("menu")
            .or(pick)
    }

    fn lines(windows: &ScriptWindows<'_>, id: u8) -> Vec<String> {
        windows.windows()[usize::from(id)]
            .as_ref()
            .expect("open")
            .lines
            .clone()
    }

    #[test]
    fn a_found_game_is_summed_up_and_the_rest_are_empty_or_broken() {
        assert_eq!(Slot::from_found(&Found::Missing), Slot::Empty);
        assert_eq!(Slot::from_found(&Found::Corrupt), Slot::Broken);
        let Slot::Game(summary) = game("アトレー", 12) else {
            panic!("a game");
        };
        assert_eq!(
            summary,
            SlotSummary {
                name: "アトレー".to_owned(),
                level: 12,
                area: 1,
                money: 305,
            }
        );
    }

    #[test]
    fn the_latest_game_wins_and_broken_or_empty_slots_never_do() {
        let at = |seconds| Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(seconds));
        let contents = [game("ア", 1), Slot::Broken, game("イ", 2), Slot::Empty];
        assert_eq!(latest(&[at(5), at(9), at(7), at(8)], &contents), Some(2));
        assert_eq!(latest(&[at(7), at(9), at(7), None], &contents), Some(0));
        assert_eq!(latest(&[None, None, at(1), None], &contents), Some(2));
        assert_eq!(latest(&[None; 4], &contents), Some(0));
        assert_eq!(latest(&[at(1), at(2)], &[Slot::Empty, Slot::Broken]), None);
    }

    #[test]
    fn lists_the_slots_and_describes_the_one_under_the_cursor() {
        let mut windows = ScriptWindows::new(&[], "");
        let slots = vec![game("アトレー", 12), Slot::Empty, Slot::Broken];
        let mut picker = SlotPicker::new(slots, Purpose::Save, TITLE_LAYOUT, 0);
        picker.open(Input::default(), &mut windows);
        assert_eq!(
            lines(&windows, 1),
            [
                "１\u{3000}アトレー\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}Ｌｖ１２",
                "２\u{3000}データなし",
                "３\u{3000}こわれたデータ",
            ]
        );
        assert_eq!(
            lines(&windows, 2),
            [
                "どのスロットにセーブしますか？",
                "エリア１\u{3000}所持金３０５Ｇ"
            ]
        );
        assert_eq!(
            step(&mut picker, &mut windows, press(platform::Button::Down)),
            None
        );
        assert_eq!(picker.line(), 1);
        assert_eq!(lines(&windows, 2), ["どのスロットにセーブしますか？"]);
        assert_eq!(
            step(&mut picker, &mut windows, press(platform::Button::A)),
            Some(Pick::Slot(1))
        );
        picker.close(&mut windows);
        assert!(!windows.any_open());
    }

    #[test]
    fn the_autosave_comes_first_marked_and_the_list_grows_to_hold_it() {
        let mut windows = ScriptWindows::new(&[], "");
        let slots = vec![
            game("アトレー", 7),
            game("アトレー", 5),
            Slot::Empty,
            Slot::Empty,
            Slot::Empty,
        ];
        let mut picker = SlotPicker::new(slots, Purpose::Load, TITLE_LAYOUT, 0).with_autosave();
        picker.open(Input::default(), &mut windows);
        let list = windows.windows()[1].as_ref().expect("open");
        assert_eq!((list.y, list.height), (1, 12));
        assert_eq!(
            list.lines[0],
            "Ａ\u{3000}アトレー\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}Ｌｖ７"
        );
        assert!(list.lines[1].starts_with("１\u{3000}アトレー"));
        assert_eq!(list.lines[4], "４\u{3000}データなし");
        assert_eq!(
            lines(&windows, 2),
            [
                "マップ移動時のオートセーブです",
                "エリア１\u{3000}所持金３０５Ｇ"
            ]
        );
        assert_eq!(
            step(&mut picker, &mut windows, press(platform::Button::Down)),
            None
        );
        assert_eq!(lines(&windows, 2)[0], "どのデータからつづけますか？");
        assert_eq!(
            step(&mut picker, &mut windows, press(platform::Button::A)),
            Some(Pick::Slot(1))
        );
    }

    #[test]
    fn continuing_refuses_an_empty_slot_and_b_leaves() {
        let mut windows = ScriptWindows::new(&[], "");
        let slots = vec![Slot::Empty, game("アトレー", 3)];
        let mut picker = SlotPicker::new(slots, Purpose::Load, TITLE_LAYOUT, 0);
        picker.open(press(platform::Button::A), &mut windows);
        assert_eq!(
            step(&mut picker, &mut windows, press(platform::Button::A)),
            None
        );
        windows.take_sounds();
        assert_eq!(step(&mut picker, &mut windows, Input::default()), None);
        assert_eq!(
            step(&mut picker, &mut windows, press(platform::Button::A)),
            None
        );
        assert!(windows.take_sounds().contains(&REFUSED_SOUND));
        assert_eq!(
            step(&mut picker, &mut windows, press(platform::Button::B)),
            Some(Pick::Canceled)
        );
    }
}
