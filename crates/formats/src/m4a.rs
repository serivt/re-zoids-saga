//! The sequenced music format of the GBA's common sound driver ("m4a", also
//! called Sappy): song headers, voice groups, samples and track bytecode.
//!
//! Source of knowledge: the layout of these structures is public (the
//! driver shipped with Nintendo's SDK and its data format has been described
//! many times); the details were checked against the songs of Zoids Saga.
//! This module only parses; playing is the runtime's job.

use thiserror::Error;

const ROM_BASE: u32 = 0x0800_0000;
const ROM_END: u32 = 0x0A00_0000;
const SONG_HEADER_SIZE: usize = 8;
const VOICE_SIZE: usize = 12;
const SAMPLE_HEADER_SIZE: usize = 16;
const MAX_TRACKS: u8 = 16;
const FIRST_WAIT: u8 = 0x80;
const LAST_WAIT: u8 = 0xB0;
const TIE: u8 = 0xCF;
const SAMPLE_LOOP_FLAG: u32 = 0x4000_0000;
const VOICE_DIRECT: u8 = 0x00;
const VOICE_DIRECT_FIXED: u8 = 0x08;
const VOICE_SQUARE_1: u8 = 0x01;
const VOICE_SQUARE_2: u8 = 0x02;
const VOICE_WAVE: u8 = 0x03;
const VOICE_NOISE: u8 = 0x04;
const VOICE_KEY_SPLIT: u8 = 0x40;
const VOICE_DRUMS: u8 = 0x80;
const VOICE_TYPE_MASK: u8 = 0x07;

/// Ticks a wait or note byte stands for, indexed by `byte - 0x80` for
/// waits and `byte - 0xCF` for notes (a tie is index 0).
pub const CLOCK_TABLE: [u8; 49] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 28,
    30, 32, 36, 40, 42, 44, 48, 52, 54, 56, 60, 64, 66, 68, 72, 76, 78, 80, 84, 88, 90, 92, 96,
];

/// Why data could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum M4aError {
    /// The structure runs past the end of the data.
    #[error("m4a data truncated at offset {offset:#x}")]
    Truncated {
        /// Where the read failed.
        offset: usize,
    },
    /// A pointer does not point into the ROM.
    #[error("pointer {pointer:#010x} at offset {offset:#x} is not in the ROM")]
    BadPointer {
        /// The pointer value.
        pointer: u32,
        /// Where it was read.
        offset: usize,
    },
    /// A song has an impossible track count.
    #[error("song at offset {offset:#x} has {count} tracks")]
    BadTrackCount {
        /// Where the song header is.
        offset: usize,
        /// The count read.
        count: u8,
    },
    /// A track byte is not a command.
    #[error("byte {byte:#04x} at offset {offset:#x} is not a track command")]
    BadCommand {
        /// The byte.
        byte: u8,
        /// Where it is.
        offset: usize,
    },
}

/// Turns a ROM pointer into an offset into the ROM image.
///
/// # Errors
///
/// Returns [`M4aError::BadPointer`] when the pointer is outside the ROM.
pub fn rom_offset(pointer: u32, at: usize) -> Result<usize, M4aError> {
    if (ROM_BASE..ROM_END).contains(&pointer) {
        Ok((pointer - ROM_BASE) as usize)
    } else {
        Err(M4aError::BadPointer {
            pointer,
            offset: at,
        })
    }
}

fn byte(bytes: &[u8], at: usize) -> Result<u8, M4aError> {
    bytes
        .get(at)
        .copied()
        .ok_or(M4aError::Truncated { offset: at })
}

fn word(bytes: &[u8], at: usize) -> Result<u32, M4aError> {
    let slice = bytes
        .get(at..at + 4)
        .ok_or(M4aError::Truncated { offset: at })?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn pointer(bytes: &[u8], at: usize) -> Result<usize, M4aError> {
    rom_offset(word(bytes, at)?, at)
}

/// A song: its tracks and the voice group they play with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongHeader {
    /// Blocks of the song (unused by the player).
    pub block_count: u8,
    /// Priority against other songs on the same player.
    pub priority: u8,
    /// Reverb setting; bit 7 says it applies.
    pub reverb: u8,
    /// Offset of the voice group.
    pub voices: usize,
    /// Offsets of the tracks, in order.
    pub tracks: Vec<usize>,
}

