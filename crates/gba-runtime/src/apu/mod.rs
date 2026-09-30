//! Audio synthesis: the sequenced sound driver's players, their voices and
//! the per-frame mix the game hears.
//!
//! Source of knowledge: own reading of Zoids Saga's (Japan, Rev 1) sound
//! driver: the note routine that gives a note its channel (`0x0805BA00`),
//! the channels' volumes (`0x0805B9D0`), the frame's order (`0x0805B108`:
//! players, programmable channels, then the mixer), and the settings read
//! from its RAM (31536 Hz, 528 samples per frame, 8 sampled channels, one
//! mono buffer of three frames, master volume 14); the hardware's mix of
//! the programmable channels and the sampled buffer from public hardware
//! documentation.

pub mod cgb;
pub mod m4a;
pub mod psg;
pub mod sampled;
pub mod tables;

use formats::m4a::{M4aError, Sample, SongHeader, Voice, rom_offset, voice_pan};

use cgb::CgbChannel;
use m4a::{Event, PITCH_CHANGED, Player, VOLUME_CHANGED};
use psg::PsgKind;
use sampled::{STARTING, STOPPING, SampledChannel};
pub use tables::{DriverLayout, Tables};

/// Output sample rate in hertz.
pub const SAMPLE_RATE: u32 = 31_536;
/// Samples the driver mixes per frame.
pub const SAMPLES_PER_FRAME: usize = 528;
/// Music players the driver keeps.
pub const PLAYERS: usize = 4;
const SAMPLED_CHANNELS: usize = 8;
const SONG_ENTRY_SIZE: usize = 8;
const BUFFER_FRAMES: usize = 3;
const REVERB_APPLIES: u8 = 0x80;
/// The mixer's rate step per unit of a channel's rate (`SoundInfo.divFreq`,
/// 2²³ over the mix rate), in 23-bit fractions of a sample.
const DIVIDED_FREQUENCY: u32 = 266;
/// The driver's counter of frames, 14 down to 0, whose 0 steps the
/// programmable envelopes once more.
const EXTRA_STEP_PERIOD: u8 = 14;
/// The hardware's levels: a sampled byte counts 4 at full DMA volume, a
/// programmable channel's step counts the master volume (7) plus one, and
/// the sum is clipped to the 10-bit output around its bias.
const DMA_SCALE: i32 = 4;
const PSG_SCALE: i32 = 8;
const OUTPUT_LIMIT: i32 = 512;
/// The 10-bit output's full range fills the 16-bit samples.
const OUTPUT_GAIN: i32 = 64;
const KIT_PAN: u8 = 0x80;
const KIT_PAN_CENTER: i32 = 0xC0;
const SUB_GROUP_TYPES: u8 = 0xC0;

/// A note as its track plays it.
#[derive(Clone, Copy)]
struct Note {
    player: usize,
    track: usize,
    key: u8,
    velocity: u8,
    gate: Option<u32>,
}

/// What ties a channel to the note that plays on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Link {
    player: usize,
    track: usize,
    linked: bool,
    serial: u64,
    key: u8,
    pitch_key: u8,
    velocity: u8,
    rhythm_pan: i8,
    gate: u8,
    priority: u8,
}

impl Link {
    /// The order of the note's track as the driver compares track
    /// addresses: players' tracks lie in player order, and a channel no
    /// longer tied to a track counts lowest.
    fn order(&self) -> usize {
        if self.linked {
            1 + self.player * 16 + self.track
        } else {
            0
        }
    }

    fn belongs(&self, player: usize, track: usize) -> bool {
        self.linked && self.player == player && self.track == track
    }
}

/// The sound driver: song table, players and mixer.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundEngine<'rom> {
    rom: &'rom [u8],
    layout: DriverLayout,
    tables: Tables,
    players: [Player; PLAYERS],
    sampled: [SampledChannel; SAMPLED_CHANNELS],
    sampled_links: [Link; SAMPLED_CHANNELS],
    cgb: [CgbChannel; 4],
    cgb_links: [Link; 4],
    events: Vec<Event>,
    output: Vec<i16>,
    buffer: Vec<i8>,
    slot: usize,
    reverb: u8,
    extra_counter: u8,
    serial: u64,
}

