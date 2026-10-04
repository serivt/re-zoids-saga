//! The game in a web browser: the page's WebAssembly. The page hands it
//! the ROM the player chose (it never leaves the browser), the options and
//! a translation if one was chosen, and a canvas; the game then runs on the
//! canvas at the hardware's pace, its sound through Web Audio, its buttons
//! from the keyboard, gamepads and, on touch screens, an on-screen pad, its
//! saves (the slots, the autosave, the achievements) in the browser's
//! storage. The page's menu comes back when
//! the player leaves the game from the pause menu's 終了.
//!
//! As in the launcher, M mutes the sound, and in the enhanced mode holding
//! Space (or the gamepad's right stick) plays the fast forward. The page
//! keeps a [`Session`] of the game it started, to pause it, resume it,
//! leave it, and change the volume, the scaling and the pad while it plays.
//!
//! Source of knowledge: this project's own design (see `docs/web.md`).

mod options;
mod pacing;

use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;

use extraction::{IdentifyError, Title};
use game_core::extension::{Event as GameEvent, Extension};
use game_core::port_text::{
    LAUNCHER_ROM_OTHER, LAUNCHER_ROM_UNREADABLE, LAUNCHER_ROM_UNSUPPORTED, LAUNCHER_ROM_VERIFIED,
    default_text,
};
use game_core::save::{Found, SaveFile};
use game_core::slots::Slot;
use game_core::{Game, PlayMode, Translation};
use gba_runtime::apu::{SAMPLE_RATE, SAMPLES_PER_FRAME};
use gba_runtime::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use platform::{AudioOut, Button, Frame, Input, Rgb, SaveStorage};
use platform_web::{
    LocalStorage, PadMode, PadStyle, Scaling, StageElements, WebAudio, WebCanvas, WebInput,
    WebStage,
};
use screen_filters::{Grid, ScreenFilters};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::{Closure, JsValue, wasm_bindgen};
use web_sys::{CustomEvent, CustomEventInit, HtmlCanvasElement, HtmlElement, Window};

pub use options::Options;
pub use pacing::Pacer;

/// The save slots the game offers, as on the desktop.
const SLOTS: usize = game_core::slots::DEFAULT_SLOTS;
/// The frames of sound queued at most, as the launcher's.
const AUDIO_QUEUE_FRAMES: usize = 6;
/// The event the page hears when the player leaves the game.
const LEFT_EVENT: &str = "re-zoids-saga:left";
/// The event the page hears when the enhanced mode's settings change in
/// the game's pause menu; its detail holds them as settings lines.
const ENHANCEMENTS_EVENT: &str = "re-zoids-saga:enhancements";

/// What the ROM is, as the launcher tells it: whether the port plays it,
/// and the message for the player.
#[wasm_bindgen]
pub struct RomCheck {
    playable: bool,
    message: String,
}

#[wasm_bindgen]
impl RomCheck {
    /// Whether the port plays it.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn playable(&self) -> bool {
        self.playable
    }

    /// What it is, in the player's words.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn message(&self) -> String {
        self.message.clone()
    }
}

/// What the ROM `rom` is: one of the supported dumps, another dump of the
/// game, another game, or no ROM at all.
#[wasm_bindgen]
#[must_use]
pub fn check_rom(rom: &[u8]) -> RomCheck {
    let (playable, key) = match extraction::identify(rom) {
        Ok(found) if found.title != Title::Saga => (false, LAUNCHER_ROM_OTHER),
        Ok(found) if found.known_release.is_some() => (true, LAUNCHER_ROM_VERIFIED),
        Ok(_) => (false, LAUNCHER_ROM_UNSUPPORTED),
        Err(IdentifyError::UnsupportedGame { .. }) => (false, LAUNCHER_ROM_OTHER),
        Err(_) => (false, LAUNCHER_ROM_UNREADABLE),
    };
    RomCheck {
        playable,
        message: default_text(key).unwrap_or_default().to_owned(),
    }
}

/// What a save holds, for the page's cards and an import's question.
#[wasm_bindgen]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveDetails {
    level: u8,
    area: u8,
    money: u32,
    played: Option<String>,
}

