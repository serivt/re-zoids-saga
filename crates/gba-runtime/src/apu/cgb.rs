//! The programmable channels as the driver runs them each frame
//! (`0x0805C8A4`): their software envelope, the level and pan it gives the
//! hardware, and the hardware generator that plays them.
//!
//! Source of knowledge: own reading of that routine and of the one that
//! derives a channel's level and pan from its volumes (`0x0805C83C`) in
//! Zoids Saga (Japan, Rev 1). The envelope counts frames, one more every
//! fifteenth frame (the driver's 0–14 counter) to keep pace with the
//! hardware's 64 steps a second.

use formats::m4a::Envelope;

use super::psg::{PsgChannel, PsgKind};
use super::sampled::{ACTIVE, FREE, STARTING, STOPPING};
use super::tables::Tables;

const PHASE: u8 = 0x03;
const ATTACK: u8 = 3;
const SUSTAIN: u8 = 1;
const DECAY: u8 = 2;
const SUSTAIN_PERIOD: u8 = 7;
const MAX_LEVEL: u8 = 15;
/// Pan masks of the sound enable register (`NR51`): right in the low
/// nibble, left in the high one.
const RIGHT_ONLY: u8 = 0x0F;
const LEFT_ONLY: u8 = 0xF0;
const BOTH: u8 = 0xFF;

/// Where the envelope goes next within a frame.
#[derive(Clone, Copy)]
enum Next {
    Step,
    DecayStart,
    SustainOrStop,
    Stop,
    Count,
}

/// One programmable channel.
#[derive(Debug, Clone, PartialEq)]
pub struct CgbChannel {
    /// Status: [`STARTING`], [`STOPPING`] and the phase.
    pub status: u8,
    /// Right volume, 0–255.
    pub right: u8,
    /// Left volume, 0–255.
    pub left: u8,
    /// The voice's envelope, in frames per level.
    pub envelope: Envelope,
    /// Frequency value: the tone register plus 2048, or the noise byte.
    pub frequency: u32,
    /// Envelope level, 0–15.
    pub level: u8,
    goal: u8,
    sustain_goal: u8,
    counter: u8,
    pan: u8,
    mask: u8,
    generator: PsgChannel,
}

impl CgbChannel {
    /// An idle channel of `kind`.
    #[must_use]
    pub fn new(kind: PsgKind) -> Self {
        Self {
            status: FREE,
            right: 0,
            left: 0,
            envelope: Envelope::default(),
            frequency: 0,
            level: 0,
            goal: 0,
            sustain_goal: 0,
            counter: 0,
            pan: 0,
            mask: 0x11 << kind.index(),
            generator: PsgChannel::new(kind),
        }
    }

