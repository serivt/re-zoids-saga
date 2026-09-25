//! コマンド作成, the battle menu's deck screen: the six slots of the deck
//! command deck, the deck commands the party learned, and their
//! descriptions.
//!
//! The original runs it as a task of its own (`0x0803B004`, slot 7) with
//! its own fades. Its windows are scripts of the `battle-menu` table: the
//! message window (window 1) for the description, the deck (window 2 at
//! (0, 1) 15×14, its menu script 15) and the learned commands (window 3 at
//! (15, 1) 15×14, its menu script 18), both move-reporting menus of mode 6.
//! Each pass of the task's loop runs one state's code, whose scripts cost
//! their frames, then waits a frame; the port runs a state's scripts at
//! once and counts the frames they would have cost.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the task
//! (`0x0803B004`), the lists (`0x0803B4A4`, `0x0803B570`, `0x0803B654`),
//! their pages (`0x0803C05C`, `0x0803B6E4`, `0x0803B780`, `0x0803C118`),
//! the descriptions (`0x0803B694`, `0x0803BFE8`, `0x080328A0`, `0x080355AC`)
//! and the deck's write (`0x0803B7B8`, `0x0802BC6C`); checked against the
//! battle menu of a world-map battle in a reference emulator. See
//! `docs/combat.md`.

use gba_runtime::ppu::{FADE_STEPS, darken};
use platform::{Button, Frame, Input, Rgb};

use crate::data::GameData;
use crate::script::{ScriptError, ScriptRunner};
use crate::translation::{BATTLE_LABEL_TABLE, BATTLE_MENU_TABLE, BATTLE_TEXT_TABLE, ITEM_TABLE};
use crate::windows::ScriptWindows;
use crate::{ScriptHost, TextPainter, WindowPainter};

/// The game state's deck: six command numbers, `0xFF` for an empty slot.
pub const DECK: usize = 0x349C;
/// The deck's slots.
pub const DECK_SLOTS: usize = 6;
/// The game state's learned commands: a byte each, not 0 once learned.
const LEARNED: usize = 0x347B;
/// The deck commands.
const COMMANDS: u8 = 0x21;
const EMPTY: u8 = 0xFF;
/// Commands a page of the list shows.
const PAGE_LINES: usize = 6;
const DECK_WINDOW: u8 = 2;
const LIST_WINDOW: u8 = 3;
const MESSAGE_WINDOW: u8 = 1;
// Scripts of the battle-menu table.
const SCRIPT_MESSAGE_WINDOW: usize = 2;
const SCRIPT_PRESENT_MESSAGE: usize = 5;
const SCRIPT_DRAW_MESSAGE: usize = 6;
const SCRIPT_CLEAR_MESSAGE: usize = 7;
const SCRIPT_PRESENT_DECK: usize = 9;
const SCRIPT_PRESENT_LIST: usize = 10;
const SCRIPT_CLEAR_DECK: usize = 11;
const SCRIPT_CLEAR_LIST: usize = 12;
const SCRIPT_DRAW_DECK: usize = 13;
const SCRIPT_DRAW_LIST: usize = 14;
const SCRIPT_DECK_MENU: usize = 15;
const SCRIPT_DECK_WINDOW: usize = 16;
const SCRIPT_LIST_WINDOW: usize = 17;
const SCRIPT_LIST_MENU: usize = 18;
const SCRIPT_RESET: usize = 1;
// battle-text: a line break, ★ for a command in the deck, the digits
// from １, ：, the empty slot's －－－－－－－ and a full-width space.
const TEXT_LINE_BREAK: usize = 0;
const TEXT_IN_DECK: usize = 0x11;
const TEXT_DIGITS: usize = 94;
const TEXT_COLON: usize = 0x24;
const TEXT_EMPTY: usize = 0x23;
const TEXT_SPACE: usize = 106;
/// The commands' names (`item` 77 + n) and descriptions (`item` 110 + n).
const COMMAND_NAMES: usize = 77;
const COMMAND_HELP: usize = 110;
/// The description of a command the link battles forbid (`battle-label`
/// 21, 通信対戦では　このコマンドは使えません).
const LABEL_FORBIDDEN: usize = 21;
/// What the menus leave in var 0 (opcode `0x3C` in mode 6).
const CONFIRMED: u16 = 1;
const PAGE_LEFT: u16 = 2;
const PAGE_RIGHT: u16 = 4;
const MOVED_UP: u16 = 0x20;
const MOVED_DOWN: u16 = 0x40;
const STARTED: u16 = 0x80;
const FADE_TOP: u32 = 31;
/// Frames after the screen opened before the battle's screen starts
/// darkening, at which the screen is built in the dark, at which it
/// starts brightening and at which the list's menu runs (states 0 to
/// `0x44C`).
const DARKEN_DELAY: u32 = 4;
const BUILD_AT: u32 = 38;
const BRIGHTEN_AT: u32 = 62;
const MENU_AT: u32 = 95;
/// Frames after B or START before the screen darkens, and at which it
/// hands back (states `0x2328` to `0x23F0`).
const LEAVE_DELAY: u32 = 3;
const LEAVE_FRAMES: u32 = 37;

