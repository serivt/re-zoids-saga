//! Audio synthesis: the sequenced sound driver's players, their voices and
//! the per-frame mix the game hears.
//!
//! Source of knowledge: the driver settings of Zoids Saga read from its
//! RAM while it played (31536 Hz, 528 samples per frame, 8 sampled
//! channels, one mono buffer read by both sound DMAs, master volume 14),
//! the song table at ROM `0x567768` and the public description of the
//! driver's data.

pub mod channel;
pub mod m4a;
pub mod psg;

use formats::m4a::{M4aError, Sample, SongHeader, Voice, rom_offset};

use channel::{Channel, Source};
use m4a::{Event, Player};
use psg::{PsgChannel, PsgKind};

/// Output sample rate in hertz.
pub const SAMPLE_RATE: u32 = 31_536;
/// Samples the driver mixes per frame.
pub const SAMPLES_PER_FRAME: usize = 528;
/// Music players the driver keeps.
pub const PLAYERS: usize = 4;
const SAMPLED_CHANNELS: usize = 8;
const SONG_ENTRY_SIZE: usize = 8;
const MIDDLE_KEY: f64 = 60.0;
const CONCERT_KEY: f64 = 69.0;
/// A direct-sound channel at full volume puts its byte ×4 on the 10-bit
/// output; the programmable channels keep four times the sampled mix's
/// weight.
const SAMPLED_SCALE: i32 = 4;
const MASTER_VOLUME_STEPS: i32 = 16;
const PSG_SCALE: i32 = 16;
/// The 10-bit output's range around its bias, where the hardware clips.
const OUTPUT_LIMIT: i32 = 512;
/// The 10-bit output's full range fills the 16-bit samples.
const OUTPUT_GAIN: i32 = 64;
const WAVE_PATTERN_SIZE: usize = 16;
const REVERB_APPLIES: u8 = 0x80;
const REVERB_DELAY_FRAMES: usize = 3;
const REVERB_STEPS: i32 = 256;

/// The sound driver: song table, players and mixer.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundEngine<'rom> {
    rom: &'rom [u8],
    song_table: usize,
    song_count: usize,
    players: [Player; PLAYERS],
    channels: Vec<Channel>,
    events: Vec<Event>,
    output: Vec<i16>,
    sampled: Vec<i8>,
    history: Vec<i8>,
    reverb: u8,
    master_volume: u8,
}

impl<'rom> SoundEngine<'rom> {
    /// Creates the engine over the song table at `song_table` with
    /// `song_count` entries, mixing at `master_volume` (0–15, the driver's
    /// own setting: 15 leaves samples at half scale).
    #[must_use]
    pub fn new(rom: &'rom [u8], song_table: usize, song_count: usize, master_volume: u8) -> Self {
        Self {
            rom,
            song_table,
            song_count,
            players: [Player::new(), Player::new(), Player::new(), Player::new()],
            channels: Vec::new(),
            events: Vec::new(),
            output: vec![0; SAMPLES_PER_FRAME * 2],
            sampled: vec![0; SAMPLES_PER_FRAME],
            history: vec![0; SAMPLES_PER_FRAME * REVERB_DELAY_FRAMES],
            reverb: 0,
            master_volume: master_volume.min(15),
        }
    }

    /// Reads the header of song `song` and the player it belongs to.
    ///
    /// # Errors
    ///
    /// Returns [`M4aError`] when the song is not in the table or unreadable.
    pub fn song(&self, song: usize) -> Result<(SongHeader, usize), M4aError> {
        let at = self.song_table + song * SONG_ENTRY_SIZE;
        if song >= self.song_count || at + SONG_ENTRY_SIZE > self.rom.len() {
            return Err(M4aError::Truncated { offset: at });
        }
        let pointer = u32::from_le_bytes([
            self.rom[at],
            self.rom[at + 1],
            self.rom[at + 2],
            self.rom[at + 3],
        ]);
        let player = usize::from(u16::from_le_bytes([self.rom[at + 4], self.rom[at + 5]]));
        let header = SongHeader::read(self.rom, rom_offset(pointer, at)?)?;
        Ok((header, player.min(PLAYERS - 1)))
    }

    /// Starts song `song` on its player, restarting it if it was playing.
    ///
    /// # Errors
    ///
    /// Returns [`M4aError`] when the song cannot be read.
    pub fn play(&mut self, song: usize) -> Result<(), M4aError> {
        let (header, player) = self.song(song)?;
        self.silence(player, None);
        if header.reverb & REVERB_APPLIES != 0 {
            self.reverb = header.reverb & !REVERB_APPLIES;
        }
        self.players[player].start(song, &header);
        Ok(())
    }

