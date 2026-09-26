//! SDL3 implementation of the platform traits, with the save kept as a
//! file.

mod storage;

pub use storage::FileStorage;

use platform::{AudioOut, Button, Display, Event, Frame, Input, PlatformError};
use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};
use sdl3::event::Event as SdlEvent;
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::pixels::PixelFormat;
use sdl3::render::{ScaleMode, TextureCreator, WindowCanvas};
use sdl3::video::WindowContext;
use sdl3::{EventPump, Sdl};

const BACKEND: &str = "sdl3";
const AUDIO_CHANNELS: i32 = 2;
const BYTES_PER_PAIR: usize = 4;
const BYTES_PER_PIXEL: usize = 3;
const KEY_MAP: [(Scancode, Button); 10] = [
    (Scancode::Up, Button::Up),
    (Scancode::Down, Button::Down),
    (Scancode::Left, Button::Left),
    (Scancode::Right, Button::Right),
    (Scancode::X, Button::A),
    (Scancode::Z, Button::B),
    (Scancode::Return, Button::Start),
    (Scancode::Backspace, Button::Select),
    (Scancode::A, Button::L),
    (Scancode::S, Button::R),
];

/// A window backed by SDL3 that shows frames of a fixed size, scaled up
/// with nearest-neighbor sampling.
pub struct Sdl3Display {
    sdl: Sdl,
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
            sdl,
            canvas,
            texture_creator,
            event_pump,
            frame_width,
            frame_height,
        })
    }
}

impl Sdl3Display {
    /// Opens the default playback device for interleaved stereo 16-bit
    /// samples at `rate` hertz; the backend converts to what the device
    /// wants.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when SDL cannot open the device.
    pub fn open_audio(&self, rate: u32) -> Result<Sdl3Audio, PlatformError> {
        let audio = self.sdl.audio().map_err(backend_error)?;
        let rate = i32::try_from(rate).map_err(backend_error)?;
        let spec = AudioSpec::new(Some(rate), Some(AUDIO_CHANNELS), Some(AudioFormat::S16LE));
        let device = audio.open_playback_device(&spec).map_err(backend_error)?;
        let stream = device
            .open_device_stream(Some(&spec))
            .map_err(backend_error)?;
        stream.resume().map_err(backend_error)?;
        Ok(Sdl3Audio { stream })
    }
}

/// A playback stream on the default device.
pub struct Sdl3Audio {
    stream: AudioStreamOwner,
}

impl AudioOut for Sdl3Audio {
    fn queue(&mut self, samples: &[i16]) -> Result<(), PlatformError> {
        self.stream.put_data_i16(samples).map_err(backend_error)
    }

    fn queued_pairs(&self) -> usize {
        self.stream
            .queued_bytes()
            .ok()
            .and_then(|bytes| usize::try_from(bytes).ok())
            .map_or(0, |bytes| bytes / BYTES_PER_PAIR)
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

    fn input(&self) -> Input {
        let keys = self.event_pump.keyboard_state();
        KEY_MAP
            .iter()
            .filter(|(scancode, _)| keys.is_scancode_pressed(*scancode))
            .fold(Input::default(), |input, (_, button)| input.with(*button))
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
                SdlEvent::KeyDown {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => function_key(keycode).map(Event::FunctionKey),
                _ => None,
            })
            .collect()
    }
}

/// The number of a function key, F1 to F12.
fn function_key(keycode: Keycode) -> Option<u8> {
    const KEYS: [Keycode; 12] = [
        Keycode::F1,
        Keycode::F2,
        Keycode::F3,
        Keycode::F4,
        Keycode::F5,
        Keycode::F6,
        Keycode::F7,
        Keycode::F8,
        Keycode::F9,
        Keycode::F10,
        Keycode::F11,
        Keycode::F12,
    ];
    let index = KEYS.iter().position(|&key| key == keycode)?;
    u8::try_from(index + 1).ok()
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
