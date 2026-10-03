//! What the frontends do to the game's picture before it reaches the
//! screen, the player's choice in the launcher's options: the colors of
//! the handheld's panels (see [`color`]) and the trail its slow panel
//! left (see [`trail`]). The scaling to the window, and the LCD grid over
//! it, stay the backend's.
//!
//! Source of knowledge: this project's own design (see
//! `docs/launcher.md`, Options).

pub mod color;
pub mod trail;

pub use color::{ColorCorrection, ColorProfile};
pub use trail::Trail;

use platform::Frame;

/// What is done to each picture: its colors, then its trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenFilters {
    colors: ColorCorrection,
    trail: Option<Trail>,
}

impl ScreenFilters {
    /// The colors of `profile`, and the trail when `trail`.
    #[must_use]
    pub fn new(profile: ColorProfile, trail: bool) -> Self {
        Self {
            colors: ColorCorrection::new(profile),
            trail: trail.then(Trail::default),
        }
    }

    /// Shows `frame`, the game's next picture, through the filters.
    pub fn apply(&mut self, frame: &mut Frame) {
        self.colors.apply(frame);
        if let Some(trail) = &mut self.trail {
            trail.apply(frame);
        }
    }
}
