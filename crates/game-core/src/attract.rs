//! The title's attract demo: after ten seconds on the title without START,
//! the game darkens it and plays four battle scenes staged by the battle
//! module, one after another, then loads the title again; the next time it
//! plays the other of its two demos. START stops it at any scene.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1). The
//! title controller (`0x080020E0`) counts the frames without START in its
//! state 200 and sets bit 4 of `0x0200E8A8` past `0x257`; states 2000 and
//! `0x834` darken with the fade task (`0x08004034`, slot 10); state `0x898`
//! starts the demo task; state `0x8FC` waits for its end, turns to the
//! other demo and loads the title again from state 0. The demo task
//! (`0x080034AC`) runs the battle module (`0x0803DC54`, slot 4) on the
//! records at `0x66413C` (`0xB0` a demo, `0x2C` a scene), skippable, four
//! scenes, then ends. With its record's `+0xC` set the battle module takes
//! START: the screen blacks at once (`0x0800196C`) and the scene ends with
//! 2, the demo with it. Checked in a reference emulator frame by frame: the
//! stretches of black and of scene start and end on the original's frames
//! (the scenes' own steps within a frame or two), the pictures match, and
//! the title loads again 7 frames after the last scene's end, or 6 after
//! START.

use platform::{Button, Frame, Input};

use crate::battle::StagedAttack;
use crate::boot::TitleScreen;
use crate::data::GameData;
use crate::script::ScriptError;
use crate::windows::ScriptWindows;
use crate::{TextPainter, WindowPainter};
use extraction::saga_battle::{DEMO_SCENE_COUNT, DEMOS};
use gba_runtime::ppu::{FADE_STEPS, darken};

/// Frames from the count's end to the first scene's start: the fade task's
/// darkening and the tasks' hand-overs.
const LEAVE_FRAMES: u32 = 38;
/// Frames of the darkening's start the fade task leaves at full light: the
/// controller's hand-over, the fade task's first frame, and the frame the
/// level reaches the screen.
const FADE_DELAY: u32 = 5;
/// Black frames before the title shows again: after the last scene, the
/// two before the title's loader runs; after START, which blacks the
/// screen at once (`0x0800196C`), the six before it runs; then the 26 the
/// loader takes before the title shows, as at the boot.
const END_FRAMES: u32 = 2 + TITLE_LOAD_FRAMES;
const STOPPED_FRAMES: u32 = 6 + TITLE_LOAD_FRAMES;
const TITLE_LOAD_FRAMES: u32 = 26;

/// What the demo does after a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttractStep {
    /// It goes on.
    Playing,
    /// It is over, or START stopped it: the title loads again.
    Title,
}

enum Phase {
    /// The title darkening: frames since the count ran out.
    Leaving(Box<TitleScreen>, u32),
    /// A scene playing.
    Scene(Box<StagedAttack>),
    /// Black after the last scene or START: frames since, and the frames
    /// it lasts.
    Ending(u32, u32),
}

/// The attract demo on screen.
pub struct Attract {
    demo: usize,
    scene: usize,
    phase: Phase,
    previous: Input,
}

impl Attract {
    /// Demo `demo` (0 or 1), over `title` as it starts to darken; `held`
    /// are the buttons down now, which are not read as a press.
    #[must_use]
    pub fn new(demo: usize, title: TitleScreen, held: Input) -> Self {
        Self {
            demo: demo % DEMOS,
            scene: 0,
            phase: Phase::Leaving(Box::new(title), 0),
            previous: held,
        }
    }

    /// The demo the title plays after `demo`.
    #[must_use]
    pub fn next(demo: usize) -> usize {
        (demo + 1) % DEMOS
    }

    /// Advances one frame.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a scene's line cannot run.
    pub fn update(
        &mut self,
        data: &GameData<'_>,
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<AttractStep, ScriptError> {
        let start = input.is_held(Button::Start) && !self.previous.is_held(Button::Start);
        self.previous = input;
        if start && !matches!(self.phase, Phase::Ending(..)) {
            self.phase = Phase::Ending(0, STOPPED_FRAMES);
            return Ok(AttractStep::Playing);
        }
        match &mut self.phase {
            Phase::Leaving(_, frames) => {
                *frames += 1;
                if *frames >= LEAVE_FRAMES {
                    return Ok(self.start_scene(data));
                }
            }
            Phase::Scene(stage) => {
                stage.update(data, Input::default(), windows)?;
                if stage.is_done() {
                    self.scene += 1;
                    return Ok(self.start_scene(data));
                }
            }
            Phase::Ending(frames, total) => {
                *frames += 1;
                if *frames >= *total {
                    return Ok(AttractStep::Title);
                }
            }
        }
        Ok(AttractStep::Playing)
    }

    /// Starts the demo's next scene, or ends the demo after its last.
    fn start_scene(&mut self, data: &GameData<'_>) -> AttractStep {
        if self.scene >= DEMO_SCENE_COUNT {
            self.phase = Phase::Ending(0, END_FRAMES);
            return AttractStep::Playing;
        }
        match StagedAttack::demo(data, self.demo, self.scene) {
            Ok(stage) => {
                self.phase = Phase::Scene(Box::new(stage));
                AttractStep::Playing
            }
            Err(_) => AttractStep::Title,
        }
    }

    /// The sound effects the scene requested since the last call.
    pub fn take_sounds(&mut self) -> Vec<u16> {
        match &mut self.phase {
            Phase::Scene(stage) => stage.take_sounds(),
            Phase::Leaving(..) | Phase::Ending(..) => Vec::new(),
        }
    }

    /// Keeps what the vertical blank copies.
    pub fn latch(&mut self) {
        if let Phase::Scene(stage) = &mut self.phase {
            stage.latch();
        }
    }

    /// Draws the title darkening, or the scene.
    pub fn draw(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
    ) {
        match &self.phase {
            Phase::Leaving(title, frames) => {
                title.draw(frame);
                let level = frames.saturating_sub(FADE_DELAY).min(u32::from(FADE_STEPS));
                darken(frame, u8::try_from(level).unwrap_or(FADE_STEPS));
            }
            Phase::Scene(stage) => stage.draw(frame, windows, skin, painter),
            Phase::Ending(..) => frame.fill(platform::Rgb::default()),
        }
    }
}
