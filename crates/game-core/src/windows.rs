//! The windows a script draws: frames, text, portraits and prompts.
//!
//! Matches the original's window records: up to eight windows, each a tile
//! rectangle with its border. Text starts one tile in from the top-left
//! corner, spans the width minus two cells, and has `(height − 2) / 2` lines
//! of 16 pixels; a line break past the last line scrolls the text up. A
//! portrait is drawn one tile in from the corner. Windows sharing a border
//! column are joined with the divider tiles. The prompt sits on the bottom
//! border, two tiles in from the right corner.

use std::collections::HashSet;

use extraction::saga::{self, Portrait};
use platform::Frame;

use crate::text::{CELL_WIDTH, TextMetrics};
use crate::translation::Translation;
use crate::{FrameStyle, ScriptHost, TextPainter, WindowPainter, draw_sprite};

/// Name the player carries when none was entered.
pub const DEFAULT_PLAYER_NAME: &str = "アトレー";
const WINDOWS: usize = 8;
const TILE: usize = 8;
const LINE_HEIGHT: usize = 16;
const TYPEWRITER_STYLE: u8 = 1;
const MENU_KIND: u8 = 1;
const LIGHT_FRAME_KIND: u8 = 0x20;
const NO_FRAME_KIND: u8 = 0x40;
const MENU_MARGIN: usize = 2;
const FULL_WIDTH_SPACE: char = '\u{3000}';
const TEXT_MARGIN: usize = 1;
const PROMPT_FROM_RIGHT: usize = 2;
const NAME_RESET_MODE: u8 = 1;

/// One open window.
#[derive(Debug, Clone)]
pub struct Window {
    /// Left column in tiles.
    pub x: usize,
    /// Top row in tiles.
    pub y: usize,
    /// Width in tiles, border included.
    pub width: usize,
    /// Height in tiles, border included.
    pub height: usize,
    /// Kind byte as the script gave it.
    pub kind: u8,
    /// Style byte as the script gave it.
    pub style: u8,
    /// Text lines, the cursor being on the last.
    pub lines: Vec<String>,
    /// Pixels each line takes, alongside `lines`.
    pub widths: Vec<usize>,
    /// How characters are spaced.
    pub layout: TextLayout,
    /// Portrait shown, if any.
    pub portrait: Option<Portrait>,
    /// Whether the "more" prompt is lit.
    pub prompt: bool,
    /// Whether the window has been presented.
    pub visible: bool,
    /// Line the menu cursor sits on, if the window is a menu being used.
    pub cursor: Option<usize>,
    /// Line the last menu ended on; the next one starts there.
    pub line: usize,
    /// A line break waiting for the next character, so a trailing break
    /// does not scroll the text away.
    pub pending_break: bool,
    /// Order in which the window was opened; later windows cover earlier ones.
    pub opened: u64,
}

impl Window {
    /// Whether the window is a menu: text starts a cell further in to
    /// leave room for the cursor brackets.
    #[must_use]
    pub fn is_menu(&self) -> bool {
        self.kind & 0x0F == MENU_KIND
    }

    /// Cells between the border and the text.
    #[must_use]
    pub fn margin(&self) -> usize {
        if self.is_menu() {
            MENU_MARGIN
        } else {
            TEXT_MARGIN
        }
    }

    /// How the border is drawn.
    #[must_use]
    pub fn frame_style(&self) -> FrameStyle {
        match self.kind & 0xF0 {
            LIGHT_FRAME_KIND => FrameStyle::Light,
            NO_FRAME_KIND => FrameStyle::None,
            _ => FrameStyle::Standard,
        }
    }

    /// Text cells per line.
    #[must_use]
    pub fn columns(&self) -> usize {
        self.width.saturating_sub(2 * self.margin())
    }

