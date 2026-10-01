//! The enhanced mode's fast forward, a port feature: while its button
//! (Space, the gamepad's right stick or the on-screen pad's `>>` by
//! default) is held the game plays two to four frames for each one shown,
//! the speed chosen on the game mode's screen or in the pause menu's
//! コンフィグ, with a mark at the frame's top right; letting go plays it at
//! its pace again. The game never sees the button. The sound of the
//! frames the audio queue has no room for is dropped, so it keeps its
//! pitch and skips ahead.
//!
//! Source of knowledge: this project's own design.

use game_core::TextMetrics;
use gba_runtime::ppu::SCREEN_WIDTH;
use platform::{Button, Frame, Input, Rgb};

/// Pixels between the mark and the frame's right edge.
const MARK_MARGIN: usize = 4;
const MARK_INK: Rgb = Rgb::new(248, 248, 248);
const MARK_SHADOW: Rgb = Rgb::new(0, 0, 0);

/// The fast forward's state on a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FastForward {
    /// Frames played for each one shown, while the button is held.
    speed: u32,
    on: bool,
}

impl FastForward {
    /// Takes this frame's buttons with the speed chosen now (1 in the
    /// classic mode): the fast forward plays while its button is held,
    /// unless `paused`; returns the buttons without it.
    pub fn take(&mut self, input: Input, speed: u32, paused: bool) -> Input {
        self.speed = speed;
        self.on = input.is_held(Button::FastForward) && speed > 1 && !paused;
        input.without(Button::FastForward)
    }

    /// The frames to play for the `due` ones the pacer asks for.
    #[must_use]
    pub fn frames(self, due: u32) -> u32 {
        if self.on { due * self.speed } else { due }
    }

    /// Draws the mark while on: `>>` and the speed in the port's small
    /// capitals, white over a black shadow.
    pub fn draw(self, frame: &mut Frame, metrics: &TextMetrics) {
        if !self.on {
            return;
        }
        let text = format!(">>{}X", self.speed);
        let width = metrics.small_width(&text);
        let x = SCREEN_WIDTH.saturating_sub(MARK_MARGIN + width + 1);
        metrics.draw_small(frame, (x + 1, 1), &text, MARK_SHADOW);
        metrics.draw_small(frame, (x, 0), &text, MARK_INK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held() -> Input {
        Input::default().with(Button::FastForward).with(Button::A)
    }

    #[test]
    fn it_plays_faster_only_while_held_and_the_game_never_sees_the_button() {
        let mut fast = FastForward::default();
        assert_eq!(
            fast.take(held(), 3, false),
            Input::default().with(Button::A)
        );
        assert_eq!(fast.frames(2), 6);
        fast.take(held(), 4, false);
        assert_eq!(fast.frames(1), 4, "a speed changed meanwhile holds at once");
        fast.take(Input::default(), 4, false);
        assert_eq!(fast.frames(2), 2);
    }

    #[test]
    fn it_stays_at_its_pace_while_paused_or_in_the_classic_mode() {
        let mut fast = FastForward::default();
        fast.take(held(), 2, true);
        assert_eq!(fast.frames(1), 1);
        fast.take(held(), 1, false);
        assert_eq!(fast.frames(1), 1);
    }

    #[test]
    fn its_mark_shows_only_while_held() {
        let metrics = TextMetrics::standard();
        let mut frame = Frame::new(240, 160, Rgb::default());
        let mut fast = FastForward::default();
        fast.take(Input::default(), 4, false);
        fast.draw(&mut frame, &metrics);
        assert_eq!(frame, Frame::new(240, 160, Rgb::default()));
        fast.take(held(), 4, false);
        fast.draw(&mut frame, &metrics);
        let lit = (200..240).any(|x| (0..12).any(|y| frame.pixel(x, y) == Some(MARK_INK)));
        assert!(lit);
    }
}
