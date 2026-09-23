//! Script strings of the Zoids GBA games.
//!
//! A string is a sequence of script opcodes; text lives inside *message
//! blocks* opened by opcode `0x20` and closed by `0x1D`. Inside a message,
//! any byte `>= 0x40` starts a two-byte character whose value is a Shift-JIS
//! code stored trail byte first (`[0x5D, 0x83]` is `ゾ`, Shift-JIS `0x835D`);
//! Shift-JIS trail bytes are never below `0x40`, so smaller bytes are
//! unambiguously control codes. Opcode `0x22` ends the string.
//!
//! The layout was recovered from the game's own interpreter; see
//! `docs/formats/script-text.md`.

use thiserror::Error;

const MESSAGE_START: u8 = 0x20;
const MESSAGE_END: u8 = 0x1D;
const STRING_END: u8 = 0x22;
const CALL: u8 = 0x21;
const JUMP: u8 = 0x08;
const SWITCH: u8 = 0x07;
const JUMP_IF_EQUAL: u8 = 0x13;
const JUMP_IF_GREATER: u8 = 0x14;
const LINE_BREAK: u8 = 0x0D;
const PLAYER_NAME: u8 = 0x1E;
const VARIABLE: u8 = 0x1F;
const CONTROL_WITH_ARG: u8 = 0x1C;
const FIRST_CHARACTER_BYTE: u8 = 0x40;

/// A decoded script string.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Script {
    /// Elements in string order, up to but excluding the end marker.
    pub elements: Vec<Element>,
}

/// A top-level element of a script string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element {
    /// A message block: what the text box shows for one `0x20`…`0x1D` span.
    Message(Vec<Piece>),
    /// A call of another string of the same table, by index (`0x21`).
    Call(u16),
    /// An unconditional jump (`0x08`): the offset is relative to the opcode.
    Jump(i16),
    /// A jump taken when two variables compare equal (`0x13`) or the first
    /// is greater (`0x14`); the offset is relative to the opcode.
    ConditionalJump {
        /// Opcode byte, `0x13` or `0x14`.
        code: u8,
        /// The two variable slots compared.
        vars: [u8; 2],
        /// Jump offset relative to the opcode.
        offset: i16,
    },
    /// A jump selected by a variable's value (`0x07`) from an inline table
    /// of offsets relative to the opcode.
    Switch {
        /// Variable slot that selects the entry.
        var: u8,
        /// Jump offsets, one per possible value.
        offsets: Vec<i16>,
    },
    /// Any other opcode with its raw arguments.
    Opcode {
        /// Opcode byte.
        code: u8,
        /// Argument bytes, as stored.
        args: Vec<u8>,
    },
}

/// One piece of a message block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// Printable text, already converted to Unicode.
    Text(String),
    /// Line break (`0x0D`).
    LineBreak,
    /// The player's name is inserted here (`0x1E`).
    PlayerName,
    /// A game variable is printed here (`0x1F`), with its raw arguments.
    Variable([u8; 2]),
    /// A control code whose effect is not modeled, with its raw argument if any.
    Control {
        /// Control byte.
        code: u8,
        /// Argument byte, only for `0x1C`.
        arg: Option<u8>,
    },
}

/// Why a byte slice could not be decoded as a script string.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ScriptTextError {
    /// The slice ended before the string's end marker.
    #[error("string ends at byte {offset} without an end marker")]
    UnexpectedEnd {
        /// Offset of the first missing byte.
        offset: usize,
    },
    /// Two bytes that should form a character are not a Shift-JIS character.
    #[error("invalid Shift-JIS character {code:#06x} at byte {offset}")]
    InvalidCharacter {
        /// Offset of the character's first byte.
        offset: usize,
        /// Shift-JIS code that was read.
        code: u16,
    },
    /// A switch whose offset table cannot be delimited: its first entry does
    /// not point past the table itself.
    #[error("switch at byte {offset} has a malformed offset table")]
    MalformedSwitch {
        /// Offset of the switch opcode.
        offset: usize,
    },
}