    /// Text lines that fit.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.height.saturating_sub(2) / 2
    }

    /// Pixels a text line can take.
    #[must_use]
    pub fn pixels(&self) -> usize {
        self.columns() * CELL_WIDTH
    }

    fn put_char(&mut self, ch: char, advance: usize, inset: usize) {
        let (advance, inset) = match self.layout {
            TextLayout::Cells => (CELL_WIDTH, 0),
            TextLayout::Proportional => (advance, inset),
        };
        self.apply_break();
        self.ensure_line();
        if self
            .widths
            .last()
            .is_some_and(|width| width + advance > self.pixels())
        {
            self.new_line();
        }
        self.push(ch, advance, inset);
    }

    /// Puts `ch` at cell `column` of the current line, padding with
    /// full-width spaces up to it and never wrapping: how the game's code
    /// places values in a window.
    fn put_at(&mut self, column: usize, ch: char) {
        self.apply_break();
        self.ensure_line();
        let used = self.lines.last().map_or(0, |line| line.chars().count());
        for _ in used..column {
            self.push(FULL_WIDTH_SPACE, CELL_WIDTH, 0);
        }
        self.push(ch, CELL_WIDTH, 0);
    }

    fn ensure_line(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
            self.widths.push(0);
        }
    }

    fn push(&mut self, ch: char, advance: usize, inset: usize) {
        self.ensure_line();
        if let (Some(line), Some(width)) = (self.lines.last_mut(), self.widths.last_mut()) {
            if line.is_empty() {
                *width = inset;
            }
            line.push(ch);
            *width += advance;
        }
    }

    fn apply_break(&mut self) {
        if self.pending_break {
            self.pending_break = false;
            self.new_line();
        }
    }

    fn new_line(&mut self) {
        self.ensure_line();
        self.lines.push(String::new());
        self.widths.push(0);
        while self.lines.len() > self.rows().max(1) {
            self.lines.remove(0);
            self.widths.remove(0);
        }
    }

    fn line_break(&mut self) {
        self.ensure_line();
        if self.pending_break {
            self.new_line();
        }
        self.pending_break = true;
    }
}

/// How a window spaces its characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextLayout {
    /// Each character at its own width.
    #[default]
    Proportional,
    /// One cell per character, for grids that align with sprites.
    Cells,
}

/// The script windows of a scene, drawn with the game's skin and font.
pub struct ScriptWindows<'rom> {
    rom: &'rom [u8],
    windows: Vec<Option<Window>>,
    flags: HashSet<u16>,
    player_name: String,
    sounds: Vec<u8>,
    opened: u64,
    translation: Translation,
    metrics: TextMetrics,
}

impl<'rom> ScriptWindows<'rom> {
    /// Creates the window set; `player_name` is what `{name}` prints.
    #[must_use]
    pub fn new(rom: &'rom [u8], player_name: &str) -> Self {
        Self {
            rom,
            windows: (0..WINDOWS).map(|_| None).collect(),
            flags: HashSet::new(),
            player_name: player_name.to_owned(),
            sounds: Vec::new(),
            opened: 0,
            translation: Translation::default(),
            metrics: TextMetrics::default(),
        }
    }

    /// Lays text out with `metrics` from now on.
    pub fn set_metrics(&mut self, metrics: TextMetrics) {
        self.metrics = metrics;
    }

    /// The character widths text is laid out with.
    #[must_use]
    pub fn metrics(&self) -> &TextMetrics {
        &self.metrics
    }

    /// Puts `ch` at cell `column` of window `id`'s current line, as the
    /// game's code places values: padded up to the column, never wrapped.
    pub fn put_at(&mut self, id: u8, column: usize, ch: char) {
        if let Some(window) = self.window_mut(id) {
            window.put_at(column, ch);
        }
    }

    /// Gives window `id` one cell per character, or its own widths.
    pub fn set_layout(&mut self, id: u8, layout: TextLayout) {
        if let Some(window) = self.window_mut(id) {
            window.layout = layout;
        }
    }

    /// Uses `translation` for the messages it covers from now on.
    pub fn set_translation(&mut self, translation: Translation) {
        self.translation = translation;
    }

