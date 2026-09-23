//! The script interpreter: runs dialogue strings one frame at a time.
//!
//! Matches the original runner (ROM `0x0803E54C`): eight 16-bit variables,
//! a saved copy of them that survives across strings, calls of other
//! strings of the same table, jumps relative to the opcode, windows the
//! host owns, and waits. Every operation that flushes the display in the
//! original costs one frame here: presenting, closing or clearing a window,
//! showing a portrait, and each character of a typewriter window. The key
//! wait polls once per frame and blinks the prompt 20 frames off, 20 on.

use formats::script_ops::{
    Comparison, Instruction, MessageStep, Operand, Operation, ScriptOpError, decode_instruction,
    decode_message_step,
};
use platform::{Button, Input};

const VARIABLES: usize = 8;
const PROMPT_HALF_PERIOD: u32 = 20;
const CONFIRM_SOUND: u8 = 0x41;
const KEY_A: u16 = 1;
const KEY_B: u16 = 2;

/// What the interpreter asks of the game: windows, text, sounds and flags.
pub trait ScriptHost {
    /// Opens (or reopens) window `id` at the tile rectangle.
    fn open_window(&mut self, id: u8, kind: u8, rect: (u8, u8, u8, u8), style: u8);
    /// Closes window `id`, or all of them.
    fn close_window(&mut self, id: Option<u8>);
    /// Shows window `id` (or all) with its current contents.
    fn present(&mut self, id: Option<u8>);
    /// Empties the text of window `id`.
    fn clear_window(&mut self, id: u8);
    /// Prints one character in window `id`.
    fn put_char(&mut self, id: u8, ch: char);
    /// Moves to the next text line of window `id`.
    fn line_break(&mut self, id: u8);
    /// Whether window `id` shows text one character per frame.
    fn typewriter(&self, id: u8) -> bool;
    /// Shows a portrait in window `id`.
    fn portrait(&mut self, id: u8, character: u8, expression: u8);
    /// Shows or hides the "more" prompt of window `id`.
    fn prompt(&mut self, id: u8, visible: bool);
    /// Plays a sound effect.
    fn play_sound(&mut self, id: u8);
    /// Reads a game flag.
    fn flag(&self, flag: u16) -> bool;
    /// Sets or clears a game flag.
    fn set_flag(&mut self, flag: u16, set: bool);
    /// The player's name.
    fn player_name(&self) -> String;
    /// Resets the text system (`0x02`).
    fn reset(&mut self, mode: u8);
}

/// What the runner is waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wait {
    None,
    Frames(u32),
    Key {
        mode: u8,
        cancelable: bool,
        elapsed: u32,
    },
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Frame {
    base: usize,
    pc: usize,
    in_message: bool,
}

/// Why a script stopped.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ScriptError {
    /// A string index has no string.
    #[error("no script string {index}")]
    NoSuchString {
        /// Requested index.
        index: usize,
    },
    /// The bytes could not be decoded.
    #[error(transparent)]
    Decode(#[from] ScriptOpError),
}

/// Runs strings of one table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptRunner {
    strings: Vec<usize>,
    frames: Vec<Frame>,
    vars: [u16; VARIABLES],
    saved: [u16; VARIABLES],
    window: u8,
    pending: Vec<char>,
    wait: Wait,
    previous: Input,
}

impl ScriptRunner {
    /// Creates a runner over a table whose strings start at the given ROM
    /// offsets, 0 marking an absent string.
    #[must_use]
    pub fn new(strings: Vec<usize>) -> Self {
        Self {
            strings,
            frames: Vec::new(),
            vars: [0; VARIABLES],
            saved: [0; VARIABLES],
            window: 0,
            pending: Vec::new(),
            wait: Wait::Done,
            previous: Input::default(),
        }
    }

    /// Starts string `index`, dropping anything that was running.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError::NoSuchString`] when the index has no string.
    pub fn start(&mut self, index: usize) -> Result<(), ScriptError> {
        let base = self.string(index)?;
        self.frames = vec![Frame {
            base,
            pc: base,
            in_message: false,
        }];
        self.vars = [0; VARIABLES];
        self.pending.clear();
        self.wait = Wait::None;
        Ok(())
    }