#[wasm_bindgen]
impl SaveDetails {
    /// The leader's level.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn level(&self) -> u8 {
        self.level
    }

    /// The area saved in, 1 to 10.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn area(&self) -> u8 {
        self.area
    }

    /// The party's money.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn money(&self) -> u32 {
        self.money
    }

    /// The time played, `87:23`, when the port counted it.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn played(&self) -> Option<String> {
        self.played.clone()
    }
}

/// What the save `bytes` of the game whose ROM is `rom` holds; `None` when
/// it holds no game to continue.
#[wasm_bindgen]
#[must_use]
pub fn save_details(rom: &[u8], bytes: &[u8]) -> Option<SaveDetails> {
    let layout = extraction::saga_save::save_layout(rom).ok()?;
    let found = SaveFile::new(layout).read(Some(bytes.to_vec()));
    let Slot::Game(game) = Slot::from_found(&found) else {
        return None;
    };
    let played = match &found {
        Found::Saved(saved) | Found::Restored(saved) if saved.stats.play_frames > 0 => {
            let (hours, minutes, _) = saved.stats.play_time();
            Some(format!("{hours}:{minutes:02}"))
        }
        _ => None,
    };
    Some(SaveDetails {
        level: game.level,
        area: game.area,
        money: game.money,
        played,
    })
}

/// What the save `bytes` of the game whose ROM is `rom` holds, in a line:
/// `Lv 31, area 10, 4698050 G` and, when the port counted it, `, 87:23
/// played`; `None` when it holds no game to continue.
#[wasm_bindgen]
#[must_use]
pub fn save_summary(rom: &[u8], bytes: &[u8]) -> Option<String> {
    let details = save_details(rom, bytes)?;
    let mut summary = format!(
        "Lv {}, area {}, {} G",
        details.level, details.area, details.money
    );
    if let Some(played) = &details.played {
        let _ = write!(summary, ", {played} played");
    }
    Some(summary)
}

/// The save `bytes` of the game whose ROM is `rom` without the port's
/// notes, as the cartridge itself would hold it (see
/// `SaveFile::without_port_notes`); as they are when the ROM has no save
/// layout.
#[wasm_bindgen]
#[must_use]
pub fn cartridge_save(rom: &[u8], bytes: Vec<u8>) -> Vec<u8> {
    match extraction::saga_save::save_layout(rom) {
        Ok(layout) => SaveFile::new(layout).without_port_notes(bytes),
        Err(_) => bytes,
    }
}

/// Starts the game of `rom` on `canvas` with the options `settings` (see
/// [`Options`]) and the PO file `translation`, if any, the on-screen pad
/// drawn on `pad`, the LCD grid on `grid`, if any, and the page's `menu`
/// button, if any, kept at the game screen's top left corner. They all
/// sit in one element over the whole window, which takes the fingers. Call it from the player's click, so the
/// browser lets the sound play.
///
/// # Errors
///
/// Returns the reason when the ROM, the translation, the canvases or the
/// sound cannot be used.
#[wasm_bindgen]
pub fn start(
    rom: Vec<u8>,
    settings: &str,
    translation: Option<String>,
    canvas: HtmlCanvasElement,
    pad: HtmlCanvasElement,
    menu: Option<HtmlElement>,
    grid: Option<HtmlCanvasElement>,
) -> Result<Session, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let options = Options::parse(settings);
    let rom: &'static [u8] = Box::leak(rom.into_boxed_slice());
    let mut game = Game::new(rom).map_err(error)?;
    keep_saves(&mut game);
    if let Some(text) = translation {
        let translation = Translation::from_po(&text).map_err(error)?;
        game.set_translation(translation).map_err(error)?;
    }
    game.set_play_mode(options.mode);
    game.extensions()
        .borrow_mut()
        .insert(Box::new(ModeReporter(window.clone())));
    let audio = WebAudio::new(SAMPLE_RATE).map_err(error)?;
    audio.resume();
    let stage = canvas
        .parent_element()
        .and_then(|element| element.dyn_into::<HtmlElement>().ok())
        .ok_or_else(|| JsValue::from_str("the canvas has no stage"))?;
    let style = PadStyle {
        mode: options.touch,
        size: f32::from(options.touch_size) / f32::from(options::USUAL_TOUCH),
        opacity: f32::from(options.touch_opacity) / f32::from(options::USUAL_TOUCH),
        fast_forward: matches!(options.mode, PlayMode::Enhanced(_)),
    };
    let elements = StageElements {
        stage,
        screen: canvas.clone(),
        pad,
        menu,
        grid,
    };
    let mut stage = WebStage::new(
        window.clone(),
        elements,
        (SCREEN_WIDTH, SCREEN_HEIGHT),
        options.scaling,
        style,
    )
    .map_err(error)?;
    stage.set_grid(options.grid);
    let runner = Runner {
        stage,
        filters: ScreenFilters::new(options.color, options.trail, options.upscaler),
        canvas: WebCanvas::new(canvas).map_err(error)?,
        input: WebInput::listen(window.clone()).map_err(error)?,
        audio,
        volume: i32::from(options.volume),
        frame: Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::default()),
        pacer: Pacer::new(),
        muted: options.muted,
        paused: false,
        leaving: false,
        held: Input::default(),
        game,
    };
    let runner = Rc::new(RefCell::new(runner));
    run(&window, Rc::clone(&runner));
    Ok(Session { runner })
}

