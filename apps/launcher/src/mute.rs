//! The mute, a port feature: its button (M by default) turns the sound off,
//! and pressed again turns it back on, in either mode, with a mark at the
//! frame's top right while it is off. The game never sees the button and
//! goes on making its sound, which is only not heard; the setting lasts
//! until the launcher closes.
//!
//! Source of knowledge: this project's own design.

use game_core::TextMetrics;
use gba_runtime::ppu::SCREEN_WIDTH;
use platform::{Button, Frame, Input, Rgb};

/// The mark while the sound is off, in the port's small capitals; the
/// binding screens name the button with it too.
pub const MARK: &str = "MUTE";
/// Pixels between the mark and the frame's right edge, and between it and
/// a mark drawn to its left.
const MARK_MARGIN: usize = 4;
const MARK_INK: Rgb = Rgb::new(248, 248, 248);
const MARK_SHADOW: Rgb = Rgb::new(0, 0, 0);

/// Whether the sound is off, and the button's state on the frame before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mute {
    on: bool,
    held: bool,
}

impl Mute {
    /// Takes this frame's buttons: a press of the mute's button turns the
    /// sound off or on again; returns the buttons without it.
    pub fn take(&mut self, input: Input) -> Input {
        let held = input.is_held(Button::Mute);
        if held && !self.held {
            self.on = !self.on;
        }
        self.held = held;
        input.without(Button::Mute)
    }

    /// The volume the sound plays at: `volume`, or nothing while muted.
    #[must_use]
    pub fn volume(self, volume: i32) -> i32 {
        if self.on { 0 } else { volume }
    }

    /// Draws the mark while muted, white over a black shadow, at the
    /// frame's top right; returns the pixels it takes there, for the marks
    /// drawn to its left.
    pub fn draw(self, frame: &mut Frame, metrics: &TextMetrics) -> usize {
        if !self.on {
            return 0;
        }
        let width = metrics.small_width(MARK);
        let x = SCREEN_WIDTH.saturating_sub(MARK_MARGIN + width + 1);
        metrics.draw_small(frame, (x + 1, 1), MARK, MARK_SHADOW);
        metrics.draw_small(frame, (x, 0), MARK, MARK_INK);
        width + MARK_MARGIN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pressed() -> Input {
        Input::default().with(Button::Mute).with(Button::A)
    }

    #[test]
    fn each_press_turns_the_sound_off_or_on_and_the_game_never_sees_it() {
        let mut mute = Mute::default();
        assert_eq!(mute.take(pressed()), Input::default().with(Button::A));
        assert_eq!(mute.volume(80), 0);
        mute.take(pressed());
        assert_eq!(mute.volume(80), 0, "held, it does not switch again");
        mute.take(Input::default());
        mute.take(pressed());
        assert_eq!(mute.volume(80), 80);
    }

    #[test]
    fn the_mark_shows_only_while_muted() {
        let metrics = TextMetrics::standard();
        let mut frame = Frame::new(240, 160, Rgb::default());
        let mut mute = Mute::default();
        assert_eq!(mute.draw(&mut frame, &metrics), 0);
        assert_eq!(frame, Frame::new(240, 160, Rgb::default()));
        mute.take(pressed());
        assert!(mute.draw(&mut frame, &metrics) > MARK_MARGIN);
        let lit = (200..240).any(|x| (0..12).any(|y| frame.pixel(x, y) == Some(MARK_INK)));
        assert!(lit);
    }
}
