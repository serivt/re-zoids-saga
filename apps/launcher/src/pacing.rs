//! The frames' pace: the game runs at the hardware's rate, about 59.73
//! frames a second, whatever the screen's refresh. The display waits for
//! the screen's vertical blank before it shows a picture (vsync), so a
//! picture is never shown half drawn; each time around, the loop runs the
//! frames due since the last, usually one, and waits for the next only
//! when none is due, which also paces a display without vsync.
//!
//! Source of knowledge: this project's own design; the frame's length is
//! the GBA's (280 896 cycles of its 16.78 MHz clock).

use std::time::{Duration, Instant};

/// The most frames run in one go; a longer hold up (a window dragged, the
/// machine asleep) is skipped rather than caught up.
const MOST_FRAMES: u32 = 4;

/// When the next frame is due.
pub struct Pacer {
    frame: Duration,
    next: Instant,
}

impl Pacer {
    /// Paces frames of `frame`, the first due at `now`.
    #[must_use]
    pub fn new(frame: Duration, now: Instant) -> Self {
        Self { frame, next: now }
    }

    /// The frames due at `now`, counted as run: none before the next is
    /// due, at most [`MOST_FRAMES`].
    pub fn due(&mut self, now: Instant) -> u32 {
        let mut frames = 0;
        while self.next <= now && frames < MOST_FRAMES {
            self.next += self.frame;
            frames += 1;
        }
        if self.next <= now {
            self.next = now + self.frame;
        }
        frames
    }

    /// How long from `now` until the next frame is due.
    #[must_use]
    pub fn until_next(&self, now: Instant) -> Duration {
        self.next.saturating_duration_since(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: Duration = Duration::from_micros(16_743);

    #[test]
    fn a_frame_is_due_each_frame_s_length() {
        let start = Instant::now();
        let mut pacer = Pacer::new(FRAME, start);
        assert_eq!(pacer.due(start), 1);
        assert_eq!(pacer.due(start + FRAME / 2), 0);
        assert_eq!(pacer.until_next(start + FRAME / 2), FRAME / 2);
        assert_eq!(pacer.due(start + FRAME), 1);
        assert_eq!(pacer.due(start + FRAME * 3), 2);
    }

    #[test]
    fn a_screen_refreshing_at_60_hertz_repeats_a_picture_now_and_then() {
        let start = Instant::now();
        let mut pacer = Pacer::new(FRAME, start);
        let refresh = Duration::from_micros(16_667);
        let frames: Vec<u32> = (0..300)
            .map(|blank| pacer.due(start + refresh * blank))
            .collect();
        assert!(frames.iter().all(|&frames| frames <= 1));
        assert_eq!(frames.iter().filter(|&&frames| frames == 0).count(), 2);
    }

    #[test]
    fn a_long_hold_up_is_skipped() {
        let start = Instant::now();
        let mut pacer = Pacer::new(FRAME, start);
        assert_eq!(pacer.due(start + Duration::from_secs(5)), MOST_FRAMES);
        assert_eq!(pacer.until_next(start + Duration::from_secs(5)), FRAME);
    }
}
