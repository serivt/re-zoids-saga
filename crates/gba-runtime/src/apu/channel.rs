//! A voice being played: a sampled channel mixed in software or one of the
//! programmable channels, with the driver's per-frame envelope.
//!
//! Source of knowledge: the envelope rules of the GBA's common sound driver
//! as its data documents them (attack added per frame, decay and release
//! as fractions of 256 per frame, sustain held) and the game's driver
//! settings read from its RAM (mono output, 8 sampled channels).

use formats::m4a::{Envelope, Sample};

use super::psg::{PsgChannel, PsgKind, fixed_step};

const FULL: i32 = 255;
const FULL_PSG: i32 = 15;
const FRACTION_BITS: u32 = 32;

/// Where the envelope is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Rising after the key went down.
    Attack,
    /// Falling to the sustain level.
    Decay,
    /// Holding while the key is down.
    Sustain,
    /// Falling to silence after the key went up.
    Release,
    /// Done; the channel is free.
    Off,
}

/// The sound a channel makes.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Sampled data played at a rate.
    Sampled {
        /// The sample.
        sample: Sample,
        /// Position in samples, 32.32 fixed point.
        position: u64,
        /// Samples advanced per output sample, 32.32 fixed point.
        step: u64,
    },
    /// A programmable channel.
    Psg(PsgChannel),
}

/// One playing voice.
#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    /// Track that owns the note, for later commands.
    pub track: usize,
    /// Player the track belongs to.
    pub player: usize,
    /// Key the note was started with.
    pub key: u8,
    /// Whether the note is a tie, released only by the track.
    pub tied: bool,
    /// Ticks left before the key goes up; ignored for ties.
    pub gate: u32,
    /// Priority of the track.
    pub priority: u8,
    /// Frames since the note started; older channels are stolen first.
    pub age: u32,
    /// Velocity times track volume over 128, 0–126.
    pub volume: u8,
    goal: i32,
    envelope: Envelope,
    phase: Phase,
    level: i32,
    counter: u8,
    source: Source,
}

impl Channel {
    /// Starts a note on `source` with `envelope`.
    #[must_use]
    pub fn start(track: usize, player: usize, key: u8, envelope: Envelope, source: Source) -> Self {
        let mut channel = Self {
            track,
            player,
            key,
            tied: false,
            gate: 0,
            priority: 0,
            age: 0,
            volume: 0,
            goal: FULL_PSG,
            envelope,
            phase: Phase::Attack,
            level: 0,
            counter: 0,
            source,
        };
        channel.begin_envelope();
        channel
    }

    fn is_psg(&self) -> bool {
        matches!(self.source, Source::Psg(_))
    }

    /// The programmable channel this voice drives, if any.
    #[must_use]
    pub fn psg_kind(&self) -> Option<PsgKind> {
        match &self.source {
            Source::Psg(channel) => Some(channel.kind()),
            Source::Sampled { .. } => None,
        }
    }

    /// Sets the volume a programmable channel's envelope rises to, from
    /// the note's volume (0–126): the driver's 0–15 goal, an eighth of the
    /// volume, with the sustain scaled to it.
    pub fn set_psg_goal(&mut self, volume: u8) {
        self.goal = (i32::from(volume) >> 3).min(FULL_PSG);
        if self.phase == Phase::Decay && self.level > self.goal {
            self.level = self.goal;
        }
    }

    fn psg_sustain(&self) -> i32 {
        (self.goal * i32::from(self.envelope.sustain.min(15)) + FULL_PSG) >> 4
    }

    fn begin_envelope(&mut self) {
        if self.is_psg() && self.envelope.attack == 0 {
            self.level = self.goal;
            self.phase = Phase::Decay;
            self.counter = self.envelope.decay;
        } else {
            self.level = 0;
            self.phase = Phase::Attack;
            self.counter = self.envelope.attack;
        }
    }

    /// Whether the channel is free.
    #[must_use]
    pub fn is_off(&self) -> bool {
        self.phase == Phase::Off
    }

    /// Whether the key is still down.
    #[must_use]
    pub fn is_held(&self) -> bool {
        matches!(self.phase, Phase::Attack | Phase::Decay | Phase::Sustain)
    }

    /// Envelope level, 0–255 for sampled voices, 0–15 for programmable ones.
    #[must_use]
    pub fn level(&self) -> i32 {
        self.level
    }

    /// Lets the key up.
    pub fn release(&mut self) {
        if self.is_held() {
            self.phase = Phase::Release;
            self.counter = self.envelope.release;
            if self.envelope.release == 0 {
                self.phase = Phase::Off;
                self.level = 0;
            }
        }
    }

    /// Silences the channel at once.
    pub fn kill(&mut self) {
        self.phase = Phase::Off;
        self.level = 0;
    }

    /// Sets the playback rate of a sampled voice or the frequency of a
    /// programmable one.
    pub fn set_pitch(&mut self, hertz: f64, mix_rate: f64) {
        match &mut self.source {
            Source::Sampled { step, .. } => *step = fixed_step(hertz / mix_rate),
            Source::Psg(channel) => channel.set_hertz(hertz, mix_rate),
        }
    }

    /// Advances the envelope by one frame.
    pub fn frame(&mut self) {
        self.age = self.age.saturating_add(1);
        if self.is_psg() {
            self.psg_frame();
        } else {
            self.sampled_frame();
        }
        if let Source::Psg(channel) = &mut self.source {
            channel.set_level(u8::try_from(self.level).unwrap_or(0));
        }
    }