impl SongHeader {
    /// Reads the header at `at`.
    ///
    /// # Errors
    ///
    /// Returns [`M4aError`] when the header or a pointer is invalid. A song
    /// with no tracks has no voice group and is silence.
    pub fn read(bytes: &[u8], at: usize) -> Result<Self, M4aError> {
        let count = byte(bytes, at)?;
        if count > MAX_TRACKS {
            return Err(M4aError::BadTrackCount { offset: at, count });
        }
        let block_count = byte(bytes, at + 1)?;
        let priority = byte(bytes, at + 2)?;
        let reverb = byte(bytes, at + 3)?;
        let voices = if count == 0 {
            0
        } else {
            pointer(bytes, at + 4)?
        };
        let tracks = (0..usize::from(count))
            .map(|index| pointer(bytes, at + SONG_HEADER_SIZE + index * 4))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            block_count,
            priority,
            reverb,
            voices,
            tracks,
        })
    }
}

/// The envelope of a voice, one byte each as the driver stores them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Envelope {
    /// Added to the level each frame while rising (0 is instant for PSG).
    pub attack: u8,
    /// Multiplier (of 256) applied each frame while falling to the sustain.
    pub decay: u8,
    /// Level held while the key is down.
    pub sustain: u8,
    /// Multiplier (of 256) applied each frame after the key is released.
    pub release: u8,
}

/// One entry of a voice group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Voice {
    /// A sampled instrument.
    Sample {
        /// Key at which the sample plays at its own rate.
        base_key: u8,
        /// Whether the sample ignores the key (plays at its own rate).
        fixed: bool,
        /// Offset of the sample.
        sample: usize,
        /// Envelope.
        envelope: Envelope,
    },
    /// A square wave on PSG channel 1 or 2.
    Square {
        /// PSG channel, 1 or 2.
        channel: u8,
        /// Key at which the base frequency is reached.
        base_key: u8,
        /// Sweep setting of channel 1.
        sweep: u8,
        /// Duty cycle 0–3 (12.5%, 25%, 50%, 75%).
        duty: u8,
        /// Envelope in PSG steps.
        envelope: Envelope,
    },
    /// The programmable wave channel.
    Wave {
        /// Key at which the base frequency is reached.
        base_key: u8,
        /// Offset of the 16-byte wave pattern.
        pattern: usize,
        /// Envelope in PSG steps.
        envelope: Envelope,
    },
    /// The noise channel.
    Noise {
        /// Key at which the base frequency is reached.
        base_key: u8,
        /// Short (7-bit) instead of long noise.
        short: bool,
        /// Envelope in PSG steps.
        envelope: Envelope,
    },
    /// A voice that picks a sub-voice by key through a table.
    KeySplit {
        /// Offset of the sub voice group.
        group: usize,
        /// Offset of the 128-entry key table.
        table: usize,
    },
    /// A voice whose sub-group is indexed by the key itself (drum kits).
    Drums {
        /// Offset of the sub voice group.
        group: usize,
    },
    /// A type this parser does not know.
    Unknown(u8),
}

impl Voice {
    /// Reads entry `index` of the voice group at `group`.
    ///
    /// # Errors
    ///
    /// Returns [`M4aError`] when the entry is truncated or points outside
    /// the ROM.
    pub fn read(bytes: &[u8], group: usize, index: u8) -> Result<Self, M4aError> {
        let at = group + usize::from(index) * VOICE_SIZE;
        let kind = byte(bytes, at)?;
        let base_key = byte(bytes, at + 1)?;
        let pan_sweep = byte(bytes, at + 3)?;
        let envelope = Envelope {
            attack: byte(bytes, at + 8)?,
            decay: byte(bytes, at + 9)?,
            sustain: byte(bytes, at + 10)?,
            release: byte(bytes, at + 11)?,
        };
        let voice = match kind {
            VOICE_KEY_SPLIT => Self::KeySplit {
                group: pointer(bytes, at + 4)?,
                table: pointer(bytes, at + 8)?,
            },
            VOICE_DRUMS => Self::Drums {
                group: pointer(bytes, at + 4)?,
            },
            _ => match kind & VOICE_TYPE_MASK {
                VOICE_DIRECT => Self::Sample {
                    base_key,
                    fixed: kind & VOICE_DIRECT_FIXED != 0,
                    sample: pointer(bytes, at + 4)?,
                    envelope,
                },
                VOICE_SQUARE_1 | VOICE_SQUARE_2 => Self::Square {
                    channel: kind & VOICE_TYPE_MASK,
                    base_key,
                    sweep: pan_sweep,
                    duty: (word(bytes, at + 4)? & 3) as u8,
                    envelope,
                },
                VOICE_WAVE => Self::Wave {
                    base_key,
                    pattern: pointer(bytes, at + 4)?,
                    envelope,
                },
                VOICE_NOISE => Self::Noise {
                    base_key,
                    short: word(bytes, at + 4)? & 1 != 0,
                    envelope,
                },
                other => Self::Unknown(other),
            },
        };
        Ok(voice)
    }
}

