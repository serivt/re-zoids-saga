//! The script interpreter: runs dialogue strings one frame at a time.
//!
//! Matches the original runner (ROM `0x0803E54C`): eight 16-bit variables,
//! a saved copy of them that survives across strings, calls of other
//! strings of the same table, jumps relative to the opcode, windows the
//! host owns, and waits. Every operation that flushes the display in the
//! original costs one frame here: opening, presenting or clearing a window,
//! showing a portrait, and each character of a typewriter window; closing a
//! window costs two while another stays open (the others are redrawn) and
//! resetting the text system three. The
//! key wait polls once per frame and blinks the prompt 20 frames off, 20
//! on; after the key that ends a key wait or a menu, the script goes on the
//! next frame.
//!
//! One addition to the original: a message that would scroll its window
//! (a translation longer than the Japanese text) stops before the line that
//! scrolls and waits for A with the same prompt; the window then turns a
//! page, keeping only its first line (the speaker's name), so nothing
//! leaves the screen unread.

use crate::extension::Event;
use formats::script_ops::{
    Comparison, Instruction, MessageStep, Operand, Operation, ScriptOpError, decode_instruction,
    decode_message_step,
};
use platform::{Button, Input};

const VARIABLES: usize = 8;
const WINDOWS: u8 = 8;
const PROMPT_HALF_PERIOD: u32 = 20;
const CONFIRM_SOUND: u8 = 0x41;
/// Frames the text system's reset takes to redraw the cleared screen.
const RESET_FRAMES: u32 = 3;
/// Frames between the key that ends a key wait or a menu and the next
/// operation.
const KEY_ACCEPT_FRAMES: u32 = 1;
const MENU_MOVE_SOUND: u8 = 0x40;
const MENU_CONFIRM_SOUND: u8 = 0x47;
const KEY_A: u16 = 1;
const KEY_B: u16 = 2;
const KEY_UP: u16 = 0x40;
const KEY_DOWN: u16 = 0x80;
const KEY_R: u16 = 0x100;
const KEY_L: u16 = 0x200;
/// What a move-reporting menu (`0x36`) leaves in var0 for a cursor moved
/// up or down, and for L or R in the modes that take them.
const MOVED_UP: u16 = 0x20;
const MOVED_DOWN: u16 = 0x40;
const PAGE_LEFT: u16 = 2;
const PAGE_RIGHT: u16 = 4;