/// What the screen is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Darkening, built, brightening; frames since it opened.
    Opening(u32),
    /// The learned commands' menu (`0x44C`).
    List,
    /// The deck's menu, to put command `n` in a slot (`0x834`).
    Deck(u8),
    /// Darkening after B or START; frames since.
    Leaving(u32),
    Closed,
}

/// The deck screen.
pub struct DeckScreen {
    menu: ScriptRunner,
    text: ScriptRunner,
    labels: ScriptRunner,
    items: ScriptRunner,
    phase: Phase,
    busy: u32,
    link: bool,
    /// The learned commands, six a page.
    pages: Vec<Vec<u8>>,
    page: usize,
    line: usize,
    deck_line: usize,
    /// The command whose description the message window shows.
    shown: Option<u8>,
    held: Input,
}

/// Which table a script belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Table {
    Menu,
    Text,
    Labels,
    Items,
}

impl DeckScreen {
    /// The screen over `state`, opened the frame after コマンド作成 was
    /// chosen; `link` is a link battle's (`0x0200EB84` bit `0x20`), which
    /// forbids some commands.
    #[must_use]
    pub fn new(data: &GameData<'_>, state: &[u8], link: bool) -> Self {
        let offsets = |table| {
            data.script_offsets(table)
                .ok()
                .flatten()
                .unwrap_or_default()
        };
        Self {
            menu: ScriptRunner::named(BATTLE_MENU_TABLE, offsets(BATTLE_MENU_TABLE)),
            text: ScriptRunner::named(BATTLE_TEXT_TABLE, offsets(BATTLE_TEXT_TABLE)),
            labels: ScriptRunner::named(BATTLE_LABEL_TABLE, offsets(BATTLE_LABEL_TABLE)),
            items: ScriptRunner::named(ITEM_TABLE, offsets(ITEM_TABLE)),
            phase: Phase::Opening(0),
            busy: 0,
            link,
            pages: pages(state),
            page: 0,
            line: 0,
            deck_line: 0,
            shown: None,
            held: Input::default(),
        }
    }

