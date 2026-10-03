//! The frames' pace in the browser: the game runs at the hardware's rate,
//! about 59.73 frames a second, whatever the screen's refresh. The page
//! calls the loop once for each picture the screen shows (60, 120 or 144 a
//! second), with the time; the loop runs the frames due since the last
//! call, none on a fast screen's extra pictures, at most a few after a
//! hold up (the tab hidden), which is skipped rather than caught up.
//!
//! Source of knowledge: this project's own design, as the launcher's pace
//! (`apps/launcher/src/pacing.rs`); the frame's length is the GBA's.

/// A frame's length in milliseconds: 280 896 cycles of the 16.78 MHz clock.
pub const FRAME_MILLIS: f64 = 16.743;
/// The most frames run in one go.
const MOST_FRAMES: u32 = 4;

/// When the next frame is due.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pacer {
    next: Option<f64>,
}

impl Pacer {
    /// A pace whose first frame is due at the first call.
    #[must_use]
    pub const fn new() -> Self {
        Self { next: None }
    }

    /// The frames due at `now` (milliseconds), counted as run.
    pub fn due(&mut self, now: f64) -> u32 {
        let mut next = self.next.unwrap_or(now);
        let mut frames = 0;
        while next <= now && frames < MOST_FRAMES {
            next += FRAME_MILLIS;
            frames += 1;
        }
        if next <= now {
            next = now + FRAME_MILLIS;
        }
        self.next = Some(next);
        frames
    }
}

impl Default for Pacer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_due_each_frame_s_length_whatever_the_screen() {
        let mut pacer = Pacer::new();
        assert_eq!(pacer.due(1000.0), 1);
        assert_eq!(pacer.due(1008.3), 0, "a 120 Hz screen's extra picture");
        assert_eq!(pacer.due(1016.7), 0);
        assert_eq!(pacer.due(1016.8), 1);
        assert_eq!(pacer.due(5000.0), MOST_FRAMES, "a hold up is not caught up");
        assert_eq!(pacer.due(5000.0 + FRAME_MILLIS), 1);
    }
}
