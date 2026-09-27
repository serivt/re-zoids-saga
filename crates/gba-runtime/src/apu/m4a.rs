//! The sequencer of the GBA's common sound driver: music players that walk
//! their tracks' bytecode once per tick and raise note events.
//!
//! Source of knowledge: own reading of the driver in Zoids Saga (Japan,
//! Rev 1): the player's frame (`0x0805B724`: a tempo of `t` adds `2t` per
//! frame and ticks every 150; per tick each track's notes count their
//! gates down, its commands run until a wait, and its modulation steps),
//! a track's defaults as a song starts it (`0x0805B7C2`), the modulation's
//! reset (`0x0805BC40`) and the volume, pan and pitch it derives
//! (`0x0805C690`).

use formats::m4a::{Command, M4aError, Running, SongHeader};

const TICK: u32 = 150;
const CENTER: u8 = 64;
const DEFAULT_TEMPO: u8 = 75;
const DEFAULT_BEND_RANGE: u8 = 2;
const DEFAULT_LFO_SPEED: u8 = 0x16;
const DEFAULT_VOLUME_SCALE: u8 = 64;
const MAX_PATTERN_DEPTH: usize = 3;
const LFO_QUARTER: u8 = 64;
/// A track's pending changes (the low bits of its flags): its volume or
/// pan, and its pitch.
pub const VOLUME_CHANGED: u8 = 0x03;
/// See [`VOLUME_CHANGED`].
pub const PITCH_CHANGED: u8 = 0x0C;

/// What a tick asks of the mixer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Start a note.
    NoteOn {
        /// Player the track belongs to.
        player: usize,
        /// Track index in the player.
        track: usize,
        /// Key as the track plays it, before its key shift.
        key: u8,
        /// Velocity 0–127.
        velocity: u8,
        /// Ticks until the key goes up; `None` for a tie.
        gate: Option<u32>,
        /// The track's right and left volumes as the note starts.
        volumes: (u8, u8),
        /// The track's pitch as the note starts.
        pitch: (i8, u8),
    },
    /// Let the track's latest note of `key` still held go.
    EndTie {
        /// Player the track belongs to.
        player: usize,
        /// Track index in the player.
        track: usize,
        /// The key.
        key: u8,
    },
    /// One tick reached a track, before its commands run: its notes'
    /// gates count down.
    Tick {
        /// The player.
        player: usize,
        /// Track index in the player.
        track: usize,
    },
    /// The track ended: let its notes go.
    TrackEnd {
        /// Player the track belongs to.
        player: usize,
        /// Track index in the player.
        track: usize,
    },
    /// The player restarted: drop its notes.
    Silence {
        /// Player the track belongs to.
        player: usize,
        /// Track index in the player, or `None` for every track.
        track: Option<usize>,
    },
}

/// One track's cursor and settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pc: usize,
    running: Running,
    wait: u32,
    stack: Vec<usize>,
    key: u8,
    velocity: u8,
    gate: u8,
    finished: bool,
    /// Semitones added to every key.
    pub key_shift: i8,
    /// Voice index in the song's group.
    pub voice: u8,
    /// Volume 0–127.
    pub volume: u8,
    /// Pan 0–127.
    pub pan: u8,
    /// Pitch bend 0–127, 64 in the middle.
    pub bend: u8,
    /// Pitch bend range in semitones.
    pub bend_range: u8,
    /// Fine tune 0–127, 64 in the middle.
    pub tune: u8,
    /// Priority of the track's notes.
    pub priority: u8,
    /// Modulation speed: added to the modulation's phase every tick.
    pub lfo_speed: u8,
    /// Modulation depth.
    pub modulation: u8,
    /// Modulation type: 0 pitch, 1 volume, 2 pan.
    pub modulation_type: u8,
    /// Ticks the modulation waits after a note before it starts.
    pub lfo_delay: u8,
    /// Volume scale, 64 for none (no command of this game's songs sets
    /// it).
    pub volume_scale: u8,
    lfo_phase: u8,
    lfo_wait: u8,
    modulation_value: i8,
    changes: u8,
}

