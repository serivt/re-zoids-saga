//! The question asked before closing on Escape, from the launcher's screen
//! or the game, so that a key pressed by mistake closes nothing: No is
//! chosen at first, left and right switch the answer, X or Return answers,
//! Z or Escape again keeps playing; a touch or a click on an answer gives
//! it.
//!
//! Source of knowledge: this project's own design; the original has no
//! way to close.

use game_core::TextMetrics;
use game_core::port_text::{
    LAUNCHER_NO, LAUNCHER_QUIT_QUESTION, LAUNCHER_QUIT_UNSAVED, LAUNCHER_YES,
};
use platform::{Button, Frame, Input, Rgb};

use crate::front::{DIM, TEXT, TITLE_COLOR, WARNING, draw_panel};

/// The question's box, across the middle of the screen.
const BOX: (usize, usize, usize, usize) = (8, 52, 224, 56);
const QUESTION_Y: usize = BOX.1 + 9;
const WARNING_Y: usize = QUESTION_Y + 12;
const ANSWERS_Y: usize = BOX.1 + BOX.3 - 17;
/// Pixels between the two answers' centers.
const ANSWERS_APART: usize = 72;
/// How far above and below an answer's text a tap still gives it.
const ANSWER_REACH: usize = 6;
/// How much of its light the screen keeps behind the box, in eighths.
const SHADE_EIGHTHS: u16 = 3;

/// The question, open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuitPrompt {
    yes: bool,
    unsaved: bool,
    previous: Input,
}

impl QuitPrompt {
    /// The question, asked while `held` are held, which it then ignores
    /// until released; `unsaved` adds that the game's progress not saved
    /// will be lost.
    #[must_use]
    pub fn new(held: Input, unsaved: bool) -> Self {
        Self {
            yes: false,
            unsaved,
            previous: held,
        }
    }

    /// A frame of the buttons: the answer once given, `true` to close.
    pub fn update(&mut self, input: Input) -> Option<bool> {
        let pressed = |button| input.is_held(button) && !self.previous.is_held(button);
        let (left, right) = (pressed(Button::Left), pressed(Button::Right));
        let (chosen, back) = (
            pressed(Button::A) || pressed(Button::Start),
            pressed(Button::B),
        );
        self.previous = input;
        if left || right {
            self.yes = !self.yes;
        }
        if back {
            return Some(false);
        }
        chosen.then_some(self.yes)
    }

    /// The answer under a touch or a click at (`x`, `y`) of the frame,
    /// `true` to close, or `None` away from both.
    #[must_use]
    pub fn answer_at(x: i32, y: i32) -> Option<bool> {
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        let middle = BOX.0 + BOX.2 / 2;
        let rows = ANSWERS_Y.saturating_sub(ANSWER_REACH)..ANSWERS_Y + 8 + ANSWER_REACH;
        if !rows.contains(&y) {
            return None;
        }
        if (middle - ANSWERS_APART..middle).contains(&x) {
            Some(true)
        } else if (middle..middle + ANSWERS_APART).contains(&x) {
            Some(false)
        } else {
            None
        }
    }

    /// Draws the question over `frame`, which it darkens, with the texts
    /// `text` gives for its keys.
    pub fn draw(self, frame: &mut Frame, metrics: &TextMetrics, text: &dyn Fn(&str) -> String) {
        shade(frame);
        draw_panel(frame, BOX);
        let middle = BOX.0 + BOX.2 / 2;
        let centered = |frame: &mut Frame, center: usize, y: usize, line: &str, color: Rgb| {
            let x = center.saturating_sub(metrics.plain_width(line, 1) / 2);
            metrics.draw_plain(frame, (x, y), line, color, 1);
        };
        let question_y = if self.unsaved {
            QUESTION_Y
        } else {
            usize::midpoint(QUESTION_Y, WARNING_Y)
        };
        let question = text(LAUNCHER_QUIT_QUESTION);
        centered(frame, middle, question_y, &question, TEXT);
        if self.unsaved {
            centered(
                frame,
                middle,
                WARNING_Y,
                &text(LAUNCHER_QUIT_UNSAVED),
                WARNING,
            );
        }
        for (answer, key, center) in [
            (true, LAUNCHER_YES, middle - ANSWERS_APART / 2),
            (false, LAUNCHER_NO, middle + ANSWERS_APART / 2),
        ] {
            let label = text(key);
            let chosen = answer == self.yes;
            centered(
                frame,
                center,
                ANSWERS_Y,
                &label,
                if chosen { TEXT } else { DIM },
            );
            if chosen {
                let x = center.saturating_sub(metrics.plain_width(&label, 1) / 2 + 8);
                metrics.draw_plain(frame, (x, ANSWERS_Y), ">", TITLE_COLOR, 1);
            }
        }
    }
}

/// Darkens every pixel of `frame`.
fn shade(frame: &mut Frame) {
    let dim = |channel: u8| u8::try_from(u16::from(channel) * SHADE_EIGHTHS / 8).unwrap_or(channel);
    for y in 0..frame.height() {
        for x in 0..frame.width() {
            if let Some(color) = frame.pixel(x, y) {
                frame.set_pixel(x, y, Rgb::new(dim(color.r), dim(color.g), dim(color.b)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(button: Button) -> Input {
        Input::default().with(button)
    }

    #[test]
    fn no_is_the_first_answer_and_held_buttons_wait_for_release() {
        let mut prompt = QuitPrompt::new(press(Button::A), false);
        assert_eq!(prompt.update(press(Button::A)), None);
        assert_eq!(prompt.update(Input::default()), None);
        assert_eq!(prompt.update(press(Button::A)), Some(false));
        let mut prompt = QuitPrompt::new(Input::default(), true);
        assert_eq!(prompt.update(press(Button::Right)), None);
        assert_eq!(prompt.update(press(Button::Start)), Some(true));
        let mut prompt = QuitPrompt::new(Input::default(), true);
        assert_eq!(prompt.update(press(Button::Left)), None);
        assert_eq!(prompt.update(press(Button::B)), Some(false));
    }

    #[test]
    fn a_tap_on_an_answer_gives_it() {
        let pixel = |value: usize| i32::try_from(value).unwrap_or_default();
        let (y, middle) = (pixel(ANSWERS_Y + 4), pixel(BOX.0 + BOX.2 / 2));
        assert_eq!(QuitPrompt::answer_at(middle - 36, y), Some(true));
        assert_eq!(QuitPrompt::answer_at(middle + 36, y), Some(false));
        assert_eq!(QuitPrompt::answer_at(middle - 36, pixel(QUESTION_Y)), None);
        assert_eq!(QuitPrompt::answer_at(2, y), None);
        assert_eq!(QuitPrompt::answer_at(-1, -1), None);
    }
}
