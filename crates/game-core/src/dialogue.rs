//! The talk box: what a character says when the player speaks to it.
//!
//! Matches the original's box for characters on the map: the bottom eight
//! tile rows across the full width, no portrait, the text from cell (1, 13)
//! with the speaker's name on the first line. Text appears one character per
//! frame starting four frames after the box opens, line breaks costing
//! nothing; 22 frames after the last character a prompt blinks on the bottom
//! border, 21 frames on and 21 off. A shows the rest of a page at once,
//! then turns the page or closes the box.

use formats::script_text::Script;
use localization::{TextArea, monospace};
use platform::{Button, Frame, Input};

use crate::{TextPainter, WindowPainter};

/// Name the player carries when none was entered.
pub const DEFAULT_PLAYER_NAME: &str = "アトレー";
/// Text cells of the talk box: the speaker line and two more.
pub const TALK_TEXT_AREA: TextArea = TextArea {
    columns: 28,
    rows: 3,
};
const BOX_ROW: usize = 12;
const BOX_ROWS: usize = 8;
const BOX_COLUMNS: usize = 30;
const TEXT_COLUMN: usize = 1;
const LINE_HEIGHT: i32 = 16;
const CELL_WIDTH: i32 = 8;
const OPEN_FRAMES: u32 = 4;
const PROMPT_DELAY: u32 = 22;
const PROMPT_HALF_PERIOD: u32 = 21;
const PROMPT_CELL: (usize, usize) = (28, 19);
const NAME_PLACEHOLDER: &str = "{name}";

/// A conversation shown page by page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TalkBox {
    pages: Vec<Vec<String>>,
    page: usize,
    frames: u32,
    revealed: usize,
    previous: Input,
}

impl TalkBox {
    /// Opens the box on the messages of `script`, naming the player `name`.
    #[must_use]
    pub fn new(script: &Script, name: &str) -> Self {
        let pages = script
            .message_texts()
            .iter()
            .map(|text| {
                TALK_TEXT_AREA
                    .layout(&text.replace(NAME_PLACEHOLDER, name), monospace)
                    .visible_lines()
                    .to_vec()
            })
            .collect();
        Self {
            pages,
            page: 0,
            frames: 0,
            revealed: 0,
            previous: Input::default(),
        }
    }

    /// The lines of the page being shown.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        self.pages.get(self.page).map_or(&[], Vec::as_slice)
    }

    /// Characters of the current page shown so far.
    #[must_use]
    pub fn revealed(&self) -> usize {
        self.revealed
    }

    /// Whether the current page is fully shown.
    #[must_use]
    pub fn page_complete(&self) -> bool {
        self.revealed >= self.page_length()
    }

    /// Whether the prompt is lit this frame.
    #[must_use]
    pub fn prompt_visible(&self) -> bool {
        self.frames_since_complete().is_some_and(|frames| {
            frames >= PROMPT_DELAY && (frames - PROMPT_DELAY) / PROMPT_HALF_PERIOD % 2 == 0
        })
    }

    /// Advances one frame; returns `true` when the box has closed.
    pub fn update(&mut self, input: Input) -> bool {
        let pressed = input.is_held(Button::A) && !self.previous.is_held(Button::A);
        self.previous = input;
        if self.pages.is_empty() {
            return true;
        }
        self.frames += 1;
        if pressed {
            if self.page_complete() {
                self.page += 1;
                self.frames = 0;
                self.revealed = 0;
                return self.page >= self.pages.len();
            }
            self.revealed = self.page_length();
            return false;
        }
        if self.frames > OPEN_FRAMES && !self.page_complete() {
            self.revealed += 1;
        }
        false
    }

    /// Draws the box and the revealed text over `frame`.
    pub fn draw(&self, frame: &mut Frame, window: &WindowPainter, painter: &TextPainter) {
        window.draw_window(frame, 0, BOX_ROW, BOX_COLUMNS, BOX_ROWS);
        let mut left = self.revealed;
        for (index, line) in self.lines().iter().enumerate() {
            let shown: String = line.chars().take(left).collect();
            left = left.saturating_sub(line.chars().count());
            let x = i32::try_from(TEXT_COLUMN).unwrap_or(0) * CELL_WIDTH;
            let y = i32::try_from(BOX_ROW + 1).unwrap_or(0) * CELL_WIDTH
                + i32::try_from(index).unwrap_or(0) * LINE_HEIGHT;
            painter.draw(frame, x, y, &shown, window.palette());
        }
        if self.prompt_visible() {
            window.draw_prompt(frame, PROMPT_CELL.0, PROMPT_CELL.1);
        }
    }

    fn page_length(&self) -> usize {
        self.lines().iter().map(|line| line.chars().count()).sum()
    }

    fn frames_since_complete(&self) -> Option<u32> {
        let length = u32::try_from(self.page_length()).ok()?;
        self.page_complete()
            .then(|| self.frames.saturating_sub(OPEN_FRAMES + length))
    }
}