impl Track {
    fn new(pc: usize) -> Self {
        Self {
            pc,
            running: Running::default(),
            wait: 0,
            stack: Vec::new(),
            key: 0,
            velocity: 0,
            gate: 0,
            finished: false,
            key_shift: 0,
            voice: 0,
            volume: 0,
            pan: CENTER,
            bend: CENTER,
            bend_range: DEFAULT_BEND_RANGE,
            tune: CENTER,
            priority: 0,
            lfo_speed: DEFAULT_LFO_SPEED,
            modulation: 0,
            modulation_type: 0,
            lfo_delay: 0,
            volume_scale: DEFAULT_VOLUME_SCALE,
            lfo_phase: 0,
            lfo_wait: 0,
            modulation_value: 0,
            changes: 0,
        }
    }

    /// Whether the track has reached its end.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// The track's right and left volumes (`0x0805C690`): volume times its
    /// scale over 32, split by the pan, 0 the left edge and 127 the right,
    /// each a byte.
    #[must_use]
    pub fn volumes(&self) -> (u8, u8) {
        let mut level = (u32::from(self.volume) * u32::from(self.volume_scale)) >> 5;
        if self.modulation_type == 1 {
            let scale = i32::from(self.modulation_value) + 128;
            level = (level * u32::try_from(scale).unwrap_or(0)) >> 7;
        }
        let mut pan = (i32::from(self.pan) - i32::from(CENTER)) * 2;
        if self.modulation_type == 2 {
            pan += i32::from(self.modulation_value);
        }
        let pan = pan.clamp(-128, 127);
        let level = i32::try_from(level).unwrap_or(0);
        let byte = |value: i32| u8::try_from(value & 0xFF).unwrap_or(0);
        (
            byte((level * (pan + 128)) >> 8),
            byte((level * (127 - pan)) >> 8),
        )
    }

    /// The track's pitch as whole keys and 256ths of a key (`0x0805C690`):
    /// the bend times its range and the tune in quarters of 1/64 key, the
    /// key shift, and the pitch modulation in 16ths of a key.
    #[must_use]
    pub fn pitch(&self) -> (i8, u8) {
        let bend = i32::from(self.bend) - i32::from(CENTER);
        let tune = i32::from(self.tune) - i32::from(CENTER);
        let mut pitch = (bend * i32::from(self.bend_range) + tune) * 4;
        pitch += i32::from(self.key_shift) << 8;
        if self.modulation_type == 0 {
            pitch += i32::from(self.modulation_value) << 4;
        }
        let keys = i8::from_le_bytes([u8::try_from((pitch >> 8) & 0xFF).unwrap_or(0)]);
        (keys, u8::try_from(pitch & 0xFF).unwrap_or(0))
    }

    /// Applies a command that only sets one of the track's values,
    /// marking what it changes.
    fn set(&mut self, command: Command) {
        match command {
            Command::KeyShift(value) => {
                self.key_shift = value;
                self.changes |= PITCH_CHANGED;
            }
            Command::Volume(value) => {
                self.volume = value;
                self.changes |= VOLUME_CHANGED;
            }
            Command::Pan(value) => {
                self.pan = value;
                self.changes |= VOLUME_CHANGED;
            }
            Command::Bend(value) => {
                self.bend = value;
                self.changes |= PITCH_CHANGED;
            }
            Command::BendRange(value) => {
                self.bend_range = value;
                self.changes |= PITCH_CHANGED;
            }
            Command::ModulationType(value) => {
                if self.modulation_type != value {
                    self.modulation_type = value;
                    self.changes |= VOLUME_CHANGED | PITCH_CHANGED;
                }
            }
            Command::Tune(value) => {
                self.tune = value;
                self.changes |= PITCH_CHANGED;
            }
            Command::Priority(value) => self.priority = value,
            Command::Voice(value) => self.voice = value,
            Command::LfoSpeed(value) => {
                self.lfo_speed = value;
                if value == 0 {
                    self.reset_lfo();
                }
            }
            Command::LfoDelay(value) => self.lfo_delay = value,
            Command::Modulation(value) => {
                self.modulation = value;
                if value == 0 {
                    self.reset_lfo();
                }
            }
            _ => {}
        }
    }