    /// The translation in use.
    #[must_use]
    pub fn translation(&self) -> &Translation {
        &self.translation
    }

    /// The open windows, by slot.
    #[must_use]
    pub fn windows(&self) -> &[Option<Window>] {
        &self.windows
    }

    /// Whether any window is open.
    #[must_use]
    pub fn any_open(&self) -> bool {
        self.windows.iter().any(Option::is_some)
    }

    /// Changes what `{name}` prints.
    pub fn set_player_name(&mut self, name: &str) {
        name.clone_into(&mut self.player_name);
    }

    /// Sounds requested so far, oldest first; clearing is the caller's job.
    pub fn take_sounds(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.sounds)
    }

    /// Draws every presented window over `frame`, in the order they were
    /// opened so later windows cover earlier ones, as one tilemap would.
    pub fn draw(&self, frame: &mut Frame, skin: &WindowPainter, painter: &TextPainter) {
        let mut order: Vec<(usize, &Window)> = self
            .windows
            .iter()
            .enumerate()
            .filter_map(|(index, window)| Some((index, window.as_ref()?)))
            .filter(|(_, window)| window.visible)
            .collect();
        order.sort_by_key(|(_, window)| window.opened);
        for (index, window) in order {
            skin.draw_framed(
                frame,
                window.x,
                window.y,
                window.width,
                window.height,
                window.frame_style(),
            );
            if self.shares_left_border(index, window) {
                skin.draw_divider(frame, window.x, window.y, window.height);
            }
            let origin = (pixels(window.x + window.margin()), pixels(window.y + 1));
            if let Some(portrait) = &window.portrait {
                draw_sprite(
                    frame,
                    origin.0,
                    origin.1,
                    &portrait.image,
                    &portrait.palette,
                    false,
                );
            }
            for (row, line) in window.lines.iter().enumerate() {
                let y = origin.1 + i32::try_from(row * LINE_HEIGHT).unwrap_or(i32::MAX);
                match window.layout {
                    TextLayout::Cells => {
                        painter.draw_cells(frame, origin.0, y, line, skin.palette());
                    }
                    TextLayout::Proportional => {
                        painter.draw(frame, origin.0, y, line, skin.palette());
                    }
                }
            }
            if window.prompt {
                skin.draw_prompt(
                    frame,
                    (window.x + window.width).saturating_sub(PROMPT_FROM_RIGHT),
                    window.y + window.height - 1,
                );
            }
            if let Some(line) = window.cursor {
                skin.draw_cursor(
                    frame,
                    window.x + 1,
                    (window.x + window.width).saturating_sub(2),
                    window.y + 1 + line * 2,
                );
            }
        }
    }

    fn shares_left_border(&self, index: usize, window: &Window) -> bool {
        self.windows
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .filter_map(|(_, other)| other.as_ref())
            .any(|other| {
                other.visible
                    && other.x + other.width == window.x + 1
                    && other.y == window.y
                    && other.height == window.height
            })
    }

    fn window_mut(&mut self, id: u8) -> Option<&mut Window> {
        self.windows.get_mut(usize::from(id))?.as_mut()
    }
}

fn pixels(tiles: usize) -> i32 {
    i32::try_from(tiles * TILE).unwrap_or(i32::MAX)
}

