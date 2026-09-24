//! Typed decoding of script instructions at byte offsets, for execution.
//!
//! [`crate::script_text`] decodes a whole string into elements for reading;
//! this module decodes one instruction at a time so an interpreter can keep
//! a byte cursor, follow jumps whose offsets are relative to the opcode
//! address, and step through a message character by character.

use thiserror::Error;

const MESSAGE_START: u8 = 0x20;
const MESSAGE_END: u8 = 0x1D;
const LINE_BREAK: u8 = 0x0D;
const SWITCH_WINDOW: u8 = 0x1C;
const PLAYER_NAME: u8 = 0x1E;
const VARIABLE: u8 = 0x1F;
const FIRST_CHARACTER_BYTE: u8 = 0x40;
const VAR_MASK: u8 = 7;
const ALL_WINDOWS: u8 = 0xFF;
const SOUND_BASE: u8 = 0x3C;

/// A script instruction outside a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    /// `0x22`: the string ends.
    End,
    /// `0x00` and the unassigned opcodes: nothing.
    Nop,
    /// `0x01`: open window `id` with the given tile rectangle. `kind` low
    /// nibble 1 is a menu, 0 a text window; `style` bit 0 makes text appear
    /// one character per frame.
    OpenWindow {
        /// Window slot, 0–7.
        id: u8,
        /// Kind byte as stored.
        kind: u8,
        /// Left column in tiles.
        x: u8,
        /// Top row in tiles.
        y: u8,
        /// Width in tiles, border included.
        width: u8,
        /// Height in tiles, border included.
        height: u8,
        /// Style byte as stored.
        style: u8,
    },
    /// `0x02`: reset the text system; mode 1 also resets the name buffer,
    /// and modes below `0x10` clear the variables.
    Reset {
        /// Mode byte.
        mode: u8,
    },
    /// `0x03`: close window `id`, or every window for `None`.
    CloseWindow {
        /// Window slot, `None` for all.
        id: Option<u8>,
    },
    /// `0x04`: present window `id` (draw it), or every window for `None`.
    Present {
        /// Window slot, `None` for all.
        id: Option<u8>,
    },
    /// `0x05`: wait for a key while the prompt blinks; `mode` picks the
    /// keys accepted, `cancelable` lets B end the wait too.
    WaitKey {
        /// Key set, 0 for A only.
        mode: u8,
        /// Whether B also ends the wait.
        cancelable: bool,
    },
    /// `0x06`: let the player pick a line of the current window with the
    /// cursor; A stores 1 in var0 and the line in var1, and with `0x10` in
    /// the high nibble B ends it with var0 = 0.
    Menu {
        /// Mode byte as stored.
        mode: u8,
    },
    /// `0x36`: the menu the game's own screens drive: like `0x06`, but a
    /// cursor move also ends it, with var0 = `0x20` (up) or `0x40` (down)
    /// and the new line in var1, so the caller can redraw; `mode` 2 also
    /// ends it on L (var0 = 2) or R (var0 = 4), 3 on L only and 4 on R only.
    MoveMenu {
        /// Key set.
        mode: u8,
        /// Whether B also ends it, with var0 = 0.
        cancelable: bool,
    },
    /// `0x07`: jump by the entry the variable selects, relative to the opcode.
    /// The table has no length: it ends where its nearest target begins or
    /// at the first value that does not point past it, which is code (the
    /// system scripts follow a table with a jump for the other values).
    Switch {
        /// Variable slot.
        var: u8,
        /// Offsets from the opcode address.
        offsets: Vec<i16>,
    },
    /// `0x08`: jump relative to the opcode.
    Jump(i16),
    /// `0x09`: set a variable to a 16-bit value.
    SetVar {
        /// Variable slot.
        var: u8,
        /// Value.
        value: u16,
    },
    /// `0x0A`: copy a variable.
    CopyVar {
        /// Destination slot.
        dst: u8,
        /// Source slot.
        src: u8,
    },
    /// `0x0B`: keep the eight variables across strings.
    StoreVars,
    /// `0x0C`: restore the kept variables.
    LoadVars,
    /// `0x0D`: draw window `id` without marking it.
    Draw {
        /// Window slot.
        id: u8,
    },
    /// `0x0E`: clear the text of window `id`.
    ClearWindow {
        /// Window slot.
        id: u8,
    },
    /// `0x0F`: show a portrait in a window.
    Portrait {
        /// Window slot.
        window: u8,
        /// Character index.
        character: u8,
        /// Expression index.
        expression: u8,
    },
    /// `0x10`–`0x12`: jump when the variable compares to the value.
    JumpIfValue {
        /// Variable slot.
        var: u8,
        /// Comparison.
        test: Comparison,
        /// Value compared against.
        value: u16,
        /// Offset from the opcode address.
        offset: i16,
    },
    /// `0x13`, `0x14`: jump when two variables compare.
    JumpIfVars {
        /// First variable slot.
        var: u8,
        /// Comparison.
        test: Comparison,
        /// Second variable slot.
        other: u8,
        /// Offset from the opcode address.
        offset: i16,
    },
    /// `0x15`–`0x1C`: arithmetic on a variable.
    Arithmetic {
        /// Variable slot.
        var: u8,
        /// Operation.
        op: Operation,
        /// Right operand.
        operand: Operand,
    },
    /// `0x20`: a message follows; the payload starts at the returned offset
    /// and is stepped with [`decode_message_step`].
    Message,
    /// `0x21`: run another string of the table, then continue.
    Call(u16),
    /// `0x32`, `0x33`: set or clear a game flag.
    SetFlag {
        /// Which flag.
        flag: Operand,
        /// `true` to set, `false` to clear.
        set: bool,
    },
    /// `0x34`, `0x35`: load a game flag into a variable.
    VarFromFlag {
        /// Variable slot.
        var: u8,
        /// Which flag.
        flag: Operand,
    },
    /// `0x37`: show the portrait named by two variables.
    PortraitFromVars {
        /// Window slot.
        window: u8,
        /// Slot holding the character.
        character: u8,
        /// Slot holding the expression.
        expression: u8,
    },
    /// `0x39`: set a variable to a byte value.
    SetVarByte {
        /// Variable slot.
        var: u8,
        /// Value.
        value: u8,
    },
    /// `0x3A`: play sound effect `0x3C + n`.
    Sound(u8),
    /// `0x3B`: set flag bit 4 of window `id`; meaning not modeled.
    WindowFlag {
        /// Window slot.
        id: u8,
    },
    /// `0x3D`: wait this many frames.
    Delay(u8),
    /// An opcode whose behavior is not modeled; its arguments are skipped.
    Unknown {
        /// Opcode byte.
        code: u8,
        /// Argument bytes.
        args: Vec<u8>,
    },
}

