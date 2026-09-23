//! Platform abstraction: traits and shared types for window, input, audio output, filesystem and timing. Contains no backend code.

pub mod audio;
pub mod storage;
pub mod video;

pub use audio::AudioOut;
pub use storage::{SaveStorage, StorageError};
pub use video::{Button, Display, Event, Frame, Input, PlatformError, Rgb};