    /// Whether no script is running.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.wait == Wait::Done
    }

    /// Whether the runner is waiting for a key.
    #[must_use]
    pub fn is_waiting_for_key(&self) -> bool {
        matches!(self.wait, Wait::Key { .. })
    }

    /// The eight script variables.
    #[must_use]
    pub fn vars(&self) -> &[u16; VARIABLES] {
        &self.vars
    }

    /// Advances one frame; returns `true` when the script has finished.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a string cannot be decoded or called.
    pub fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        host: &mut impl ScriptHost,
    ) -> Result<bool, ScriptError> {
        let pressed = |button: Button| input.is_held(button) && !self.previous.is_held(button);
        let keys = u16::from(pressed(Button::A)) * KEY_A + u16::from(pressed(Button::B)) * KEY_B;
        self.previous = input;
        match self.wait {
            Wait::Done => return Ok(true),
            Wait::Frames(left) => {
                if left > 1 {
                    self.wait = Wait::Frames(left - 1);
                    return Ok(false);
                }
                self.wait = Wait::None;
            }
            Wait::Key {
                mode,
                cancelable,
                elapsed,
            } => {
                if !self.poll_key(mode, cancelable, elapsed, keys, host) {
                    return Ok(false);
                }
            }
            Wait::None => {}
        }
        while self.wait == Wait::None {
            self.step(rom, host)?;
        }
        Ok(self.wait == Wait::Done)
    }

    fn poll_key(
        &mut self,
        mode: u8,
        cancelable: bool,
        elapsed: u32,
        keys: u16,
        host: &mut impl ScriptHost,
    ) -> bool {
        let accepted = mode == 0 && keys & KEY_A != 0;
        let canceled = cancelable && keys & KEY_B != 0;
        if !accepted && !canceled {
            let elapsed = elapsed + 1;
            host.prompt(self.window, elapsed / PROMPT_HALF_PERIOD % 2 == 1);
            self.wait = Wait::Key {
                mode,
                cancelable,
                elapsed,
            };
            return false;
        }
        if accepted {
            self.vars[0] = 1;
            host.play_sound(CONFIRM_SOUND);
        } else {
            self.vars[0] = 0;
        }
        host.prompt(self.window, false);
        self.wait = Wait::Frames(1);
        true
    }

    fn string(&self, index: usize) -> Result<usize, ScriptError> {
        self.strings
            .get(index)
            .copied()
            .filter(|offset| *offset != 0)
            .ok_or(ScriptError::NoSuchString { index })
    }

    fn step(&mut self, rom: &[u8], host: &mut impl ScriptHost) -> Result<(), ScriptError> {
        let Some(frame) = self.frames.last_mut() else {
            self.wait = Wait::Done;
            return Ok(());
        };
        if frame.in_message {
            return self.message_step(rom, host);
        }
        let opcode_at = frame.pc;
        let (instruction, next) = decode_instruction(rom, opcode_at)?;
        frame.pc = next;
        match instruction {
            Instruction::End => {
                self.frames.pop();
                if self.frames.is_empty() {
                    self.wait = Wait::Done;
                }
            }
            Instruction::Message => frame.in_message = true,
            Instruction::Call(index) => {
                let base = self.string(usize::from(index))?;
                self.frames.push(Frame {
                    base,
                    pc: base,
                    in_message: false,
                });
            }
            Instruction::Switch { .. }
            | Instruction::Jump(_)
            | Instruction::JumpIfValue { .. }
            | Instruction::JumpIfVars { .. } => self.branch(&instruction, opcode_at),
            Instruction::SetVar { .. }
            | Instruction::SetVarByte { .. }
            | Instruction::CopyVar { .. }
            | Instruction::StoreVars
            | Instruction::LoadVars
            | Instruction::Arithmetic { .. }
            | Instruction::SetFlag { .. }
            | Instruction::VarFromFlag { .. } => self.assign(&instruction, host),
            other => self.window_op(&other, host),
        }
        Ok(())
    }

    fn window_op(&mut self, instruction: &Instruction, host: &mut impl ScriptHost) {
        match *instruction {
            Instruction::OpenWindow {
                id,
                kind,
                x,
                y,
                width,
                height,
                style,
            } => {
                host.open_window(id, kind, (x, y, width, height), style);
                self.window = id;
            }
            Instruction::Reset { mode } => {
                host.reset(mode);
                if mode & 0xF0 == 0 {
                    self.vars = [0; VARIABLES];
                }
            }
            Instruction::CloseWindow { id } => {
                host.close_window(id);
                self.wait = Wait::Frames(1);
            }
            Instruction::Present { id } => {
                host.present(id);
                self.wait = Wait::Frames(1);
            }
            Instruction::Draw { id } => host.present(Some(id)),
            Instruction::ClearWindow { id } => {
                host.clear_window(id);
                self.wait = Wait::Frames(1);
            }
            Instruction::WaitKey { mode, cancelable } => {
                self.wait = Wait::Key {
                    mode,
                    cancelable,
                    elapsed: 0,
                };
            }
            Instruction::Portrait {
                window,
                character,
                expression,
            } => {
                host.portrait(window, character, expression);
                self.wait = Wait::Frames(1);
            }
            Instruction::PortraitFromVars {
                window,
                character,
                expression,
            } => {
                let character = u8::try_from(self.var(character)).unwrap_or(u8::MAX);
                let expression = u8::try_from(self.var(expression)).unwrap_or(u8::MAX);
                host.portrait(window, character, expression);
                self.wait = Wait::Frames(1);
            }
            Instruction::Sound(id) => host.play_sound(id),
            Instruction::Delay(frames) if frames > 0 => self.wait = Wait::Frames(u32::from(frames)),
            _ => {}
        }
    }

    fn branch(&mut self, instruction: &Instruction, opcode_at: usize) {
        let taken = match *instruction {
            Instruction::Switch { var, ref offsets } => {
                offsets.get(usize::from(self.var(var))).copied()
            }
            Instruction::Jump(offset) => Some(offset),
            Instruction::JumpIfValue {
                var,
                test,
                value,
                offset,
            } => compare(self.var(var), test, value).then_some(offset),
            Instruction::JumpIfVars {
                var,
                test,
                other,
                offset,
            } => compare(self.var(var), test, self.var(other)).then_some(offset),
            _ => None,
        };
        if let (Some(offset), Some(frame)) = (taken, self.frames.last_mut()) {
            frame.pc = opcode_at.wrapping_add_signed(isize::from(offset));
        }
    }

    fn assign(&mut self, instruction: &Instruction, host: &mut impl ScriptHost) {
        match *instruction {
            Instruction::SetVar { var, value } => self.set_var(var, value),
            Instruction::SetVarByte { var, value } => self.set_var(var, u16::from(value)),
            Instruction::CopyVar { dst, src } => self.set_var(dst, self.var(src)),
            Instruction::StoreVars => self.saved = self.vars,
            Instruction::LoadVars => self.vars = self.saved,
            Instruction::Arithmetic { var, op, operand } => {
                let right = self.operand(operand);
                let left = self.var(var);
                let value = match op {
                    Operation::Add => left.wrapping_add(right),
                    Operation::Subtract => left.wrapping_sub(right),
                    Operation::Multiply => left.wrapping_mul(right),
                    Operation::Divide => left.checked_div(right).unwrap_or(0),
                };
                self.set_var(var, value);
            }
            Instruction::SetFlag { flag, set } => host.set_flag(self.operand(flag), set),
            Instruction::VarFromFlag { var, flag } => {
                let value = u16::from(host.flag(self.operand(flag)));
                self.set_var(var, value);
            }
            _ => {}
        }
    }

    fn message_step(&mut self, rom: &[u8], host: &mut impl ScriptHost) -> Result<(), ScriptError> {
        if let Some(ch) = self.pending.pop() {
            self.print(ch, host);
            return Ok(());
        }
        let Some(frame) = self.frames.last_mut() else {
            self.wait = Wait::Done;
            return Ok(());
        };
        let (step, next) = decode_message_step(rom, frame.pc)?;
        frame.pc = next;
        match step {
            MessageStep::Character(ch) => self.print(ch, host),
            MessageStep::LineBreak => host.line_break(self.window),
            MessageStep::SwitchWindow(id) => self.window = id,
            MessageStep::PlayerName => self.queue_text(&host.player_name()),
            MessageStep::Variable { var, digits, .. } => {
                let text = format!(
                    "{:>width$}",
                    self.var(var),
                    width = usize::from(digits.max(1))
                );
                self.queue_text(&text);
            }
            MessageStep::Ignored(_) => {}
            MessageStep::End => {
                if let Some(frame) = self.frames.last_mut() {
                    frame.in_message = false;
                }
            }
        }
        Ok(())
    }

    fn queue_text(&mut self, text: &str) {
        self.pending.extend(text.chars().rev());
    }

    fn print(&mut self, ch: char, host: &mut impl ScriptHost) {
        host.put_char(self.window, ch);
        if host.typewriter(self.window) {
            host.present(Some(self.window));
            self.wait = Wait::Frames(1);
        }
    }

    fn var(&self, slot: u8) -> u16 {
        self.vars[usize::from(slot) % VARIABLES]
    }

    fn set_var(&mut self, slot: u8, value: u16) {
        self.vars[usize::from(slot) % VARIABLES] = value;
    }

    fn operand(&self, operand: Operand) -> u16 {
        match operand {
            Operand::Value(value) => value,
            Operand::Var(slot) => self.var(slot),
        }
    }
}

