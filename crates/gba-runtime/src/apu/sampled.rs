//! The sampled channels as the driver's mixer runs them (its RAM routine,
//! copied from ROM `0x0805B18C`): each frame a channel's envelope steps and
//! its sample is resampled into the driver's 8-bit buffer.
//!
//! Source of knowledge: own reading of that routine in Zoids Saga (Japan,
//! Rev 1): the envelope's phases, the channel's volume from its right and
//! left volumes, the envelope and the master volume, the linear
//! interpolation between samples at a 23-bit fraction, the loop, and the
//! per-byte addition into the buffer, which wraps rather than saturates.

use formats::m4a::{Envelope, Sample};

/// Free.
pub const FREE: u8 = 0;
/// A note just started; the mixer begins it.
pub const STARTING: u8 = 0x80;
/// The key went up.
pub const STOPPING: u8 = 0x40;
/// The sample loops.
const LOOPING: u8 = 0x10;
/// The envelope's phase in the low two bits.
const PHASE: u8 = 0x03;
const ATTACK: u8 = 3;
const DECAY: u8 = 2;
/// What makes a channel busy (`0xC7`).
pub const ACTIVE: u8 = 0xC7;
const FULL: u32 = 0xFF;
const FRACTION_BITS: u32 = 23;
const FRACTION_MASK: u32 = (1 << FRACTION_BITS) - 1;

/// One sampled channel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SampledChannel {
    /// Status: [`STARTING`], [`STOPPING`], the loop flag and the phase.
    pub status: u8,
    /// Envelope level, 0–255.
    pub level: u8,
    /// Right volume, 0–255.
    pub right: u8,
    /// Left volume, 0–255.
    pub left: u8,
    /// The voice's envelope.
    pub envelope: Envelope,
    /// Whether the voice plays its sample at the mix rate, one sample per
    /// output sample, without interpolation.
    pub fixed: bool,
    /// The sample.
    pub sample: Option<Sample>,
    /// Rate in the driver's units (about hertz), from the key.
    pub frequency: u32,
    position: usize,
    remaining: i64,
    fraction: u32,
}

impl SampledChannel {
    /// A note starting on `sample` with `envelope`.
    #[must_use]
    pub fn start(envelope: Envelope, fixed: bool, sample: Sample) -> Self {
        Self {
            status: STARTING,
            envelope,
            fixed,
            sample: Some(sample),
            ..Self::default()
        }
    }

    /// Whether the channel is busy.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.status & ACTIVE != 0
    }

    /// Whether the key has gone up.
    #[must_use]
    pub fn is_stopping(&self) -> bool {
        self.status & STOPPING != 0
    }

    /// Steps the envelope for this frame, as the mixer does before mixing
    /// the channel, and returns its mix volume (0 when it stopped): the
    /// right and left volumes times the envelope times the master volume
    /// plus one over 16, over 512.
    pub fn step_envelope(&mut self, master_volume: u8) -> u8 {
        let Some(sample) = self.sample else {
            self.status = FREE;
            return 0;
        };
        let Envelope {
            attack,
            decay,
            sustain,
            release,
        } = self.envelope;
        let mut level = u32::from(self.level);
        if self.status & STARTING != 0 {
            if self.status & STOPPING != 0 {
                self.status = FREE;
                return 0;
            }
            self.status = ATTACK | if sample.looped { LOOPING } else { 0 };
            self.position = 0;
            self.remaining = i64::from(sample.length);
            self.fraction = 0;
            level = 0;
            level = self.attack_step(level, attack);
        } else if self.status & STOPPING != 0 {
            level = (level * u32::from(release)) >> 8;
            if level == 0 {
                self.status = FREE;
                return 0;
            }
        } else {
            match self.status & PHASE {
                DECAY => {
                    level = (level * u32::from(decay)) >> 8;
                    if level <= u32::from(sustain) {
                        level = u32::from(sustain);
                        if level == 0 {
                            self.status = FREE;
                            return 0;
                        }
                        self.status -= 1;
                    }
                }
                ATTACK => level = self.attack_step(level, attack),
                _ => {}
            }
        }
        self.level = u8::try_from(level).unwrap_or(u8::MAX);
        let scaled = (level * (u32::from(master_volume) + 1)) >> 4;
        let volume = ((u32::from(self.right) + u32::from(self.left)) * scaled) >> 9;
        u8::try_from(volume & 0xFF).unwrap_or(0)
    }

    fn attack_step(&mut self, level: u32, attack: u8) -> u32 {
        let level = level + u32::from(attack);
        if level >= FULL {
            self.status -= 1;
            FULL
        } else {
            level
        }
    }

    /// Adds this frame's samples at `volume` into `buffer`, each the
    /// sample times the volume over 256 added to the buffer's byte with
    /// wrapping; stops the channel at the end of a sample that does not
    /// loop.
    pub fn mix(&mut self, rom: &[u8], buffer: &mut [i8], volume: u8, step: u32) {
        let Some(sample) = self.sample else {
            return;
        };
        let read = |index: usize| {
            rom.get(sample.data + index)
                .map_or(0, |byte| i32::from(i8::from_le_bytes([*byte])))
        };
        let volume = i32::from(volume);
        let loop_length = i64::from(sample.length) - i64::from(sample.loop_start);
        let loops = self.status & LOOPING != 0 && loop_length > 0;
        let loop_start = usize::try_from(sample.loop_start).unwrap_or(0);
        if self.fixed {
            for out in buffer.iter_mut() {
                add(out, read(self.position) * volume);
                self.position += 1;
                self.remaining -= 1;
                if self.remaining == 0 {
                    if loops {
                        self.position = loop_start;
                        self.remaining = loop_length;
                    } else {
                        self.status = FREE;
                        return;
                    }
                }
            }
            return;
        }
        let mut base = read(self.position);
        let mut delta = read(self.position + 1) - base;
        for out in buffer.iter_mut() {
            let fraction = i32::try_from(self.fraction).unwrap_or(0);
            let value = base + ((delta.wrapping_mul(fraction)) >> FRACTION_BITS);
            add(out, value * volume);
            self.fraction = self.fraction.wrapping_add(step);
            let advance = self.fraction >> FRACTION_BITS;
            if advance == 0 {
                continue;
            }
            self.fraction &= FRACTION_MASK;
            self.remaining -= i64::from(advance);
            if self.remaining <= 0 {
                if !loops {
                    self.status = FREE;
                    return;
                }
                let mut over = -self.remaining;
                self.remaining += loop_length;
                while self.remaining <= 0 {
                    over -= loop_length;
                    self.remaining += loop_length;
                }
                self.position = loop_start + usize::try_from(over).unwrap_or(0);
            } else {
                self.position += usize::try_from(advance).unwrap_or(0);
            }
            base = read(self.position);
            delta = read(self.position + 1) - base;
        }
    }
}