/// A sampled waveform: 8-bit signed samples after a 16-byte header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    /// Whether playback loops back to `loop_start` at the end.
    pub looped: bool,
    /// Playback rate at the base key, in Hz times 1024.
    pub frequency: u32,
    /// Sample index the loop returns to.
    pub loop_start: u32,
    /// Number of samples.
    pub length: u32,
    /// Offset of the first sample byte.
    pub data: usize,
}

impl Sample {
    /// Reads the sample header at `at`.
    ///
    /// # Errors
    ///
    /// Returns [`M4aError::Truncated`] when the header or the samples run
    /// past the data.
    pub fn read(bytes: &[u8], at: usize) -> Result<Self, M4aError> {
        let flags = word(bytes, at)?;
        let frequency = word(bytes, at + 4)?;
        let loop_start = word(bytes, at + 8)?;
        let length = word(bytes, at + 12)?;
        let data = at + SAMPLE_HEADER_SIZE;
        let end = data
            .checked_add(length as usize)
            .filter(|end| *end <= bytes.len())
            .ok_or(M4aError::Truncated { offset: data })?;
        let _ = end;
        Ok(Self {
            looped: flags & SAMPLE_LOOP_FLAG != 0,
            frequency,
            loop_start,
            length,
            data,
        })
    }
}

/// One track command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Wait this many ticks.
    Wait(u8),
    /// End of the track.
    Fine,
    /// Jump to an offset (loops).
    Goto(usize),
    /// Call a pattern at an offset.
    Pattern(usize),
    /// Return from a pattern.
    PatternEnd,
    /// Repeat marker (unused by this game's songs).
    Repeat,
    /// Memory access (unused by this game's songs).
    MemoryAccess,
    /// Track priority.
    Priority(u8),
    /// Tempo in half beats per minute.
    Tempo(u8),
    /// Key shift in semitones.
    KeyShift(i8),
    /// Voice index in the song's group.
    Voice(u8),
    /// Volume 0–127.
    Volume(u8),
    /// Pan 0–127, 64 in the middle.
    Pan(u8),
    /// Pitch bend 0–127, 64 in the middle.
    Bend(u8),
    /// Pitch bend range in semitones.
    BendRange(u8),
    /// Modulation speed.
    LfoSpeed(u8),
    /// Modulation delay.
    LfoDelay(u8),
    /// Modulation depth.
    Modulation(u8),
    /// Modulation type (pitch, volume or pan).
    ModulationType(u8),
    /// Fine tune.
    Tune(u8),
    /// Extended command (unused by this game's songs).
    Extended(u8, u8),
    /// End the tied note.
    EndOfTie,
    /// Play a note: length in ticks (0 is a tie), key, velocity, extra
    /// gate ticks; missing arguments repeat the previous note's.
    Note {
        /// Length in ticks, 0 for a tie held until [`Command::EndOfTie`].
        length: u8,
        /// Key, if given.
        key: Option<u8>,
        /// Velocity, if given.
        velocity: Option<u8>,
        /// Extra ticks added to the length, if given.
        gate: Option<u8>,
    },
}

/// Running status of a track: the command byte a bare argument repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Running(Option<u8>);