/// How a conditional jump compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    /// Equal.
    Equal,
    /// Left greater than right.
    Greater,
    /// Left less than right.
    Less,
}

/// Arithmetic operation on a variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// Add.
    Add,
    /// Subtract.
    Subtract,
    /// Multiply.
    Multiply,
    /// Divide (unsigned).
    Divide,
}

/// A value taken either literally or from a variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    /// A literal value.
    Value(u16),
    /// The value of a variable slot.
    Var(u8),
}

/// One step of a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageStep {
    /// A character to print.
    Character(char),
    /// Line break.
    LineBreak,
    /// Send the following text to another window.
    SwitchWindow(u8),
    /// Print the player's name.
    PlayerName,
    /// Print a variable: `digits` cells wide, `mode` as stored.
    Variable {
        /// Variable slot.
        var: u8,
        /// Digit cells.
        digits: u8,
        /// Format mode.
        mode: u8,
    },
    /// A control code the game ignores.
    Ignored(u8),
    /// The message ends.
    End,
}

/// Why an instruction could not be decoded.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ScriptOpError {
    /// The bytes end inside the instruction.
    #[error("script ends at byte {offset} inside an instruction")]
    UnexpectedEnd {
        /// Offset of the first missing byte.
        offset: usize,
    },
    /// Two bytes that should form a character are not Shift-JIS.
    #[error("invalid Shift-JIS character {code:#06x} at byte {offset}")]
    InvalidCharacter {
        /// Offset of the character's first byte.
        offset: usize,
        /// Code read.
        code: u16,
    },
    /// A switch table cannot be delimited.
    #[error("switch at byte {offset} has a malformed offset table")]
    MalformedSwitch {
        /// Offset of the switch opcode.
        offset: usize,
    },
}