    /// Whether the screen has handed back to the battle.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.phase == Phase::Closed
    }

    /// Whether the screen shows itself rather than the darkening battle.
    #[must_use]
    pub fn covers(&self) -> bool {
        !matches!(self.phase, Phase::Opening(frames) if frames < BUILD_AT)
    }

    /// The fade level, 0 to 31.
    #[must_use]
    pub fn fade(&self) -> u32 {
        match self.phase {
            Phase::Opening(frames) if frames < BRIGHTEN_AT => {
                frames.saturating_sub(DARKEN_DELAY).min(FADE_TOP)
            }
            Phase::Opening(frames) => FADE_TOP.saturating_sub(frames - BRIGHTEN_AT),
            Phase::Leaving(frames) => frames.saturating_sub(LEAVE_DELAY).min(FADE_TOP),
            Phase::Closed => FADE_TOP,
            Phase::List | Phase::Deck(_) => 0,
        }
    }

    /// Advances one frame; `state` is the game-state block whose deck the
    /// screen changes.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a script cannot run.
    pub fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
        state: &mut [u8],
    ) -> Result<(), ScriptError> {
        self.held = input;
        match self.phase {
            Phase::Opening(frames) => {
                let frames = frames + 1;
                self.phase = Phase::Opening(frames);
                if frames == BUILD_AT {
                    self.build(rom, windows, state)?;
                }
                if frames == MENU_AT {
                    self.busy = 0;
                    self.show_list(windows)?;
                }
                return Ok(());
            }
            Phase::Leaving(frames) => {
                let frames = frames + 1;
                self.phase = if frames >= LEAVE_FRAMES {
                    Phase::Closed
                } else {
                    Phase::Leaving(frames)
                };
                return Ok(());
            }
            Phase::Closed => return Ok(()),
            Phase::List | Phase::Deck(_) => {}
        }
        if self.busy > 0 {
            self.busy -= 1;
            return Ok(());
        }
        match self.phase {
            Phase::List => self.list_step(rom, input, windows, state),
            Phase::Deck(command) => self.deck_step(rom, input, command, windows, state),
            _ => Ok(()),
        }
    }

    /// Builds the screen in the dark (state 200): the text system's reset,
    /// the three windows, the deck, the list and the first command's
    /// description.
    fn build(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
        state: &[u8],
    ) -> Result<(), ScriptError> {
        for script in [
            SCRIPT_RESET,
            SCRIPT_MESSAGE_WINDOW,
            SCRIPT_PRESENT_MESSAGE,
            SCRIPT_DECK_WINDOW,
            SCRIPT_PRESENT_DECK,
            SCRIPT_LIST_WINDOW,
            SCRIPT_PRESENT_LIST,
        ] {
            self.run(rom, Table::Menu, script, windows)?;
        }
        self.deck_line = 0;
        self.print_deck(rom, windows, state)?;
        self.pages = pages(state);
        self.page = 0;
        self.line = 0;
        self.shown = None;
        self.describe(rom, self.command(), windows)?;
        self.print_page(rom, windows, state)
    }

    /// The list's menu (`0x44C`, script 18), its cursor on the line it
    /// left.
    fn show_list(&mut self, windows: &mut ScriptWindows<'_>) -> Result<(), ScriptError> {
        windows.set_cursor(LIST_WINDOW, Some(self.line));
        windows.set_cursor(LIST_WINDOW, None);
        self.menu.start(SCRIPT_LIST_MENU)?;
        self.phase = Phase::List;
        Ok(())
    }

    /// One frame of the list's menu, and what its end asks for: a move
    /// shows the command's description, L and R turn the page, A takes a
    /// command not in the deck to the deck, B or START leaves.
    fn list_step(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
        state: &[u8],
    ) -> Result<(), ScriptError> {
        if !self.menu.update(rom, input, windows)? {
            return Ok(());
        }
        let [code, line, ..] = *self.menu.vars();
        match code {
            MOVED_UP | MOVED_DOWN => {
                self.run(rom, Table::Menu, SCRIPT_DRAW_MESSAGE, windows)?;
                self.line = usize::from(line);
                self.describe(rom, self.command(), windows)?;
            }
            PAGE_LEFT | PAGE_RIGHT => {
                let last = self.pages.len().saturating_sub(1);
                let page = if code == PAGE_LEFT {
                    self.page.saturating_sub(1)
                } else {
                    (self.page + 1).min(last)
                };
                if page != self.page {
                    self.page = page;
                    self.print_page(rom, windows, state)?;
                    self.run(rom, Table::Menu, SCRIPT_DRAW_MESSAGE, windows)?;
                    self.line = 0;
                    self.describe(rom, self.command(), windows)?;
                }
            }
            CONFIRMED => {
                if let Some(command) = self
                    .command()
                    .filter(|&command| self.can_take(command, state))
                {
                    return self.enter_deck(rom, command, windows, state);
                }
            }
            STARTED => {
                self.phase = Phase::Leaving(0);
                return Ok(());
            }
            0 if input.is_held(Button::B) => {
                self.phase = Phase::Leaving(0);
                return Ok(());
            }
            _ => {}
        }
        self.busy += 1;
        self.show_list(windows)
    }

    /// Whether A can take `command` to the deck: it is not in it, and a
    /// link battle does not forbid it.
    fn can_take(&self, command: u8, state: &[u8]) -> bool {
        !(in_deck(state, command) || self.link && forbidden(command))
    }

    /// The deck's menu (state 2000): the description goes, the cursor
    /// starts on the slot it left, whose command is described.
    fn enter_deck(
        &mut self,
        rom: &[u8],
        command: u8,
        windows: &mut ScriptWindows<'_>,
        state: &[u8],
    ) -> Result<(), ScriptError> {
        self.run(rom, Table::Menu, SCRIPT_CLEAR_MESSAGE, windows)?;
        self.run(rom, Table::Menu, SCRIPT_PRESENT_MESSAGE, windows)?;
        self.shown = None;
        self.describe_slot(rom, state, windows)?;
        self.busy += 1;
        self.start_deck_menu(windows, command)
    }

    fn start_deck_menu(
        &mut self,
        windows: &mut ScriptWindows<'_>,
        command: u8,
    ) -> Result<(), ScriptError> {
        windows.set_cursor(DECK_WINDOW, Some(self.deck_line));
        windows.set_cursor(DECK_WINDOW, None);
        self.menu.start(SCRIPT_DECK_MENU)?;
        self.phase = Phase::Deck(command);
        Ok(())
    }

    /// One frame of the deck's menu (`0x834`): a move describes the slot's
    /// command, A puts `command` in the slot (`0x898`), B goes back to the
    /// list (`0xB54`).
    fn deck_step(
        &mut self,
        rom: &[u8],
        input: Input,
        command: u8,
        windows: &mut ScriptWindows<'_>,
        state: &mut [u8],
    ) -> Result<(), ScriptError> {
        if !self.menu.update(rom, input, windows)? {
            return Ok(());
        }
        let [code, line, ..] = *self.menu.vars();
        match code {
            MOVED_UP | MOVED_DOWN => {
                self.run(rom, Table::Menu, SCRIPT_DRAW_MESSAGE, windows)?;
                self.deck_line = usize::from(line);
                self.describe_slot(rom, state, windows)?;
                self.busy += 1;
                return self.start_deck_menu(windows, command);
            }
            CONFIRMED => {
                if let Some(slot) = state.get_mut(DECK + self.deck_line.min(DECK_SLOTS - 1)) {
                    *slot = command;
                }
                self.pages = pages(state);
                self.page = 0;
                self.line = 0;
            }
            0 if input.is_held(Button::B) => {}
            _ => {
                self.busy += 1;
                return self.start_deck_menu(windows, command);
            }
        }
        self.back_to_list(rom, windows, state)
    }

    /// Back to the list (`0xB54`, then 1000): the deck and the page are
    /// printed again and the command under the cursor described.
    fn back_to_list(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
        state: &[u8],
    ) -> Result<(), ScriptError> {
        self.run(rom, Table::Menu, SCRIPT_DRAW_DECK, windows)?;
        self.print_deck(rom, windows, state)?;
        self.run(rom, Table::Menu, SCRIPT_DRAW_LIST, windows)?;
        self.print_page(rom, windows, state)?;
        self.shown = None;
        self.describe(rom, self.command(), windows)?;
        self.busy += 1;
        self.show_list(windows)
    }

    /// The command under the list's cursor.
    fn command(&self) -> Option<u8> {
        self.pages.get(self.page)?.get(self.line).copied()
    }

    /// The deck (`0x0803C05C`): per slot its number, ：, and its command's
    /// name or －－－－－－－.
    fn print_deck(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
        state: &[u8],
    ) -> Result<(), ScriptError> {
        windows.set_scroll_marks(DECK_WINDOW, (false, false));
        self.run(rom, Table::Menu, SCRIPT_CLEAR_DECK, windows)?;
        self.run(rom, Table::Menu, SCRIPT_PRESENT_DECK, windows)?;
        for slot in 0..DECK_SLOTS {
            if slot > 0 {
                self.print(rom, Table::Text, TEXT_LINE_BREAK, DECK_WINDOW, windows)?;
            }
            self.print(
                rom,
                Table::Text,
                TEXT_DIGITS + slot + 1,
                DECK_WINDOW,
                windows,
            )?;
            self.print(rom, Table::Text, TEXT_COLON, DECK_WINDOW, windows)?;
            match state
                .get(DECK + slot)
                .copied()
                .filter(|&command| command != EMPTY)
            {
                Some(command) => self.print(
                    rom,
                    Table::Items,
                    COMMAND_NAMES + usize::from(command),
                    DECK_WINDOW,
                    windows,
                )?,
                None => self.print(rom, Table::Text, TEXT_EMPTY, DECK_WINDOW, windows)?,
            }
        }
        self.run(rom, Table::Menu, SCRIPT_PRESENT_DECK, windows)
    }

    /// A page of the learned commands (`0x0803B6E4`, `0x0803B780`): ★
    /// before one in the deck, a space before the others, then its name;
    /// the marks show when the pages go on to either side.
    fn print_page(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
        state: &[u8],
    ) -> Result<(), ScriptError> {
        let last = self.pages.len().saturating_sub(1);
        windows.set_scroll_marks(LIST_WINDOW, (self.page > 0, self.page < last));
        self.run(rom, Table::Menu, SCRIPT_CLEAR_LIST, windows)?;
        self.run(rom, Table::Menu, SCRIPT_PRESENT_LIST, windows)?;
        let commands = self.pages.get(self.page).cloned().unwrap_or_default();
        for (index, command) in commands.into_iter().enumerate() {
            if index > 0 {
                self.print(rom, Table::Text, TEXT_LINE_BREAK, LIST_WINDOW, windows)?;
            }
            let mark = if in_deck(state, command) {
                TEXT_IN_DECK
            } else {
                TEXT_SPACE
            };
            self.print(rom, Table::Text, mark, LIST_WINDOW, windows)?;
            let name = COMMAND_NAMES + usize::from(command);
            self.print(rom, Table::Items, name, LIST_WINDOW, windows)?;
        }
        self.run(rom, Table::Menu, SCRIPT_PRESENT_LIST, windows)
    }

    /// The description of `command` in the message window, when it is not
    /// the one shown (`0x0803B694`, `0x080328A0`).
    fn describe(
        &mut self,
        rom: &[u8],
        command: Option<u8>,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if command == self.shown {
            return Ok(());
        }
        self.shown = command;
        self.run(rom, Table::Menu, SCRIPT_CLEAR_MESSAGE, windows)?;
        self.run(rom, Table::Menu, SCRIPT_PRESENT_MESSAGE, windows)?;
        if let Some(command) = command {
            if self.link && forbidden(command) {
                self.print(rom, Table::Labels, LABEL_FORBIDDEN, MESSAGE_WINDOW, windows)?;
            } else {
                let help = COMMAND_HELP + usize::from(command);
                self.print(rom, Table::Items, help, MESSAGE_WINDOW, windows)?;
            }
            self.run(rom, Table::Menu, SCRIPT_PRESENT_MESSAGE, windows)?;
        }
        Ok(())
    }

    /// The description of the command in the slot under the deck's cursor,
    /// or none for an empty slot (`0x0803BFE8`).
    fn describe_slot(
        &mut self,
        rom: &[u8],
        state: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let command = state
            .get(DECK + self.deck_line)
            .copied()
            .filter(|&command| command != EMPTY);
        self.describe(rom, command, windows)
    }

    /// Runs `script` of `table` to its end at once, counting the frames the
    /// original would spend on it.
    fn run(
        &mut self,
        rom: &[u8],
        table: Table,
        script: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let held = self.held;
        let runner = self.runner(table);
        runner.start(script)?;
        let mut frames = 0;
        while !runner.update(rom, held, windows)? {
            if runner.is_waiting_for_key() {
                break;
            }
            frames += 1;
        }
        self.busy += frames;
        Ok(())
    }

    /// Prints `script` of `table` in `window`.
    fn print(
        &mut self,
        rom: &[u8],
        table: Table,
        script: usize,
        window: u8,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.runner(table).select_window(window);
        self.run(rom, table, script, windows)
    }

    fn runner(&mut self, table: Table) -> &mut ScriptRunner {
        match table {
            Table::Menu => &mut self.menu,
            Table::Text => &mut self.text,
            Table::Labels => &mut self.labels,
            Table::Items => &mut self.items,
        }
    }

    /// Draws the windows over black, darkened by the fade.
    pub fn draw(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
    ) {
        frame.fill(Rgb::default());
        windows.draw(frame, skin, painter);
        let level = self.fade().min(u32::from(FADE_STEPS));
        darken(frame, u8::try_from(level).unwrap_or(FADE_STEPS));
    }
}

