//! The four programmable sound channels' generators, synthesized in
//! software at the mixer's rate from the values the driver writes to their
//! registers.
//!
//! Source of knowledge: public hardware documentation of the GBA's sound
//! channels (the tone and wave registers' frequency, the duty cycles, the
//! wave pattern layout and its volume codes, the noise channel's frequency
//! byte and shift register).

const CLOCK: f64 = 131_072.0;
const WAVE_CLOCK: f64 = 2_097_152.0;
const NOISE_CLOCK: f64 = 524_288.0;
const WAVE_SAMPLES: usize = 32;
const FRACTION_BITS: u32 = 32;
const ONE: u64 = 1 << FRACTION_BITS;
const DUTY_HIGH: [u64; 4] = [ONE / 8, ONE / 4, ONE / 2, ONE * 3 / 4];
const MAX_LEVEL: i32 = 15;
const REGISTER_MASK: u32 = 0x7FF;
/// The wave channel's volume codes (`NR32`): muted, full, half, quarter,
/// and bit 7 forcing three quarters.
const WAVE_FULL: u8 = 0x20;
const WAVE_HALF: u8 = 0x40;
const WAVE_QUARTER: u8 = 0x60;
const WAVE_THREE_QUARTERS: u8 = 0x80;
const WAVE_CODE_MASK: u8 = 0x60;

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

impl PsgKind {
    /// The channel's index, 0–3.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Square1 => 0,
            Self::Square2 => 1,
            Self::Wave => 2,
            Self::Noise => 3,
        }
    }
}

/// Converts a tone channel's frequency register (its low 11 bits) to hertz.
#[must_use]
pub fn tone_hertz(register: u32) -> f64 {
    CLOCK / f64::from(2048 - (register & REGISTER_MASK))
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
    wave_volume: u8,
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
            wave_volume: 0,
            lfsr: 0x7FFF,
            short_noise: false,
        }
    }

    /// Which channel this is.
    #[must_use]
    pub fn kind(&self) -> PsgKind {
        self.kind
    }

    /// Restarts the waveform, as writing the channel's trigger bit does.
    pub fn retrigger(&mut self) {
        self.phase = 0;
        self.lfsr = 0x7FFF;
    }

    /// Sets the duty cycle of a square channel (0–3).
    pub fn set_duty(&mut self, duty: u8) {
        self.duty = usize::from(duty & 3);
    }

    /// Loads the 16 bytes (32 nibbles, high first) of a wave pattern.
    pub fn set_pattern(&mut self, bytes: &[u8; 16]) {
        for (index, byte) in bytes.iter().enumerate() {
            self.pattern[index * 2] = byte >> 4;
            self.pattern[index * 2 + 1] = byte & 0x0F;
        }
    }

    /// Chooses the noise shift register's width (7 bits when short).
    pub fn set_short_noise(&mut self, short: bool) {
        self.short_noise = short;
    }

    /// Sets a tone or wave channel's frequency register (its low 11 bits),
    /// given the mixer's rate.
    pub fn set_register(&mut self, register: u32, mix_rate: f64) {
        let period = f64::from(2048 - (register & REGISTER_MASK));
        self.step = fixed_step(match self.kind {
            PsgKind::Wave => WAVE_CLOCK / period / mix_rate,
            _ => CLOCK / period / mix_rate,
        });
    }

    /// Sets the noise channel's frequency byte (`NR43`: divisor in bits
    /// 0–2, 0 standing for a half, shift in bits 4–7), given the mixer's
    /// rate.
    pub fn set_noise(&mut self, byte: u8, mix_rate: f64) {
        let divisor = match byte & 7 {
            0 => 0.5,
            other => f64::from(other),
        };
        let shift = i32::from(byte >> 4) + 1;
        self.step = fixed_step(NOISE_CLOCK / divisor / 2f64.powi(shift) / mix_rate);
    }

    /// Sets the envelope level of a square or noise channel, 0–15.
    pub fn set_level(&mut self, level: u8) {
        self.level = i32::from(level.min(15));
    }

    /// Sets the wave channel's volume byte (`NR32`).
    pub fn set_wave_volume(&mut self, byte: u8) {
        self.wave_volume = byte;
    }

    /// Current envelope level, 0–15.
    #[must_use]
    pub fn level(&self) -> u8 {
        u8::try_from(self.level).unwrap_or(0)
    }

    /// Produces one sample, -15..=15 at full volume, and advances.
    pub fn sample(&mut self) -> i32 {
        if self.step == 0 {
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
                let (numerator, denominator) = self.wave_scale();
                (i32::from(self.pattern[index]) * 2 - MAX_LEVEL) * numerator / denominator
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

    fn wave_scale(&self) -> (i32, i32) {
        if self.wave_volume & WAVE_THREE_QUARTERS != 0 {
            return (3, 4);
        }
        match self.wave_volume & WAVE_CODE_MASK {
            WAVE_FULL => (1, 1),
            WAVE_HALF => (1, 2),
            WAVE_QUARTER => (1, 4),
            _ => (0, 1),
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
    fn the_tone_register_gives_hertz() {
        assert!((tone_hertz(1024) - 128.0).abs() < 1e-9);
        assert!((tone_hertz(1750 + 2048) - tone_hertz(1750)).abs() < 1e-9);
    }

    #[test]
    fn a_square_wave_spends_the_duty_fraction_high() {
        let mut channel = PsgChannel::new(PsgKind::Square1);
        channel.set_level(15);
        channel.set_duty(1);
        channel.set_register(2048 - 1311, 10_000.0);
        let samples: Vec<i32> = (0..100).map(|_| channel.sample()).collect();
        let high = samples.iter().filter(|value| **value > 0).count();
        assert!((24..=26).contains(&high));
        assert_eq!(samples[0], 15);
    }

    #[test]
    fn the_wave_channel_scales_by_its_volume_code() {
        let mut wave = PsgChannel::new(PsgKind::Wave);
        wave.set_pattern(&[0xF0; 16]);
        wave.set_register(2048 - 2, 2_097_152.0 / 2.0);
        wave.set_wave_volume(WAVE_FULL);
        assert_eq!(wave.sample(), 15);
        assert_eq!(wave.sample(), -15);
        wave.set_wave_volume(WAVE_HALF);
        assert_eq!(wave.sample(), 7);
        wave.set_wave_volume(WAVE_THREE_QUARTERS);
        assert_eq!(wave.sample(), -11);
        wave.set_wave_volume(0);
        assert_eq!(wave.sample(), 0);
    }

    #[test]
    fn noise_shifts_at_its_byte_rate() {
        let mut noise = PsgChannel::new(PsgKind::Noise);
        noise.set_level(8);
        noise.set_noise(0x00, 1_048_576.0);
        let samples: Vec<i32> = (0..64).map(|_| noise.sample()).collect();
        assert!(samples.contains(&8));
        assert!(samples.contains(&-8));
    }
}