/// The game the page started, while it plays.
#[wasm_bindgen]
pub struct Session {
    runner: Rc<RefCell<Runner>>,
}

#[wasm_bindgen]
impl Session {
    /// Stops the game where it is, its sound with it.
    pub fn pause(&self) {
        self.runner.borrow_mut().paused = true;
    }

    /// Plays the game on from where it stopped.
    pub fn resume(&self) {
        let mut runner = self.runner.borrow_mut();
        runner.paused = false;
        runner.pacer = Pacer::new();
    }

    /// Leaves the game: the page hears `re-zoids-saga:left`, as when the
    /// player leaves from the pause menu. What was not saved is lost.
    pub fn quit(&self) {
        self.runner.borrow_mut().leaving = true;
    }

    /// Plays the sound at `volume` percent.
    pub fn set_volume(&self, volume: u8) {
        self.runner.borrow_mut().volume = i32::from(volume.min(options::FULL_VOLUME));
    }

    /// Fills the window as the settings' `scaling` line names it.
    pub fn set_scaling(&self, key: &str) {
        let scaling = Scaling::from_key(key).unwrap_or_default();
        self.runner.borrow_mut().stage.set_scaling(scaling);
    }

    /// Draws the grid the settings' `filter` line names, or none.
    pub fn set_filter(&self, key: &str) {
        self.runner.borrow_mut().stage.set_grid(Grid::from_key(key));
    }

    /// Shows the pad as the settings' `touch` line names it.
    pub fn set_touch(&self, key: &str) {
        let mode = PadMode::from_key(key).unwrap_or_default();
        self.runner.borrow_mut().stage.set_mode(mode);
    }

    /// Makes the pad `opacity` percent as opaque as usual.
    pub fn set_touch_opacity(&self, opacity: u8) {
        let opacity = f32::from(opacity) / f32::from(options::USUAL_TOUCH);
        self.runner.borrow_mut().stage.set_opacity(opacity);
    }
}

/// Tells the page when the enhanced mode's settings change in the game's
/// pause menu, so it remembers them for the next game.
struct ModeReporter(Window);

impl Extension for ModeReporter {
    fn name(&self) -> &'static str {
        "web-mode-reporter"
    }

    fn on_event(&mut self, event: &GameEvent) {
        let GameEvent::EnhancementsChanged(enhancements) = event else {
            return;
        };
        let init = CustomEventInit::new();
        init.set_detail(&JsValue::from_str(&options::enhancement_lines(
            *enhancements,
        )));
        if let Ok(event) = CustomEvent::new_with_event_init_dict(ENHANCEMENTS_EVENT, &init) {
            let _ = self.0.dispatch_event(&event);
        }
    }
}

/// Keeps the game's saves in the browser's storage: the slots, the
/// autosave and the achievements, each under a key of its own.
fn keep_saves(game: &mut Game<'static>) {
    let slots: Vec<Box<dyn SaveStorage>> = (0..SLOTS)
        .map(|slot| Box::new(LocalStorage::new(&format!("slot-{}", slot + 1))) as Box<_>)
        .collect();
    game.set_save_slots(slots);
    game.set_autosave_storage(Box::new(LocalStorage::new("autosave")));
    game.set_achievement_storage(Box::new(LocalStorage::new("achievements")));
}