    /// Which channel this is.
    #[must_use]
    pub fn kind(&self) -> PsgKind {
        self.generator.kind()
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

    /// The generator, to set up a new note's duty, pattern or noise width.
    pub fn generator_mut(&mut self) -> &mut PsgChannel {
        &mut self.generator
    }

    /// The envelope's goal, its sustain level and its frame counter.
    #[must_use]
    pub fn envelope_state(&self) -> (u8, u8, u8) {
        (self.goal, self.sustain_goal, self.counter)
    }

    /// The pan mask of the sound enable register this channel sets
    /// (`NR51`'s bits for it).
    #[must_use]
    pub fn pan_mask(&self) -> u8 {
        self.pan & self.mask
    }

    /// Whether the channel plays on the right and on the left.
    #[must_use]
    pub fn sides(&self) -> (bool, bool) {
        let enabled = self.pan & self.mask;
        (enabled & RIGHT_ONLY != 0, enabled & LEFT_ONLY != 0)
    }

    /// Derives the level the envelope rises to, its sustain and the pan
    /// from the volumes (`0x0805C83C`): a side at least twice the other
    /// plays alone, and the level is the sum over 16 (at most 15 unless
    /// both sides play).
    fn modulate(&mut self) {
        let (right, left) = (self.right, self.left);
        let sum = (u32::from(right) + u32::from(left)) >> 4;
        let sum = u8::try_from(sum).unwrap_or(u8::MAX);
        let alone = if right >= left {
            (right >> 1 >= left).then_some(RIGHT_ONLY)
        } else {
            (left >> 1 >= right).then_some(LEFT_ONLY)
        };
        if let Some(pan) = alone {
            self.pan = pan;
            self.goal = sum.min(MAX_LEVEL);
        } else {
            self.pan = BOTH;
            self.goal = sum;
        }
        let sustain = (u32::from(self.envelope.sustain) * u32::from(self.goal) + 15) >> 4;
        self.sustain_goal = u8::try_from(sustain).unwrap_or(u8::MAX);
    }

    fn stop(&mut self) {
        self.status = FREE;
        self.level = 0;
    }

    /// One frame of the driver's envelope; `extra` for the frame on which
    /// its 0–14 counter is 0, which counts one step more.
    pub fn frame(&mut self, extra: bool) {
        if !self.is_active() {
            return;
        }
        let Envelope {
            attack,
            decay,
            sustain,
            release,
        } = self.envelope;
        let mut extra = extra;
        let mut next = if self.status & STARTING != 0 {
            if self.status & STOPPING != 0 {
                self.stop();
                return;
            }
            self.status = ATTACK;
            self.modulate();
            self.generator.retrigger();
            self.counter = attack;
            if attack == 0 {
                Next::DecayStart
            } else {
                self.level = 0;
                Next::Count
            }
        } else if self.status & STOPPING != 0 && self.status & PHASE != 0 {
            self.status &= !PHASE;
            self.counter = release;
            if release == 0 {
                Next::Stop
            } else {
                Next::Count
            }
        } else {
            Next::Step
        };
        loop {
            next = match next {
                Next::Step => {
                    if self.counter == 0 {
                        self.step(attack, decay, release)
                    } else {
                        Next::Count
                    }
                }
                Next::DecayStart => {
                    self.status -= 1;
                    self.counter = decay;
                    if decay == 0 {
                        Next::SustainOrStop
                    } else {
                        self.level = self.goal;
                        Next::Count
                    }
                }
                Next::SustainOrStop => {
                    if sustain == 0 {
                        self.status &= !PHASE;
                        Next::Stop
                    } else {
                        self.status -= 1;
                        self.level = self.sustain_goal;
                        self.counter = SUSTAIN_PERIOD;
                        Next::Count
                    }
                }
                Next::Stop => {
                    self.stop();
                    return;
                }
                Next::Count => {
                    self.counter = self.counter.wrapping_sub(1);
                    if extra {
                        extra = false;
                        Next::Step
                    } else {
                        break;
                    }
                }
            };
        }
    }

    /// The envelope's step once its counter has run out: a level toward
    /// the phase's goal, or the next phase.
    fn step(&mut self, attack: u8, decay: u8, release: u8) -> Next {
        self.modulate();
        match self.status & PHASE {
            0 => {
                self.level = self.level.wrapping_sub(1);
                if i8::from_le_bytes([self.level]) > 0 {
                    self.counter = release;
                    Next::Count
                } else {
                    Next::Stop
                }
            }
            SUSTAIN => {
                self.level = self.sustain_goal;
                self.counter = SUSTAIN_PERIOD;
                Next::Count
            }
            DECAY => {
                self.level = self.level.wrapping_sub(1);
                if i8::from_le_bytes([self.level]) > i8::from_le_bytes([self.sustain_goal]) {
                    self.counter = decay;
                    Next::Count
                } else {
                    Next::SustainOrStop
                }
            }
            _ => {
                self.level = self.level.wrapping_add(1);
                if self.level < self.goal {
                    self.counter = attack;
                    Next::Count
                } else {
                    Next::DecayStart
                }
            }
        }
    }

    /// Hands this frame's level and frequency to the generator.
    pub fn apply(&mut self, tables: &Tables, mix_rate: f64) {
        let level = if self.is_active() { self.level } else { 0 };
        match self.kind() {
            PsgKind::Wave => {
                self.generator.set_wave_volume(tables.wave_volume(level));
                self.generator.set_register(self.frequency, mix_rate);
            }
            PsgKind::Noise => {
                self.generator.set_level(level);
                self.generator
                    .set_noise(u8::try_from(self.frequency & 0xFF).unwrap_or(0), mix_rate);
            }
            PsgKind::Square1 | PsgKind::Square2 => {
                self.generator.set_level(level);
                self.generator.set_register(self.frequency, mix_rate);
            }
        }
    }

    /// The next sample of the generator, -15..=15 at full volume, or
    /// silence while the channel is idle.
    pub fn sample(&mut self) -> i32 {
        if self.is_active() {
            self.generator.sample()
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(envelope: Envelope, right: u8, left: u8) -> CgbChannel {
        let mut channel = CgbChannel::new(PsgKind::Square1);
        channel.status = STARTING;
        channel.envelope = envelope;
        channel.right = right;
        channel.left = left;
        channel
    }

    #[test]
    fn a_note_without_attack_or_decay_holds_its_sustain() {
        let envelope = Envelope {
            attack: 0,
            decay: 0,
            sustain: 15,
            release: 0,
        };
        let mut channel = note(envelope, 100, 100);
        channel.frame(false);
        assert_eq!(channel.level, 12);
        assert_eq!(channel.status & PHASE, SUSTAIN);
        assert_eq!(channel.sides(), (true, true));
        channel.status |= STOPPING;
        channel.frame(false);
        assert_eq!(channel.status, FREE);
    }

    #[test]
    fn decay_steps_every_period_and_the_fifteenth_frame_counts_twice() {
        let envelope = Envelope {
            attack: 0,
            decay: 2,
            sustain: 8,
            release: 1,
        };
        let mut channel = note(envelope, 240, 0);
        channel.frame(false);
        assert_eq!((channel.level, channel.counter), (15, 1));
        assert_eq!(channel.sides(), (true, false));
        channel.frame(false);
        assert_eq!((channel.level, channel.counter), (15, 0));
        channel.frame(false);
        assert_eq!((channel.level, channel.counter), (14, 1));
        channel.frame(true);
        assert_eq!((channel.level, channel.counter), (13, 1));
        channel.status |= STOPPING;
        channel.frame(false);
        assert_eq!(channel.status & PHASE, 0);
        channel.frame(false);
        assert_eq!(channel.level, 12);
    }

    #[test]
    fn an_attack_rises_a_level_per_period_to_the_goal() {
        let envelope = Envelope {
            attack: 1,
            decay: 0,
            sustain: 15,
            release: 0,
        };
        let mut channel = note(envelope, 16, 16);
        channel.frame(false);
        assert_eq!(channel.level, 0);
        channel.frame(false);
        assert_eq!(channel.level, 1);
        channel.frame(false);
        assert_eq!(channel.level, 2);
        assert_eq!(channel.status & PHASE, SUSTAIN);
    }
}
