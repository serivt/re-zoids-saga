//! What the frontends do to the game's picture before it reaches the
//! screen, the player's choice in the launcher's options: for now the
//! colors of the handheld's panels (see [`color`]). The scaling to the
//! window stays the backend's.
//!
//! Source of knowledge: this project's own design (see
//! `docs/launcher.md`, Options).

pub mod color;

pub use color::{ColorCorrection, ColorProfile};
