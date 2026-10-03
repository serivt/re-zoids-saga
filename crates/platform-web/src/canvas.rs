//! The picture on a canvas: each frame's pixels written into the canvas's
//! own, the canvas as large as the frame; the page's style scales it to
//! the window (sharp, `image-rendering: pixelated`, or smooth), so the
//! filters stay the page's to choose.

use platform::{Display, Event, Frame, Input, PlatformError};
use wasm_bindgen::{Clamped, JsCast};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

use crate::keys::WebInput;
use crate::web_error;

/// The bytes a canvas keeps for a pixel: red, green, blue, opacity.
const RGBA: usize = 4;
const OPAQUE: u8 = u8::MAX;

/// A canvas the frames are drawn on.
pub struct WebCanvas {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    rgba: Vec<u8>,
}

impl WebCanvas {
    /// Draws on `canvas`.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the canvas has no 2D context.
    pub fn new(canvas: HtmlCanvasElement) -> Result<Self, PlatformError> {
        let context = canvas
            .get_context("2d")
            .map_err(web_error)?
            .ok_or_else(|| web_error("no 2D context"))?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(web_error)?;
        context.set_image_smoothing_enabled(false);
        Ok(Self {
            canvas,
            context,
            rgba: Vec::new(),
        })
    }

    /// Draws `frame`, the canvas taking its size.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the canvas refuses the pixels.
    pub fn draw(&mut self, frame: &Frame) -> Result<(), PlatformError> {
        let (width, height) = (frame.width(), frame.height());
        let (Ok(across), Ok(down)) = (u32::try_from(width), u32::try_from(height)) else {
            return Err(web_error("frame too large"));
        };
        if self.canvas.width() != across || self.canvas.height() != down {
            self.canvas.set_width(across);
            self.canvas.set_height(down);
        }
        to_rgba(frame, &mut self.rgba);
        let image = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&self.rgba), across, down)
            .map_err(web_error)?;
        self.context
            .put_image_data(&image, 0.0, 0.0)
            .map_err(web_error)
    }
}

/// `frame`'s pixels as the canvas keeps them, into `rgba`.
fn to_rgba(frame: &Frame, rgba: &mut Vec<u8>) {
    rgba.clear();
    rgba.reserve(frame.pixels().len() * RGBA);
    for pixel in frame.pixels() {
        rgba.extend_from_slice(&[pixel.r, pixel.g, pixel.b, OPAQUE]);
    }
}

/// The canvas and the keys and gamepads, as the platform's display.
pub struct WebDisplay {
    /// The picture.
    pub canvas: WebCanvas,
    /// The buttons.
    pub input: WebInput,
}

impl Display for WebDisplay {
    fn present(&mut self, frame: &Frame) -> Result<(), PlatformError> {
        self.canvas.draw(frame)
    }

    fn poll_events(&mut self) -> Vec<Event> {
        Vec::new()
    }

    fn input(&self) -> Input {
        self.input.input()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform::Rgb;

    #[test]
    fn a_frame_becomes_opaque_rgba() {
        let mut frame = Frame::new(2, 1, Rgb::new(1, 2, 3));
        frame.set_pixel(1, 0, Rgb::new(9, 8, 7));
        let mut rgba = vec![5; 3];
        to_rgba(&frame, &mut rgba);
        assert_eq!(rgba, [1, 2, 3, 255, 9, 8, 7, 255]);
    }
}