impl Script {
    /// Decodes the string at the start of `bytes`, returning it with the
    /// number of bytes it occupies, end marker included.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptTextError`] when the slice ends early, a character is
    /// not valid Shift-JIS, or a switch table cannot be delimited.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), ScriptTextError> {
        let mut decoder = Decoder { bytes, offset: 0 };
        let mut elements = Vec::new();
        loop {
            let code = decoder.next_byte()?;
            match code {
                STRING_END => return Ok((Self { elements }, decoder.offset)),
                MESSAGE_START => elements.push(Element::Message(decoder.message()?)),
                CALL => elements.push(Element::Call(decoder.next_u16()?)),
                JUMP => elements.push(Element::Jump(decoder.next_i16()?)),
                JUMP_IF_EQUAL | JUMP_IF_GREATER => {
                    let vars = [decoder.next_byte()?, decoder.next_byte()?];
                    let offset = decoder.next_i16()?;
                    elements.push(Element::ConditionalJump { code, vars, offset });
                }
                SWITCH => elements.push(decoder.switch()?),
                _ => elements.push(decoder.opcode(code)?),
            }
        }
    }

    /// The readable text of each message, in order, with `{name}` and
    /// `{var}` placeholders.
    #[must_use]
    pub fn message_texts(&self) -> Vec<String> {
        self.elements
            .iter()
            .filter_map(|element| match element {
                Element::Message(pieces) => Some(message_text(pieces)),
                Element::Call(_)
                | Element::Jump(_)
                | Element::ConditionalJump { .. }
                | Element::Switch { .. }
                | Element::Opcode { .. } => None,
            })
            .collect()
    }

    /// The character and expression the script selects for its first message,
    /// from the variable-setting opcodes (`0x09`) that precede it: slot 0
    /// holds the character, slot 1 the expression. `None` when either is unset.
    #[must_use]
    pub fn first_speaker(&self) -> Option<(u8, u8)> {
        let mut character = None;
        let mut expression = None;
        for element in &self.elements {
            match element {
                Element::Message(_) => break,
                Element::Opcode { code: 0x09, args } if args.len() == 3 => match args[0] & 7 {
                    0 => character = Some(args[1]),
                    1 => expression = Some(args[1]),
                    _ => {}
                },
                _ => {}
            }
        }
        Some((character?, expression?))
    }

    /// The readable text of every message, messages separated by a blank line.
    #[must_use]
    pub fn plain_text(&self) -> String {
        self.message_texts().join("\n\n")
    }
}

fn message_text(pieces: &[Piece]) -> String {
    let mut out = String::new();
    for piece in pieces {
        match piece {
            Piece::Text(text) => out.push_str(text),
            Piece::LineBreak => out.push('\n'),
            Piece::PlayerName => out.push_str("{name}"),
            Piece::Variable(_) => out.push_str("{var}"),
            Piece::Control { .. } => {}
        }
    }
    out
}