/// What the interpreter asks of the game: windows, text, sounds and flags.
pub trait ScriptHost {
    /// Opens (or reopens) window `id` at the tile rectangle.
    fn open_window(&mut self, id: u8, kind: u8, rect: (u8, u8, u8, u8), style: u8);
    /// Closes window `id`, or all of them.
    fn close_window(&mut self, id: Option<u8>);
    /// Redraws window `id` (or all of them) with its current contents.
    fn present(&mut self, id: Option<u8>);
    /// Makes window `id` visible as its text is typed, without a redraw.
    fn reveal(&mut self, id: u8);
    /// Empties the text of window `id`.
    fn clear_window(&mut self, id: u8);
    /// Prints one character in window `id`.
    fn put_char(&mut self, id: u8, ch: char);
    /// Moves to the next text line of window `id`.
    fn line_break(&mut self, id: u8);
    /// Whether window `id` shows text one character per frame.
    fn typewriter(&self, id: u8) -> bool;
    /// Whether printing `ch` in window `id` would scroll its text: the
    /// runner then waits for a key before going on, so a translation
    /// longer than the window is read a line at a time.
    fn page_full(&self, id: u8, ch: char) -> bool {
        let _ = (id, ch);
        false
    }
    /// Turns the page of window `id` after the key of a full page: its
    /// first line stays, the rest is cleared.
    fn turn_page(&mut self, id: u8) {
        let _ = id;
    }
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
    /// Number of selectable lines in window `id`.
    fn menu_lines(&self, id: u8) -> usize;
    /// Places (or removes) the menu cursor of window `id`.
    fn set_cursor(&mut self, id: u8, line: Option<usize>);
    /// Whether window `id` is open.
    fn is_open(&self, id: u8) -> bool;
    /// Line the next menu in window `id` starts on: where its last one ended.
    fn menu_line(&self, id: u8) -> usize;
    /// The translation of the message at `offset` bytes into string `index`
    /// of script table `table`, if one is loaded.
    fn translate(&self, table: &str, index: usize, offset: usize) -> Option<String> {
        let _ = (table, index, offset);
        None
    }
    /// Hears what the runner did (a script starting or ending, a message
    /// showing).
    fn notify(&mut self, event: Event) {
        let _ = event;
    }
    /// The rectangle window `id` of kind `kind`, opened by string `index`
    /// of `table` as `rect`, should take: a translation may enlarge it.
    fn fit_window(
        &self,
        table: &str,
        index: usize,
        id: u8,
        kind: u8,
        rect: (u8, u8, u8, u8),
    ) -> (u8, u8, u8, u8) {
        let _ = (table, index, id, kind);
        rect
    }
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
    Menu {
        cancelable: bool,
        cursor: usize,
        reports: Option<u8>,
    },
    Page {
        ch: char,
        elapsed: u32,
    },
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Frame {
    index: usize,
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

/// The part of the interpreter's state that outlives a string: see
/// [`ScriptRunner::context`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptContext {
    window: u8,
    text_window: u8,
    saved: [u16; VARIABLES],
}

/// Runs strings of one table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptRunner {
    table: &'static str,
    strings: Vec<usize>,
    frames: Vec<Frame>,
    vars: [u16; VARIABLES],
    saved: [u16; VARIABLES],
    window: u8,
    text_window: u8,
    pending: Vec<MessageStep>,
    substituted: bool,
    announce: Option<usize>,
    started: Option<usize>,
    wait: Wait,
    previous: Input,
}

impl ScriptRunner {
    /// Creates a runner over a table whose strings start at the given ROM
    /// offsets, 0 marking an absent string.
    #[must_use]
    pub fn new(strings: Vec<usize>) -> Self {
        Self::named("", strings)
    }

    /// Creates a runner over the script table called `table`, the name
    /// translations key their messages by.
    #[must_use]
    pub fn named(table: &'static str, strings: Vec<usize>) -> Self {
        Self {
            table,
            strings,
            frames: Vec::new(),
            vars: [0; VARIABLES],
            saved: [0; VARIABLES],
            window: 0,
            text_window: 0,
            pending: Vec::new(),
            substituted: false,
            announce: None,
            started: None,
            wait: Wait::Done,
            previous: Input::default(),
        }
    }