/// Decodes the instruction at `at`, returning it with the offset of the
/// next byte (for [`Instruction::Message`], the first payload byte).
///
/// # Errors
///
/// Returns [`ScriptOpError`] when the bytes end early or a switch table is
/// malformed.
pub fn decode_instruction(bytes: &[u8], at: usize) -> Result<(Instruction, usize), ScriptOpError> {
    let mut cursor = Cursor { bytes, offset: at };
    let code = cursor.byte()?;
    let instruction = match code {
        0x22 => Instruction::End,
        0x00 => Instruction::Nop,
        0x01..=0x06 | 0x0D..=0x0F | 0x36 | 0x37 | 0x3A | 0x3B | 0x3D => {
            window_instruction(code, &mut cursor)?
        }
        0x07 | 0x08 | 0x10..=0x14 => control_flow(code, &mut cursor, at)?,
        0x09..=0x0C | 0x15..=0x1C | 0x32..=0x35 | 0x39 => variable_instruction(code, &mut cursor)?,
        MESSAGE_START => Instruction::Message,
        0x21 => Instruction::Call(cursor.u16()?),
        _ => {
            let count = crate::script_text::opcode_arg_count(code).unwrap_or(0);
            let args = (0..count)
                .map(|_| cursor.byte())
                .collect::<Result<Vec<u8>, _>>()?;
            Instruction::Unknown { code, args }
        }
    };
    Ok((instruction, cursor.offset))
}

fn window_instruction(code: u8, cursor: &mut Cursor<'_>) -> Result<Instruction, ScriptOpError> {
    Ok(match code {
        0x01 => {
            let args = cursor.bytes::<7>()?;
            Instruction::OpenWindow {
                id: args[0],
                kind: args[1],
                x: args[2],
                y: args[3],
                width: args[4],
                height: args[5],
                style: args[6],
            }
        }
        0x02 => Instruction::Reset {
            mode: cursor.byte()?,
        },
        0x03 => Instruction::CloseWindow {
            id: window_or_all(cursor.byte()?),
        },
        0x04 => Instruction::Present {
            id: window_or_all(cursor.byte()?),
        },
        0x05 => {
            let arg = cursor.byte()?;
            Instruction::WaitKey {
                mode: arg & 0x0F,
                cancelable: arg & 0xF0 == 0x10,
            }
        }
        0x06 => Instruction::Menu {
            mode: cursor.byte()?,
        },
        0x36 => {
            let arg = cursor.byte()?;
            Instruction::MoveMenu {
                mode: arg & 0x0F,
                cancelable: arg & 0xF0 == 0x10,
            }
        }
        0x0D => Instruction::Draw { id: cursor.byte()? },
        0x0E => Instruction::ClearWindow { id: cursor.byte()? },
        0x0F => {
            let args = cursor.bytes::<3>()?;
            Instruction::Portrait {
                window: args[0],
                character: args[1],
                expression: args[2],
            }
        }
        0x37 => {
            let args = cursor.bytes::<3>()?;
            Instruction::PortraitFromVars {
                window: args[0],
                character: args[1] & VAR_MASK,
                expression: args[2] & VAR_MASK,
            }
        }
        0x3A => Instruction::Sound(cursor.byte()?.wrapping_add(SOUND_BASE)),
        0x3B => Instruction::WindowFlag { id: cursor.byte()? },
        _ => Instruction::Delay(cursor.byte()?),
    })
}

