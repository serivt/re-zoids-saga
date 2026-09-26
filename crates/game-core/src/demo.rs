//! The end of the demo, a port feature: once chapter 1 is over (the party
//! has landed in Sand Colony's field, where the port's story stops), the
//! game thanks the player, offers to save and goes back to the title.
//!
//! Source of knowledge: this project's own design, drawn with the game's
//! windows: the thanks and the question in a story box, the original's
//! はい/いいえ window (`pause-menu` 61), the save slots' list (see
//! [`crate::slots`]) and a fade to black.

use platform::{Button, Input};

use crate::port_text::{DEMO_QUESTION, DEMO_SAVED, DEMO_THANKS, port_text};
use crate::script::{ScriptError, ScriptHost, ScriptRunner};
use crate::slots::{Pick, Purpose, Slot, SlotPicker, TITLE_LAYOUT};
use crate::windows::ScriptWindows;

/// The story box the messages go in: the width of the screen, three lines.
const BOX: u8 = 0;
const BOX_RECT: (u8, u8, u8, u8) = (0, 12, 30, 8);
const BOX_KIND: u8 = 0x10;
const BOX_STYLE: u8 = 1;
/// The pause menu's はい/いいえ window, a cancelable menu.
const YES_NO_SCRIPT: usize = 61;
const YES: u16 = 0;
const PROMPT_HALF_PERIOD: u32 = 21;
const CONFIRM_SOUND: u8 = 0x41;
/// Levels of the fade to black, one a frame, and the frames it stays black
/// before the title.
const FADE_LEVELS: u8 = 16;
const BLACK_FRAMES: u32 = 30;
/// Frames the field is left to be seen before the thanks.
const ARRIVAL_FRAMES: u32 = 60;

/// Where the end of the demo stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Frames since the demo ended, before the thanks.
    Arriving(u32),
    /// The thanks, frames since shown, until A.
    Thanks(u32),
    /// The question and its はい/いいえ.
    Question,
    /// The list of slots to save into.
    Slots,
    /// The game is to be written into this slot.
    Saving(usize),
    /// The notice that it was saved, frames since shown, until A.
    Saved(u32),
    /// Frames since the fade to black began.
    Leaving(u32),
}

/// What the game has to do for the end of the demo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoStep {
    /// Nothing: it goes on.
    Continue,
    /// Write the game into save slot `n`, then call [`DemoEnd::saved`].
    Save(usize),
    /// The fade to black has begun: stop the music.
    Leaving,
    /// Go back to the title.
    Title,
}

/// The end of the demo on screen.
pub struct DemoEnd {
    phase: Phase,
    runner: ScriptRunner,
    slots: Vec<Slot>,
    line: usize,
    picker: Option<SlotPicker>,
    /// The buttons of the frame before, for the waits for A.
    previous: Input,
}

impl DemoEnd {
    /// Ends the demo; `yes_no` are the pause menu's script offsets, and
    /// `slots` what each save slot holds, the list starting on `line` (with
    /// one slot or none, the game is written into the first directly).
    #[must_use]
    pub fn new(yes_no: Vec<usize>, slots: Vec<Slot>, line: usize) -> Self {
        Self {
            phase: Phase::Arriving(0),
            runner: ScriptRunner::named(crate::translation::PAUSE_MENU_TABLE, yes_no),
            slots,
            line,
            picker: None,
            previous: Input::default(),
        }
    }

    /// Starts the end; `down` are the buttons held now, which are not
    /// read as presses. The thanks come a second later.
    pub fn open(&mut self, down: Input) {
        self.previous = down;
        self.phase = Phase::Arriving(0);
    }

    /// How dark the screen is, 0 to 16.
    #[must_use]
    pub fn darkness(&self) -> u8 {
        match self.phase {
            Phase::Leaving(frames) => {
                u8::try_from(frames).map_or(FADE_LEVELS, |level| level.min(FADE_LEVELS))
            }
            _ => 0,
        }
    }

    /// Advances a frame.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when the はい/いいえ script cannot run.
    pub fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<DemoStep, ScriptError> {
        let pressed = input.is_held(Button::A) && !self.previous.is_held(Button::A);
        self.previous = input;
        match self.phase {
            Phase::Arriving(frames) if frames + 1 >= ARRIVAL_FRAMES => {
                show_message(windows, DEMO_THANKS);
                self.phase = Phase::Thanks(0);
            }
            Phase::Arriving(frames) => self.phase = Phase::Arriving(frames + 1),
            Phase::Thanks(frames) => {
                if key_wait(pressed, frames, windows) {
                    windows.clear_window(BOX);
                    put_text(windows, &port_text(windows.extensions(), DEMO_QUESTION));
                    self.runner.hold(input);
                    self.runner.start(YES_NO_SCRIPT)?;
                    self.phase = Phase::Question;
                } else {
                    self.phase = Phase::Thanks(frames + 1);
                }
            }
            Phase::Question => {
                if !self.runner.update(rom, input, windows)? {
                    return Ok(DemoStep::Continue);
                }
                let [code, choice, ..] = *self.runner.vars();
                return Ok(if code != 0 && choice == YES {
                    self.choose_slot(input, windows)
                } else {
                    self.leave(windows)
                });
            }
            Phase::Slots => {
                let Some(picker) = self.picker.as_mut() else {
                    return Ok(self.leave(windows));
                };
                let Some(pick) = picker.update(rom, input, windows)? else {
                    return Ok(DemoStep::Continue);
                };
                picker.close(windows);
                self.picker = None;
                return match pick {
                    Pick::Slot(slot) => {
                        self.phase = Phase::Saving(slot);
                        Ok(DemoStep::Save(slot))
                    }
                    Pick::Canceled => {
                        show_message(windows, DEMO_QUESTION);
                        self.runner.hold(input);
                        self.runner.start(YES_NO_SCRIPT)?;
                        self.phase = Phase::Question;
                        Ok(DemoStep::Continue)
                    }
                };
            }
            Phase::Saving(slot) => return Ok(DemoStep::Save(slot)),
            Phase::Saved(frames) => {
                if key_wait(pressed, frames, windows) {
                    return Ok(self.leave(windows));
                }
                self.phase = Phase::Saved(frames + 1);
            }
            Phase::Leaving(frames) => {
                if frames >= u32::from(FADE_LEVELS) + BLACK_FRAMES {
                    return Ok(DemoStep::Title);
                }
                self.phase = Phase::Leaving(frames + 1);
            }
        }
        Ok(DemoStep::Continue)
    }