/// Number of argument bytes of a plain top-level opcode, `None` for the
/// opcodes that move the cursor (`0x07`, `0x08`, `0x13`, `0x14`), which
/// [`Script::decode`] handles separately.
#[must_use]
pub const fn opcode_arg_count(code: u8) -> Option<usize> {
    match code {
        0x01 => Some(7),
        0x02..=0x05 | 0x0D | 0x0E | 0x30 | 0x31 | 0x36 | 0x3A..=0x3D => Some(1),
        0x06 | 0x09 | 0x0F | 0x15..=0x18 | 0x33 | 0x35 | 0x37 | 0x39 => Some(3),
        0x0A | 0x19..=0x1C | 0x32 | 0x34 | 0x38 => Some(2),
        0x10..=0x12 => Some(5),
        0x07 | 0x08 | 0x13 | 0x14 => None,
        _ => Some(0),
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Decoder<'_> {
    fn next_byte(&mut self) -> Result<u8, ScriptTextError> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or(ScriptTextError::UnexpectedEnd {
                offset: self.offset,
            })?;
        self.offset += 1;
        Ok(byte)
    }

    fn next_u16(&mut self) -> Result<u16, ScriptTextError> {
        let lo = self.next_byte()?;
        let hi = self.next_byte()?;
        Ok(u16::from_le_bytes([lo, hi]))
    }

    fn next_i16(&mut self) -> Result<i16, ScriptTextError> {
        let lo = self.next_byte()?;
        let hi = self.next_byte()?;
        Ok(i16::from_le_bytes([lo, hi]))
    }

    /// The table has no length field: the game skips `2 * value` bytes and
    /// reads the entry there, so the table extends up to the nearest target,
    /// which is where the first case's code begins.
    fn switch(&mut self) -> Result<Element, ScriptTextError> {
        let opcode_offset = self.offset - 1;
        let var = self.next_byte()?;
        let mut offsets = Vec::new();
        let mut table_end = None;
        loop {
            let table_len = self.offset - opcode_offset;
            if table_end.is_some_and(|end| table_len >= end) {
                break;
            }
            let offset = self.next_i16()?;
            let target = usize::try_from(offset)
                .ok()
                .filter(|target| *target > table_len)
                .ok_or(ScriptTextError::MalformedSwitch {
                    offset: opcode_offset,
                })?;
            offsets.push(offset);
            table_end = Some(table_end.map_or(target, |end: usize| end.min(target)));
        }
        Ok(Element::Switch { var, offsets })
    }

    fn opcode(&mut self, code: u8) -> Result<Element, ScriptTextError> {
        let count = opcode_arg_count(code).unwrap_or(0);
        let args = (0..count)
            .map(|_| self.next_byte())
            .collect::<Result<Vec<u8>, _>>()?;
        Ok(Element::Opcode { code, args })
    }

    fn message(&mut self) -> Result<Vec<Piece>, ScriptTextError> {
        let mut pieces = Vec::new();
        let mut text = String::new();
        loop {
            let byte = self.next_byte()?;
            if byte >= FIRST_CHARACTER_BYTE {
                let trail = byte;
                let lead = self.next_byte()?;
                text.push_str(&self.character(lead, trail)?);
                continue;
            }
            flush_text(&mut text, &mut pieces);
            match byte {
                MESSAGE_END => return Ok(pieces),
                LINE_BREAK => pieces.push(Piece::LineBreak),
                PLAYER_NAME => pieces.push(Piece::PlayerName),
                VARIABLE => {
                    let args = [self.next_byte()?, self.next_byte()?];
                    pieces.push(Piece::Variable(args));
                }
                CONTROL_WITH_ARG => {
                    let arg = self.next_byte()?;
                    pieces.push(Piece::Control {
                        code: byte,
                        arg: Some(arg),
                    });
                }
                _ => pieces.push(Piece::Control {
                    code: byte,
                    arg: None,
                }),
            }
        }
    }

    fn character(&self, lead: u8, trail: u8) -> Result<String, ScriptTextError> {
        let stored = [lead, trail];
        let (decoded, had_errors) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(&stored);
        if had_errors {
            return Err(ScriptTextError::InvalidCharacter {
                offset: self.offset - 2,
                code: u16::from_be_bytes([lead, trail]),
            });
        }
        Ok(decoded.into_owned())
    }
}