fn control_flow(
    code: u8,
    cursor: &mut Cursor<'_>,
    at: usize,
) -> Result<Instruction, ScriptOpError> {
    Ok(match code {
        0x07 => cursor.switch(at)?,
        0x08 => Instruction::Jump(cursor.i16()?),
        0x10..=0x12 => {
            let var = cursor.var()?;
            let value = cursor.u16()?;
            let offset = cursor.i16()?;
            Instruction::JumpIfValue {
                var,
                test: match code {
                    0x10 => Comparison::Equal,
                    0x11 => Comparison::Greater,
                    _ => Comparison::Less,
                },
                value,
                offset,
            }
        }
        _ => {
            let var = cursor.var()?;
            let other = cursor.var()?;
            let offset = cursor.i16()?;
            Instruction::JumpIfVars {
                var,
                test: if code == 0x13 {
                    Comparison::Equal
                } else {
                    Comparison::Greater
                },
                other,
                offset,
            }
        }
    })
}

fn variable_instruction(code: u8, cursor: &mut Cursor<'_>) -> Result<Instruction, ScriptOpError> {
    Ok(match code {
        0x09 => Instruction::SetVar {
            var: cursor.var()?,
            value: cursor.u16()?,
        },
        0x0A => Instruction::CopyVar {
            dst: cursor.var()?,
            src: cursor.var()?,
        },
        0x0B => Instruction::StoreVars,
        0x0C => Instruction::LoadVars,
        0x15..=0x18 => {
            let var = cursor.var()?;
            let value = cursor.u16()?;
            Instruction::Arithmetic {
                var,
                op: operation(code - 0x15),
                operand: Operand::Value(value),
            }
        }
        0x19..=0x1C => {
            let var = cursor.var()?;
            let other = cursor.var()?;
            Instruction::Arithmetic {
                var,
                op: operation(code - 0x19),
                operand: Operand::Var(other),
            }
        }
        0x32 => {
            let var = cursor.var()?;
            let set = cursor.byte()? == 1;
            Instruction::SetFlag {
                flag: Operand::Var(var),
                set,
            }
        }
        0x33 => {
            let flag = cursor.u16()?;
            let set = cursor.byte()? == 1;
            Instruction::SetFlag {
                flag: Operand::Value(flag),
                set,
            }
        }
        0x34 => Instruction::VarFromFlag {
            var: cursor.var()?,
            flag: Operand::Var(cursor.var()?),
        },
        0x35 => Instruction::VarFromFlag {
            var: cursor.var()?,
            flag: Operand::Value(cursor.u16()?),
        },
        _ => {
            let var = cursor.var()?;
            let value = cursor.byte()?;
            cursor.byte()?;
            Instruction::SetVarByte { var, value }
        }
    })
}

/// Decodes the message step at `at`, returning it with the offset of the
/// next byte.
///
/// # Errors
///
/// Returns [`ScriptOpError`] when the bytes end early or a character is not
/// Shift-JIS.
pub fn decode_message_step(bytes: &[u8], at: usize) -> Result<(MessageStep, usize), ScriptOpError> {
    let mut cursor = Cursor { bytes, offset: at };
    let byte = cursor.byte()?;
    let step = if byte >= FIRST_CHARACTER_BYTE {
        let lead = cursor.byte()?;
        MessageStep::Character(character(lead, byte, at)?)
    } else {
        match byte {
            MESSAGE_END => MessageStep::End,
            LINE_BREAK => MessageStep::LineBreak,
            SWITCH_WINDOW => MessageStep::SwitchWindow(cursor.byte()?),
            PLAYER_NAME => MessageStep::PlayerName,
            VARIABLE => {
                let var = cursor.var()?;
                let format = cursor.byte()?;
                MessageStep::Variable {
                    var,
                    digits: format & 7,
                    mode: format >> 4,
                }
            }
            other => MessageStep::Ignored(other),
        }
    };
    Ok((step, cursor.offset))
}