    /// Starts song `song` unless its player is already playing it.
    ///
    /// # Errors
    ///
    /// Returns [`M4aError`] when the song cannot be read.
    pub fn play_if_changed(&mut self, song: usize) -> Result<(), M4aError> {
        let (_, player) = self.song(song)?;
        if self.players[player].song() == Some(song) {
            return Ok(());
        }
        self.play(song)
    }

    /// Stops the music player.
    pub fn stop_music(&mut self) {
        self.silence(0, None);
        self.players[0].stop();
    }

    /// Voices sounding now: sampled ones and programmable ones.
    #[must_use]
    pub fn voice_counts(&self) -> (usize, usize) {
        let psg = self
            .channels
            .iter()
            .filter(|channel| channel.psg_kind().is_some() && channel.level() > 0)
            .count();
        let sampled = self
            .channels
            .iter()
            .filter(|channel| channel.psg_kind().is_none() && channel.level() > 0)
            .count();
        (sampled, psg)
    }

    /// The last frame's samples, stereo interleaved.
    #[must_use]
    pub fn output(&self) -> &[i16] {
        &self.output
    }

    /// The last frame's mix of the sampled channels alone, in the driver's
    /// own 8-bit units (what its sound buffer holds).
    #[must_use]
    pub fn sampled_buffer(&self) -> &[i8] {
        &self.sampled
    }

    /// Whether song `song` has ended: its player no longer plays it (the
    /// status's pause bit, `0x08001A28`). A player sees its last track end
    /// in the frame after, as the driver sets that bit.
    #[must_use]
    pub fn song_ended(&self, song: usize) -> bool {
        let Ok((_, player)) = self.song(song) else {
            return true;
        };
        self.players
            .get(player)
            .is_none_or(|player| player.song() != Some(song))
    }

    /// Song on player `player`, if one is playing.
    #[must_use]
    pub fn playing(&self, player: usize) -> Option<usize> {
        self.players.get(player).and_then(Player::song)
    }

    /// Advances one frame and returns its samples, stereo interleaved.
    ///
    /// # Errors
    ///
    /// Returns [`M4aError`] when a song's data cannot be read.
    pub fn frame(&mut self) -> Result<&[i16], M4aError> {
        self.events.clear();
        let mut events = std::mem::take(&mut self.events);
        for player in &mut self.players {
            if player.song().is_some() && player.is_finished() {
                player.stop();
            }
        }
        for (index, player) in self.players.iter_mut().enumerate() {
            player.frame(self.rom, index, &mut events)?;
        }
        for event in events.drain(..) {
            self.handle(event)?;
        }
        self.events = events;
        self.age_channels();
        self.mix();
        Ok(&self.output)
    }

    fn handle(&mut self, event: Event) -> Result<(), M4aError> {
        match event {
            Event::NoteOn {
                player,
                track,
                key,
                velocity,
                gate,
            } => self.note_on(player, track, key, velocity, gate),
            Event::EndTie { player, track } => {
                for channel in &mut self.channels {
                    if channel.player == player && channel.track == track && channel.tied {
                        channel.release();
                    }
                }
                Ok(())
            }
            Event::Silence { player, track } => {
                self.silence(player, track);
                Ok(())
            }
            Event::Tick { player } => {
                for channel in &mut self.channels {
                    if channel.player == player && !channel.tied && channel.is_held() {
                        channel.gate = channel.gate.saturating_sub(1);
                        if channel.gate == 0 {
                            channel.release();
                        }
                    }
                }
                Ok(())
            }
            Event::TrackEnd { player, track } => {
                for channel in &mut self.channels {
                    if channel.player == player && channel.track == track {
                        channel.release();
                    }
                }
                Ok(())
            }
        }
    }

    fn silence(&mut self, player: usize, track: Option<usize>) {
        for channel in &mut self.channels {
            if channel.player == player && track.is_none_or(|track| track == channel.track) {
                channel.kill();
            }
        }
    }

    fn resolve_voice(&self, group: usize, index: u8, key: u8) -> Result<(Voice, u8), M4aError> {
        match Voice::read(self.rom, group, index)? {
            Voice::KeySplit { group, table } => {
                let sub = self
                    .rom
                    .get(table + usize::from(key))
                    .copied()
                    .ok_or(M4aError::Truncated { offset: table })?;
                Ok((Voice::read(self.rom, group, sub)?, key))
            }
            Voice::Drums { group } => {
                let voice = Voice::read(self.rom, group, key)?;
                let base = match &voice {
                    Voice::Sample { base_key, .. }
                    | Voice::Square { base_key, .. }
                    | Voice::Wave { base_key, .. }
                    | Voice::Noise { base_key, .. } => *base_key,
                    _ => key,
                };
                Ok((voice, base))
            }
            voice => Ok((voice, key)),
        }
    }