impl<'rom> SoundEngine<'rom> {
    /// Creates the engine over the driver's song table and tables.
    #[must_use]
    pub fn new(rom: &'rom [u8], layout: DriverLayout) -> Self {
        Self {
            rom,
            layout: DriverLayout {
                master_volume: layout.master_volume.min(15),
                ..layout
            },
            tables: Tables::read(rom, &layout),
            players: [Player::new(), Player::new(), Player::new(), Player::new()],
            sampled: std::array::from_fn(|_| SampledChannel::default()),
            sampled_links: [Link::default(); SAMPLED_CHANNELS],
            cgb: [
                CgbChannel::new(PsgKind::Square1),
                CgbChannel::new(PsgKind::Square2),
                CgbChannel::new(PsgKind::Wave),
                CgbChannel::new(PsgKind::Noise),
            ],
            cgb_links: [Link::default(); 4],
            events: Vec::new(),
            output: vec![0; SAMPLES_PER_FRAME * 2],
            buffer: vec![0; SAMPLES_PER_FRAME * BUFFER_FRAMES],
            slot: 0,
            reverb: 0,
            extra_counter: 0,
            serial: 0,
        }
    }

    /// Reads the header of song `song` and the player it belongs to.
    ///
    /// # Errors
    ///
    /// Returns [`M4aError`] when the song is not in the table or unreadable.
    pub fn song(&self, song: usize) -> Result<(SongHeader, usize), M4aError> {
        let at = self.layout.song_table + song * SONG_ENTRY_SIZE;
        if song >= self.layout.song_count || at + SONG_ENTRY_SIZE > self.rom.len() {
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

    /// Starts song `song` on its player, restarting it if it was playing
    /// (`0x0805C4A4`: the player's notes stop at once).
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

    /// Stops every player, the music's and the sound effects'
    /// (`0x0805BEEC`, `0x0805C588` on each).
    pub fn stop_all(&mut self) {
        for player in 0..PLAYERS {
            self.silence(player, None);
            self.players[player].stop();
        }
    }

    /// Voices sounding now: sampled ones and programmable ones.
    #[must_use]
    pub fn voice_counts(&self) -> (usize, usize) {
        let sampled = self
            .sampled
            .iter()
            .filter(|channel| channel.is_active() && channel.level > 0)
            .count();
        let psg = self
            .cgb
            .iter()
            .filter(|channel| channel.is_active() && channel.level > 0)
            .count();
        (sampled, psg)
    }

    /// The sampled channels, in the driver's order.
    #[must_use]
    pub fn sampled_channels(&self) -> &[SampledChannel] {
        &self.sampled
    }

    /// The programmable channels: square 1, square 2, wave and noise.
    #[must_use]
    pub fn cgb_channels(&self) -> &[CgbChannel] {
        &self.cgb
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
        let start = self.slot * SAMPLES_PER_FRAME;
        &self.buffer[start..start + SAMPLES_PER_FRAME]
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

    /// Advances one frame and returns its samples, stereo interleaved: the
    /// players (from player 0 up), the notes' volumes and pitches, the
    /// programmable channels, then the mixer.
    ///
    /// # Errors
    ///
    /// Returns [`M4aError`] when a song's data cannot be read.
    pub fn frame(&mut self) -> Result<&[i16], M4aError> {
        let mut events = std::mem::take(&mut self.events);
        events.clear();
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
        self.update_notes();
        self.run_cgb();
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
                volumes,
                pitch,
            } => self.note_on(
                Note {
                    player,
                    track,
                    key,
                    velocity,
                    gate,
                },
                volumes,
                pitch,
            ),
            Event::EndTie { player, track, key } => {
                self.end_tie(player, track, key);
                Ok(())
            }
            Event::Silence { player, track } => {
                self.silence(player, track);
                Ok(())
            }
            Event::Tick { player, track } => {
                self.count_gates(player, track);
                Ok(())
            }
            Event::TrackEnd { player, track } => {
                self.end_track(player, track);
                Ok(())
            }
        }
    }

    /// Each tick, before its track's commands, a note's gate counts down;
    /// at 0 its key goes up.
    fn count_gates(&mut self, player: usize, track: usize) {
        let sampled = self
            .sampled
            .iter_mut()
            .zip(&mut self.sampled_links)
            .map(|(channel, link)| (&mut channel.status, link));
        let cgb = self
            .cgb
            .iter_mut()
            .zip(&mut self.cgb_links)
            .map(|(channel, link)| (&mut channel.status, link));
        for (status, link) in sampled.chain(cgb) {
            if !link.belongs(player, track) || *status & sampled::ACTIVE == 0 {
                continue;
            }
            if link.gate != 0 {
                link.gate -= 1;
                if link.gate == 0 {
                    *status |= STOPPING;
                }
            }
        }
    }

    /// The track's latest note of `key` still held lets go
    /// (`0x0805BC00`).
    fn end_tie(&mut self, player: usize, track: usize, key: u8) {
        let held = |status: u8, link: &Link| {
            link.belongs(player, track)
                && status & (STARTING | 0x03) != 0
                && status & STOPPING == 0
                && link.key == key
        };
        let sampled = (0..SAMPLED_CHANNELS)
            .filter(|&index| held(self.sampled[index].status, &self.sampled_links[index]))
            .map(|index| (self.sampled_links[index].serial, false, index));
        let cgb = (0..4)
            .filter(|&index| held(self.cgb[index].status, &self.cgb_links[index]))
            .map(|index| (self.cgb_links[index].serial, true, index));
        match sampled.chain(cgb).max() {
            Some((_, false, index)) => self.sampled[index].status |= STOPPING,
            Some((_, true, index)) => self.cgb[index].status |= STOPPING,
            None => {}
        }
    }

    /// A track's end lets its notes go and unties them (`0x0805B4F4`).
    fn end_track(&mut self, player: usize, track: usize) {
        for (channel, link) in self.sampled.iter_mut().zip(&mut self.sampled_links) {
            if link.belongs(player, track) {
                if channel.is_active() {
                    channel.status |= STOPPING;
                }
                link.linked = false;
            }
        }
        for (channel, link) in self.cgb.iter_mut().zip(&mut self.cgb_links) {
            if link.belongs(player, track) {
                if channel.is_active() {
                    channel.status |= STOPPING;
                }
                link.linked = false;
            }
        }
    }

    /// Stops a track's notes at once and unties them (`0x0805B98C`).
    fn silence(&mut self, player: usize, track: Option<usize>) {
        let matches = |link: &Link| {
            link.linked && link.player == player && track.is_none_or(|track| link.track == track)
        };
        for (channel, link) in self.sampled.iter_mut().zip(&mut self.sampled_links) {
            if matches(link) {
                channel.status = sampled::FREE;
                link.linked = false;
            }
        }
        for (channel, link) in self.cgb.iter_mut().zip(&mut self.cgb_links) {
            if matches(link) {
                channel.status = sampled::FREE;
                channel.level = 0;
                link.linked = false;
            }
        }
    }

    /// The voice a note plays, the key it plays it at, and a drum kit's
    /// pan: a key split picks its sub-voice by the key, a drum kit by the
    /// key too, playing the drum at its own key; a sub-voice that is
    /// itself a split or kit plays nothing.
    fn resolve_voice(&self, group: usize, index: u8, key: u8) -> Option<(Voice, u8, i8)> {
        let voice = Voice::read(self.rom, group, index).ok()?;
        let (voice, key, pan) = match voice {
            Voice::KeySplit { group, table } => {
                let sub = *self.rom.get(table + usize::from(key))?;
                (self.sub_voice(group, sub)?, key, 0)
            }
            Voice::Drums { group } => {
                let voice = self.sub_voice(group, key)?;
                let base = match &voice {
                    Voice::Sample { base_key, .. }
                    | Voice::Square { base_key, .. }
                    | Voice::Wave { base_key, .. }
                    | Voice::Noise { base_key, .. } => *base_key,
                    _ => key,
                };
                let pan = voice_pan(self.rom, group, key).ok()?;
                let pan = if pan & KIT_PAN == 0 {
                    0
                } else {
                    let centered = (i32::from(pan) - KIT_PAN_CENTER) * 2;
                    i8::from_le_bytes([u8::try_from(centered & 0xFF).unwrap_or(0)])
                };
                (voice, base, pan)
            }
            voice => (voice, key, 0),
        };
        Some((voice, key, pan))
    }

    fn sub_voice(&self, group: usize, index: u8) -> Option<Voice> {
        let kind = *self.rom.get(group + usize::from(index) * 12)?;
        if kind & SUB_GROUP_TYPES != 0 {
            return None;
        }
        Voice::read(self.rom, group, index).ok()
    }

    /// Starts a note (`0x0805BA00`): its priority is the song's plus the
    /// track's, it takes a channel as the driver gives them out, and its
    /// volumes and rate come from the track as it stands at the note.
    fn note_on(&mut self, note: Note, volumes: (u8, u8), pitch: (i8, u8)) -> Result<(), M4aError> {
        let Note {
            player,
            track,
            key,
            velocity,
            gate,
        } = note;
        let group = self.players[player].voices();
        let state = &self.players[player].tracks()[track];
        let Some((voice, pitch_key, rhythm_pan)) = self.resolve_voice(group, state.voice, key)
        else {
            return Ok(());
        };
        let priority = self.players[player]
            .priority()
            .saturating_add(state.priority);
        self.serial += 1;
        let link = Link {
            player,
            track,
            linked: true,
            serial: self.serial,
            key,
            pitch_key,
            velocity,
            rhythm_pan,
            gate: u8::try_from(gate.unwrap_or(0)).unwrap_or(u8::MAX),
            priority,
        };
        let (right, left) = note_volumes(&link, volumes);
        let (keys, fine) = pitch;
        let sounding = pitch_key_of(pitch_key, keys);
        match voice {
            Voice::Sample {
                fixed,
                sample,
                envelope,
                ..
            } => {
                let Some(index) = self.sampled_channel_for(&link) else {
                    return Ok(());
                };
                let sample = Sample::read(self.rom, sample)?;
                let mut channel = SampledChannel::start(envelope, fixed, sample);
                channel.right = right;
                channel.left = left;
                channel.frequency = self
                    .tables
                    .sample_frequency(sample.frequency, sounding, fine);
                self.sampled[index] = channel;
                self.sampled_links[index] = link;
            }
            Voice::Square { envelope, duty, .. } => {
                let kind = PsgKind::from_square(&voice);
                let frequency = self.tables.tone_frequency(sounding, fine);
                self.start_cgb(
                    kind,
                    link,
                    (envelope, right, left, frequency),
                    |generator| {
                        generator.set_duty(duty);
                    },
                );
            }
            Voice::Wave {
                envelope, pattern, ..
            } => {
                let bytes: [u8; 16] = self
                    .rom
                    .get(pattern..pattern + 16)
                    .and_then(|slice| slice.try_into().ok())
                    .ok_or(M4aError::Truncated { offset: pattern })?;
                let frequency = self.tables.tone_frequency(sounding, fine);
                self.start_cgb(
                    PsgKind::Wave,
                    link,
                    (envelope, right, left, frequency),
                    |generator| {
                        generator.set_pattern(&bytes);
                    },
                );
            }
            Voice::Noise {
                envelope, short, ..
            } => {
                let frequency = u32::from(self.tables.noise_frequency(sounding));
                self.start_cgb(
                    PsgKind::Noise,
                    link,
                    (envelope, right, left, frequency),
                    |generator| {
                        generator.set_short_noise(short);
                    },
                );
            }
            Voice::KeySplit { .. } | Voice::Drums { .. } | Voice::Unknown(_) => {}
        }
        Ok(())
    }

    /// The sampled channel a note takes (`0x0805BA00`): the first free
    /// one; else the lowest in priority among those letting go, if any,
    /// else among all whose priority is at most the note's, a tie going to
    /// the note of the later track and then to the later channel; or none.
    fn sampled_channel_for(&self, note: &Link) -> Option<usize> {
        let mut best = None;
        let mut priority = note.priority;
        let mut order = note.order();
        let mut releasing = false;
        for (index, (channel, link)) in self.sampled.iter().zip(&self.sampled_links).enumerate() {
            if !channel.is_active() {
                return Some(index);
            }
            if channel.is_stopping() {
                if !releasing {
                    releasing = true;
                    priority = link.priority;
                    order = link.order();
                    best = Some(index);
                    continue;
                }
            } else if releasing {
                continue;
            }
            if link.priority < priority {
                priority = link.priority;
                order = link.order();
                best = Some(index);
            } else if link.priority == priority && link.order() >= order {
                order = link.order();
                best = Some(index);
            }
        }
        best
    }

    /// Starts a note on the programmable channel of `kind` when it is
    /// free, letting go, of lower priority, or of the same priority and
    /// the same or a later track (`0x0805BA00`).
    fn start_cgb(
        &mut self,
        kind: PsgKind,
        note: Link,
        (envelope, right, left, frequency): (formats::m4a::Envelope, u8, u8, u32),
        set_up: impl FnOnce(&mut psg::PsgChannel),
    ) {
        let index = kind.index();
        let channel = &mut self.cgb[index];
        let link = &self.cgb_links[index];
        let takes = !channel.is_active()
            || channel.is_stopping()
            || link.priority < note.priority
            || (link.priority == note.priority && link.order() >= note.order());
        if !takes {
            return;
        }
        channel.status = STARTING;
        channel.envelope = envelope;
        channel.right = right;
        channel.left = left;
        channel.frequency = frequency;
        set_up(channel.generator_mut());
        self.cgb_links[index] = note;
    }

    /// Passes the frame's changes of each track to its notes, as the
    /// driver does after the ticks (`0x0805B8C4`): a note clears its
    /// track's pending changes, so the notes before it keep their volumes
    /// and rates.
    fn update_notes(&mut self) {
        for player in 0..PLAYERS {
            for track in 0..self.players[player].tracks().len() {
                let changes = self.players[player].take_changes(track);
                if changes != 0 {
                    self.update_track(player, track, changes);
                }
            }
        }
    }

    fn update_track(&mut self, player: usize, track: usize, changes: u8) {
        let state = &self.players[player].tracks()[track];
        let volumes = state.volumes();
        let (keys, fine) = state.pitch();
        for (channel, link) in self.sampled.iter_mut().zip(&self.sampled_links) {
            if !link.belongs(player, track) || !channel.is_active() {
                continue;
            }
            if changes & VOLUME_CHANGED != 0 {
                (channel.right, channel.left) = note_volumes(link, volumes);
            }
            if changes & PITCH_CHANGED != 0 {
                let key = pitch_key_of(link.pitch_key, keys);
                channel.frequency = channel.sample.map_or(0, |sample| {
                    self.tables.sample_frequency(sample.frequency, key, fine)
                });
            }
        }
        for (channel, link) in self.cgb.iter_mut().zip(&self.cgb_links) {
            if !link.belongs(player, track) || !channel.is_active() {
                continue;
            }
            if changes & VOLUME_CHANGED != 0 {
                (channel.right, channel.left) = note_volumes(link, volumes);
            }
            if changes & PITCH_CHANGED != 0 {
                let key = pitch_key_of(link.pitch_key, keys);
                channel.frequency = match channel.kind() {
                    PsgKind::Noise => u32::from(self.tables.noise_frequency(key)),
                    _ => self.tables.tone_frequency(key, fine),
                };
            }
        }
    }

    /// The programmable channels' frame (`0x0805C8A4`).
    fn run_cgb(&mut self) {
        if self.extra_counter == 0 {
            self.extra_counter = EXTRA_STEP_PERIOD;
        } else {
            self.extra_counter -= 1;
        }
        let extra = self.extra_counter == 0;
        for channel in &mut self.cgb {
            channel.frame(extra);
            channel.apply(&self.tables, f64::from(SAMPLE_RATE));
        }
        for (channel, link) in self.cgb.iter().zip(&mut self.cgb_links) {
            if !channel.is_active() {
                link.linked = false;
            }
        }
    }

    /// Mixes one frame into the next slot of the driver's buffer: the
    /// reverb of the two frames before it, then each sampled channel in
    /// turn; then the hardware adds the programmable channels.
    fn mix(&mut self) {
        let next = (self.slot + 1) % BUFFER_FRAMES;
        let two_ago = (next + 1) % BUFFER_FRAMES;
        self.slot = next;
        let frame = |slot: usize| slot * SAMPLES_PER_FRAME..(slot + 1) * SAMPLES_PER_FRAME;
        let older = self.buffer[frame(next)].to_vec();
        let old = self.buffer[frame(two_ago)].to_vec();
        let buffer = &mut self.buffer[frame(next)];
        sampled::reverb(buffer, &older, &old, self.reverb);
        for (channel, link) in self.sampled.iter_mut().zip(&mut self.sampled_links) {
            if !channel.is_active() {
                continue;
            }
            let volume = channel.step_envelope(self.layout.master_volume);
            if channel.is_active() {
                let step = channel.frequency.wrapping_mul(DIVIDED_FREQUENCY);
                channel.mix(self.rom, buffer, volume, step);
            }
            if !channel.is_active() {
                link.linked = false;
            }
        }
        for index in 0..SAMPLES_PER_FRAME {
            let dma = i32::from(self.buffer[next * SAMPLES_PER_FRAME + index]) * DMA_SCALE;
            let (mut right, mut left) = (dma, dma);
            for channel in &mut self.cgb {
                let (on_right, on_left) = channel.sides();
                let value = channel.sample() * PSG_SCALE;
                if on_right {
                    right += value;
                }
                if on_left {
                    left += value;
                }
            }
            let sample = |value: i32| {
                let clipped = value.clamp(-OUTPUT_LIMIT, OUTPUT_LIMIT - 1) * OUTPUT_GAIN;
                i16::try_from(clipped).unwrap_or(0)
            };
            self.output[index * 2] = sample(left);
            self.output[index * 2 + 1] = sample(right);
        }
    }
}

impl PsgKind {
    fn from_square(voice: &Voice) -> Self {
        match voice {
            Voice::Square { channel: 1, .. } => Self::Square1,
            _ => Self::Square2,
        }
    }
}

/// A note's right and left volumes from its velocity, a drum kit's pan and
/// the track's volumes (`0x0805B9D0`), each at most 255.
fn note_volumes(link: &Link, (right, left): (u8, u8)) -> (u8, u8) {
    let velocity = u32::from(link.velocity);
    let pan = i32::from(link.rhythm_pan);
    let side = |weight: i32, volume: u8| {
        let weight = u32::try_from(weight).unwrap_or(0);
        u8::try_from(((velocity * weight * u32::from(volume)) >> 14).min(255)).unwrap_or(255)
    };
    (side(128 + pan, right), side(127 - pan, left))
}

/// The key a note sounds: its own plus the track's whole keys, not below 0.
fn pitch_key_of(key: u8, keys: i8) -> u8 {
    let sum = i32::from(key) + i32::from(keys);
    u8::try_from(sum.max(0) & 0xFF).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn ptr(offset: usize) -> [u8; 4] {
        (0x0800_0000 + u32::try_from(offset).unwrap()).to_le_bytes()
    }

    /// Song 0 on player 0: one track, sample voice 0, one note of key 60
    /// held two ticks; song 1 on player 2 with no tracks.
    fn rom() -> (Vec<u8>, DriverLayout) {
        let mut rom = vec![0; 256];
        rom[0..4].copy_from_slice(&ptr(16));
        rom[8..12].copy_from_slice(&ptr(32));
        rom[12..14].copy_from_slice(&2u16.to_le_bytes());
        rom[16..24].copy_from_slice(&[1, 0, 128, 0, 0, 0, 0, 0]);
        rom[20..24].copy_from_slice(&ptr(48));
        rom[24..28].copy_from_slice(&ptr(96));
        rom[48..60].copy_from_slice(&[0x00, 60, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255]);
        rom[52..56].copy_from_slice(&ptr(128));
        rom[96..107].copy_from_slice(&[0xBB, 75, 0xBD, 0, 0xBE, 127, 0xD1, 60, 127, 0x82, 0xB1]);
        rom[128..132].copy_from_slice(&0u32.to_le_bytes());
        rom[132..136].copy_from_slice(&(SAMPLE_RATE * 1024).to_le_bytes());
        rom[140..144].copy_from_slice(&64u32.to_le_bytes());
        for index in 0..64 {
            rom[144 + index] = 100;
        }
        let tables = tables::tests::tables_in(&mut rom, 256);
        let layout = DriverLayout {
            song_table: 0,
            song_count: 2,
            master_volume: 15,
            ..tables
        };
        (rom, layout)
    }

    #[test]
    fn a_song_plays_its_sample_and_stops() {
        let (rom, layout) = rom();
        let mut engine = SoundEngine::new(&rom, layout);
        engine.play(0).unwrap();
        assert_eq!(engine.playing(0), Some(0));
        engine.frame().unwrap();
        let (right, left) = ((127 * 128 * 127) >> 14, (127 * 127 * 126) >> 14);
        let volume = ((right + left) * 255) >> 9;
        assert_eq!(i32::from(engine.sampled_buffer()[0]), (100 * volume) >> 8);
        let frame = engine.output();
        assert_eq!(frame.len(), SAMPLES_PER_FRAME * 2);
        assert_eq!(
            i32::from(frame[0]),
            i32::from(engine.sampled_buffer()[0]) * DMA_SCALE * OUTPUT_GAIN
        );
        assert_eq!(frame[0], frame[1]);
        assert!(engine.sampled_buffer()[60] != 0 && engine.sampled_buffer()[70] == 0);
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
        let (rom, layout) = rom();
        let mut engine = SoundEngine::new(&rom, layout);
        engine.play(0).unwrap();
        let before = engine.clone();
        engine.play_if_changed(0).unwrap();
        assert_eq!(engine, before);
        engine.stop_music();
        assert_eq!(engine.playing(0), None);
    }

    #[test]
    fn a_full_channel_set_gives_way_to_a_releasing_note_then_to_lower_priority() {
        let (rom, layout) = rom();
        let mut engine = SoundEngine::new(&rom, layout);
        for index in 0..SAMPLED_CHANNELS {
            engine.sampled[index].status = 1;
            engine.sampled_links[index] = Link {
                linked: true,
                priority: 10,
                track: index,
                ..Link::default()
            };
        }
        let note = Link {
            linked: true,
            priority: 10,
            track: 3,
            ..Link::default()
        };
        assert_eq!(engine.sampled_channel_for(&note), Some(7));
        engine.sampled_links[5].priority = 4;
        assert_eq!(engine.sampled_channel_for(&note), Some(5));
        engine.sampled[2].status |= STOPPING;
        engine.sampled_links[2].priority = 200;
        assert_eq!(engine.sampled_channel_for(&note), Some(2));
        engine.sampled[2].status = sampled::FREE;
        engine.sampled[1].status = sampled::FREE;
        assert_eq!(engine.sampled_channel_for(&note), Some(1));
        let low = Link {
            priority: 3,
            ..note
        };
        engine.sampled[1].status = 1;
        engine.sampled[2].status = 1;
        engine.sampled_links[5].priority = 10;
        assert_eq!(engine.sampled_channel_for(&low), None);
    }

    #[test]
    fn a_drum_kits_pan_and_the_velocity_scale_the_volumes() {
        let link = Link {
            velocity: 127,
            rhythm_pan: 0,
            ..Link::default()
        };
        assert_eq!(note_volumes(&link, (100, 99)), (99, 97));
        let left = Link {
            rhythm_pan: -128,
            ..link
        };
        assert_eq!(note_volumes(&left, (100, 99)), (0, 195));
        assert_eq!(pitch_key_of(10, -12), 0);
        assert_eq!(pitch_key_of(60, 2), 62);
    }
}
