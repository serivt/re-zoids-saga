//! Audio output: a queue of stereo 16-bit samples the backend plays at a
//! fixed rate.

use crate::PlatformError;

/// Somewhere to send interleaved stereo samples.
pub trait AudioOut {
    /// Queues `samples` (left, right, left, right…) for playback.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the backend cannot take the samples.
    fn queue(&mut self, samples: &[i16]) -> Result<(), PlatformError>;

    /// Stereo sample pairs queued and not yet played.
    fn queued_pairs(&self) -> usize;
}
