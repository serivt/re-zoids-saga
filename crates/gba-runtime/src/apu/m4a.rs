//! The sequencer of the GBA's common sound driver: music players that walk
//! their tracks' bytecode once per tick and raise note events.
//!
//! Source of knowledge: the public description of the driver's track
//! commands and tempo (a tempo of `t` half-beats per minute advances
//! `2t` per frame and ticks every 150), checked against the frame counts
//! of the game's own songs.

use formats::m4a::{Command, M4aError, Running, SongHeader};

const TICK: u32 = 150;
const CENTER: u8 = 64;
const DEFAULT_TEMPO: u8 = 75;
const DEFAULT_VOLUME: u8 = 100;
const DEFAULT_BEND_RANGE: u8 = 2;
const MAX_PATTERN_DEPTH: usize = 3;
const LFO_CYCLE: u32 = 256;
const LFO_AMPLITUDE: u8 = 64;
const PITCH_STEPS: f64 = 256.0;

/// What a tick asks of the mixer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Start a note.
    NoteOn {
        /// Player the track belongs to.
        player: usize,
        /// Track index in the player.
        track: usize,
        /// Key after the track's key shift.
        key: u8,
        /// Velocity 0–127.
        velocity: u8,
        /// Ticks until the key goes up; `None` for a tie.
        gate: Option<u32>,
    },
    /// Let the track's tied notes go.
    EndTie {
        /// Player the track belongs to.
        player: usize,
        /// Track index in the player.
        track: usize,
    },
    /// One tick passed on the player: its notes' gates count down.
    Tick {
        /// The player.
        player: usize,
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
    /// Modulation speed.
    pub lfo_speed: u8,
    /// Modulation depth.
    pub modulation: u8,
    /// Modulation type: 0 pitch, 1 volume, 2 pan.
    pub modulation_type: u8,
    /// Frames the modulation waits after a note before it starts.
    pub lfo_delay: u8,
    lfo_phase: u32,
    lfo_wait: u8,
}

impl Track {
    fn new(pc: usize) -> Self {
        Self {
            pc,
            running: Running::default(),
            wait: 0,
            stack: Vec::new(),
            key: 60,
            velocity: 127,
            gate: 0,
            finished: false,
            key_shift: 0,
            voice: 0,
            volume: DEFAULT_VOLUME,
            pan: CENTER,
            bend: CENTER,
            bend_range: DEFAULT_BEND_RANGE,
            tune: CENTER,
            priority: 0,
            lfo_speed: 0,
            modulation: 0,
            modulation_type: 0,
            lfo_delay: 0,
            lfo_phase: 0,
            lfo_wait: 0,
        }
    }

    /// Whether the track has reached its end.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Pitch offset in semitones from bend, tune and modulation.
    #[must_use]
    pub fn pitch_offset(&self) -> f64 {
        let bend = f64::from(i32::from(self.bend) - i32::from(CENTER)) * f64::from(self.bend_range)
            / f64::from(CENTER);
        let tune = f64::from(i32::from(self.tune) - i32::from(CENTER)) / f64::from(CENTER);
        let vibrato = if self.modulation_type == 0 && self.lfo_wait == 0 {
            self.lfo() * f64::from(LFO_AMPLITUDE) * f64::from(self.modulation)
                / PITCH_STEPS
                / f64::from(CENTER)
        } else {
            0.0
        };
        bend + tune + vibrato
    }

    /// Triangle wave of the modulation, -1..=1.
    fn lfo(&self) -> f64 {
        let phase = i32::try_from(self.lfo_phase % LFO_CYCLE).unwrap_or(0);
        let cycle = i32::try_from(LFO_CYCLE).unwrap_or(0);
        let quarter = cycle / 4;
        let value = if phase < quarter {
            phase
        } else if phase < 3 * quarter {
            2 * quarter - phase
        } else {
            phase - cycle
        };
        f64::from(value) / f64::from(quarter)
    }

