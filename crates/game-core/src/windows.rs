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

use extraction::Revision;
use extraction::saga::Portrait;
use platform::{Frame, Rgb};

use crate::data::GameData;
use crate::extension::{Event, SharedExtensions};
use crate::text::{CELL_WIDTH, TextMetrics, cell_stop_mark};
use crate::{FrameStyle, ScriptHost, TextPainter, WindowPainter, draw_sprite};

/// Name the player carries when none was entered.
pub const DEFAULT_PLAYER_NAME: &str = "アトレー";
/// Name the player carries when none was entered and the game plays in a
/// translation: [`DEFAULT_PLAYER_NAME`] in Latin letters.
pub const TRANSLATED_PLAYER_NAME: &str = "Atory";
const WINDOWS: usize = 8;
const TILE: usize = 8;
const LINE_HEIGHT: usize = 16;
const TYPEWRITER_STYLE: u8 = 1;
const MENU_KIND: u8 = 1;
/// A window its portrait fills, from its top-left corner: the battle
/// scenes' pilot window.
const PORTRAIT_KIND: u8 = 4;
const LIGHT_FRAME_KIND: u8 = 0x20;
const NO_FRAME_KIND: u8 = 0x40;
const MENU_MARGIN: usize = 2;
const FULL_WIDTH_SPACE: char = '\u{3000}';
const TEXT_MARGIN: usize = 1;
const PROMPT_FROM_RIGHT: usize = 2;
/// The auto text's wait (a port feature): frames before a text box goes on
/// by itself, and frames more for each character on it, Latin letters
/// read faster than the ROM font's.
const AUTO_BASE_FRAMES: u32 = 30;
const AUTO_LATIN_FRAMES: u32 = 3;
const AUTO_GLYPH_FRAMES: u32 = 6;
/// The auto text's mark: its text, and how far its plate's right edge is
/// from the window's, over the top border.
const AUTO_MARK: &str = "AUTO";
const AUTO_MARK_FROM_RIGHT: usize = 12;
/// The room between the plate's frame and its text, across and down, and
/// the Latin font's capitals: their height and the row they start on.
const MARK_PADDING: (usize, usize) = (2, 1);
const MARK_CAPITALS: usize = 8;
const MARK_CAPITALS_TOP: usize = 2;
/// A notice's look: the rows its letters may take, how far its shadow
/// falls, the colors of its letters, of their shadow and of the scratch
/// they are drawn on first, their opacity at its fullest (of 256), and the
/// levels it fades through.
const NOTICE_ROWS: usize = formats::pixel_font::PIXEL_FONT_ROWS;
const NOTICE_SHADOW: usize = 1;
const NOTICE_INK: Rgb = Rgb::new(248, 248, 248);
const NOTICE_SHADE: Rgb = Rgb::new(0, 0, 0);
const NOTICE_CLEAR: Rgb = Rgb::new(1, 2, 3);
const NOTICE_INK_ALPHA: u16 = 176;
const NOTICE_SHADE_ALPHA: u16 = 128;
/// See [`NOTICE_ROWS`].
pub const NOTICE_LEVELS: u8 = 16;
/// The skin's colors the mark takes: the windows' background and ink.
const BACKGROUND_COLOR: u8 = 1;
const INK_COLOR: u8 = 15;

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
    /// The portrait replacing the one shown, which appears a frame after
    /// the old one goes: the game loads its tiles in between.
    pub next_portrait: Option<Portrait>,
    /// Whether the "more" prompt is lit.
    pub prompt: bool,
    /// Whether the window has been presented.
    pub visible: bool,
    /// The lines a draw left on screen: the text printed since shows once
    /// the window is presented (`0x0803E950` without the flush).
    pub frozen: Option<Vec<String>>,
    /// Line the menu cursor sits on, if the window is a menu being used.
    pub cursor: Option<usize>,
    /// Line the last menu ended on; the next one starts there.
    pub line: usize,
    /// First line shown: a menu keeps every line and scrolls to keep its
    /// cursor in view.
    pub top: usize,
    /// A line break waiting for the next character, so a trailing break
    /// does not scroll the text away.
    pub pending_break: bool,
    /// Order in which the window was opened; later windows cover earlier ones.
    pub opened: u64,
    /// Page marks the game's code sets itself for a list it pages
    /// through: whether pages lie before and after the shown one.
    pub marks: Option<(bool, bool)>,
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

    /// Whether putting a character `advance` pixels wide would scroll the
    /// text up: the lines are full and a break is pending or the line has
    /// no room left.
    #[must_use]
    pub fn would_scroll(&self, advance: usize) -> bool {
        if self.is_menu() || self.lines.len() < self.rows().max(1) {
            return false;
        }
        if self.pending_break {
            return true;
        }
        let advance = match self.layout {
            TextLayout::Cells => CELL_WIDTH,
            TextLayout::Proportional => advance,
        };
        self.widths
            .last()
            .is_some_and(|width| width + advance > self.pixels())
    }

    /// Turns a page: the first line (a speaker's name) stays and the rest
    /// is cleared, the next characters starting on a fresh line below it.
    pub fn turn_page(&mut self) {
        self.lines.truncate(1);
        self.widths.truncate(1);
        self.pending_break = !self.lines.is_empty();
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
        self.pad_to(column);
        self.push(ch, CELL_WIDTH, 0);
    }

    /// Moves the current line's text to cell `column`: with spaces, as
    /// the original prints them, when every character so far takes a
    /// cell, with a column mark after a translation's narrower letters.
    fn pad_to(&mut self, column: usize) {
        self.apply_break();
        self.ensure_line();
        let target = column * CELL_WIDTH;
        let used = self.widths.last().copied().unwrap_or(0);
        if used >= target {
            return;
        }
        if used % CELL_WIDTH == 0 {
            for _ in used / CELL_WIDTH..column {
                self.push(FULL_WIDTH_SPACE, CELL_WIDTH, 0);
            }
        } else {
            self.push(cell_stop_mark(column), target - used, 0);
        }
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
        while !self.is_menu() && self.lines.len() > self.rows().max(1) {
            self.lines.remove(0);
            self.widths.remove(0);
        }
    }

    /// The lines drawn: all of a short window, a menu's from its first
    /// shown line.
    #[must_use]
    pub fn shown_lines(&self) -> &[String] {
        let lines = self.frozen.as_ref().unwrap_or(&self.lines);
        let start = self.top.min(lines.len());
        let end = (start + self.rows().max(1)).min(lines.len());
        &lines[start..end]
    }

    /// Whether lines are hidden above and below the shown ones.
    #[must_use]
    pub fn hidden_lines(&self) -> (bool, bool) {
        let filled = self
            .lines
            .iter()
            .rposition(|line| !line.trim().is_empty())
            .map_or(0, |last| last + 1);
        (self.top > 0, self.top + self.rows() < filled)
    }

    fn scroll_to(&mut self, line: usize) {
        let rows = self.rows().max(1);
        if line < self.top {
            self.top = line;
        } else if line >= self.top + rows {
            self.top = line + 1 - rows;
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
    extensions: SharedExtensions,
    metrics: TextMetrics,
    /// The windows the screen shows, kept by [`ScriptWindows::latch`].
    shown: Option<Vec<Option<Window>>>,
    /// How bright the portraits' palette is, 16 as loaded: its copy
    /// reaches the screen in the frame it is made, unlike the windows'.
    portrait_level: u8,
    /// Whether the text boxes go on by themselves (a port feature).
    auto_text: bool,
}

/// A palette level that leaves the colors as loaded.
pub const NORMAL_LEVEL: u8 = 16;

/// A BGR555 color at a palette level, as the battle scenes' palette fades
/// write it (`0x08047E88`, `0x08047F88`): each level above 16 adds 2 to
/// each channel, up to 31, each below takes 2 off, down to 0.
#[must_use]
pub fn faded_color(color: u16, level: u8) -> u16 {
    let step = (i32::from(level) - i32::from(NORMAL_LEVEL)) * 2;
    let channel = |shift: u16| {
        let value = i32::from((color >> shift) & 0x1F) + step;
        u16::try_from(value.clamp(0, 0x1F)).unwrap_or(0) << shift
    };
    channel(0) | channel(5) | channel(10)
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
            extensions: SharedExtensions::default(),
            metrics: TextMetrics::default(),
            shown: None,
            portrait_level: NORMAL_LEVEL,
            auto_text: false,
        }
    }

    /// Lets the text boxes go on by themselves once their text has been on
    /// screen long enough to read, with their mark, as the enhanced mode's
    /// auto text does (a port feature); or wait for A, as the original.
    pub fn set_auto_text(&mut self, auto: bool) {
        self.auto_text = auto;
    }

    /// Whether a text box, a window that types its text, is on screen.
    #[must_use]
    pub fn shows_text_box(&self) -> bool {
        self.windows
            .iter()
            .flatten()
            .any(|window| window.visible && window.style & TYPEWRITER_STYLE != 0)
    }

    /// Sets the portraits' palette to `level` (see [`faded_color`]).
    pub fn set_portrait_level(&mut self, level: u8) {
        self.portrait_level = level;
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

    /// Pads window `id`'s current line with full-width spaces up to cell
    /// `column`, as the game's code moves the text position before a script
    /// prints there.
    pub fn pad_to(&mut self, id: u8, column: usize) {
        if let Some(window) = self.window_mut(id) {
            window.pad_to(column);
        }
    }

    /// Cuts window `id`'s current line back until `mark` fits after it
    /// within `pixels`, then puts `mark`: a translated name longer than the
    /// column the game's code leaves it, so what follows on the line stays
    /// on it, without a space or a `mark` of its own before `mark`. A line
    /// that fits is left alone.
    pub fn clip_line(&mut self, id: u8, pixels: usize, mark: char) {
        let metrics = self.metrics.clone();
        let Some(window) = self.window_mut(id) else {
            return;
        };
        let (Some(line), Some(width)) = (window.lines.last_mut(), window.widths.last_mut()) else {
            return;
        };
        if *width <= pixels {
            return;
        }
        let room = pixels.saturating_sub(metrics.advance(mark));
        while *width > room || line.ends_with([' ', mark]) {
            let Some(ch) = line.pop() else {
                break;
            };
            *width = if line.is_empty() {
                0
            } else {
                width.saturating_sub(metrics.advance(ch))
            };
        }
        line.push(mark);
        *width += metrics.advance(mark);
    }

    /// Sets the page marks of window `id`, on its left and right borders:
    /// whether pages lie before and after the shown one, as the game's code
    /// does for a list it pages through with L and R (the window's byte
    /// `+0xF`, which the menus of modes 2 to 6 draw).
    pub fn set_scroll_marks(&mut self, id: u8, marks: (bool, bool)) {
        if let Some(window) = self.window_mut(id) {
            window.marks = Some(marks);
        }
    }

    /// Gives window `id` one cell per character, or its own widths.
    pub fn set_layout(&mut self, id: u8, layout: TextLayout) {
        if let Some(window) = self.window_mut(id) {
            window.layout = layout;
        }
    }

    /// Raises events to, and asks questions of, `extensions` from now on.
    pub fn set_extensions(&mut self, extensions: SharedExtensions) {
        self.extensions = extensions;
    }

    /// The extensions in use.
    #[must_use]
    pub fn extensions(&self) -> &SharedExtensions {
        &self.extensions
    }

    fn emit(&self, event: &Event) {
        self.extensions.borrow_mut().emit(event);
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

    /// The game flags that are set, in order.
    #[must_use]
    pub fn flags(&self) -> Vec<u16> {
        let mut flags: Vec<u16> = self.flags.iter().copied().collect();
        flags.sort_unstable();
        flags
    }

    /// Replaces every game flag with `flags`, without raising events.
    pub fn set_flags(&mut self, flags: impl IntoIterator<Item = u16>) {
        self.flags = flags.into_iter().collect();
    }

    /// Sounds requested so far, oldest first; clearing is the caller's job.
    pub fn take_sounds(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.sounds)
    }

    /// Draws every presented window over `frame`, in the order they were
    /// opened so later windows cover earlier ones, as one tilemap would.
    pub fn draw(&self, frame: &mut Frame, skin: &WindowPainter, painter: &TextPainter) {
        Self::draw_set(&self.windows, frame, skin, painter, self.portrait_level);
    }

    /// Keeps the windows as they are for [`ScriptWindows::draw_shown`]: on
    /// the field the game's text layers reach the screen a frame late, as
    /// its sprites do (see [`crate::field::Field::latch`]).
    pub fn latch(&mut self) {
        self.shown = Some(self.windows.clone());
        for window in self.windows.iter_mut().flatten() {
            if let Some(portrait) = window.next_portrait.take() {
                window.portrait = Some(portrait);
            }
        }
    }

    /// Draws the windows kept by the last [`ScriptWindows::latch`], or the
    /// current ones before any.
    pub fn draw_shown(&self, frame: &mut Frame, skin: &WindowPainter, painter: &TextPainter) {
        let windows = self.shown.as_deref().unwrap_or(&self.windows);
        Self::draw_set(windows, frame, skin, painter, self.portrait_level);
        if self.auto_text {
            for window in windows.iter().flatten() {
                if window.visible && window.style & TYPEWRITER_STYLE != 0 {
                    draw_auto_mark(frame, window, skin, painter);
                }
            }
        }
    }

    fn draw_set(
        windows: &[Option<Window>],
        frame: &mut Frame,
        skin: &WindowPainter,
        painter: &TextPainter,
        portrait_level: u8,
    ) {
        let mut order: Vec<(usize, &Window)> = windows
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
            if Self::shares_left_border(windows, index, window) {
                skin.draw_divider(frame, window.x, window.y, window.height);
            }
            let origin = (pixels(window.x + window.margin()), pixels(window.y + 1));
            if let Some(portrait) = &window.portrait {
                let corner = if window.kind & 0x0F == PORTRAIT_KIND {
                    (pixels(window.x), pixels(window.y))
                } else {
                    origin
                };
                let palette = portrait
                    .palette
                    .map(|color| faded_color(color, portrait_level));
                draw_sprite(frame, corner.0, corner.1, &portrait.image, &palette, false);
            }
            for (row, line) in window.shown_lines().iter().enumerate() {
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
                    window.frame_style(),
                );
            }
            if let Some(line) = window.cursor {
                skin.draw_cursor(
                    frame,
                    window.x + 1,
                    (window.x + window.width).saturating_sub(2),
                    window.y + 1 + line.saturating_sub(window.top) * 2,
                );
            }
            if window.is_menu() {
                skin.draw_scroll_marks(
                    frame,
                    window.x + window.width / 2,
                    (window.y, window.y + window.height - 1),
                    window.hidden_lines(),
                );
                if let Some(marks) = window.marks {
                    skin.draw_page_marks(
                        frame,
                        (window.x, (window.x + window.width).saturating_sub(1)),
                        window.y + window.height / 2,
                        marks,
                    );
                }
            }
        }
    }

    /// Whether `window`'s left border lies on another window's right
    /// border, where the story skin joins them with the divider tiles; a
    /// light-framed window (the status screens') keeps its own border.
    fn shares_left_border(windows: &[Option<Window>], index: usize, window: &Window) -> bool {
        window.frame_style() == FrameStyle::Standard
            && windows
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .filter_map(|(_, other)| other.as_ref())
                .any(|other| {
                    other.visible
                        && other.frame_style() == FrameStyle::Standard
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
        self.emit(&Event::WindowOpened { id, rect, kind });
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
                next_portrait: None,
                prompt: false,
                visible: false,
                frozen: None,
                cursor: None,
                line: 0,
                top: 0,
                pending_break: false,
                opened,
                marks: None,
            });
        }
    }

    fn close_window(&mut self, id: Option<u8>) {
        self.emit(&Event::WindowClosed(id));
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
                    window.frozen = None;
                    window.cursor = None;
                }
            }
            None => {
                for window in self.windows.iter_mut().flatten() {
                    self.opened += 1;
                    window.opened = self.opened;
                    window.visible = true;
                    window.frozen = None;
                    window.cursor = None;
                }
            }
        }
    }

    fn draw_window(&mut self, id: u8) {
        self.opened += 1;
        let opened = self.opened;
        self.present(Some(id));
        if let Some(window) = self.window_mut(id) {
            window.opened = opened;
            window.frozen = Some(window.lines.clone());
        }
    }

    fn reveal(&mut self, id: u8) {
        if let Some(window) = self.window_mut(id) {
            window.visible = true;
            window.frozen = None;
        }
    }

    fn clear_window(&mut self, id: u8) {
        if let Some(window) = self.window_mut(id) {
            window.lines.clear();
            window.widths.clear();
            // Clearing flushes: a drawn window shows its text gone.
            if window.frozen.is_some() {
                window.frozen = Some(Vec::new());
            }
            window.pending_break = false;
            window.prompt = false;
            window.top = 0;
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

    fn page_full(&self, id: u8, ch: char) -> bool {
        self.windows
            .get(usize::from(id))
            .and_then(Option::as_ref)
            .is_some_and(|window| window.would_scroll(self.metrics.advance(ch)))
    }

    fn turn_page(&mut self, id: u8) {
        if let Some(window) = self.window_mut(id) {
            window.turn_page();
        }
    }

    fn auto_advance(&self, id: u8) -> Option<u32> {
        if !self.auto_text || !self.typewriter(id) {
            return None;
        }
        let window = self.windows.get(usize::from(id))?.as_ref()?;
        let read: u32 = window
            .lines
            .iter()
            .flat_map(|line| line.chars())
            .filter(|ch| !ch.is_whitespace() && !ch.is_control())
            .map(|ch| {
                if self.metrics.is_latin(ch) {
                    AUTO_LATIN_FRAMES
                } else {
                    AUTO_GLYPH_FRAMES
                }
            })
            .sum();
        Some(AUTO_BASE_FRAMES + read)
    }

    fn typewriter(&self, id: u8) -> bool {
        self.windows
            .get(usize::from(id))
            .and_then(Option::as_ref)
            .is_some_and(|window| window.style & TYPEWRITER_STYLE != 0)
    }

    fn portrait(&mut self, id: u8, character: u8, expression: u8) {
        let portrait = GameData::new(self.rom)
            .portrait(usize::from(character), usize::from(expression))
            .ok();
        if let Some(window) = self.window_mut(id) {
            if window.portrait.is_some() {
                window.portrait = None;
                window.next_portrait = portrait;
            } else {
                window.portrait = portrait;
            }
        }
    }

    fn prompt(&mut self, id: u8, visible: bool) {
        if let Some(window) = self.window_mut(id) {
            window.prompt = visible;
        }
    }

    fn play_sound(&mut self, id: u8) {
        self.emit(&Event::SoundRequested(usize::from(id)));
        self.sounds.push(id);
    }

    fn flag(&self, flag: u16) -> bool {
        self.flags.contains(&flag)
    }

    fn set_flag(&mut self, flag: u16, set: bool) {
        self.emit(&Event::FlagChanged { flag, set });
        if set {
            self.flags.insert(flag);
        } else {
            self.flags.remove(&flag);
        }
    }

    fn player_name(&self) -> String {
        self.player_name.clone()
    }

    fn reset(&mut self, _mode: u8) {
        self.windows.iter_mut().for_each(|slot| *slot = None);
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
                window.scroll_to(line);
            }
        }
    }

    fn translate(&self, table: &str, index: usize, offset: usize) -> Option<String> {
        let offset = Revision::of(self.rom).message_offset(table, index, offset);
        self.extensions
            .borrow()
            .translate_message(table, index, offset)
    }

    fn notify(&mut self, event: Event) {
        self.emit(&event);
    }

    fn fit_window(
        &self,
        table: &str,
        index: usize,
        id: u8,
        kind: u8,
        rect: (u8, u8, u8, u8),
    ) -> (u8, u8, u8, u8) {
        self.extensions
            .borrow()
            .fit_window(table, index, id, kind, rect)
            .unwrap_or(rect)
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

/// The auto text's mark over `window`'s top border, near its right corner:
/// AUTO in the Latin font's capitals on a plate of the window's background,
/// framed in its ink.
fn draw_auto_mark(frame: &mut Frame, window: &Window, skin: &WindowPainter, painter: &TextPainter) {
    let metrics = painter.metrics();
    let (background, ink) = (
        skin.palette().color(BACKGROUND_COLOR),
        skin.palette().color(INK_COLOR),
    );
    let width = metrics.plain_width(AUTO_MARK, 1) + 2 * (MARK_PADDING.0 + 1);
    let height = MARK_CAPITALS + 2 * (MARK_PADDING.1 + 1);
    let right = (window.x + window.width) * TILE;
    let (Some(left), Some(top)) = (
        right.checked_sub(AUTO_MARK_FROM_RIGHT + width),
        (window.y * TILE).checked_sub(MARK_PADDING.1 + 1),
    ) else {
        return;
    };
    for y in top..top + height {
        for x in left..left + width {
            let edge = y == top || y == top + height - 1 || x == left || x == left + width - 1;
            frame.set_pixel(x, y, if edge { ink } else { background });
        }
    }
    let text = (
        left + MARK_PADDING.0 + 1,
        (top + MARK_PADDING.1 + 1).saturating_sub(MARK_CAPITALS_TOP),
    );
    metrics.draw_plain(frame, text, AUTO_MARK, ink, 1);
}

/// A notice of the port's over the screen, such as the autosave's: `text`
/// in the small capitals with their glyphs' top-left corner at `(x, y)`,
/// no plate, light and see-through with a faint shadow under it, `level` 0
/// (unseen) to 16 (its fullest) so it can fade in and out; characters the
/// font lacks are skipped.
pub fn draw_notice(
    frame: &mut Frame,
    (x, y): (usize, usize),
    text: &str,
    painter: &TextPainter,
    level: u8,
) {
    let metrics = painter.metrics();
    let width = metrics.small_width(text) + NOTICE_SHADOW;
    let mut ink = Frame::new(width, NOTICE_ROWS, NOTICE_CLEAR);
    metrics.draw_small(&mut ink, (0, 0), text, NOTICE_INK);
    let level = u16::from(level.min(NOTICE_LEVELS));
    let inked = |column: usize, row: usize| ink.pixel(column, row) == Some(NOTICE_INK);
    let rows = NOTICE_ROWS + NOTICE_SHADOW;
    for row in 0..rows {
        for column in 0..width {
            let (color, alpha) = if inked(column, row) {
                (NOTICE_INK, NOTICE_INK_ALPHA)
            } else if column >= NOTICE_SHADOW
                && row >= NOTICE_SHADOW
                && inked(column - NOTICE_SHADOW, row - NOTICE_SHADOW)
            {
                (NOTICE_SHADE, NOTICE_SHADE_ALPHA)
            } else {
                continue;
            };
            let Some(base) = frame.pixel(x + column, y + row) else {
                continue;
            };
            let alpha = alpha * level / u16::from(NOTICE_LEVELS);
            frame.set_pixel(x + column, y + row, blend(base, color, alpha));
        }
    }
}

/// `over` laid on `base` at `alpha` of 256.
fn blend(base: Rgb, over: Rgb, alpha: u16) -> Rgb {
    let mix = |under: u8, top: u8| {
        let value = (u16::from(under) * (256 - alpha) + u16::from(top) * alpha) / 256;
        u8::try_from(value).unwrap_or(u8::MAX)
    };
    Rgb::new(
        mix(base.r, over.r),
        mix(base.g, over.g),
        mix(base.b, over.b),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn windows() -> ScriptWindows<'static> {
        ScriptWindows::new(&[], "アトレー")
    }

    struct Watcher(std::rc::Rc<std::cell::RefCell<Vec<Event>>>);

    impl crate::extension::Extension for Watcher {
        fn name(&self) -> &'static str {
            "watcher"
        }
        fn on_event(&mut self, event: &Event) {
            self.0.borrow_mut().push(event.clone());
        }
        fn translate_message(&self, _: &str, index: usize, _: usize) -> Option<String> {
            (index == 3).then(|| "hi".to_owned())
        }
    }

    #[test]
    fn the_host_raises_events_and_asks_its_extensions() {
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let extensions = SharedExtensions::default();
        extensions
            .borrow_mut()
            .insert(Box::new(Watcher(seen.clone())));
        let mut host = windows();
        host.set_extensions(extensions);
        host.open_window(1, 0x20, (0, 0, 10, 4), 0);
        host.play_sound(0x47);
        host.set_flag(9, true);
        host.close_window(None);
        assert_eq!(
            *seen.borrow(),
            [
                Event::WindowOpened {
                    id: 1,
                    rect: (0, 0, 10, 4),
                    kind: 0x20
                },
                Event::SoundRequested(0x47),
                Event::FlagChanged { flag: 9, set: true },
                Event::WindowClosed(None),
            ]
        );
        assert_eq!(host.translate("dialogue", 3, 0), Some("hi".to_owned()));
        assert_eq!(host.translate("dialogue", 4, 0), None);
        assert_eq!(
            host.fit_window("dialogue", 3, 1, 0x20, (0, 0, 10, 4)),
            (0, 0, 10, 4)
        );
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
    fn a_full_window_reports_that_the_next_character_would_scroll() {
        let mut host = windows();
        host.open_window(1, 0x10, (0, 12, 6, 8), 1);
        for ch in "abcdefghij".chars() {
            host.put_char(1, ch);
        }
        assert!(!host.page_full(1, 'k'));
        host.put_char(1, 'k');
        host.put_char(1, 'l');
        assert!(host.page_full(1, 'm'));
        host.turn_page(1);
        host.put_char(1, 'm');
        let window = host.windows()[1].as_ref().unwrap();
        assert_eq!(window.lines, ["abcd", "m"]);
        host.line_break(1);
        host.put_char(1, 'n');
        host.line_break(1);
        assert!(host.page_full(1, 'o'));
        host.turn_page(1);
        host.put_char(1, 'o');
        let window = host.windows()[1].as_ref().unwrap();
        assert_eq!(window.lines, ["abcd", "o"]);
        host.clear_window(1);
        assert!(!host.page_full(1, 'o'));
    }

    #[test]
    fn windows_open_present_clear_and_close() {
        let mut host = windows();
        host.open_window(0, 0x10, (0, 12, 8, 8), 0);
        host.open_window(1, 0x10, (7, 12, 23, 8), 1);
        assert!(!host.windows()[0].as_ref().unwrap().visible);
        host.present(None);
        assert!(host.windows()[0].as_ref().unwrap().visible);
        assert!(ScriptWindows::shares_left_border(
            host.windows(),
            1,
            host.windows()[1].as_ref().unwrap()
        ));
        assert!(!ScriptWindows::shares_left_border(
            host.windows(),
            0,
            host.windows()[0].as_ref().unwrap()
        ));
        host.open_window(2, 0x20, (0, 0, 18, 12), 4);
        host.open_window(3, 0x21, (17, 0, 13, 12), 4);
        host.present(None);
        assert!(!ScriptWindows::shares_left_border(
            host.windows(),
            3,
            host.windows()[3].as_ref().unwrap()
        ));
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
        host.set_flags([9, 3]);
        assert_eq!(host.flags(), vec![3, 9]);
        host.play_sound(0x41);
        assert_eq!(host.take_sounds(), [0x41]);
        assert!(host.take_sounds().is_empty());
        host.reset(1);
        assert_eq!(host.player_name(), "アトレー");
    }

    #[test]
    fn padding_reaches_the_cell_after_narrow_letters() {
        let mut host = ScriptWindows::new(&[], "");
        host.open_window(2, 0x21, (15, 4, 15, 12), 4);
        host.put_char(2, '★');
        host.pad_to(2, 3);
        let window = |host: &ScriptWindows<'_>| host.windows()[2].clone().unwrap();
        assert_eq!(window(&host).lines[0], "★\u{3000}\u{3000}");
        if let Some(Some(window)) = host.windows.get_mut(2) {
            window.widths[0] = 13;
        }
        host.pad_to(2, 10);
        assert_eq!(window(&host).widths[0], 10 * CELL_WIDTH);
        assert!(window(&host).lines[0].ends_with(cell_stop_mark(10)));
    }

    #[test]
    fn a_long_menu_keeps_its_lines_and_scrolls_to_the_cursor() {
        let mut host = ScriptWindows::new(&[], "");
        host.open_window(2, 0x21, (11, 0, 13, 8), 4);
        for line in 0..6 {
            host.put_char(2, char::from(b'a' + line));
            host.line_break(2);
        }
        let window = |host: &ScriptWindows<'_>| host.windows()[2].clone().unwrap();
        assert_eq!(host.menu_lines(2), 6);
        assert_eq!(window(&host).shown_lines(), ["a", "b", "c"]);
        assert_eq!(window(&host).hidden_lines(), (false, true));
        assert!(!window(&host).would_scroll(8));
        host.set_cursor(2, Some(4));
        assert_eq!(window(&host).shown_lines(), ["c", "d", "e"]);
        assert_eq!(window(&host).hidden_lines(), (true, true));
        host.set_cursor(2, Some(5));
        assert_eq!(window(&host).hidden_lines(), (true, false));
        host.set_cursor(2, Some(1));
        assert_eq!(window(&host).shown_lines(), ["b", "c", "d"]);
        host.clear_window(2);
        assert_eq!(window(&host).top, 0);
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

    #[test]
    fn a_line_too_long_for_its_column_is_cut_with_a_mark() {
        let mut host = windows();
        host.set_metrics(TextMetrics::standard());
        host.open_window(1, 0x21, (0, 0, 20, 4), 0);
        for ch in "Aslt. Beam C.".chars() {
            host.put_char(1, ch);
        }
        let full = host.windows()[1].as_ref().unwrap().widths[0];
        let spaced = full - TextMetrics::standard().advance('C');
        host.clip_line(1, spaced, '.');
        let window = host.windows()[1].as_ref().unwrap();
        assert_eq!(window.lines.last().map(String::as_str), Some("Aslt. Beam."));
        assert!(window.widths.last().is_some_and(|width| *width <= spaced));
        for ch in "Short".chars() {
            host.put_char(1, ch);
        }
        host.line_break(1);
        host.put_char(1, 'A');
        host.clip_line(1, 48, '.');
        let window = host.windows()[1].as_ref().unwrap();
        assert_eq!(window.lines.last().map(String::as_str), Some("A"));
    }

    #[test]
    fn a_notice_shades_its_letters_over_the_screen_and_fades_with_its_level() {
        let index = formats::font::GlyphIndex::parse(&[]).unwrap();
        let painter = TextPainter::new(&[], index, None);
        let gray = Rgb::new(100, 100, 100);
        let draw = |level| {
            let mut frame = Frame::new(40, 16, gray);
            draw_notice(&mut frame, (2, 2), "Ab", &painter, level);
            frame
        };
        let colors = |frame: &Frame| {
            (0..16)
                .flat_map(|y| (0..40).map(move |x| (x, y)))
                .filter_map(|(x, y)| frame.pixel(x, y))
                .filter(|color| *color != gray)
                .collect::<Vec<_>>()
        };
        assert!(colors(&draw(0)).is_empty());
        let full = colors(&draw(NOTICE_LEVELS));
        assert!(full.contains(&blend(gray, NOTICE_INK, NOTICE_INK_ALPHA)));
        assert!(full.contains(&blend(gray, NOTICE_SHADE, NOTICE_SHADE_ALPHA)));
        assert!(full.iter().all(|color| *color != NOTICE_INK));
        let half = colors(&draw(NOTICE_LEVELS / 2));
        assert!(half.contains(&blend(gray, NOTICE_INK, NOTICE_INK_ALPHA / 2)));
    }

    #[test]
    fn a_text_box_goes_on_by_itself_only_with_the_auto_text() {
        let mut host = windows();
        host.open_window(1, 0x10, (0, 12, 30, 8), 1);
        host.open_window(2, 0x10, (0, 0, 10, 4), 0);
        for ch in "はい。".chars() {
            host.put_char(1, ch);
        }
        host.present(None);
        assert_eq!(host.auto_advance(1), None);
        host.set_auto_text(true);
        assert!(host.shows_text_box());
        let read = 2 * AUTO_GLYPH_FRAMES + AUTO_GLYPH_FRAMES;
        assert_eq!(host.auto_advance(1), Some(AUTO_BASE_FRAMES + read));
        assert_eq!(host.auto_advance(2), None);
    }
}