    /// Clears the modulation's phase and value (`0x0805BC40`).
    fn reset_lfo(&mut self) {
        self.modulation_value = 0;
        self.lfo_phase = 0;
        self.changes |= self.modulation_changes();
    }

    /// What a change of the modulation's value changes.
    fn modulation_changes(&self) -> u8 {
        if self.modulation_type == 0 {
            PITCH_CHANGED
        } else {
            VOLUME_CHANGED
        }
    }

    /// One tick of the modulation: after its delay the phase advances by
    /// the speed, and the value is the depth times a triangle of the phase
    /// (-64 to 64) over 64.
    fn step_lfo(&mut self) {
        if self.lfo_speed == 0 || self.modulation == 0 {
            return;
        }
        if self.lfo_wait > 0 {
            self.lfo_wait -= 1;
            return;
        }
        self.lfo_phase = self.lfo_phase.wrapping_add(self.lfo_speed);
        let phase = self.lfo_phase;
        let triangle = if i8::from_le_bytes([phase.wrapping_sub(LFO_QUARTER)]) >= 0 {
            128 - i32::from(phase)
        } else {
            i32::from(i8::from_le_bytes([phase]))
        };
        let value = (i32::from(self.modulation) * triangle) >> 6;
        let value = i8::from_le_bytes([u8::try_from(value & 0xFF).unwrap_or(0)]);
        if value != self.modulation_value {
            self.modulation_value = value;
            self.changes |= self.modulation_changes();
        }
    }
}

/// A music player: the tracks of one song and its tempo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    tracks: Vec<Track>,
    tempo: u32,
    tempo_counter: u32,
    voices: usize,
    priority: u8,
    song: Option<usize>,
}