    fn advance_lfo(&mut self) {
        if self.lfo_wait > 0 {
            self.lfo_wait -= 1;
        } else if self.modulation > 0 {
            self.lfo_phase = self.lfo_phase.wrapping_add(u32::from(self.lfo_speed));
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
            song: None,
        }
    }

    /// Starts `header` as song `song`, dropping what was playing.
    pub fn start(&mut self, song: usize, header: &SongHeader) {
        self.tracks = header.tracks.iter().map(|pc| Track::new(*pc)).collect();
        self.tempo = u32::from(DEFAULT_TEMPO) * 2;
        self.tempo_counter = 0;
        self.voices = header.voices;
        self.song = Some(song).filter(|_| !header.tracks.is_empty());
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
        for track in &mut self.tracks {
            track.advance_lfo();
        }
        self.tempo_counter += self.tempo;
        while self.tempo_counter >= TICK {
            self.tempo_counter -= TICK;
            self.tick(rom, index, events)?;
        }
        Ok(())
    }

    fn tick(&mut self, rom: &[u8], index: usize, events: &mut Vec<Event>) -> Result<(), M4aError> {
        events.push(Event::Tick { player: index });
        for number in 0..self.tracks.len() {
            if self.tracks[number].finished {
                continue;
            }
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
            Command::Priority(value) => track.priority = value,
            Command::KeyShift(value) => track.key_shift = value,
            Command::Voice(value) => track.voice = value,
            Command::Volume(value) => track.volume = value,
            Command::Pan(value) => track.pan = value,
            Command::Bend(value) => track.bend = value,
            Command::BendRange(value) => track.bend_range = value,
            Command::LfoSpeed(value) => track.lfo_speed = value,
            Command::LfoDelay(value) => track.lfo_delay = value,
            Command::Modulation(value) => track.modulation = value,
            Command::ModulationType(value) => track.modulation_type = value,
            Command::Tune(value) => track.tune = value,
            Command::EndOfTie => events.push(Event::EndTie {
                player,
                track: number,
            }),
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
                track.lfo_phase = 0;
                let shifted = i32::from(track.key) + i32::from(track.key_shift);
                events.push(Event::NoteOn {
                    player,
                    track: number,
                    key: u8::try_from(shifted.clamp(0, 127)).unwrap_or(0),
                    velocity: track.velocity,
                    gate: (length > 0).then(|| u32::from(length) + u32::from(track.gate)),
                });
            }
            Command::Repeat | Command::MemoryAccess | Command::Extended(..) => {}
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
                Event::Tick { player: 0 },
                Event::NoteOn {
                    player: 0,
                    track: 0,
                    key: 62,
                    velocity: 100,
                    gate: Some(2)
                }
            ]
        );
        events.clear();
        player.frame(&rom, 0, &mut events).unwrap();
        assert_eq!(
            events,
            [
                Event::Tick { player: 0 },
                Event::NoteOn {
                    player: 0,
                    track: 0,
                    key: 64,
                    velocity: 100,
                    gate: None
                }
            ]
        );
        events.clear();
        player.frame(&rom, 0, &mut events).unwrap();
        assert_eq!(events, [Event::Tick { player: 0 }]);
        events.clear();
        player.frame(&rom, 0, &mut events).unwrap();
        assert_eq!(
            events,
            [
                Event::Tick { player: 0 },
                Event::EndTie {
                    player: 0,
                    track: 0
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
    fn bend_and_tune_move_the_pitch() {
        let mut track = Track::new(0);
        track.bend = 96;
        track.bend_range = 12;
        assert!((track.pitch_offset() - 6.0).abs() < 1e-9);
        track.bend = CENTER;
        track.tune = 0;
        assert!((track.pitch_offset() + 1.0).abs() < 1e-9);
        track.tune = CENTER;
        track.modulation = 128;
        track.lfo_phase = 64;
        assert!((track.pitch_offset() - 0.5).abs() < 1e-9);
        track.lfo_phase = 192;
        assert!((track.pitch_offset() + 0.5).abs() < 1e-9);
        track.lfo_wait = 3;
        assert!(track.pitch_offset().abs() < 1e-9);
    }
}
