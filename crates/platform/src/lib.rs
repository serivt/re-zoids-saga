//! Platform abstraction: traits and shared types for window, input, audio output, filesystem and timing. Contains no backend code.

pub mod video;

pub use video::{Display, Event, Frame, PlatformError, Rgb};
