//! The four programmable sound channels, synthesized in software at the
//! mixer's rate.
//!
//! Source of knowledge: public hardware documentation of the GBA's sound
//! channels (register frequency to Hz, duty cycles, the wave pattern layout
//! and the noise shift register). Volumes are in the hardware's 0–15 steps.

const CLOCK: f64 = 131_072.0;
const WAVE_CLOCK: f64 = 2_097_152.0;
const WAVE_SAMPLES: usize = 32;
const FRACTION_BITS: u32 = 32;
const ONE: u64 = 1 << FRACTION_BITS;
const NOISE_CLOCK: f64 = 524_288.0;
const DUTY_HIGH: [u64; 4] = [ONE / 8, ONE / 4, ONE / 2, ONE * 3 / 4];
const MAX_LEVEL: i32 = 15;
/// Register value that stands for the highest frequency.
pub const MAX_FREQUENCY: u32 = 2047;

/// Which channel a voice drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsgKind {
    /// Square wave with sweep.
    Square1,
    /// Square wave.
    Square2,
    /// 32-sample programmable wave.
    Wave,
    /// Noise.
    Noise,
}

/// Converts a hardware frequency register (0–2047) to hertz for a tone
/// channel.
#[must_use]
pub fn tone_hertz(register: u32) -> f64 {
    CLOCK / f64::from(2048 - register.min(MAX_FREQUENCY))
}

/// Converts a hertz value to the register a tone channel needs.
#[must_use]
pub fn tone_register(hertz: f64) -> u32 {
    let raw = 2048.0 - CLOCK / hertz.max(1.0);
    to_integer(raw.round().clamp(0.0, f64::from(MAX_FREQUENCY)))
}

/// Truncates a non-negative, in-range float to an integer.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_integer(value: f64) -> u32 {
    value as u32
}