#[cfg(test)]
mod tests {
    use formats::script_text::{Element, Piece};

    use super::*;

    fn script(messages: &[&str]) -> Script {
        Script {
            elements: messages
                .iter()
                .map(|text| {
                    let pieces = text
                        .split('\n')
                        .enumerate()
                        .flat_map(|(i, line)| {
                            let mut pieces = Vec::new();
                            if i > 0 {
                                pieces.push(Piece::LineBreak);
                            }
                            if line == "{name}" {
                                pieces.push(Piece::PlayerName);
                            } else {
                                pieces.push(Piece::Text(line.to_owned()));
                            }
                            pieces
                        })
                        .collect();
                    Element::Message(pieces)
                })
                .collect(),
        }
    }

    fn pressed_a() -> Input {
        Input::default().with(Button::A)
    }

    #[test]
    fn reveals_one_character_per_frame_after_opening() {
        let mut talk = TalkBox::new(&script(&["王妃\n「こんにちは"]), "X");
        assert_eq!(talk.lines(), ["王妃", "「こんにちは"]);
        for _ in 0..OPEN_FRAMES {
            assert!(!talk.update(Input::default()));
        }
        assert_eq!(talk.revealed(), 0);
        talk.update(Input::default());
        assert_eq!(talk.revealed(), 1);
        for _ in 0..7 {
            talk.update(Input::default());
        }
        assert_eq!(talk.revealed(), 8);
        assert!(talk.page_complete());
        talk.update(Input::default());
        assert_eq!(talk.revealed(), 8);
    }

    #[test]
    fn blinks_the_prompt_after_the_page_completes() {
        let mut talk = TalkBox::new(&script(&["ab"]), "X");
        for _ in 0..OPEN_FRAMES + 2 {
            talk.update(Input::default());
        }
        assert!(talk.page_complete());
        for _ in 0..PROMPT_DELAY {
            assert!(!talk.prompt_visible());
            talk.update(Input::default());
        }
        for _ in 0..PROMPT_HALF_PERIOD {
            assert!(talk.prompt_visible());
            talk.update(Input::default());
        }
        for _ in 0..PROMPT_HALF_PERIOD {
            assert!(!talk.prompt_visible());
            talk.update(Input::default());
        }
        assert!(talk.prompt_visible());
    }

    #[test]
    fn a_completes_the_page_then_turns_pages_and_closes() {
        let mut talk = TalkBox::new(&script(&["first page", "second"]), "X");
        for _ in 0..6 {
            talk.update(Input::default());
        }
        assert!(!talk.update(pressed_a()));
        assert!(talk.page_complete());
        assert!(!talk.update(Input::default()));
        assert!(!talk.update(pressed_a()));
        assert_eq!(talk.lines(), ["second"]);
        assert_eq!(talk.revealed(), 0);
        assert!(!talk.update(pressed_a()));
        assert_eq!(talk.revealed(), 0);
        assert!(!talk.update(Input::default()));
        assert!(!talk.update(pressed_a()));
        assert!(talk.page_complete());
        assert!(!talk.update(Input::default()));
        assert!(talk.update(pressed_a()));
    }

    #[test]
    fn substitutes_the_player_name_and_wraps_long_lines() {
        let talk = TalkBox::new(
            &script(&["{name}\n「abcdefghijklmnopqrstuvwxyz0123"]),
            "アトレー",
        );
        assert_eq!(talk.lines()[0], "アトレー");
        assert_eq!(talk.lines()[1].chars().count(), 28);
        assert_eq!(talk.lines()[2], "123");
        assert!(TalkBox::new(&Script::default(), "X").update(Input::default()));
    }
}