/// The learned commands in their order, six a page (`0x0803B570`).
fn pages(state: &[u8]) -> Vec<Vec<u8>> {
    let learned: Vec<u8> = (0..COMMANDS)
        .filter(|&command| {
            state
                .get(LEARNED + usize::from(command))
                .is_some_and(|&byte| byte != 0)
        })
        .collect();
    let mut pages: Vec<Vec<u8>> = learned.chunks(PAGE_LINES).map(<[u8]>::to_vec).collect();
    if pages.is_empty() {
        pages.push(Vec::new());
    }
    pages
}

/// Whether `command` is in the deck (`0x0803B654`).
fn in_deck(state: &[u8], command: u8) -> bool {
    (0..DECK_SLOTS).any(|slot| state.get(DECK + slot) == Some(&command))
}

/// Whether the link battles forbid `command` (`0x080355AC`): 0, 1, 2, 4
/// and 5.
fn forbidden(command: u8) -> bool {
    matches!(command, 0..=2 | 4 | 5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_takes_the_learned_commands_six_a_page() {
        let mut state = vec![0; 0x3F10];
        for command in [0, 3, 19, 20, 21, 22, 30] {
            state[LEARNED + command] = 1;
        }
        assert_eq!(pages(&state), [vec![0, 3, 19, 20, 21, 22], vec![30]]);
        assert_eq!(pages(&vec![0; 0x3F10]), [Vec::<u8>::new()]);
    }

    #[test]
    fn a_command_in_the_deck_or_forbidden_cannot_be_taken() {
        let rom = Vec::new();
        let data = GameData::new(&rom);
        let mut state = vec![0; 0x3F10];
        state[DECK..DECK + DECK_SLOTS].fill(EMPTY);
        state[DECK + 2] = 19;
        let screen = DeckScreen::new(&data, &state, false);
        assert!(!screen.can_take(19, &state));
        assert!(screen.can_take(4, &state));
        let link = DeckScreen::new(&data, &state, true);
        assert!(!link.can_take(4, &state));
        assert!(link.can_take(3, &state));
    }
}
