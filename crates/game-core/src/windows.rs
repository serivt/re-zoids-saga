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

use crate::{ScriptHost, TextPainter, WindowPainter, draw_sprite};

/// Name the player carries when none was entered.
pub const DEFAULT_PLAYER_NAME: &str = "アトレー";
const WINDOWS: usize = 8;
const TILE: usize = 8;
const LINE_HEIGHT: usize = 16;
const TYPEWRITER_STYLE: u8 = 1;
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
    /// Portrait shown, if any.
    pub portrait: Option<Portrait>,
    /// Whether the "more" prompt is lit.
    pub prompt: bool,
    /// Whether the window has been presented.
    pub visible: bool,
}

impl Window {
    /// Text cells per line.
    #[must_use]
    pub fn columns(&self) -> usize {
        self.width.saturating_sub(2)
    }

    /// Text lines that fit.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.height.saturating_sub(2) / 2
    }

    fn put_char(&mut self, ch: char) {
        if self
            .lines
            .last()
            .is_some_and(|line| line.chars().count() >= self.columns())
        {
            self.line_break();
        }
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if let Some(line) = self.lines.last_mut() {
            line.push(ch);
        }
    }

    fn line_break(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.lines.push(String::new());
        while self.lines.len() > self.rows().max(1) {
            self.lines.remove(0);
        }
    }
}

/// The script windows of a scene, drawn with the game's skin and font.
pub struct ScriptWindows<'rom> {
    rom: &'rom [u8],
    windows: Vec<Option<Window>>,
    flags: HashSet<u16>,
    player_name: String,
    sounds: Vec<u8>,
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
        }
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

    /// Sounds requested so far, oldest first; clearing is the caller's job.
    pub fn take_sounds(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.sounds)
    }

    /// Draws every presented window over `frame`.
    pub fn draw(&self, frame: &mut Frame, skin: &WindowPainter, painter: &TextPainter) {
        for window in self.windows.iter().flatten() {
            if !window.visible {
                continue;
            }
            skin.draw_window(frame, window.x, window.y, window.width, window.height);
        }
        for (index, window) in self.windows.iter().enumerate() {
            let Some(window) = window.as_ref().filter(|window| window.visible) else {
                continue;
            };
            if self.shares_left_border(index, window) {
                skin.draw_divider(frame, window.x, window.y, window.height);
            }
            let origin = (pixels(window.x + 1), pixels(window.y + 1));
            if let Some(portrait) = &window.portrait {
                draw_sprite(
                    frame,
                    origin.0,
                    origin.1,
                    &portrait.image,
                    &portrait.palette,
                );
            }
            for (row, line) in window.lines.iter().enumerate() {
                let y = origin.1 + i32::try_from(row * LINE_HEIGHT).unwrap_or(i32::MAX);
                painter.draw(frame, origin.0, y, line, skin.palette());
            }
            if window.prompt {
                skin.draw_prompt(
                    frame,
                    (window.x + window.width).saturating_sub(PROMPT_FROM_RIGHT),
                    window.y + window.height - 1,
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
        if let Some(slot) = self.windows.get_mut(usize::from(id)) {
            *slot = Some(Window {
                x: usize::from(rect.0),
                y: usize::from(rect.1),
                width: usize::from(rect.2),
                height: usize::from(rect.3),
                kind,
                style,
                lines: Vec::new(),
                portrait: None,
                prompt: false,
                visible: false,
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
                }
            }
            None => self
                .windows
                .iter_mut()
                .flatten()
                .for_each(|window| window.visible = true),
        }
    }

    fn clear_window(&mut self, id: u8) {
        if let Some(window) = self.window_mut(id) {
            window.lines.clear();
            window.prompt = false;
        }
    }

    fn put_char(&mut self, id: u8, ch: char) {
        if let Some(window) = self.window_mut(id) {
            window.put_char(ch);
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
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn windows() -> ScriptWindows<'static> {
        ScriptWindows::new(&[], "アトレー")
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
}