/// The game and what plays it in the page.
struct Runner {
    game: Game<'static>,
    canvas: WebCanvas,
    stage: WebStage,
    input: WebInput,
    audio: WebAudio,
    filters: ScreenFilters,
    frame: Frame,
    pacer: Pacer,
    volume: i32,
    muted: bool,
    /// Stopped by the page's menu.
    paused: bool,
    /// Left from the page's menu.
    leaving: bool,
    /// The buttons of the frame before, for the mute's press.
    held: Input,
}

impl Runner {
    /// A picture of the screen at `now` (milliseconds): the frames due,
    /// their sound, then the picture; `false` once the player has left.
    fn tick(&mut self, now: f64) -> bool {
        if self.leaving {
            return false;
        }
        if self.paused {
            return true;
        }
        let due = self.pacer.due(now);
        if due == 0 {
            return true;
        }
        self.stage.fit(self.input.has_gamepad());
        let touch = self.stage.input();
        let input = self.input.input().union(touch);
        if input.is_held(Button::Mute) && !self.held.is_held(Button::Mute) {
            self.muted = !self.muted;
        }
        self.held = input;
        let speed = match (self.game.play_mode(), input.is_held(Button::FastForward)) {
            (PlayMode::Enhanced(enhancements), true) => u32::from(enhancements.fast_forward),
            _ => 1,
        };
        let input = input.without(Button::Mute).without(Button::FastForward);
        self.game.set_time_scale(speed);
        for _ in 0..due * speed {
            if self.game.update(input).is_err() {
                return false;
            }
            if self.audio.queued_pairs() < SAMPLES_PER_FRAME * AUDIO_QUEUE_FRAMES {
                let samples = scaled(self.game.audio(), if self.muted { 0 } else { self.volume });
                let _ = self.audio.queue(&samples);
            }
            if self.game.wants_to_leave() {
                return false;
            }
        }
        self.game.draw(&mut self.frame);
        self.filters.apply(&mut self.frame);
        let magnified = self.filters.magnify(&self.frame);
        let _ = self.canvas.draw(magnified.as_ref().unwrap_or(&self.frame));
        self.stage.draw(touch);
        true
    }
}

/// `samples` at `volume` percent.
fn scaled(samples: &[i16], volume: i32) -> Vec<i16> {
    let full = i32::from(options::FULL_VOLUME);
    samples
        .iter()
        .map(|&sample| i16::try_from(i32::from(sample) * volume / full).unwrap_or(sample))
        .collect()
}

/// What the browser calls for each picture the screen shows, with the time.
type FrameCallback = Closure<dyn FnMut(f64)>;

/// Runs `runner` once for each picture the screen shows, until the player
/// leaves; then tells the page.
fn run(window: &Window, runner: Rc<RefCell<Runner>>) {
    let frame: Rc<RefCell<Option<FrameCallback>>> = Rc::new(RefCell::new(None));
    let next = Rc::clone(&frame);
    let page = window.clone();
    *frame.borrow_mut() = Some(Closure::new(move |now: f64| {
        if runner.borrow_mut().tick(now) {
            if let Some(callback) = next.borrow().as_ref() {
                let _ = page.request_animation_frame(callback.as_ref().unchecked_ref());
            }
        } else {
            if let Ok(event) = CustomEvent::new(LEFT_EVENT) {
                let _ = page.dispatch_event(&event);
            }
            next.borrow_mut().take();
        }
    }));
    if let Some(callback) = frame.borrow().as_ref() {
        let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
    }
}

/// `value` as the page's error.
fn error(value: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rom_that_is_none_is_not_played() {
        let check = check_rom(&[0; 64]);
        assert!(!check.playable());
        assert_eq!(
            check.message(),
            default_text(LAUNCHER_ROM_UNREADABLE).unwrap_or_default()
        );
    }

    #[test]
    fn a_rom_without_a_save_layout_has_no_saves() {
        assert_eq!(save_summary(&[0; 64], &[0; 64]), None);
        assert_eq!(cartridge_save(&[0; 64], vec![7, 8]), vec![7, 8]);
    }

    #[test]
    fn the_volume_scales_the_samples() {
        assert_eq!(scaled(&[1000, -1000], 50), [500, -500]);
        assert_eq!(scaled(&[1000], 0), [0]);
    }
}
