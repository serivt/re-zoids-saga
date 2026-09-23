//! Video output: a CPU-side frame and the display that shows it.

use thiserror::Error;

/// An opaque 8-bit-per-channel color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Rgb {
    /// Builds a color from its channels.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// A frame of pixels rendered on the CPU, row-major, top-left origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    width: usize,
    height: usize,
    pixels: Vec<Rgb>,
}

impl Frame {
    /// Creates a frame filled with `fill`.
    #[must_use]
    pub fn new(width: usize, height: usize, fill: Rgb) -> Self {
        Self {
            width,
            height,
            pixels: vec![fill; width * height],
        }
    }

    /// Width in pixels.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Color at `(x, y)`; `None` outside the frame.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> Option<Rgb> {
        (x < self.width && y < self.height).then(|| self.pixels[y * self.width + x])
    }

    /// Sets the color at `(x, y)`; writes outside the frame are ignored.
    pub fn set_pixel(&mut self, x: usize, y: usize, color: Rgb) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = color;
        }
    }

    /// Fills the whole frame.
    pub fn fill(&mut self, color: Rgb) {
        self.pixels.fill(color);
    }

    /// The pixels as packed 24-bit RGB bytes, row-major.
    #[must_use]
    pub fn to_rgb24(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|pixel| [pixel.r, pixel.g, pixel.b])
            .collect()
    }
}

/// A button on the pad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// D-pad up.
    Up,
    /// D-pad down.
    Down,
    /// D-pad left.
    Left,
    /// D-pad right.
    Right,
    /// Confirm.
    A,
    /// Cancel.
    B,
    /// Start.
    Start,
}

impl Button {
    const fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// The set of buttons held at this moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Input {
    held: u8,
}

impl Input {
    /// Whether `button` is held.
    #[must_use]
    pub const fn is_held(self, button: Button) -> bool {
        self.held & button.bit() != 0
    }

    /// This input with `button` held as well.
    #[must_use]
    pub const fn with(self, button: Button) -> Self {
        Self {
            held: self.held | button.bit(),
        }
    }
}

/// Something the person did to the display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The window was closed or the person asked to quit.
    Quit,
}

/// A failure of the platform backend.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlatformError {
    /// The backend reported an error.
    #[error("{backend}: {message}")]
    Backend {
        /// Backend name, e.g. `sdl3`.
        backend: &'static str,
        /// Backend error text.
        message: String,
    },
}

/// A window that shows frames and reports the person's actions.
pub trait Display {
    /// Shows `frame`, scaled to the window.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the backend cannot present.
    fn present(&mut self, frame: &Frame) -> Result<(), PlatformError>;

    /// Events that happened since the last call, in order.
    fn poll_events(&mut self) -> Vec<Event>;

    /// Buttons held right now.
    fn input(&self) -> Input;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_bounds_are_enforced() {
        let mut frame = Frame::new(2, 2, Rgb::new(1, 2, 3));
        frame.set_pixel(1, 1, Rgb::new(9, 9, 9));
        frame.set_pixel(2, 0, Rgb::new(7, 7, 7));
        assert_eq!(frame.pixel(1, 1), Some(Rgb::new(9, 9, 9)));
        assert_eq!(frame.pixel(2, 0), None);
        assert_eq!(frame.pixel(0, 0), Some(Rgb::new(1, 2, 3)));
    }

    #[test]
    fn input_tracks_held_buttons() {
        let input = Input::default().with(Button::Left).with(Button::A);
        assert!(input.is_held(Button::Left));
        assert!(input.is_held(Button::A));
        assert!(!input.is_held(Button::Right));
        assert_eq!(Input::default().held, 0);
    }

    #[test]
    fn packs_rgb24_row_major() {
        let mut frame = Frame::new(2, 1, Rgb::new(0, 0, 0));
        frame.set_pixel(1, 0, Rgb::new(10, 20, 30));
        assert_eq!(frame.to_rgb24(), vec![0, 0, 0, 10, 20, 30]);
    }
}
