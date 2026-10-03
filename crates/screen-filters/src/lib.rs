//! What the frontends do to the game's picture before it reaches the
//! screen, the player's choice in the launcher's options: the colors of
//! the handheld's panels (see [`color`]), the trail its slow panel left
//! (see [`trail`]) and pixel-art magnification (see [`upscale`]). The
//! scaling to the window, and the LCD grid over it, stay the backend's.
//!
//! Source of knowledge: this project's own design (see
//! `docs/launcher.md`, Options).

pub mod color;
pub mod trail;
pub mod upscale;

pub use color::{ColorCorrection, ColorProfile};
pub use trail::{Trail, TrailMode};
pub use upscale::Upscaler;

use platform::Frame;

/// What is done to each picture: its colors, then its trail, and last,
/// once the launcher's marks are on it, its magnification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenFilters {
    colors: ColorCorrection,
    trail: Trail,
    upscaler: Upscaler,
}

impl ScreenFilters {
    /// The colors of `profile`, the trail of `trail`, and `upscaler`.
    #[must_use]
    pub fn new(profile: ColorProfile, trail: TrailMode, upscaler: Upscaler) -> Self {
        Self {
            colors: ColorCorrection::new(profile),
            trail: Trail::new(trail),
            upscaler,
        }
    }

    /// Shows `frame`, the game's next picture, through the filters.
    pub fn apply(&mut self, frame: &mut Frame) {
        self.colors.apply(frame);
        self.trail.apply(frame);
    }

    /// `frame` magnified by the upscaler, or `None` when it has none.
    #[must_use]
    pub fn magnify(&self, frame: &Frame) -> Option<Frame> {
        self.upscaler.scale(frame)
    }
}