fn compare(left: u16, test: Comparison, right: u16) -> bool {
    match test {
        Comparison::Equal => left == right,
        Comparison::Greater => left > right,
        Comparison::Less => left < right,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::collections::HashSet;

    use super::*;

    #[derive(Default)]
    struct Recorder {
        log: Vec<String>,
        typewriter: HashSet<u8>,
        flags: HashSet<u16>,
    }

    impl ScriptHost for Recorder {
        fn open_window(&mut self, id: u8, kind: u8, rect: (u8, u8, u8, u8), style: u8) {
            self.log
                .push(format!("open {id} {kind:#x} {rect:?} {style}"));
            if style & 1 != 0 {
                self.typewriter.insert(id);
            }
        }
        fn close_window(&mut self, id: Option<u8>) {
            self.log.push(format!("close {id:?}"));
        }
        fn present(&mut self, id: Option<u8>) {
            self.log.push(format!("present {id:?}"));
        }
        fn clear_window(&mut self, id: u8) {
            self.log.push(format!("clear {id}"));
        }
        fn put_char(&mut self, id: u8, ch: char) {
            self.log.push(format!("char {id} {ch}"));
        }
        fn line_break(&mut self, id: u8) {
            self.log.push(format!("break {id}"));
        }
        fn typewriter(&self, id: u8) -> bool {
            self.typewriter.contains(&id)
        }
        fn portrait(&mut self, id: u8, character: u8, expression: u8) {
            self.log
                .push(format!("portrait {id} {character} {expression}"));
        }
        fn prompt(&mut self, id: u8, visible: bool) {
            let entry = format!("prompt {id} {visible}");
            if self.log.last() != Some(&entry) {
                self.log.push(entry);
            }
        }
        fn play_sound(&mut self, id: u8) {
            self.log.push(format!("sound {id:#x}"));
        }
        fn flag(&self, flag: u16) -> bool {
            self.flags.contains(&flag)
        }
        fn set_flag(&mut self, flag: u16, set: bool) {
            if set {
                self.flags.insert(flag);
            } else {
                self.flags.remove(&flag);
            }
        }
        fn player_name(&self) -> String {
            "AB".to_owned()
        }
        fn reset(&mut self, mode: u8) {
            self.log.push(format!("reset {mode}"));
        }
    }

    /// Lays strings out one after another from offset 16 and returns the
    /// bytes with the table of offsets.
    fn rom(strings: &[Vec<u8>]) -> (Vec<u8>, Vec<usize>) {
        let mut bytes = vec![0; 16];
        let mut offsets = Vec::new();
        for string in strings {
            offsets.push(bytes.len());
            bytes.extend(string);
        }
        (bytes, offsets)
    }

    /// `あ` and `い` as the ROM stores them: Shift-JIS with the trail byte first.
    fn text(s: &str) -> Vec<u8> {
        s.chars()
            .flat_map(|ch| match ch {
                'あ' => [0xA0, 0x82],
                'い' => [0xA2, 0x82],
                other => [u8::try_from(other).unwrap_or(0x20), 0x00],
            })
            .collect()
    }

    fn run(rom: &[u8], runner: &mut ScriptRunner, host: &mut Recorder, frames: usize) -> bool {
        (0..frames).any(|_| runner.update(rom, Input::default(), host).unwrap())
    }

    #[test]
    fn opens_a_window_and_types_a_message_one_character_per_frame() {
        let mut open = vec![0x01, 1, 0x10, 0, 12, 30, 8, 1, 0x04, 0xFF, 0x22];
        let mut talk = vec![0x21, 0, 0, 0x20];
        talk.extend(text("あい"));
        talk.extend([0x0D, 0x1E, 0x1D, 0x22]);
        let (bytes, offsets) = rom(&[std::mem::take(&mut open), talk]);
        let mut runner = ScriptRunner::new(offsets);
        let mut host = Recorder::default();
        runner.start(1).unwrap();
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log, ["open 1 0x10 (0, 12, 30, 8) 1", "present None"]);
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log.len(), 4);
        assert_eq!(host.log[2..], ["char 1 あ", "present Some(1)"]);
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log.len(), 6);
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log[6..], ["break 1", "char 1 A", "present Some(1)"]);
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log[9..], ["char 1 B", "present Some(1)"]);
        assert!(runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert!(runner.is_done());
    }

    #[test]
    fn waits_for_a_while_blinking_the_prompt() {
        let script = vec![0x01, 1, 0x10, 0, 12, 30, 8, 1, 0x05, 0x00, 0x03, 1, 0x22];
        let (bytes, offsets) = rom(&[script]);
        let mut runner = ScriptRunner::new(offsets);
        let mut host = Recorder::default();
        runner.start(0).unwrap();
        runner.update(&bytes, Input::default(), &mut host).unwrap();
        assert!(runner.is_waiting_for_key());
        for _ in 0..19 {
            runner.update(&bytes, Input::default(), &mut host).unwrap();
        }
        assert_eq!(host.log.last().unwrap(), "prompt 1 false");
        runner.update(&bytes, Input::default(), &mut host).unwrap();
        assert_eq!(host.log.last().unwrap(), "prompt 1 true");
        for _ in 0..20 {
            runner.update(&bytes, Input::default(), &mut host).unwrap();
        }
        assert_eq!(host.log.last().unwrap(), "prompt 1 false");
        let a = Input::default().with(Button::A);
        assert!(!runner.update(&bytes, a, &mut host).unwrap());
        assert_eq!(runner.vars()[0], 1);
        assert_eq!(
            host.log[host.log.len() - 2..],
            ["sound 0x41", "prompt 1 false"]
        );
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log.last().unwrap(), "close Some(1)");
        assert!(runner.update(&bytes, Input::default(), &mut host).unwrap());
    }

    #[test]
    fn variables_jumps_and_flags_follow_the_original_rules() {
        let script = vec![
            0x09, 0, 5, 0, // var0 = 5
            0x15, 0, 3, 0, // var0 += 3
            0x0A, 1, 0, // var1 = var0
            0x1B, 1, 0,    // var1 *= var0
            0x0B, // store
            0x33, 0x34, 0x12, 1, // set flag 0x1234
            0x35, 2, 0x34, 0x12, // var2 = flag
            0x10, 0, 8, 0, 12, 0, // if var0 == 8 jump +12 (skip the next 2 instrs)
            0x09, 3, 0xFF, 0xFF, // skipped
            0x3A, 1, // skipped
            0x07, 2, 6, 0, 8, 0, // switch var2 (=1) -> +8
            0x3A, 9, // skipped
            0x3D, 2, // delay 2
            0x22,
        ];
        let (bytes, offsets) = rom(&[script]);
        let mut runner = ScriptRunner::new(offsets);
        let mut host = Recorder::default();
        runner.start(0).unwrap();
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(runner.vars()[..4], [8, 64, 1, 0]);
        assert!(host.flags.contains(&0x1234));
        assert!(host.log.is_empty());
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert!(runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert!(host.log.is_empty());
    }

    #[test]
    fn calls_return_to_the_caller_and_keep_saved_variables() {
        let callee = vec![0x0C, 0x37, 0, 0, 1, 0x22];
        let main = vec![
            0x09, 0, 2, 0, 0x09, 1, 6, 0, 0x0B, 0x21, 0, 0, 0x02, 0, 0x3A, 0, 0x22,
        ];
        let (bytes, offsets) = rom(&[callee, main]);
        let mut runner = ScriptRunner::new(offsets);
        let mut host = Recorder::default();
        runner.start(1).unwrap();
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log, ["portrait 0 2 6"]);
        assert!(runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log[1..], ["reset 0", "sound 0x3c"]);
        assert_eq!(runner.vars()[0], 0);
        assert_eq!(runner.start(5), Err(ScriptError::NoSuchString { index: 5 }));
    }

    #[test]
    fn stops_on_undecodable_bytes() {
        let (bytes, offsets) = rom(&[vec![0x09, 0]]);
        let mut runner = ScriptRunner::new(offsets);
        runner.start(0).unwrap();
        assert!(matches!(
            runner.update(&bytes, Input::default(), &mut Recorder::default()),
            Err(ScriptError::Decode(_))
        ));
    }

    #[test]
    fn prints_variables_and_switches_windows_inside_messages() {
        let script = vec![
            0x01, 0, 0x10, 0, 0, 8, 8, 0, 0x01, 1, 0x10, 8, 0, 8, 8, 0, 0x09, 3, 42, 0, 0x20, 0x1C,
            0, 0x1F, 3, 0x03, 0x1D, 0x22,
        ];
        let (bytes, offsets) = rom(&[script]);
        let mut runner = ScriptRunner::new(offsets);
        let mut host = Recorder::default();
        runner.start(0).unwrap();
        assert!(run(&bytes, &mut runner, &mut host, 3));
        assert_eq!(host.log[2..], ["char 0  ", "char 0 4", "char 0 2"]);
    }
}