    fn sampled_frame(&mut self) {
        let Envelope {
            attack,
            decay,
            sustain,
            release,
        } = self.envelope;
        match self.phase {
            Phase::Attack => {
                self.level += i32::from(attack);
                if self.level >= FULL {
                    self.level = FULL;
                    self.phase = Phase::Decay;
                }
            }
            Phase::Decay => {
                self.level = self.level * i32::from(decay) / 256;
                if self.level <= i32::from(sustain) {
                    self.level = i32::from(sustain);
                    self.phase = Phase::Sustain;
                }
            }
            Phase::Sustain => self.level = i32::from(sustain),
            Phase::Release => {
                self.level = self.level * i32::from(release) / 256;
                if self.level == 0 {
                    self.phase = Phase::Off;
                }
            }
            Phase::Off => {}
        }
        if self.phase == Phase::Sustain && sustain == 0 {
            self.phase = Phase::Off;
        }
    }

    fn psg_frame(&mut self) {
        let Envelope {
            attack,
            decay,
            release,
            ..
        } = self.envelope;
        let sustain = self.psg_sustain();
        match self.phase {
            Phase::Attack => {
                if self.tick(attack) {
                    self.level += 1;
                    if self.level >= self.goal {
                        self.level = self.goal;
                        self.phase = Phase::Decay;
                        self.counter = decay;
                    }
                }
            }
            Phase::Decay => {
                if decay == 0 {
                    self.level = sustain;
                    self.phase = Phase::Sustain;
                } else if self.tick(decay) {
                    self.level -= 1;
                    if self.level <= sustain {
                        self.level = sustain;
                        self.phase = Phase::Sustain;
                    }
                }
            }
            Phase::Sustain => {
                if sustain == 0 {
                    self.phase = Phase::Off;
                }
            }
            Phase::Release => {
                if release == 0 || self.tick(release) {
                    self.level -= 1;
                }
                if self.level <= 0 {
                    self.level = 0;
                    self.phase = Phase::Off;
                }
            }
            Phase::Off => {}
        }
    }

    fn tick(&mut self, period: u8) -> bool {
        if period == 0 {
            return true;
        }
        self.counter = self.counter.saturating_sub(1);
        if self.counter == 0 {
            self.counter = period;
            true
        } else {
            false
        }
    }

    /// The next output sample, scaled by the envelope and volume: sampled
    /// voices in -32768..=32767 for full scale, programmable ones in the
    /// hardware's -15..=15 steps.
    pub fn sample(&mut self, rom: &[u8]) -> i32 {
        match &mut self.source {
            Source::Psg(channel) => channel.sample(),
            Source::Sampled {
                sample,
                position,
                step,
            } => {
                let length = u64::from(sample.length) << FRACTION_BITS;
                if *position >= length {
                    let loop_start = u64::from(sample.loop_start) << FRACTION_BITS;
                    if sample.looped && length > loop_start {
                        *position = loop_start + (*position - loop_start) % (length - loop_start);
                    } else {
                        self.phase = Phase::Off;
                        self.level = 0;
                        return 0;
                    }
                }
                let index = usize::try_from(*position >> FRACTION_BITS).unwrap_or(0);
                let value = rom
                    .get(sample.data + index)
                    .map_or(0, |byte| i32::from(i8::from_le_bytes([*byte])));
                *position += *step;
                value * self.level * i32::from(self.volume) / (FULL * FULL) * 256
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_source(rom_len: u32) -> Source {
        Source::Sampled {
            sample: Sample {
                looped: false,
                frequency: 1024,
                loop_start: 0,
                length: rom_len,
                data: 0,
            },
            position: 0,
            step: 1 << FRACTION_BITS,
        }
    }

    #[test]
    fn a_sampled_envelope_rises_decays_holds_and_releases() {
        let envelope = Envelope {
            attack: 128,
            decay: 128,
            sustain: 32,
            release: 0,
        };
        let mut channel = Channel::start(0, 0, 60, envelope, sample_source(4));
        channel.frame();
        assert_eq!(channel.level(), 128);
        channel.frame();
        assert_eq!(channel.level(), 255);
        channel.frame();
        assert_eq!(channel.level(), 127);
        channel.frame();
        channel.frame();
        assert_eq!(channel.level(), 32);
        channel.release();
        assert!(channel.is_off());
    }

    #[test]
    fn a_sample_plays_through_and_stops() {
        let envelope = Envelope {
            attack: 255,
            decay: 255,
            sustain: 255,
            release: 255,
        };
        let rom = [64u8, 0x80, 0, 0];
        let mut channel = Channel::start(0, 0, 60, envelope, sample_source(2));
        channel.volume = 255;
        channel.frame();
        assert_eq!(channel.sample(&rom), 64 * 256);
        assert_eq!(channel.sample(&rom), -128 * 256);
        assert_eq!(channel.sample(&rom), 0);
        assert!(channel.is_off());
    }

    #[test]
    fn a_psg_envelope_counts_frames_per_step() {
        let envelope = Envelope {
            attack: 0,
            decay: 0,
            sustain: 7,
            release: 2,
        };
        let source = Source::Psg(PsgChannel::new(PsgKind::Square1));
        let mut channel = Channel::start(0, 0, 60, envelope, source);
        assert_eq!(channel.level(), 15);
        channel.frame();
        assert_eq!(channel.level(), 7);
        let source = Source::Psg(PsgChannel::new(PsgKind::Square2));
        let mut half = Channel::start(0, 0, 60, envelope, source);
        half.set_psg_goal(66);
        half.frame();
        assert_eq!(half.level(), 4);
        channel.release();
        channel.frame();
        assert_eq!(channel.level(), 7);
        channel.frame();
        assert_eq!(channel.level(), 6);
    }
}