impl Player {
    /// An idle player.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            tempo: u32::from(DEFAULT_TEMPO) * 2,
            tempo_counter: 0,
            voices: 0,
            priority: 0,
            song: None,
        }
    }

    /// Starts `header` as song `song`, dropping what was playing.
    pub fn start(&mut self, song: usize, header: &SongHeader) {
        self.tracks = header.tracks.iter().map(|pc| Track::new(*pc)).collect();
        self.tempo = u32::from(DEFAULT_TEMPO) * 2;
        self.tempo_counter = 0;
        self.voices = header.voices;
        self.priority = header.priority;
        self.song = Some(song).filter(|_| !header.tracks.is_empty());
    }

    /// The song's priority, which its notes add their track's to.
    #[must_use]
    pub fn priority(&self) -> u8 {
        self.priority
    }

    /// Stops the player.
    pub fn stop(&mut self) {
        self.tracks.clear();
        self.song = None;
    }

    /// Song being played, if any.
    #[must_use]
    pub fn song(&self) -> Option<usize> {
        self.song
    }

    /// Whether every track has ended.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.tracks.iter().all(Track::is_finished)
    }

    /// Offset of the voice group.
    #[must_use]
    pub fn voices(&self) -> usize {
        self.voices
    }

    /// The tracks.
    #[must_use]
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Takes the changes pending on track `track` since its last note or
    /// the last frame ([`VOLUME_CHANGED`], [`PITCH_CHANGED`]).
    pub fn take_changes(&mut self, track: usize) -> u8 {
        self.tracks
            .get_mut(track)
            .filter(|track| !track.finished)
            .map_or(0, |track| std::mem::take(&mut track.changes))
    }

    /// Advances one frame; the events tell the mixer what to start or stop.
    ///
    /// # Errors
    ///
    /// Returns [`M4aError`] when a track's bytecode cannot be read.
    pub fn frame(
        &mut self,
        rom: &[u8],
        index: usize,
        events: &mut Vec<Event>,
    ) -> Result<(), M4aError> {
        if self.song.is_none() {
            return Ok(());
        }
        self.tempo_counter += self.tempo;
        while self.tempo_counter >= TICK {
            self.tempo_counter -= TICK;
            self.tick(rom, index, events)?;
        }
        Ok(())
    }

    fn tick(&mut self, rom: &[u8], index: usize, events: &mut Vec<Event>) -> Result<(), M4aError> {
        for number in 0..self.tracks.len() {
            if self.tracks[number].finished {
                continue;
            }
            events.push(Event::Tick {
                player: index,
                track: number,
            });
            if self.tracks[number].wait > 0 {
                self.tracks[number].wait -= 1;
            }
            while !self.tracks[number].finished && self.tracks[number].wait == 0 {
                let track = &mut self.tracks[number];
                let (command, next) =
                    formats::m4a::read_command(rom, track.pc, &mut track.running)?;
                track.pc = next;
                if let Some(tempo) = self.apply(number, command, index, events) {
                    self.tempo = u32::from(tempo) * 2;
                }
            }
            if !self.tracks[number].finished {
                self.tracks[number].step_lfo();
            }
        }
        Ok(())
    }

    fn apply(
        &mut self,
        number: usize,
        command: Command,
        player: usize,
        events: &mut Vec<Event>,
    ) -> Option<u8> {
        let track = &mut self.tracks[number];
        match command {
            Command::Wait(ticks) => track.wait = u32::from(ticks),
            Command::Fine => {
                track.finished = true;
                events.push(Event::TrackEnd {
                    player,
                    track: number,
                });
            }
            Command::Goto(target) => track.pc = target,
            Command::Pattern(target) => {
                if track.stack.len() < MAX_PATTERN_DEPTH {
                    track.stack.push(track.pc);
                    track.pc = target;
                }
            }
            Command::PatternEnd => {
                if let Some(back) = track.stack.pop() {
                    track.pc = back;
                }
            }
            Command::Tempo(tempo) => return Some(tempo),
            Command::EndOfTie(key) => {
                track.key = key.unwrap_or(track.key);
                events.push(Event::EndTie {
                    player,
                    track: number,
                    key: track.key,
                });
            }
            Command::Note {
                length,
                key,
                velocity,
                gate,
            } => {
                track.key = key.unwrap_or(track.key);
                track.velocity = velocity.unwrap_or(track.velocity);
                track.gate = gate.unwrap_or(0);
                track.lfo_wait = track.lfo_delay;
                if track.lfo_delay != 0 {
                    track.reset_lfo();
                }
                let volumes = track.volumes();
                let pitch = track.pitch();
                track.changes = 0;
                events.push(Event::NoteOn {
                    player,
                    track: number,
                    key: track.key,
                    velocity: track.velocity,
                    volumes,
                    pitch,
                    gate: (length > 0).then(|| u32::from(length) + u32::from(track.gate)),
                });
            }
            Command::Repeat | Command::MemoryAccess | Command::Extended(..) => {}
            setting => track.set(setting),
        }
        None
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn header(tracks: Vec<usize>) -> SongHeader {
        SongHeader {
            block_count: 0,
            priority: 0,
            reverb: 0,
            voices: 0,
            tracks,
        }
    }

    #[test]
    fn a_tempo_of_75_ticks_once_per_frame_and_plays_notes() {
        let rom = [
            0xBB, 75, 0xBC, 2, 0xD1, 60, 100, 0x81, 0xCF, 62, 0x82, 0xCE, 0xB1,
        ];
        let mut player = Player::new();
        player.start(3, &header(vec![0]));
        let mut events = Vec::new();
        player.frame(&rom, 0, &mut events).unwrap();
        assert_eq!(
            events,
            [
                Event::Tick {
                    player: 0,
                    track: 0
                },
                Event::NoteOn {
                    player: 0,
                    track: 0,
                    key: 60,
                    velocity: 100,
                    gate: Some(2),
                    volumes: (0, 0),
                    pitch: (2, 0)
                }
            ]
        );
        events.clear();
        player.frame(&rom, 0, &mut events).unwrap();
        assert_eq!(
            events,
            [
                Event::Tick {
                    player: 0,
                    track: 0
                },
                Event::NoteOn {
                    player: 0,
                    track: 0,
                    key: 62,
                    velocity: 100,
                    gate: None,
                    volumes: (0, 0),
                    pitch: (2, 0)
                }
            ]
        );
        events.clear();
        player.frame(&rom, 0, &mut events).unwrap();
        assert_eq!(
            events,
            [Event::Tick {
                player: 0,
                track: 0
            }]
        );
        events.clear();
        player.frame(&rom, 0, &mut events).unwrap();
        assert_eq!(
            events,
            [
                Event::Tick {
                    player: 0,
                    track: 0
                },
                Event::EndTie {
                    player: 0,
                    track: 0,
                    key: 62
                },
                Event::TrackEnd {
                    player: 0,
                    track: 0
                }
            ]
        );
        assert!(player.is_finished());
        assert_eq!(player.song(), Some(3));
    }

    #[test]
    fn patterns_return_and_loops_jump() {
        let mut rom = vec![
            0xB3, 0, 0, 0, 0x08, 0x81, 0xB2, 0, 0, 0, 0x08, 0xBE, 50, 0xB4,
        ];
        rom[1..5].copy_from_slice(&0x0800_000Bu32.to_le_bytes());
        rom[7..11].copy_from_slice(&0x0800_0000u32.to_le_bytes());
        let mut player = Player::new();
        player.start(0, &header(vec![0]));
        let mut events = Vec::new();
        player.tempo = 150;
        player.frame(&rom, 0, &mut events).unwrap();
        assert_eq!(player.tracks()[0].volume, 50);
        assert_eq!(player.tracks()[0].pc, 6);
        player.tracks[0].volume = 0;
        player.frame(&rom, 0, &mut events).unwrap();
        assert_eq!(player.tracks()[0].volume, 50);
        assert_eq!(player.tracks()[0].pc, 6);
    }

    #[test]
    fn bend_tune_and_key_shift_make_the_pitch() {
        let mut track = Track::new(0);
        track.bend = 96;
        track.bend_range = 12;
        assert_eq!(track.pitch(), (6, 0));
        track.bend = CENTER;
        track.tune = 0;
        assert_eq!(track.pitch(), (-1, 0));
        track.tune = 96;
        track.key_shift = 2;
        assert_eq!(track.pitch(), (2, 128));
    }

    #[test]
    fn pan_splits_the_volume() {
        let mut track = Track::new(0);
        track.volume = 100;
        assert_eq!(track.volumes(), (100, 99));
        track.pan = 0;
        assert_eq!(track.volumes(), (0, 199));
        track.pan = 127;
        assert_eq!(track.volumes(), (198, 0));
    }

    #[test]
    fn the_modulation_steps_a_triangle_per_tick_after_its_delay() {
        let mut track = Track::new(0);
        track.modulation = 64;
        track.lfo_speed = 32;
        track.lfo_wait = 1;
        track.step_lfo();
        assert_eq!(track.modulation_value, 0);
        track.step_lfo();
        assert_eq!(track.modulation_value, 32);
        track.step_lfo();
        assert_eq!(track.modulation_value, 64);
        track.step_lfo();
        assert_eq!(track.modulation_value, 32);
        assert_eq!(track.pitch(), (2, 0));
        for _ in 0..3 {
            track.step_lfo();
        }
        assert_eq!(track.modulation_value, -64);
    }
}