    fn note_on(
        &mut self,
        player: usize,
        track: usize,
        key: u8,
        velocity: u8,
        gate: Option<u32>,
    ) -> Result<(), M4aError> {
        let group = self.players[player].voices();
        let voice_index = self.players[player].tracks()[track].voice;
        let (voice, key) = self.resolve_voice(group, voice_index, key)?;
        let (envelope, source) = match voice {
            Voice::Sample {
                sample, envelope, ..
            } => (
                envelope,
                Source::Sampled {
                    sample: Sample::read(self.rom, sample)?,
                    position: 0,
                    step: 0,
                },
            ),
            Voice::Square {
                channel,
                duty,
                envelope,
                ..
            } => {
                let kind = if channel == 1 {
                    PsgKind::Square1
                } else {
                    PsgKind::Square2
                };
                let mut psg = PsgChannel::new(kind);
                psg.set_duty(duty);
                (envelope, Source::Psg(psg))
            }
            Voice::Wave {
                pattern, envelope, ..
            } => {
                let mut psg = PsgChannel::new(PsgKind::Wave);
                let bytes: [u8; WAVE_PATTERN_SIZE] = self
                    .rom
                    .get(pattern..pattern + WAVE_PATTERN_SIZE)
                    .and_then(|slice| slice.try_into().ok())
                    .ok_or(M4aError::Truncated { offset: pattern })?;
                psg.set_pattern(&bytes);
                (envelope, Source::Psg(psg))
            }
            Voice::Noise {
                short, envelope, ..
            } => {
                let mut psg = PsgChannel::new(PsgKind::Noise);
                psg.set_short_noise(short);
                (envelope, Source::Psg(psg))
            }
            Voice::KeySplit { .. } | Voice::Drums { .. } | Voice::Unknown(_) => return Ok(()),
        };
        let mut channel = Channel::start(track, player, key, envelope, source);
        channel.tied = gate.is_none();
        channel.gate = gate.unwrap_or(0);
        channel.priority = self.players[player].tracks()[track].priority;
        channel.volume = scaled_volume(velocity, self.players[player].tracks()[track].volume);
        channel.set_psg_goal(channel.volume);
        self.set_pitch(&mut channel, &voice);
        self.place(channel);
        Ok(())
    }

    fn set_pitch(&self, channel: &mut Channel, voice: &Voice) {
        let track = &self.players[channel.player].tracks()[channel.track];
        let offset = track.pitch_offset();
        let hertz = match voice {
            Voice::Sample { fixed, sample, .. } => {
                let base =
                    f64::from(Sample::read(self.rom, *sample).map_or(0, |s| s.frequency)) / 1024.0;
                if *fixed {
                    base
                } else {
                    base * semitones(f64::from(channel.key) - MIDDLE_KEY + offset)
                }
            }
            Voice::Square { .. } | Voice::Wave { .. } | Voice::Noise { .. } => {
                let key = f64::from(channel.key) + offset;
                psg::tone_hertz(psg::tone_register(440.0 * semitones(key - CONCERT_KEY)))
            }
            _ => 0.0,
        };
        channel.set_pitch(hertz, f64::from(SAMPLE_RATE));
    }

    fn place(&mut self, channel: Channel) {
        if let Some(kind) = channel.psg_kind() {
            self.channels
                .retain(|other| other.psg_kind() != Some(kind) || !other.is_held());
            for other in &mut self.channels {
                if other.psg_kind() == Some(kind) {
                    other.kill();
                }
            }
            self.channels.push(channel);
            return;
        }
        let sampled = self
            .channels
            .iter()
            .filter(|other| other.psg_kind().is_none() && !other.is_off())
            .count();
        if sampled >= SAMPLED_CHANNELS {
            let victim = self
                .channels
                .iter()
                .enumerate()
                .filter(|(_, other)| other.psg_kind().is_none() && !other.is_off())
                .min_by_key(|(_, other)| (other.priority, std::cmp::Reverse(other.age)))
                .map(|(index, _)| index);
            if let Some(index) = victim {
                self.channels.remove(index);
            }
        }
        self.channels.push(channel);
    }

    fn age_channels(&mut self) {
        for channel in &mut self.channels {
            channel.frame();
        }
        self.channels.retain(|channel| !channel.is_off());
    }