fn flush_text(text: &mut String, pieces: &mut Vec<Piece>) {
    if !text.is_empty() {
        pieces.push(Piece::Text(std::mem::take(text)));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// Encodes `text` the way the ROM stores it: Shift-JIS, trail byte first.
    fn stored(text: &str) -> Vec<u8> {
        let (sjis, _, had_errors) = encoding_rs::SHIFT_JIS.encode(text);
        assert!(!had_errors);
        sjis.chunks(2).flat_map(|pair| [pair[1], pair[0]]).collect()
    }

    fn message(text: &str) -> Vec<u8> {
        let mut bytes = vec![MESSAGE_START];
        bytes.extend(stored(text));
        bytes.push(MESSAGE_END);
        bytes
    }

    #[test]
    fn decodes_a_message_with_swapped_shift_jis() {
        let mut bytes = message("ゾイド");
        bytes.push(STRING_END);
        assert_eq!(&bytes[..3], &[0x20, 0x5D, 0x83]);
        let (script, consumed) = Script::decode(&bytes).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(
            script.elements,
            vec![Element::Message(vec![Piece::Text("ゾイド".to_owned())])]
        );
    }

    #[test]
    fn decodes_line_breaks_name_and_variables_inside_a_message() {
        let mut bytes = vec![MESSAGE_START];
        bytes.extend(stored("「"));
        bytes.push(PLAYER_NAME);
        bytes.push(LINE_BREAK);
        bytes.extend([VARIABLE, 0x01, 0x02]);
        bytes.extend([CONTROL_WITH_ARG, 0x07]);
        bytes.push(MESSAGE_END);
        bytes.push(STRING_END);
        let (script, _) = Script::decode(&bytes).unwrap();
        assert_eq!(
            script.elements,
            vec![Element::Message(vec![
                Piece::Text("「".to_owned()),
                Piece::PlayerName,
                Piece::LineBreak,
                Piece::Variable([0x01, 0x02]),
                Piece::Control {
                    code: CONTROL_WITH_ARG,
                    arg: Some(0x07)
                },
            ])]
        );
        assert_eq!(script.plain_text(), "「{name}\n{var}");
    }

    #[test]
    fn decodes_opcodes_and_calls_between_messages() {
        let mut bytes = vec![0x09, 0x00, 0x02, 0x00, 0x0B];
        bytes.extend(message("王子"));
        bytes.extend([CALL, 0x20, 0x00]);
        bytes.push(STRING_END);
        let (script, consumed) = Script::decode(&bytes).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(
            script.elements,
            vec![
                Element::Opcode {
                    code: 0x09,
                    args: vec![0x00, 0x02, 0x00]
                },
                Element::Opcode {
                    code: 0x0B,
                    args: vec![]
                },
                Element::Message(vec![Piece::Text("王子".to_owned())]),
                Element::Call(0x0020),
            ]
        );
        assert_eq!(script.plain_text(), "王子");
    }

    #[test]
    fn reads_the_first_speaker_from_the_variable_opcodes() {
        let mut bytes = vec![0x09, 0x00, 0x02, 0x00, 0x09, 0x01, 0x03, 0x00, 0x0B];
        bytes.extend(message("王子"));
        bytes.extend([0x09, 0x00, 0x05, 0x00]);
        bytes.push(STRING_END);
        let (script, _) = Script::decode(&bytes).unwrap();
        assert_eq!(script.first_speaker(), Some((2, 3)));
        let (no_speaker, _) = Script::decode(&[STRING_END]).unwrap();
        assert_eq!(no_speaker.first_speaker(), None);
    }

    #[test]
    fn joins_messages_with_a_blank_line() {
        let mut bytes = message("一");
        bytes.extend(message("二"));
        bytes.push(STRING_END);
        let (script, _) = Script::decode(&bytes).unwrap();
        assert_eq!(script.plain_text(), "一\n\n二");
    }

    #[test]
    fn stops_at_the_end_marker_and_reports_the_consumed_length() {
        let mut bytes = message("あ");
        bytes.push(STRING_END);
        bytes.extend([0xFF, 0xFF]);
        let (_, consumed) = Script::decode(&bytes).unwrap();
        assert_eq!(consumed, bytes.len() - 2);
    }

    #[test]
    fn rejects_a_truncated_message() {
        let bytes = message("あ");
        assert_eq!(
            Script::decode(&bytes[..bytes.len() - 1]),
            Err(ScriptTextError::UnexpectedEnd {
                offset: bytes.len() - 1
            })
        );
    }

    #[test]
    fn rejects_an_invalid_character() {
        let bytes = [MESSAGE_START, 0x40, 0xFF, MESSAGE_END, STRING_END];
        assert_eq!(
            Script::decode(&bytes),
            Err(ScriptTextError::InvalidCharacter {
                offset: 1,
                code: 0xFF40
            })
        );
    }

    #[test]
    fn decodes_jumps_relative_to_the_opcode() {
        let bytes = [0x08, 0xFE, 0xFF, 0x13, 0x01, 0x02, 0x10, 0x00, STRING_END];
        let (script, _) = Script::decode(&bytes).unwrap();
        assert_eq!(
            script.elements,
            vec![
                Element::Jump(-2),
                Element::ConditionalJump {
                    code: 0x13,
                    vars: [0x01, 0x02],
                    offset: 16
                },
            ]
        );
    }

    #[test]
    fn delimits_a_switch_table_by_its_nearest_target() {
        let bytes = [
            0x07, 0x03, 0x0A, 0x00, 0x13, 0x00, 0x20, 0x00, 0x29, 0x00, 0x0B, STRING_END,
        ];
        let (script, consumed) = Script::decode(&bytes).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(
            script.elements,
            vec![
                Element::Switch {
                    var: 0x03,
                    offsets: vec![10, 19, 32, 41]
                },
                Element::Opcode {
                    code: 0x0B,
                    args: vec![]
                },
            ]
        );
    }

    #[test]
    fn rejects_a_switch_whose_first_target_is_inside_the_table() {
        let bytes = [0x07, 0x00, 0x02, 0x00, STRING_END];
        assert_eq!(
            Script::decode(&bytes),
            Err(ScriptTextError::MalformedSwitch { offset: 0 })
        );
    }
}
