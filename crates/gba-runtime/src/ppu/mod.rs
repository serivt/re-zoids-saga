//! Tile, background and sprite rendering to the framebuffer.

pub mod background;
pub mod indexed;
pub mod palette;

pub use background::{PaletteBank, TileMapEntry, draw_background};
pub use indexed::{IndexedImage, draw_indexed};
pub use palette::Palette;

/// Width of the GBA screen in pixels.
pub const SCREEN_WIDTH: usize = 240;
/// Height of the GBA screen in pixels.
pub const SCREEN_HEIGHT: usize = 160;
