//! The sound through Web Audio: each frame's samples become a buffer at
//! the game's own rate, which the browser converts to the device's, played
//! right after the one before so they follow without gaps. A browser lets
//! a page make sound only after the player has pressed something; the page
//! resumes the sound on its Play button.

use platform::{AudioOut, PlatformError};
use web_sys::{AudioContext, AudioContextState};

use crate::web_error;

/// How far ahead of now the first buffer starts, and the next after a gap:
/// room for a frame's delay without a click.
const LEAD_SECONDS: f64 = 0.05;
const CHANNELS: u32 = 2;
const SAMPLE_SCALE: f32 = 32_768.0;

/// The sound output.
pub struct WebAudio {
    context: AudioContext,
    rate: f32,
    /// When the next buffer starts, in the context's seconds.
    next: f64,
}

impl WebAudio {
    /// Opens the sound for samples at `rate` hertz.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the browser has no Web Audio.
    pub fn new(rate: u32) -> Result<Self, PlatformError> {
        let context = AudioContext::new().map_err(web_error)?;
        Ok(Self {
            context,
            rate: f32_of(rate),
            next: 0.0,
        })
    }

    /// Lets the sound play, once the player has pressed something.
    pub fn resume(&self) {
        if self.context.state() != AudioContextState::Running {
            let _ = self.context.resume();
        }
    }
}

impl AudioOut for WebAudio {
    fn queue(&mut self, samples: &[i16]) -> Result<(), PlatformError> {
        let (left, right) = split(samples);
        let Ok(length) = u32::try_from(left.len()) else {
            return Ok(());
        };
        if length == 0 {
            return Ok(());
        }
        let buffer = self
            .context
            .create_buffer(CHANNELS, length, self.rate)
            .map_err(web_error)?;
        buffer.copy_to_channel(&left, 0).map_err(web_error)?;
        buffer.copy_to_channel(&right, 1).map_err(web_error)?;
        let source = self.context.create_buffer_source().map_err(web_error)?;
        source.set_buffer(Some(&buffer));
        source
            .connect_with_audio_node(&self.context.destination())
            .map_err(web_error)?;
        let now = self.context.current_time();
        let start = self.next.max(now + LEAD_SECONDS);
        source.start_with_when(start).map_err(web_error)?;
        self.next = start + f64::from(length) / f64::from(self.rate);
        Ok(())
    }

    fn queued_pairs(&self) -> usize {
        let ahead = (self.next - self.context.current_time()).max(0.0);
        pairs_in(ahead, self.rate)
    }
}

/// Interleaved samples as the two channels' values, -1 to 1.
fn split(samples: &[i16]) -> (Vec<f32>, Vec<f32>) {
    samples
        .chunks_exact(2)
        .map(|pair| {
            (
                f32::from(pair[0]) / SAMPLE_SCALE,
                f32::from(pair[1]) / SAMPLE_SCALE,
            )
        })
        .unzip()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pairs_in(seconds: f64, rate: f32) -> usize {
    (seconds * f64::from(rate)) as usize
}

#[allow(clippy::cast_precision_loss)]
fn f32_of(value: u32) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_split_into_channels_of_one_at_most() {
        let (left, right) = split(&[i16::MIN, 16_384, 0, i16::MAX, 7]);
        assert!(left[0] <= -1.0 && left[1].abs() < f32::EPSILON);
        assert!((right[0] - 0.5).abs() < f32::EPSILON);
        assert!(right[1] < 1.0 && right[1] > 0.999);
        assert_eq!(pairs_in(0.5, 31_536.0), 15_768);
    }
}