    /// The name of the table the runner runs strings of.
    #[must_use]
    pub fn table(&self) -> &'static str {
        self.table
    }

    /// Starts string `index`, dropping anything that was running.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError::NoSuchString`] when the index has no string.
    pub fn start(&mut self, index: usize) -> Result<(), ScriptError> {
        let base = self.string(index)?;
        self.frames = vec![Frame {
            index,
            base,
            pc: base,
            in_message: false,
        }];
        self.vars = [0; VARIABLES];
        self.pending.clear();
        self.substituted = false;
        self.announce = Some(index);
        self.wait = Wait::None;
        Ok(())
    }

    /// Whether no script is running.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.wait == Wait::Done
    }

    /// Whether the runner is waiting for a key, to go on with the script
    /// or with a message that would scroll.
    #[must_use]
    pub fn is_waiting_for_key(&self) -> bool {
        matches!(self.wait, Wait::Key { .. } | Wait::Page { .. })
    }

    /// The eight script variables.
    #[must_use]
    pub fn vars(&self) -> &[u16; VARIABLES] {
        &self.vars
    }

    /// The copy of the variables the last `StoreVars` made, which outlives
    /// a reset of the text system.
    #[must_use]
    pub fn saved_vars(&self) -> &[u16; VARIABLES] {
        &self.saved
    }

    /// The interpreter state the game keeps from one string to the next:
    /// the original has a single interpreter for every table, so a string
    /// goes on in the window the previous one left current, with the
    /// variables it saved.
    #[must_use]
    pub fn context(&self) -> ScriptContext {
        ScriptContext {
            window: self.window,
            text_window: self.text_window,
            saved: self.saved,
        }
    }

    /// Takes over the state another runner left, before starting a string
    /// of this runner's table.
    pub fn resume(&mut self, context: ScriptContext) {
        self.window = context.window;
        self.text_window = context.text_window;
        self.saved = context.saved;
    }

    /// Where string `index` starts in the ROM, if the table has it.
    #[must_use]
    pub fn string_offset(&self, index: usize) -> Option<usize> {
        self.string(index).ok()
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
        let keys = u16::from(pressed(Button::A)) * KEY_A
            + u16::from(pressed(Button::B)) * KEY_B
            + u16::from(pressed(Button::Up)) * KEY_UP
            + u16::from(pressed(Button::Down)) * KEY_DOWN
            + u16::from(pressed(Button::R)) * KEY_R
            + u16::from(pressed(Button::L)) * KEY_L;
        self.previous = input;
        if let Some(index) = self.announce.take() {
            self.started = Some(index);
            host.notify(Event::ScriptStarted {
                table: self.table.to_owned(),
                index,
            });
        }
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
            Wait::Menu {
                cancelable,
                cursor,
                reports,
            } => {
                if !self.poll_menu(cancelable, cursor, reports, keys, host) {
                    return Ok(false);
                }
            }
            Wait::Page { ch, elapsed } => {
                if !self.poll_page(ch, elapsed, keys, host) {
                    return Ok(false);
                }
            }
            Wait::None => {}
        }
        while self.wait == Wait::None {
            self.step(rom, host)?;
        }
        let done = self.wait == Wait::Done;
        if done && let Some(index) = self.started.take() {
            host.notify(Event::ScriptEnded {
                table: self.table.to_owned(),
                index,
            });
        }
        Ok(done)
    }

    /// Starts a menu on the current window, which the original shows even
    /// when the script did not present it (the guides' popups).
    fn begin_menu(&mut self, cancelable: bool, reports: Option<u8>, host: &mut impl ScriptHost) {
        host.reveal(self.window);
        let cursor = host
            .menu_line(self.window)
            .min(host.menu_lines(self.window).saturating_sub(1));
        host.set_cursor(self.window, Some(cursor));
        self.wait = Wait::Menu {
            cancelable,
            cursor,
            reports,
        };
    }

    /// Makes window `id` the current one, as the game's code does before
    /// printing into a window a script did not open last.
    pub fn select_window(&mut self, id: u8) {
        self.window = id;
    }

    /// Runs a menu on window `id` without a script, as the game's code
    /// does for lists it fills itself; the result lands in the variables
    /// like the menu opcode's.
    pub fn run_menu(&mut self, id: u8, cancelable: bool, host: &mut impl ScriptHost) {
        self.frames.clear();
        self.pending.clear();
        self.window = id;
        self.begin_menu(cancelable, None, host);
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
        self.wait = Wait::Frames(KEY_ACCEPT_FRAMES);
        true
    }

    fn poll_page(&mut self, ch: char, elapsed: u32, keys: u16, host: &mut impl ScriptHost) -> bool {
        if keys & KEY_A == 0 {
            let elapsed = elapsed + 1;
            host.prompt(self.text_window, elapsed / PROMPT_HALF_PERIOD % 2 == 1);
            self.wait = Wait::Page { ch, elapsed };
            return false;
        }
        host.prompt(self.text_window, false);
        host.play_sound(CONFIRM_SOUND);
        host.turn_page(self.text_window);
        self.wait = Wait::None;
        self.print(ch, host);
        true
    }

    fn poll_menu(
        &mut self,
        cancelable: bool,
        cursor: usize,
        reports: Option<u8>,
        keys: u16,
        host: &mut impl ScriptHost,
    ) -> bool {
        let lines = host.menu_lines(self.window).max(1);
        let moved = if keys & KEY_UP != 0 && cursor > 0 {
            Some((cursor - 1, MOVED_UP))
        } else if keys & KEY_DOWN != 0 && cursor + 1 < lines {
            Some((cursor + 1, MOVED_DOWN))
        } else {
            None
        };
        if let Some((cursor, code)) = moved {
            host.play_sound(MENU_MOVE_SOUND);
            host.set_cursor(self.window, Some(cursor));
            if reports.is_none() {
                self.wait = Wait::Menu {
                    cancelable,
                    cursor,
                    reports,
                };
                return false;
            }
            self.vars[0] = code;
            self.vars[1] = u16::try_from(cursor).unwrap_or(u16::MAX);
            self.wait = Wait::Frames(KEY_ACCEPT_FRAMES);
            return true;
        }
        let page = match reports {
            Some(2 | 3) if keys & KEY_L != 0 => Some(PAGE_LEFT),
            Some(2 | 4) if keys & KEY_R != 0 => Some(PAGE_RIGHT),
            _ => None,
        };
        if let Some(code) = page {
            self.vars[0] = code;
        } else if keys & KEY_A != 0 {
            self.vars[0] = 1;
            host.play_sound(MENU_CONFIRM_SOUND);
        } else if cancelable && keys & KEY_B != 0 {
            self.vars[0] = 0;
        } else {
            return false;
        }
        self.vars[1] = u16::try_from(cursor).unwrap_or(u16::MAX);
        self.wait = Wait::Frames(KEY_ACCEPT_FRAMES);
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
            Instruction::Message => {
                frame.in_message = true;
                self.text_window = self.window;
                let (index, offset) = (frame.index, opcode_at - frame.base);
                host.notify(Event::MessageShown {
                    table: self.table.to_owned(),
                    index,
                    offset,
                    window: self.window,
                });
                let translated = host.translate(self.table, index, offset);
                if let Some(text) = translated {
                    self.substitute(rom, &text)?;
                }
            }
            Instruction::Call(index) => {
                let base = self.string(usize::from(index))?;
                self.frames.push(Frame {
                    index: usize::from(index),
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
                let index = self.frames.last().map_or(0, |frame| frame.index);
                let rect = host.fit_window(self.table, index, id, kind, (x, y, width, height));
                host.open_window(id, kind, rect, style);
                self.window = id;
                self.wait = Wait::Frames(1);
            }
            Instruction::Reset { mode } => {
                host.reset(mode);
                if mode & 0xF0 == 0 {
                    self.vars = [0; VARIABLES];
                }
                self.wait = Wait::Frames(RESET_FRAMES);
            }
            Instruction::CloseWindow { id } => {
                host.close_window(id);
                let redrawn = (0..WINDOWS).any(|id| host.is_open(id));
                self.wait = Wait::Frames(1 + u32::from(redrawn));
            }
            Instruction::Present { id } => {
                host.present(id);
                self.window = id
                    .or_else(|| (0..WINDOWS).rev().find(|id| host.is_open(*id)))
                    .unwrap_or(self.window);
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
            Instruction::Menu { mode } => self.begin_menu(mode & 0xF0 == 0x10, None, host),
            Instruction::MoveMenu { mode, cancelable } => {
                self.begin_menu(cancelable, Some(mode), host);
            }
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

    /// Replaces the message starting at the frame's cursor with translated
    /// text: its leading window switches still apply, the rest is skipped
    /// and the translation's steps are queued instead.
    fn substitute(&mut self, rom: &[u8], text: &str) -> Result<(), ScriptError> {
        let Some(frame) = self.frames.last_mut() else {
            return Ok(());
        };
        let mut leading = true;
        loop {
            let (step, next) = decode_message_step(rom, frame.pc)?;
            frame.pc = next;
            match step {
                MessageStep::SwitchWindow(id) if leading => self.text_window = id,
                MessageStep::End => break,
                _ => leading = false,
            }
        }
        let mut steps = crate::translation::steps(text);
        steps.reverse();
        self.pending = steps;
        self.substituted = true;
        Ok(())
    }

    fn message_step(&mut self, rom: &[u8], host: &mut impl ScriptHost) -> Result<(), ScriptError> {
        let step = match self.pending.pop() {
            Some(step) => step,
            None if self.substituted => {
                self.substituted = false;
                MessageStep::End
            }
            None => {
                let Some(frame) = self.frames.last_mut() else {
                    self.wait = Wait::Done;
                    return Ok(());
                };
                let (step, next) = decode_message_step(rom, frame.pc)?;
                frame.pc = next;
                step
            }
        };
        match step {
            MessageStep::Character(ch) if host.page_full(self.text_window, ch) => {
                self.wait = Wait::Page { ch, elapsed: 0 };
            }
            MessageStep::Character(ch) => self.print(ch, host),
            MessageStep::LineBreak => host.line_break(self.text_window),
            MessageStep::SwitchWindow(id) => self.text_window = id,
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
        self.pending
            .extend(text.chars().rev().map(MessageStep::Character));
    }

    fn print(&mut self, ch: char, host: &mut impl ScriptHost) {
        host.put_char(self.text_window, ch);
        if host.typewriter(self.text_window) {
            host.reveal(self.text_window);
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
        open: HashSet<u8>,
        translations: std::collections::HashMap<String, String>,
        page_rows: usize,
        lines: std::collections::HashMap<u8, usize>,
        pending_break: HashSet<u8>,
    }

    impl ScriptHost for Recorder {
        fn open_window(&mut self, id: u8, kind: u8, rect: (u8, u8, u8, u8), style: u8) {
            self.log
                .push(format!("open {id} {kind:#x} {rect:?} {style}"));
            if style & 1 != 0 {
                self.typewriter.insert(id);
            }
            self.open.insert(id);
        }
        fn close_window(&mut self, id: Option<u8>) {
            self.log.push(format!("close {id:?}"));
            match id {
                Some(id) => {
                    self.open.remove(&id);
                }
                None => self.open.clear(),
            }
        }
        fn present(&mut self, id: Option<u8>) {
            self.log.push(format!("present {id:?}"));
        }
        fn reveal(&mut self, id: u8) {
            self.log.push(format!("reveal {id}"));
        }
        fn clear_window(&mut self, id: u8) {
            self.log.push(format!("clear {id}"));
        }
        fn put_char(&mut self, id: u8, ch: char) {
            self.log.push(format!("char {id} {ch}"));
            let lines = self.lines.entry(id).or_insert(1);
            if self.pending_break.remove(&id) {
                *lines += 1;
            }
        }
        fn line_break(&mut self, id: u8) {
            self.log.push(format!("break {id}"));
            if !self.pending_break.insert(id) {
                *self.lines.entry(id).or_insert(1) += 1;
            }
        }
        fn page_full(&self, id: u8, _ch: char) -> bool {
            self.page_rows > 0
                && self.lines.get(&id).copied().unwrap_or(0) >= self.page_rows
                && self.pending_break.contains(&id)
        }
        fn turn_page(&mut self, id: u8) {
            self.log.push(format!("turn {id}"));
            self.lines.insert(id, 1);
            self.pending_break.insert(id);
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
        fn menu_lines(&self, _: u8) -> usize {
            3
        }
        fn set_cursor(&mut self, id: u8, line: Option<usize>) {
            self.log.push(format!("cursor {id} {line:?}"));
        }
        fn is_open(&self, id: u8) -> bool {
            self.open.contains(&id)
        }
        fn menu_line(&self, _: u8) -> usize {
            0
        }
        fn translate(&self, table: &str, index: usize, offset: usize) -> Option<String> {
            self.translations
                .get(&crate::translation::key(table, index, offset))
                .cloned()
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
        assert_eq!(host.log, ["open 1 0x10 (0, 12, 30, 8) 1"]);
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log[1..], ["present None"]);
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log.len(), 4);
        assert_eq!(host.log[2..], ["char 1 あ", "reveal 1"]);
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log.len(), 6);
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log[6..], ["break 1", "char 1 A", "reveal 1"]);
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log[9..], ["char 1 B", "reveal 1"]);
        assert!(runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert!(runner.is_done());
    }

    #[test]
    fn a_message_that_would_scroll_waits_for_a_key_and_turns_a_page() {
        let mut script = vec![0x01, 1, 0x10, 0, 12, 30, 8, 1, 0x20];
        for (line, ch) in ["あ", "い", "あ", "い", "あ"].into_iter().enumerate() {
            if line > 0 {
                script.push(0x0D);
            }
            script.extend(text(ch));
        }
        script.extend([0x1D, 0x22]);
        let (bytes, offsets) = rom(&[script]);
        let mut runner = ScriptRunner::new(offsets);
        let mut host = Recorder {
            page_rows: 3,
            ..Recorder::default()
        };
        runner.start(0).unwrap();
        for _ in 0..5 {
            assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        }
        assert_eq!(host.log.last().unwrap(), "break 1");
        assert!(runner.is_waiting_for_key());
        for _ in 0..20 {
            runner.update(&bytes, Input::default(), &mut host).unwrap();
        }
        assert_eq!(host.log.last().unwrap(), "prompt 1 true");
        assert_eq!(
            host.log
                .iter()
                .filter(|entry| *entry == "char 1 い")
                .count(),
            1
        );
        let a = Input::default().with(Button::A);
        assert!(!runner.update(&bytes, a, &mut host).unwrap());
        assert_eq!(
            host.log[host.log.len() - 5..],
            [
                "prompt 1 false",
                "sound 0x41",
                "turn 1",
                "char 1 い",
                "reveal 1"
            ]
        );
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(
            host.log[host.log.len() - 3..],
            ["break 1", "char 1 あ", "reveal 1"]
        );
        assert!(!runner.is_waiting_for_key());
        assert!(runner.update(&bytes, Input::default(), &mut host).unwrap());
    }

    #[test]
    fn waits_for_a_while_blinking_the_prompt() {
        let script = vec![0x01, 1, 0x10, 0, 12, 30, 8, 1, 0x05, 0x00, 0x03, 1, 0x22];
        let (bytes, offsets) = rom(&[script]);
        let mut runner = ScriptRunner::new(offsets);
        let mut host = Recorder::default();
        runner.start(0).unwrap();
        runner.update(&bytes, Input::default(), &mut host).unwrap();
        assert!(!runner.is_waiting_for_key());
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
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log[1..], ["reset 0"]);
        assert!(!run(&bytes, &mut runner, &mut host, 2));
        assert!(runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log[1..], ["reset 0", "sound 0x3c"]);
        assert_eq!(runner.vars()[0], 0);
        assert_eq!(runner.saved_vars()[..2], [2, 6]);
        assert_eq!(runner.start(5), Err(ScriptError::NoSuchString { index: 5 }));
    }

    #[test]
    fn a_move_reporting_menu_ends_on_each_move_and_page() {
        let script = vec![0x01, 0, 0x21, 10, 10, 9, 8, 4, 0x36, 0x12, 0x22];
        let (bytes, offsets) = rom(&[script]);
        let mut runner = ScriptRunner::new(offsets);
        let mut host = Recorder::default();
        runner.start(0).unwrap();
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        let down = Input::default().with(Button::Down);
        assert!(!runner.update(&bytes, down, &mut host).unwrap());
        assert_eq!(runner.vars()[..2], [MOVED_DOWN, 1]);
        assert!(runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(
            host.log[host.log.len() - 2..],
            ["sound 0x40", "cursor 0 Some(1)"]
        );
        for (button, code) in [
            (Button::L, PAGE_LEFT),
            (Button::R, PAGE_RIGHT),
            (Button::B, 0),
        ] {
            runner.start(0).unwrap();
            runner.update(&bytes, Input::default(), &mut host).unwrap();
            runner.update(&bytes, Input::default(), &mut host).unwrap();
            runner
                .update(&bytes, Input::default().with(button), &mut host)
                .unwrap();
            assert_eq!(runner.vars()[0], code);
        }
    }

    #[test]
    fn menus_move_the_cursor_and_report_the_choice() {
        let script = vec![0x01, 0, 0x21, 10, 10, 9, 8, 4, 0x06, 0x10, 0x22];
        let (bytes, offsets) = rom(&[script]);
        let mut runner = ScriptRunner::new(offsets);
        let mut host = Recorder::default();
        runner.start(0).unwrap();
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert!(!runner.update(&bytes, Input::default(), &mut host).unwrap());
        assert_eq!(host.log.last().unwrap(), "cursor 0 Some(0)");
        let down = Input::default().with(Button::Down);
        runner.update(&bytes, down, &mut host).unwrap();
        assert_eq!(
            host.log[host.log.len() - 2..],
            ["sound 0x40", "cursor 0 Some(1)"]
        );
        runner.update(&bytes, Input::default(), &mut host).unwrap();
        runner.update(&bytes, down, &mut host).unwrap();
        runner.update(&bytes, Input::default(), &mut host).unwrap();
        runner.update(&bytes, down, &mut host).unwrap();
        assert_eq!(host.log.last().unwrap(), "cursor 0 Some(2)");
        runner.update(&bytes, Input::default(), &mut host).unwrap();
        assert!(
            !runner
                .update(&bytes, Input::default().with(Button::A), &mut host)
                .unwrap()
        );
        assert_eq!(runner.vars()[..2], [1, 2]);
        assert_eq!(
            host.log[host.log.len() - 2..],
            ["cursor 0 Some(2)", "sound 0x47"]
        );
        assert!(runner.update(&bytes, Input::default(), &mut host).unwrap());
        let mut runner = ScriptRunner::new(vec![24]);
        runner.start(0).unwrap();
        runner.update(&bytes, Input::default(), &mut host).unwrap();
        runner.update(&bytes, Input::default(), &mut host).unwrap();
        runner
            .update(&bytes, Input::default().with(Button::B), &mut host)
            .unwrap();
        assert_eq!(runner.vars()[..2], [0, 0]);
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

    #[test]
    fn a_translated_message_replaces_the_text_but_keeps_its_window() {
        let mut script = vec![
            0x01, 0, 0x20, 0, 14, 30, 6, 4, 0x01, 3, 0x21, 0, 0, 9, 14, 4, 0x20, 0x1C, 0,
        ];
        script.extend(text("あい"));
        script.extend([0x1D, 0x04, 0xFF, 0x22]);
        let (bytes, offsets) = rom(&[script]);
        let mut runner = ScriptRunner::named("menu", offsets);
        let mut host = Recorder::default();
        host.translations.insert(
            crate::translation::key("menu", 0, 16),
            "Hi\n{name}".to_owned(),
        );
        runner.start(0).unwrap();
        assert!(run(&bytes, &mut runner, &mut host, 8));
        assert_eq!(
            host.log[2..],
            [
                "char 0 H",
                "char 0 i",
                "break 0",
                "char 0 A",
                "char 0 B",
                "present None"
            ]
        );
    }

    #[test]
    fn menus_and_prompts_follow_the_last_presented_window() {
        let mut script = vec![
            0x01, 0, 0x20, 0, 14, 30, 6, 4, // help line
            0x01, 3, 0x21, 0, 0, 9, 14, 4, // menu
            0x20, 0x1C, 0, // text goes to window 0
        ];
        script.extend(text("あ"));
        script.extend([
            0x1D, 0x04, 0xFF, // presenting everything selects window 3
            0x06, 0x10, // menu
            0x04, 0, // presenting window 0 selects it
            0x05, 0x00, // wait key prompts in window 0
            0x22,
        ]);
        let (bytes, offsets) = rom(&[script]);
        let mut runner = ScriptRunner::new(offsets);
        let mut host = Recorder::default();
        runner.start(0).unwrap();
        run(&bytes, &mut runner, &mut host, 5);
        assert_eq!(
            host.log[2..],
            ["char 0 あ", "present None", "reveal 3", "cursor 3 Some(0)"]
        );
        let a = Input::default().with(Button::A);
        runner.update(&bytes, a, &mut host).unwrap();
        run(&bytes, &mut runner, &mut host, 4);
        assert!(runner.is_waiting_for_key());
        assert_eq!(host.log.last().unwrap(), "prompt 0 false");
    }
}
