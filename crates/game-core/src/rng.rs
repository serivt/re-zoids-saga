//! The game's random number generator.
//!
//! Matches the routine at ROM `0x08001080`: a 16-bit state mixed each call
//! with a call counter and the `VBlank` frame counter, and the seeding
//! routine at `0x08001068`, which the wander logic applies with the frame
//! counter before drawing a direction. Sequences therefore depend on the
//! frame a call happens on, not only on how many calls came before.

/// Random number state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rng {
    state: u16,
    calls: u16,
}

impl Rng {
    /// A generator resumed from a state and a call counter, as the
    /// original holds them at IWRAM `0x0300233C` and EWRAM `0x0200607C`:
    /// to replay a stretch of the original's play.
    #[must_use]
    pub const fn resumed(state: u16, calls: u16) -> Self {
        Self { state, calls }
    }

    /// Seeds the state from a frame counter the way the original does.
    pub fn seed(&mut self, frame: u16) {
        self.state = frame.wrapping_add((frame ^ 0xFF).wrapping_shl(8));
    }

    /// Draws the next 16-bit value on frame `frame`.
    pub fn next(&mut self, frame: u16) -> u16 {
        self.calls = self.calls.wrapping_add(1);
        let shift = u32::from(self.state & 0x0F);
        let mixed = (self.calls >> shift).wrapping_add(1);
        self.state = self
            .state
            .wrapping_mul(5)
            .wrapping_add(mixed)
            .wrapping_add((!frame).wrapping_shl(1));
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_from_the_frame_counter() {
        let mut rng = Rng::default();
        rng.seed(0x1234);
        assert_eq!(rng.state, 0x1234u16.wrapping_add(0xCB00));
        rng.seed(0);
        assert_eq!(rng.state, 0xFF00);
    }

    #[test]
    fn mixes_the_state_the_call_counter_and_the_frame() {
        let mut rng = Rng::default();
        assert_eq!(rng.next(0), 0);
        assert_eq!(rng.next(0), 3u16.wrapping_add(0xFFFE));
        let mut again = Rng::default();
        again.next(0);
        again.next(0);
        assert_eq!(rng, again);
        assert_ne!(rng.next(7), again.next(8));
    }
}
