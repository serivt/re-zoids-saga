//! SDL3 implementation of the platform traits.

use platform::{Display, Event, Frame, PlatformError};
use sdl3::event::Event as SdlEvent;
use sdl3::keyboard::Keycode;
use sdl3::pixels::PixelFormat;
use sdl3::render::{ScaleMode, TextureCreator, WindowCanvas};
use sdl3::video::WindowContext;
use sdl3::{EventPump, Sdl};

const BACKEND: &str = "sdl3";
const BYTES_PER_PIXEL: usize = 3;

/// A window backed by SDL3 that shows frames of a fixed size, scaled up
/// with nearest-neighbor sampling.
pub struct Sdl3Display {
    _sdl: Sdl,
    canvas: WindowCanvas,
    texture_creator: TextureCreator<WindowContext>,
    event_pump: EventPump,
    frame_width: usize,
    frame_height: usize,
}

impl Sdl3Display {
    /// Opens a window titled `title` for frames of `frame_width`×`frame_height`
    /// pixels, shown at `scale` times their size.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when SDL cannot initialize or create the window.
    pub fn open(
        title: &str,
        frame_width: usize,
        frame_height: usize,
        scale: u32,
    ) -> Result<Self, PlatformError> {
        let sdl = sdl3::init().map_err(backend_error)?;
        let video = sdl.video().map_err(backend_error)?;
        let window = video
            .window(
                title,
                dimension(frame_width)? * scale,
                dimension(frame_height)? * scale,
            )
            .position_centered()
            .build()
            .map_err(backend_error)?;
        let canvas = window.into_canvas();
        let texture_creator = canvas.texture_creator();
        let event_pump = sdl.event_pump().map_err(backend_error)?;
        Ok(Self {
            _sdl: sdl,
            canvas,
            texture_creator,
            event_pump,
            frame_width,
            frame_height,
        })
    }
}

impl Display for Sdl3Display {
    fn present(&mut self, frame: &Frame) -> Result<(), PlatformError> {
        if frame.width() != self.frame_width || frame.height() != self.frame_height {
            return Err(backend_error(format!(
                "frame is {}x{}, display expects {}x{}",
                frame.width(),
                frame.height(),
                self.frame_width,
                self.frame_height
            )));
        }
        let mut texture = self
            .texture_creator
            .create_texture_streaming(
                PixelFormat::RGB24,
                dimension(self.frame_width)?,
                dimension(self.frame_height)?,
            )
            .map_err(backend_error)?;
        texture.set_scale_mode(ScaleMode::Nearest);
        texture
            .update(None, &frame.to_rgb24(), frame.width() * BYTES_PER_PIXEL)
            .map_err(backend_error)?;
        self.canvas.clear();
        self.canvas
            .copy(&texture, None, None)
            .map_err(backend_error)?;
        self.canvas.present();
        Ok(())
    }

    fn poll_events(&mut self) -> Vec<Event> {
        self.event_pump
            .poll_iter()
            .filter_map(|event| match event {
                SdlEvent::Quit { .. }
                | SdlEvent::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => Some(Event::Quit),
                _ => None,
            })
            .collect()
    }
}

fn dimension(pixels: usize) -> Result<u32, PlatformError> {
    u32::try_from(pixels).map_err(|_| backend_error(format!("dimension {pixels} is too large")))
}

fn backend_error(message: impl std::fmt::Display) -> PlatformError {
    PlatformError::Backend {
        backend: BACKEND,
        message: message.to_string(),
    }
}