const fn window_or_all(byte: u8) -> Option<u8> {
    if byte == ALL_WINDOWS {
        None
    } else {
        Some(byte)
    }
}

const fn operation(index: u8) -> Operation {
    match index {
        0 => Operation::Add,
        1 => Operation::Subtract,
        2 => Operation::Multiply,
        _ => Operation::Divide,
    }
}

fn character(lead: u8, trail: u8, offset: usize) -> Result<char, ScriptOpError> {
    let stored = [lead, trail];
    let (decoded, had_errors) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(&stored);
    decoded
        .chars()
        .next()
        .filter(|_| !had_errors)
        .ok_or(ScriptOpError::InvalidCharacter {
            offset,
            code: u16::from_be_bytes([lead, trail]),
        })
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Cursor<'_> {
    fn byte(&mut self) -> Result<u8, ScriptOpError> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or(ScriptOpError::UnexpectedEnd {
                offset: self.offset,
            })?;
        self.offset += 1;
        Ok(byte)
    }

    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], ScriptOpError> {
        let mut out = [0; N];
        for slot in &mut out {
            *slot = self.byte()?;
        }
        Ok(out)
    }

    fn var(&mut self) -> Result<u8, ScriptOpError> {
        Ok(self.byte()? & VAR_MASK)
    }

    fn u16(&mut self) -> Result<u16, ScriptOpError> {
        Ok(u16::from_le_bytes(self.bytes::<2>()?))
    }

    fn i16(&mut self) -> Result<i16, ScriptOpError> {
        Ok(i16::from_le_bytes(self.bytes::<2>()?))
    }

    fn switch(&mut self, opcode_offset: usize) -> Result<Instruction, ScriptOpError> {
        let var = self.var()?;
        let mut offsets = Vec::new();
        let mut table_end = None;
        loop {
            let table_len = self.offset - opcode_offset;
            if table_end.is_some_and(|end| table_len >= end) {
                break;
            }
            let offset = self.i16()?;
            let Some(target) = usize::try_from(offset)
                .ok()
                .filter(|target| *target > table_len)
            else {
                if offsets.is_empty() {
                    return Err(ScriptOpError::MalformedSwitch {
                        offset: opcode_offset,
                    });
                }
                self.offset -= 2;
                break;
            };
            offsets.push(offset);
            table_end = Some(table_end.map_or(target, |end: usize| end.min(target)));
        }
        Ok(Instruction::Switch { var, offsets })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn decode(bytes: &[u8]) -> (Instruction, usize) {
        decode_instruction(bytes, 0).unwrap()
    }

    #[test]
    fn decodes_window_and_variable_instructions() {
        assert_eq!(
            decode(&[0x01, 1, 0x10, 0, 12, 30, 8, 1]),
            (
                Instruction::OpenWindow {
                    id: 1,
                    kind: 0x10,
                    x: 0,
                    y: 12,
                    width: 30,
                    height: 8,
                    style: 1,
                },
                8
            )
        );
        assert_eq!(
            decode(&[0x03, 0xFF]),
            (Instruction::CloseWindow { id: None }, 2)
        );
        assert_eq!(
            decode(&[0x04, 5]),
            (Instruction::Present { id: Some(5) }, 2)
        );
        assert_eq!(
            decode(&[0x05, 0x12]),
            (
                Instruction::WaitKey {
                    mode: 2,
                    cancelable: true
                },
                2
            )
        );
        assert_eq!(
            decode(&[0x09, 0x0A, 0x34, 0x12]),
            (
                Instruction::SetVar {
                    var: 2,
                    value: 0x1234
                },
                4
            )
        );
        assert_eq!(
            decode(&[0x37, 0, 0, 1]),
            (
                Instruction::PortraitFromVars {
                    window: 0,
                    character: 0,
                    expression: 1
                },
                4
            )
        );
        assert_eq!(decode(&[0x3A, 2]), (Instruction::Sound(0x3E), 2));
        assert_eq!(decode(&[0x3D, 30]), (Instruction::Delay(30), 2));
        assert_eq!(decode(&[0x22]), (Instruction::End, 1));
        assert_eq!(decode(&[0x20, 0x41]), (Instruction::Message, 1));
    }

    #[test]
    fn decodes_jumps_arithmetic_and_flags() {
        assert_eq!(
            decode(&[0x10, 3, 0x02, 0x00, 0xFE, 0xFF]),
            (
                Instruction::JumpIfValue {
                    var: 3,
                    test: Comparison::Equal,
                    value: 2,
                    offset: -2
                },
                6
            )
        );
        assert_eq!(
            decode(&[0x14, 1, 2, 0x10, 0x00]),
            (
                Instruction::JumpIfVars {
                    var: 1,
                    test: Comparison::Greater,
                    other: 2,
                    offset: 16
                },
                5
            )
        );
        assert_eq!(
            decode(&[0x16, 0, 5, 0]),
            (
                Instruction::Arithmetic {
                    var: 0,
                    op: Operation::Subtract,
                    operand: Operand::Value(5)
                },
                4
            )
        );
        assert_eq!(
            decode(&[0x1C, 0, 1]),
            (
                Instruction::Arithmetic {
                    var: 0,
                    op: Operation::Divide,
                    operand: Operand::Var(1)
                },
                3
            )
        );
        assert_eq!(
            decode(&[0x33, 0x34, 0x12, 1]),
            (
                Instruction::SetFlag {
                    flag: Operand::Value(0x1234),
                    set: true
                },
                4
            )
        );
        assert_eq!(
            decode(&[0x35, 2, 0x10, 0x00]),
            (
                Instruction::VarFromFlag {
                    var: 2,
                    flag: Operand::Value(16)
                },
                4
            )
        );
        assert_eq!(
            decode(&[0x07, 0, 0x05, 0x00, 0x07, 0x00, 0x22, 0x22]),
            (
                Instruction::Switch {
                    var: 0,
                    offsets: vec![5, 7]
                },
                6
            )
        );
        assert_eq!(
            decode(&[0x07, 1, 0x09, 0x00, 0x08, 0xFD, 0xFF, 0x00, 0x22]),
            (
                Instruction::Switch {
                    var: 1,
                    offsets: vec![9]
                },
                4
            )
        );
        assert_eq!(
            decode(&[0x36, 0x12]),
            (
                Instruction::MoveMenu {
                    mode: 2,
                    cancelable: true
                },
                2
            )
        );
        assert_eq!(
            decode_instruction(&[0x09, 0], 0),
            Err(ScriptOpError::UnexpectedEnd { offset: 2 })
        );
    }

    #[test]
    fn steps_through_a_message() {
        let bytes = [
            0x5D, 0x83, 0x0D, 0x1E, 0x1F, 0x02, 0x23, 0x1C, 0x01, 0x0E, 0x1D,
        ];
        let mut at = 0;
        let mut steps = Vec::new();
        loop {
            let (step, next) = decode_message_step(&bytes, at).unwrap();
            at = next;
            let done = step == MessageStep::End;
            steps.push(step);
            if done {
                break;
            }
        }
        assert_eq!(
            steps,
            [
                MessageStep::Character('ゾ'),
                MessageStep::LineBreak,
                MessageStep::PlayerName,
                MessageStep::Variable {
                    var: 2,
                    digits: 3,
                    mode: 2
                },
                MessageStep::SwitchWindow(1),
                MessageStep::Ignored(0x0E),
                MessageStep::End,
            ]
        );
        assert_eq!(at, bytes.len());
        assert!(matches!(
            decode_message_step(&[0xFF, 0xFF], 0),
            Err(ScriptOpError::InvalidCharacter { offset: 0, .. })
        ));
    }
}