/// Adds the top byte of `product` (sample × volume, over 256) to `out`,
/// wrapping as the mixer's byte lanes do.
fn add(out: &mut i8, product: i32) {
    let byte = i8::from_le_bytes([u8::try_from((product >> 8) & 0xFF).unwrap_or(0)]);
    *out = out.wrapping_add(byte);
}

/// The driver's reverb (the start of the mixer): each byte of the frame is
/// the sum of the bytes the buffer held three and two frames earlier times
/// `reverb` over 256, a negative result one closer to zero; without reverb
/// the frame starts silent.
pub fn reverb(buffer: &mut [i8], three_ago: &[i8], two_ago: &[i8], reverb: u8) {
    for ((out, old), older) in buffer.iter_mut().zip(two_ago).zip(three_ago) {
        if reverb == 0 {
            *out = 0;
            continue;
        }
        let mut value = ((i32::from(*old) + i32::from(*older)) * i32::from(reverb)) >> 8;
        if value & 0x80 != 0 {
            value += 1;
        }
        *out = i8::from_le_bytes([u8::try_from(value & 0xFF).unwrap_or(0)]);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn channel(length: u32, looped: bool) -> SampledChannel {
        SampledChannel {
            status: STARTING,
            right: 255,
            left: 255,
            envelope: Envelope {
                attack: 255,
                decay: 255,
                sustain: 255,
                release: 128,
            },
            sample: Some(Sample {
                looped,
                frequency: 0,
                loop_start: 1,
                length,
                data: 0,
            }),
            ..SampledChannel::default()
        }
    }

    fn expected(level: u32, master: u32) -> u8 {
        u8::try_from((510 * ((level * (master + 1)) >> 4)) >> 9).unwrap()
    }

    #[test]
    fn the_envelope_starts_at_its_attack_and_releases_by_its_fraction() {
        let mut voice = channel(4, false);
        voice.envelope.attack = 100;
        assert_eq!(voice.step_envelope(15), expected(100, 15));
        assert_eq!(voice.level, 100);
        assert_eq!(voice.step_envelope(15), expected(200, 15));
        assert_eq!(voice.step_envelope(15), expected(255, 15));
        assert_eq!(voice.status & PHASE, DECAY);
        voice.status |= STOPPING;
        assert_eq!(voice.step_envelope(14), expected(127, 14));
        assert_eq!(voice.level, 127);
    }

    #[test]
    fn samples_interpolate_between_bytes_and_stop_at_the_end() {
        let rom = [0u8, 100, 50, 0, 0];
        let mut voice = channel(3, false);
        voice.step_envelope(15);
        let mut buffer = [0i8; 4];
        voice.mix(&rom, &mut buffer, 255, 1 << 22);
        assert_eq!(buffer, [0, 49, 99, 74]);
        let mut voice = channel(3, false);
        voice.step_envelope(15);
        let mut buffer = [0i8; 4];
        voice.mix(&rom, &mut buffer, 255, 1 << 23);
        assert_eq!(buffer, [0, 99, 49, 0]);
        assert_eq!(voice.status, FREE);
        let mut wrapped = [100i8];
        add(&mut wrapped[0], 99 * 256);
        assert_eq!(wrapped[0], -57);
    }

    #[test]
    fn a_looping_sample_returns_to_its_loop_start() {
        let rom = [10u8, 20, 30, 40];
        let mut voice = channel(3, true);
        voice.fixed = true;
        voice.step_envelope(15);
        let mut buffer = [0i8; 6];
        voice.mix(&rom, &mut buffer, 255, 0);
        let expect: Vec<i8> = [10, 20, 30, 20, 30, 20]
            .iter()
            .map(|value| i8::try_from((value * 255) >> 8).unwrap())
            .collect();
        assert_eq!(buffer.to_vec(), expect);
    }

    #[test]
    fn reverb_sums_two_earlier_frames() {
        let mut buffer = [0i8; 3];
        reverb(&mut buffer, &[100, -100, 0], &[28, -28, 10], 64);
        assert_eq!(buffer, [32, -31, 2]);
        reverb(&mut buffer, &[100, -100, 0], &[28, -28, 10], 0);
        assert_eq!(buffer, [0, 0, 0]);
    }
}