/// Reads the command at `at`, updating the running status, and returns
/// it with the offset of the next command.
///
/// # Errors
///
/// Returns [`M4aError`] when the byte is not a command or the data ends.
pub fn read_command(
    bytes: &[u8],
    at: usize,
    running: &mut Running,
) -> Result<(Command, usize), M4aError> {
    let first = byte(bytes, at)?;
    if first < FIRST_WAIT {
        let Some(command) = running.0 else {
            return Err(M4aError::BadCommand {
                byte: first,
                offset: at,
            });
        };
        return match command {
            TIE..=0xFF => Ok(note(bytes, at, command)),
            0xBA..=0xC8 => Ok((one_argument(command, first, at)?, at + 1)),
            _ => Err(M4aError::BadCommand {
                byte: first,
                offset: at,
            }),
        };
    }
    match first {
        FIRST_WAIT..=LAST_WAIT => Ok((
            Command::Wait(CLOCK_TABLE[usize::from(first - FIRST_WAIT)]),
            at + 1,
        )),
        0xB1 => Ok((Command::Fine, at + 1)),
        0xB2 => Ok((Command::Goto(pointer(bytes, at + 1)?), at + 5)),
        0xB3 => Ok((Command::Pattern(pointer(bytes, at + 1)?), at + 5)),
        0xB4 => Ok((Command::PatternEnd, at + 1)),
        0xB5 => Ok((Command::Repeat, at + 1)),
        0xB9 => Ok((Command::MemoryAccess, at + 4)),
        0xBA..=0xC8 => {
            running.0 = Some(first);
            let argument = byte(bytes, at + 1)?;
            Ok((one_argument(first, argument, at + 1)?, at + 2))
        }
        0xCD => Ok((
            Command::Extended(byte(bytes, at + 1)?, byte(bytes, at + 2)?),
            at + 3,
        )),
        0xCE => Ok((Command::EndOfTie, at + 1)),
        TIE..=0xFF => {
            running.0 = Some(first);
            Ok(note(bytes, at + 1, first))
        }
        _ => Err(M4aError::BadCommand {
            byte: first,
            offset: at,
        }),
    }
}

fn one_argument(command: u8, argument: u8, at: usize) -> Result<Command, M4aError> {
    Ok(match command {
        0xBA => Command::Priority(argument),
        0xBB => Command::Tempo(argument),
        0xBC => Command::KeyShift(i8::from_le_bytes([argument])),
        0xBD => Command::Voice(argument),
        0xBE => Command::Volume(argument),
        0xBF => Command::Pan(argument),
        0xC0 => Command::Bend(argument),
        0xC1 => Command::BendRange(argument),
        0xC2 => Command::LfoSpeed(argument),
        0xC3 => Command::LfoDelay(argument),
        0xC4 => Command::Modulation(argument),
        0xC5 => Command::ModulationType(argument),
        0xC8 => Command::Tune(argument),
        other => {
            return Err(M4aError::BadCommand {
                byte: other,
                offset: at,
            });
        }
    })
}

