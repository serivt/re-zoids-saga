//! The game in a web browser: the page's WebAssembly. The page hands it
//! the ROM the player chose (it never leaves the browser), the options and
//! a translation if one was chosen, and a canvas; the game then runs on the
//! canvas at the hardware's pace, its sound through Web Audio, its buttons
//! from the keyboard and gamepads, its saves (the slots, the autosave, the
//! achievements) in the browser's storage. The page's menu comes back when
//! the player leaves the game from the pause menu's 終了.
//!
//! As in the launcher, M mutes the sound, and in the enhanced mode holding
//! Space (or the gamepad's right stick) plays the fast forward.
//!
//! Source of knowledge: this project's own design (see `docs/web.md`).

mod options;
mod pacing;

use std::cell::RefCell;
use std::rc::Rc;

use extraction::{IdentifyError, Title};
use game_core::port_text::{
    LAUNCHER_ROM_FIRST_RELEASE, LAUNCHER_ROM_OTHER, LAUNCHER_ROM_UNREADABLE,
    LAUNCHER_ROM_UNSUPPORTED, LAUNCHER_ROM_VERIFIED, default_text,
};
use game_core::{Game, PlayMode, Translation};
use gba_runtime::apu::{SAMPLE_RATE, SAMPLES_PER_FRAME};
use gba_runtime::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use platform::{AudioOut, Button, Frame, Input, Rgb, SaveStorage};
use platform_web::{LocalStorage, WebAudio, WebCanvas, WebInput};
use screen_filters::ScreenFilters;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::{Closure, JsValue, wasm_bindgen};
use web_sys::{CustomEvent, HtmlCanvasElement, Window};

pub use options::Options;
pub use pacing::Pacer;

/// The save slots the game offers, as on the desktop.
const SLOTS: usize = game_core::slots::DEFAULT_SLOTS;
/// The frames of sound queued at most, as the launcher's.
const AUDIO_QUEUE_FRAMES: usize = 6;
/// The event the page hears when the player leaves the game.
const LEFT_EVENT: &str = "re-zoids-saga:left";

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

/// What the ROM `rom` is: the supported dump, the game's first release,
/// another revision, another game, or no ROM at all.
#[wasm_bindgen]
#[must_use]
pub fn check_rom(rom: &[u8]) -> RomCheck {
    let (playable, key) = match extraction::identify(rom) {
        Ok(found) if found.title != Title::Saga => (false, LAUNCHER_ROM_OTHER),
        Ok(found) if found.known_release.is_some() => (true, LAUNCHER_ROM_VERIFIED),
        Ok(found) if found.header.version == 0 => (false, LAUNCHER_ROM_FIRST_RELEASE),
        Ok(_) => (false, LAUNCHER_ROM_UNSUPPORTED),
        Err(IdentifyError::UnsupportedGame { .. }) => (false, LAUNCHER_ROM_OTHER),
        Err(_) => (false, LAUNCHER_ROM_UNREADABLE),
    };
    RomCheck {
        playable,
        message: default_text(key).unwrap_or_default().to_owned(),
    }
}

/// Starts the game of `rom` on `canvas` with the options `settings` (see
/// [`Options`]) and the PO file `translation`, if any. Call it from the
/// player's click, so the browser lets the sound play.
///
/// # Errors
///
/// Returns the reason when the ROM, the translation, the canvas or the
/// sound cannot be used.
#[wasm_bindgen]
pub fn start(
    rom: Vec<u8>,
    settings: &str,
    translation: Option<String>,
    canvas: HtmlCanvasElement,
) -> Result<(), JsValue> {
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
    let audio = WebAudio::new(SAMPLE_RATE).map_err(error)?;
    audio.resume();
    let runner = Runner {
        filters: ScreenFilters::new(options.color, options.trail, options.upscaler),
        canvas: WebCanvas::new(canvas).map_err(error)?,
        input: WebInput::listen(window.clone()).map_err(error)?,
        audio,
        volume: i32::from(options.volume),
        frame: Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::default()),
        pacer: Pacer::new(),
        muted: false,
        held: Input::default(),
        game,
    };
    run(&window, runner);
    Ok(())
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
    input: WebInput,
    audio: WebAudio,
    filters: ScreenFilters,
    frame: Frame,
    pacer: Pacer,
    volume: i32,
    muted: bool,
    /// The buttons of the frame before, for the mute's press.
    held: Input,
}

impl Runner {
    /// A picture of the screen at `now` (milliseconds): the frames due,
    /// their sound, then the picture; `false` once the player has left.
    fn tick(&mut self, now: f64) -> bool {
        let due = self.pacer.due(now);
        if due == 0 {
            return true;
        }
        let input = self.input.input();
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
fn run(window: &Window, runner: Runner) {
    let runner = Rc::new(RefCell::new(runner));
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
    fn the_volume_scales_the_samples() {
        assert_eq!(scaled(&[1000, -1000], 50), [500, -500]);
        assert_eq!(scaled(&[1000], 0), [0]);
    }
}