    /// Tells the player the game was saved, or leaves when it could not be.
    pub fn saved(&mut self, written: bool, windows: &mut ScriptWindows<'_>) {
        if written {
            show_message(windows, DEMO_SAVED);
            self.phase = Phase::Saved(0);
        } else {
            self.leave(windows);
        }
    }

    /// The slots' list, or with one slot that slot at once.
    fn choose_slot(&mut self, input: Input, windows: &mut ScriptWindows<'_>) -> DemoStep {
        if self.slots.len() < 2 {
            self.phase = Phase::Saving(0);
            return DemoStep::Save(0);
        }
        windows.close_window(None);
        let mut picker =
            SlotPicker::new(self.slots.clone(), Purpose::Save, TITLE_LAYOUT, self.line);
        picker.open(input, windows);
        self.picker = Some(picker);
        self.phase = Phase::Slots;
        DemoStep::Continue
    }

    /// Closes the windows and starts the fade to black.
    fn leave(&mut self, windows: &mut ScriptWindows<'_>) -> DemoStep {
        windows.close_window(None);
        self.phase = Phase::Leaving(0);
        DemoStep::Leaving
    }
}

/// A frame of a wait for A with the prompt blinking; returns `pressed`,
/// whether A was pressed, with the confirmation's sound.
fn key_wait(pressed: bool, frames: u32, windows: &mut ScriptWindows<'_>) -> bool {
    if pressed {
        windows.prompt(BOX, false);
        windows.play_sound(CONFIRM_SOUND);
    } else {
        windows.prompt(BOX, (frames + 1) / PROMPT_HALF_PERIOD % 2 == 1);
    }
    pressed
}

/// Opens the story box with the port's message `key`.
fn show_message(windows: &mut ScriptWindows<'_>, key: &str) {
    windows.close_window(None);
    windows.open_window(BOX, BOX_KIND, BOX_RECT, BOX_STYLE);
    windows.present(Some(BOX));
    put_text(windows, &port_text(windows.extensions(), key));
}

/// Prints `text` into the story box, a new line at each line break.
fn put_text(windows: &mut ScriptWindows<'_>, text: &str) {
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            windows.line_break(BOX);
        }
        for ch in line.chars() {
            windows.put_char(BOX, ch);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn lines(windows: &ScriptWindows<'_>) -> Vec<String> {
        windows.windows()[usize::from(BOX)]
            .as_ref()
            .expect("the story box")
            .lines
            .clone()
    }

    #[test]
    fn the_thanks_come_a_second_after_the_arrival() {
        let mut windows = ScriptWindows::new(&[], "");
        let mut demo = DemoEnd::new(Vec::new(), Vec::new(), 0);
        demo.open(Input::default());
        for _ in 1..ARRIVAL_FRAMES {
            let step = demo.update(&[], Input::default(), &mut windows);
            assert_eq!(step.expect("frame"), DemoStep::Continue);
        }
        assert!(!windows.any_open());
        demo.update(&[], Input::default(), &mut windows)
            .expect("frame");
        assert_eq!(
            lines(&windows),
            [
                "あそんでくれて\u{3000}ありがとう！",
                "この体験版は\u{3000}ここまでです。",
                "つづきは\u{3000}これからのバージョンで！",
            ]
        );
        assert_eq!(demo.darkness(), 0);
    }

    #[test]
    fn a_save_that_fails_fades_to_the_title() {
        let mut windows = ScriptWindows::new(&[], "");
        let mut demo = DemoEnd::new(Vec::new(), Vec::new(), 0);
        demo.saved(false, &mut windows);
        let mut frames = 0;
        while demo
            .update(&[], Input::default(), &mut windows)
            .expect("frame")
            != DemoStep::Title
        {
            frames += 1;
        }
        assert_eq!(frames, u32::from(FADE_LEVELS) + BLACK_FRAMES);
        assert_eq!(demo.darkness(), FADE_LEVELS);
    }

    #[test]
    fn a_saved_game_is_announced_until_a() {
        let mut windows = ScriptWindows::new(&[], "");
        let mut demo = DemoEnd::new(Vec::new(), Vec::new(), 0);
        demo.saved(true, &mut windows);
        assert_eq!(lines(&windows), ["セーブしました。"]);
        let a = Input::default().with(Button::A);
        assert_eq!(
            demo.update(&[], a, &mut windows).expect("frame"),
            DemoStep::Leaving
        );
        assert!(!windows.any_open());
    }
}
