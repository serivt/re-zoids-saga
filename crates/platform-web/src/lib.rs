//! The platform's traits in a web browser, for the game compiled to
//! WebAssembly: the picture on a canvas placed in the window, the sound
//! through Web Audio, the keyboard, gamepads and an on-screen pad for touch
//! screens, and the saves in the browser's storage. The
//! parts that only decide (which key is which button, how a save becomes
//! text) are plain functions, tested on any machine; the rest calls the
//! browser and runs only there.
//!
//! Source of knowledge: this project's own design over the public Web
//! APIs (canvas 2D, Web Audio, Gamepad, Pointer Events, Web Storage); see
//! `docs/web.md`.

mod audio;
mod canvas;
mod keys;
mod stage;
mod storage;

pub use audio::WebAudio;
pub use canvas::{WebCanvas, WebDisplay};
pub use keys::{WebInput, button_for_code, button_for_pad};
pub use stage::{PadMode, PadStyle, Scaling, StageElements, WebStage, fit_screen};
pub use storage::{LocalStorage, decode, encode};

use platform::PlatformError;

const BACKEND: &str = "web";

/// `message` as the backend's error.
fn web_error(message: impl std::fmt::Debug) -> PlatformError {
    PlatformError::Backend {
        backend: BACKEND,
        message: format!("{message:?}"),
    }
}