    /// Mixes one frame. The driver's reverb adds, to every sample, the
    /// sample its buffer held three frames earlier scaled by the song's
    /// reverb over 256; the history keeps those frames.
    fn mix(&mut self) {
        let rom = self.rom;
        self.history.rotate_left(SAMPLES_PER_FRAME);
        for index in 0..SAMPLES_PER_FRAME {
            let mut sampled = 0i32;
            let mut psg = 0i32;
            for channel in &mut self.channels {
                let value = channel.sample(rom);
                if channel.psg_kind().is_some() {
                    psg += value;
                } else {
                    sampled += value;
                }
            }
            let oldest = self.history[(REVERB_DELAY_FRAMES - 1) * SAMPLES_PER_FRAME + index];
            let echo = i32::from(oldest) * i32::from(self.reverb) / REVERB_STEPS;
            let sampled = (sampled / 256 * i32::from(self.master_volume) / MASTER_VOLUME_STEPS
                + echo)
                .clamp(-128, 127);
            self.sampled[index] = i8::try_from(sampled).unwrap_or(0);
            self.history[(REVERB_DELAY_FRAMES - 1) * SAMPLES_PER_FRAME + index] =
                self.sampled[index];
            let value = (sampled * SAMPLED_SCALE + psg * PSG_SCALE)
                .clamp(-OUTPUT_LIMIT, OUTPUT_LIMIT - 1)
                * OUTPUT_GAIN;
            let value = i16::try_from(value).unwrap_or(0);
            self.output[index * 2] = value;
            self.output[index * 2 + 1] = value;
        }
    }
}

/// A note's volume as the driver computes it: velocity times track volume
/// over 128, so 0–126.
fn scaled_volume(velocity: u8, volume: u8) -> u8 {
    u8::try_from((u32::from(velocity.min(127)) * u32::from(volume.min(127))) >> 7).unwrap_or(126)
}

fn semitones(offset: f64) -> f64 {
    (offset / 12.0).exp2()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn ptr(offset: u32) -> [u8; 4] {
        (0x0800_0000 + offset).to_le_bytes()
    }

    fn rom() -> Vec<u8> {
        let mut rom = vec![0; 256];
        rom[0..4].copy_from_slice(&ptr(16));
        rom[8..12].copy_from_slice(&ptr(32));
        rom[12..14].copy_from_slice(&2u16.to_le_bytes());
        rom[16..24].copy_from_slice(&[1, 0, 128, 0, 0, 0, 0, 0]);
        rom[20..24].copy_from_slice(&ptr(48));
        rom[24..28].copy_from_slice(&ptr(96));
        rom[32..40].copy_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
        rom[48..60].copy_from_slice(&[0x00, 60, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255]);
        rom[52..56].copy_from_slice(&ptr(128));
        rom[60..72].copy_from_slice(&[0x01, 60, 0, 0, 2, 0, 0, 0, 0, 0, 15, 0]);
        rom[96..107].copy_from_slice(&[0xBB, 75, 0xBD, 0, 0xBE, 127, 0xD1, 60, 127, 0x82, 0xB1]);
        rom[128..132].copy_from_slice(&0u32.to_le_bytes());
        rom[132..136].copy_from_slice(&(SAMPLE_RATE * 1024).to_le_bytes());
        rom[140..144].copy_from_slice(&64u32.to_le_bytes());
        for index in 0..64 {
            rom[144 + index] = 100;
        }
        rom
    }

    #[test]
    fn a_song_plays_its_sample_and_stops() {
        let rom = rom();
        let mut engine = SoundEngine::new(&rom, 0, 2, 16);
        engine.play(0).unwrap();
        assert_eq!(engine.playing(0), Some(0));
        let frame = engine.frame().unwrap();
        assert_eq!(frame.len(), SAMPLES_PER_FRAME * 2);
        assert_eq!(
            i32::from(frame[0]),
            100 * 126 / 255 * 15 / 16 * SAMPLED_SCALE * OUTPUT_GAIN
        );
        assert_eq!(frame[0], frame[1]);
        assert!(frame[63 * 2] != 0 && frame[64 * 2] == 0);
        assert_eq!(
            i32::from(engine.sampled_buffer()[0]),
            100 * 126 / 255 * 15 / 16
        );
        engine.frame().unwrap();
        assert_eq!(engine.playing(0), Some(0));
        assert!(!engine.song_ended(0));
        engine.frame().unwrap();
        assert_eq!(engine.playing(0), Some(0));
        engine.frame().unwrap();
        assert_eq!(engine.playing(0), None);
        assert!(engine.song_ended(0));
        engine.play(1).unwrap();
        assert_eq!(engine.playing(2), None);
        assert!(engine.play(2).is_err());
    }

    #[test]
    fn play_if_changed_keeps_a_running_song() {
        let rom = rom();
        let mut engine = SoundEngine::new(&rom, 0, 2, 14);
        engine.play(0).unwrap();
        let before = engine.clone();
        engine.play_if_changed(0).unwrap();
        assert_eq!(engine, before);
        engine.stop_music();
        assert_eq!(engine.playing(0), None);
    }

    #[test]
    fn volumes_scale_by_velocity_and_track_volume() {
        assert_eq!(scaled_volume(127, 127), 126);
        assert_eq!(scaled_volume(127, 0), 0);
        assert_eq!(scaled_volume(92, 44), 31);
        assert_eq!(scaled_volume(76, 47), 27);
    }
}