impl ScriptHost for ScriptWindows<'_> {
    fn open_window(&mut self, id: u8, kind: u8, rect: (u8, u8, u8, u8), style: u8) {
        self.opened += 1;
        let opened = self.opened;
        if let Some(slot) = self.windows.get_mut(usize::from(id)) {
            *slot = Some(Window {
                x: usize::from(rect.0),
                y: usize::from(rect.1),
                width: usize::from(rect.2),
                height: usize::from(rect.3),
                kind,
                style,
                lines: Vec::new(),
                widths: Vec::new(),
                layout: TextLayout::Proportional,
                portrait: None,
                prompt: false,
                visible: false,
                cursor: None,
                line: 0,
                pending_break: false,
                opened,
            });
        }
    }

    fn close_window(&mut self, id: Option<u8>) {
        match id {
            Some(id) => {
                if let Some(slot) = self.windows.get_mut(usize::from(id)) {
                    *slot = None;
                }
            }
            None => self.windows.iter_mut().for_each(|slot| *slot = None),
        }
    }

    fn present(&mut self, id: Option<u8>) {
        match id {
            Some(id) => {
                if let Some(window) = self.window_mut(id) {
                    window.visible = true;
                    window.cursor = None;
                }
            }
            None => self.windows.iter_mut().flatten().for_each(|window| {
                window.visible = true;
                window.cursor = None;
            }),
        }
    }

    fn reveal(&mut self, id: u8) {
        if let Some(window) = self.window_mut(id) {
            window.visible = true;
        }
    }

    fn clear_window(&mut self, id: u8) {
        if let Some(window) = self.window_mut(id) {
            window.lines.clear();
            window.widths.clear();
            window.pending_break = false;
            window.prompt = false;
        }
    }

    fn put_char(&mut self, id: u8, ch: char) {
        let advance = self.metrics.advance(ch);
        let inset = self.metrics.inset(ch);
        if let Some(window) = self.window_mut(id) {
            window.put_char(ch, advance, inset);
        }
    }

    fn line_break(&mut self, id: u8) {
        if let Some(window) = self.window_mut(id) {
            window.line_break();
        }
    }

    fn typewriter(&self, id: u8) -> bool {
        self.windows
            .get(usize::from(id))
            .and_then(Option::as_ref)
            .is_some_and(|window| window.style & TYPEWRITER_STYLE != 0)
    }

    fn portrait(&mut self, id: u8, character: u8, expression: u8) {
        let portrait =
            saga::portrait(self.rom, usize::from(character), usize::from(expression)).ok();
        if let Some(window) = self.window_mut(id) {
            window.portrait = portrait;
        }
    }

    fn prompt(&mut self, id: u8, visible: bool) {
        if let Some(window) = self.window_mut(id) {
            window.prompt = visible;
        }
    }

    fn play_sound(&mut self, id: u8) {
        self.sounds.push(id);
    }

    fn flag(&self, flag: u16) -> bool {
        self.flags.contains(&flag)
    }

    fn set_flag(&mut self, flag: u16, set: bool) {
        if set {
            self.flags.insert(flag);
        } else {
            self.flags.remove(&flag);
        }
    }

    fn player_name(&self) -> String {
        self.player_name.clone()
    }

    fn reset(&mut self, mode: u8) {
        self.windows.iter_mut().for_each(|slot| *slot = None);
        if mode == NAME_RESET_MODE {
            self.player_name.clear();
        }
    }

    fn menu_lines(&self, id: u8) -> usize {
        self.windows
            .get(usize::from(id))
            .and_then(Option::as_ref)
            .map_or(0, |window| {
                window
                    .lines
                    .iter()
                    .filter(|line| !line.trim().is_empty())
                    .count()
            })
    }

    fn set_cursor(&mut self, id: u8, line: Option<usize>) {
        if let Some(window) = self.window_mut(id) {
            window.cursor = line;
            if let Some(line) = line {
                window.line = line;
            }
        }
    }

    fn translate(&self, table: &str, index: usize, offset: usize) -> Option<String> {
        self.translation
            .get(table, index, offset)
            .map(str::to_owned)
    }

    fn fit_window(
        &self,
        table: &str,
        index: usize,
        id: u8,
        kind: u8,
        rect: (u8, u8, u8, u8),
    ) -> (u8, u8, u8, u8) {
        self.translation.fit_window(table, index, id, kind, rect)
    }

    fn menu_line(&self, id: u8) -> usize {
        self.windows
            .get(usize::from(id))
            .and_then(Option::as_ref)
            .map_or(0, |window| window.line)
    }

    fn is_open(&self, id: u8) -> bool {
        self.windows
            .get(usize::from(id))
            .is_some_and(Option::is_some)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn windows() -> ScriptWindows<'static> {
        ScriptWindows::new(&[], "アトレー")
    }

    #[test]
    fn a_window_remembers_its_menu_line_until_it_is_redrawn() {
        let mut host = windows();
        host.open_window(3, 0x21, (0, 0, 9, 14), 4);
        host.set_cursor(3, Some(2));
        assert_eq!(host.windows()[3].as_ref().and_then(|w| w.cursor), Some(2));
        host.reveal(3);
        assert_eq!(host.windows()[3].as_ref().and_then(|w| w.cursor), Some(2));
        host.present(None);
        assert_eq!(host.windows()[3].as_ref().and_then(|w| w.cursor), None);
        assert_eq!(host.menu_line(3), 2);
        assert!(host.is_open(3));
        assert!(!host.is_open(4));
    }

    #[test]
    fn text_wraps_at_the_inner_width_and_scrolls_past_the_last_line() {
        let mut host = windows();
        host.open_window(1, 0x10, (0, 12, 6, 8), 1);
        for ch in "abcdefghij".chars() {
            host.put_char(1, ch);
        }
        let window = host.windows()[1].as_ref().unwrap();
        assert_eq!(window.lines, ["abcd", "efgh", "ij"]);
        host.line_break(1);
        host.put_char(1, 'k');
        let window = host.windows()[1].as_ref().unwrap();
        assert_eq!(window.lines, ["efgh", "ij", "k"]);
        assert!(host.typewriter(1));
        assert!(!host.typewriter(2));
    }

    #[test]
    fn windows_open_present_clear_and_close() {
        let mut host = windows();
        host.open_window(0, 0x10, (0, 12, 8, 8), 0);
        host.open_window(1, 0x10, (7, 12, 23, 8), 1);
        assert!(!host.windows()[0].as_ref().unwrap().visible);
        host.present(None);
        assert!(host.windows()[0].as_ref().unwrap().visible);
        assert!(host.shares_left_border(1, host.windows()[1].as_ref().unwrap()));
        assert!(!host.shares_left_border(0, host.windows()[0].as_ref().unwrap()));
        host.put_char(1, 'x');
        host.prompt(1, true);
        host.clear_window(1);
        let window = host.windows()[1].as_ref().unwrap();
        assert!(window.lines.is_empty() && !window.prompt);
        host.close_window(Some(0));
        assert!(host.windows()[0].is_none() && host.any_open());
        host.close_window(None);
        assert!(!host.any_open());
    }

    #[test]
    fn keeps_flags_sounds_and_the_player_name() {
        let mut host = windows();
        host.set_flag(7, true);
        assert!(host.flag(7) && !host.flag(8));
        host.set_flag(7, false);
        assert!(!host.flag(7));
        host.play_sound(0x41);
        assert_eq!(host.take_sounds(), [0x41]);
        assert!(host.take_sounds().is_empty());
        assert_eq!(host.player_name(), "アトレー");
        host.reset(1);
        assert_eq!(host.player_name(), "");
    }

    #[test]
    fn menus_reserve_the_cursor_columns_and_count_their_lines() {
        let mut host = windows();
        host.open_window(0, 0x21, (10, 10, 9, 8), 4);
        for ch in "ab\ncd\nef".chars() {
            if ch == '\n' {
                host.line_break(0);
            } else {
                host.put_char(0, ch);
            }
        }
        let window = host.windows()[0].as_ref().unwrap();
        assert!(window.is_menu());
        assert_eq!(window.frame_style(), FrameStyle::Light);
        assert_eq!(window.columns(), 5);
        assert_eq!(host.menu_lines(0), 3);
        host.set_cursor(0, Some(2));
        assert_eq!(host.windows()[0].as_ref().unwrap().cursor, Some(2));
        host.open_window(1, 0x40, (8, 0, 22, 4), 0);
        assert_eq!(
            host.windows()[1].as_ref().unwrap().frame_style(),
            FrameStyle::None
        );
    }
}