fn note(bytes: &[u8], at: usize, command: u8) -> (Command, usize) {
    let length = CLOCK_TABLE[usize::from(command - TIE)];
    let mut next = at;
    let mut arguments = [None; 3];
    for slot in &mut arguments {
        match bytes.get(next) {
            Some(value) if *value < FIRST_WAIT => {
                *slot = Some(*value);
                next += 1;
            }
            _ => break,
        }
    }
    (
        Command::Note {
            length,
            key: arguments[0],
            velocity: arguments[1],
            gate: arguments[2],
        },
        next,
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn ptr(offset: usize) -> [u8; 4] {
        (ROM_BASE + u32::try_from(offset).unwrap()).to_le_bytes()
    }

    #[test]
    fn reads_a_song_header_with_its_tracks() {
        let mut rom = vec![0; 64];
        rom[16..24].copy_from_slice(&[2, 0, 128, 192, 0, 0, 0, 0]);
        rom[20..24].copy_from_slice(&ptr(40));
        rom[24..28].copy_from_slice(&ptr(48));
        rom[28..32].copy_from_slice(&ptr(52));
        let song = SongHeader::read(&rom, 16).unwrap();
        assert_eq!(song.priority, 128);
        assert_eq!(song.reverb, 192);
        assert_eq!(song.voices, 40);
        assert_eq!(song.tracks, [48, 52]);
        let empty = SongHeader::read(&[0, 0, 0, 0], 0).unwrap();
        assert!(empty.tracks.is_empty());
        assert_eq!(
            SongHeader::read(&[17, 0, 0, 0], 0),
            Err(M4aError::BadTrackCount {
                offset: 0,
                count: 17
            })
        );
    }

    #[test]
    fn reads_every_kind_of_voice() {
        let mut rom = vec![0; 128];
        rom[0..12].copy_from_slice(&[0x00, 60, 0, 0, 0, 0, 0, 0, 0xFF, 0xFA, 0x14, 0x7F]);
        rom[4..8].copy_from_slice(&ptr(100));
        rom[12..24].copy_from_slice(&[0x01, 60, 0, 5, 2, 0, 0, 0, 0, 0, 15, 0]);
        rom[24..36].copy_from_slice(&[0x40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        rom[28..32].copy_from_slice(&ptr(0));
        rom[32..36].copy_from_slice(&ptr(96));
        rom[36..48].copy_from_slice(&[0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        rom[40..44].copy_from_slice(&ptr(0));
        rom[48..60].copy_from_slice(&[0x04, 60, 0, 0, 1, 0, 0, 0, 0, 0, 7, 12]);
        rom[60..72].copy_from_slice(&[0x08, 60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        rom[64..68].copy_from_slice(&ptr(100));
        assert_eq!(
            Voice::read(&rom, 0, 0).unwrap(),
            Voice::Sample {
                base_key: 60,
                fixed: false,
                sample: 100,
                envelope: Envelope {
                    attack: 0xFF,
                    decay: 0xFA,
                    sustain: 0x14,
                    release: 0x7F
                }
            }
        );
        assert_eq!(
            Voice::read(&rom, 0, 1).unwrap(),
            Voice::Square {
                channel: 1,
                base_key: 60,
                sweep: 5,
                duty: 2,
                envelope: Envelope {
                    attack: 0,
                    decay: 0,
                    sustain: 15,
                    release: 0
                }
            }
        );
        assert_eq!(
            Voice::read(&rom, 0, 2).unwrap(),
            Voice::KeySplit {
                group: 0,
                table: 96
            }
        );
        assert_eq!(Voice::read(&rom, 0, 3).unwrap(), Voice::Drums { group: 0 });
        assert!(matches!(
            Voice::read(&rom, 0, 4).unwrap(),
            Voice::Noise { short: true, .. }
        ));
        assert!(matches!(
            Voice::read(&rom, 0, 5).unwrap(),
            Voice::Sample { fixed: true, .. }
        ));
    }

    #[test]
    fn reads_a_sample_header() {
        let mut rom = vec![0; 32];
        rom[0..4].copy_from_slice(&0x4000_0000u32.to_le_bytes());
        rom[4..8].copy_from_slice(&(29433u32 * 1024).to_le_bytes());
        rom[8..12].copy_from_slice(&4u32.to_le_bytes());
        rom[12..16].copy_from_slice(&16u32.to_le_bytes());
        let sample = Sample::read(&rom, 0).unwrap();
        assert!(sample.looped);
        assert_eq!(sample.frequency / 1024, 29433);
        assert_eq!((sample.loop_start, sample.length, sample.data), (4, 16, 16));
        rom[12..16].copy_from_slice(&17u32.to_le_bytes());
        assert_eq!(
            Sample::read(&rom, 0),
            Err(M4aError::Truncated { offset: 16 })
        );
    }

    #[test]
    fn reads_track_commands_with_running_status() {
        let mut track = vec![
            0xBC, 0x00, 0xBB, 0x7D, 0xBD, 0x7F, 0xBE, 0x37, 0x98, 0xDA, 0x2E, 0x3C, 0x8C, 0x2A,
            0x28, 0xCF, 0x40, 0xB0, 0xCE, 0xB3,
        ];
        track.extend(ptr(0));
        track.extend([0xB4, 0xB2]);
        track.extend(ptr(2));
        track.push(0xB1);
        let mut running = Running::default();
        let mut at = 0;
        let mut commands = Vec::new();
        loop {
            let (command, next) = read_command(&track, at, &mut running).unwrap();
            commands.push(command);
            at = next;
            if command == Command::Fine {
                break;
            }
        }
        assert_eq!(
            commands,
            [
                Command::KeyShift(0),
                Command::Tempo(125),
                Command::Voice(127),
                Command::Volume(55),
                Command::Wait(24),
                Command::Note {
                    length: 11,
                    key: Some(46),
                    velocity: Some(60),
                    gate: None
                },
                Command::Wait(12),
                Command::Note {
                    length: 11,
                    key: Some(42),
                    velocity: Some(40),
                    gate: None
                },
                Command::Note {
                    length: 0,
                    key: Some(64),
                    velocity: None,
                    gate: None
                },
                Command::Wait(96),
                Command::EndOfTie,
                Command::Pattern(0),
                Command::PatternEnd,
                Command::Goto(2),
                Command::Fine,
            ]
        );
        assert_eq!(
            read_command(&[0x2A], 0, &mut Running::default()),
            Err(M4aError::BadCommand {
                byte: 0x2A,
                offset: 0
            })
        );
    }
}