/// A rate in cycles per output sample as a 32.32 fixed-point step.
#[must_use]
pub fn fixed_step(cycles_per_sample: f64) -> u64 {
    let one = f64::from(u32::MAX) + 1.0;
    let scaled = (cycles_per_sample.max(0.0) * one).min(one * 4096.0);
    to_integer_wide(scaled)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_integer_wide(value: f64) -> u64 {
    value as u64
}

/// One channel's synthesis state.
#[derive(Debug, Clone, PartialEq)]
pub struct PsgChannel {
    kind: PsgKind,
    phase: u64,
    step: u64,
    level: i32,
    duty: usize,
    pattern: [u8; WAVE_SAMPLES],
    lfsr: u16,
    short_noise: bool,
}

impl PsgChannel {
    /// A silent channel of the given kind.
    #[must_use]
    pub fn new(kind: PsgKind) -> Self {
        Self {
            kind,
            phase: 0,
            step: 0,
            level: 0,
            duty: 2,
            pattern: [0; WAVE_SAMPLES],
            lfsr: 0x7FFF,
            short_noise: false,
        }
    }

    /// Which channel this is.
    #[must_use]
    pub fn kind(&self) -> PsgKind {
        self.kind
    }

    /// Restarts the waveform.
    pub fn retrigger(&mut self) {
        self.phase = 0;
        self.lfsr = 0x7FFF;
    }

    /// Sets the duty cycle of a square channel (0–3).
    pub fn set_duty(&mut self, duty: u8) {
        self.duty = usize::from(duty & 3);
    }

    /// Loads the 16 bytes (32 nibbles) of a wave pattern.
    pub fn set_pattern(&mut self, bytes: &[u8; 16]) {
        for (index, byte) in bytes.iter().enumerate() {
            self.pattern[index * 2] = byte >> 4;
            self.pattern[index * 2 + 1] = byte & 0x0F;
        }
    }

    /// Chooses the noise shift register width.
    pub fn set_short_noise(&mut self, short: bool) {
        self.short_noise = short;
    }

    /// Sets the output frequency in hertz of a tone or wave channel, or the
    /// shift rate of the noise channel, given the mixer's rate.
    pub fn set_hertz(&mut self, hertz: f64, mix_rate: f64) {
        self.step = fixed_step(match self.kind {
            PsgKind::Square1 | PsgKind::Square2 => hertz / mix_rate,
            PsgKind::Wave => hertz * (WAVE_CLOCK / CLOCK) / mix_rate,
            PsgKind::Noise => hertz.min(NOISE_CLOCK) / mix_rate,
        });
    }

    /// Sets the volume, 0–15.
    pub fn set_level(&mut self, level: u8) {
        self.level = i32::from(level.min(15));
    }

    /// Current volume, 0–15.
    #[must_use]
    pub fn level(&self) -> u8 {
        u8::try_from(self.level).unwrap_or(0)
    }

    /// Produces one sample in the range -15..=15 and advances the phase.
    pub fn sample(&mut self) -> i32 {
        if self.level == 0 || self.step == 0 {
            return 0;
        }
        match self.kind {
            PsgKind::Square1 | PsgKind::Square2 => {
                let high = self.phase < DUTY_HIGH[self.duty];
                self.phase = (self.phase + self.step) % ONE;
                if high { self.level } else { -self.level }
            }
            PsgKind::Wave => {
                let index =
                    usize::try_from(self.phase >> FRACTION_BITS).unwrap_or(0) % WAVE_SAMPLES;
                self.phase = (self.phase + self.step) % (ONE * WAVE_SAMPLES as u64);
                (i32::from(self.pattern[index]) * 2 - MAX_LEVEL) * self.level / MAX_LEVEL
            }
            PsgKind::Noise => {
                self.phase += self.step;
                while self.phase >= ONE {
                    self.phase -= ONE;
                    self.shift_noise();
                }
                if self.lfsr & 1 == 0 {
                    self.level
                } else {
                    -self.level
                }
            }
        }
    }

    fn shift_noise(&mut self) {
        let feedback = (self.lfsr ^ (self.lfsr >> 1)) & 1;
        self.lfsr >>= 1;
        if self.short_noise {
            self.lfsr = (self.lfsr & !0x40) | (feedback << 6);
        } else {
            self.lfsr |= feedback << 14;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_hertz_convert_both_ways() {
        assert_eq!(tone_register(tone_hertz(1750)), 1750);
        assert!((tone_hertz(1024) - 128.0).abs() < 1e-9);
        assert_eq!(tone_register(0.5), 0);
        assert_eq!(tone_register(1.0e9), MAX_FREQUENCY);
    }

    #[test]
    fn a_square_wave_spends_the_duty_fraction_high() {
        let mut channel = PsgChannel::new(PsgKind::Square1);
        channel.set_level(15);
        channel.set_duty(1);
        channel.set_hertz(100.0, 1000.0);
        let samples: Vec<i32> = (0..10).map(|_| channel.sample()).collect();
        assert_eq!(samples.iter().filter(|value| **value > 0).count(), 3);
        assert_eq!(samples[0], 15);
        assert_eq!(samples[5], -15);
    }

    #[test]
    fn wave_and_noise_channels_follow_their_data() {
        let mut wave = PsgChannel::new(PsgKind::Wave);
        wave.set_level(15);
        wave.set_pattern(&[0xF0; 16]);
        wave.set_hertz(1.0, 16.0);
        assert_eq!(wave.sample(), 15);
        assert_eq!(wave.sample(), -15);
        let mut noise = PsgChannel::new(PsgKind::Noise);
        noise.set_level(8);
        noise.set_hertz(1000.0, 1000.0);
        let samples: Vec<i32> = (0..64).map(|_| noise.sample()).collect();
        assert!(samples.contains(&8));
        assert!(samples.contains(&-8));
        noise.set_level(0);
        assert_eq!(noise.sample(), 0);
    }
}
