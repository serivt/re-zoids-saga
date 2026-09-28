//! SDL3 implementation of the platform traits, with the save kept as a
//! file and the system's dialogs.

mod dialog;
mod pad;
mod storage;

pub use dialog::{FileChoice, open_url, preferences_dir};
pub use pad::{default_pad_buttons, pad_button_label, pad_button_name};
pub use storage::{FileStorage, slot_path};

use platform::{AudioOut, Button, Display, Event, Frame, Input, PlatformError};
use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};
use sdl3::event::Event as SdlEvent;
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::pixels::PixelFormat;
use sdl3::render::{ScaleMode, TextureCreator, WindowCanvas};
use sdl3::sys::render::{
    SDL_LOGICAL_PRESENTATION_INTEGER_SCALE, SDL_LOGICAL_PRESENTATION_LETTERBOX,
};
use sdl3::video::{WindowContext, WindowPos};
use sdl3::{EventPump, Sdl};

const BACKEND: &str = "sdl3";
const AUDIO_CHANNELS: i32 = 2;
const BYTES_PER_PAIR: usize = 4;
const BYTES_PER_PIXEL: usize = 3;
/// The keys the buttons have unless the player chose others.
const DEFAULT_KEYS: [(Scancode, Button); 10] = [
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

/// How a frame is scaled up to the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    /// Whole multiples of the frame, each pixel a sharp square.
    #[default]
    Sharp,
    /// As large as the window allows, blended between pixels.
    Smooth,
}

/// A window backed by SDL3 that shows frames of a fixed size, scaled up
/// with nearest-neighbor sampling unless a smooth filter is chosen, and
/// reads the keyboard and every gamepad connected.
pub struct Sdl3Display {
    sdl: Sdl,
    keys: Vec<(Scancode, Button)>,
    pads: Option<pad::Pads>,
    filter: Filter,
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
        let mut canvas = window.into_canvas();
        canvas
            .set_logical_size(
                dimension(frame_width)?,
                dimension(frame_height)?,
                SDL_LOGICAL_PRESENTATION_INTEGER_SCALE,
            )
            .map_err(backend_error)?;
        let texture_creator = canvas.texture_creator();
        let event_pump = sdl.event_pump().map_err(backend_error)?;
        let pads = pad::Pads::open(&sdl);
        Ok(Self {
            sdl,
            keys: DEFAULT_KEYS.to_vec(),
            pads,
            filter: Filter::Sharp,
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
        texture.set_scale_mode(match self.filter {
            Filter::Sharp => ScaleMode::Nearest,
            Filter::Smooth => ScaleMode::Linear,
        });
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
        let pads = self.pads.as_ref().map(pad::Pads::input).unwrap_or_default();
        self.keys
            .iter()
            .filter(|(scancode, _)| keys.is_scancode_pressed(*scancode))
            .fold(pads, |input, (_, button)| input.with(*button))
    }

    fn poll_events(&mut self) -> Vec<Event> {
        let events: Vec<SdlEvent> = self.event_pump.poll_iter().collect();
        events
            .into_iter()
            .filter_map(|event| match event {
                SdlEvent::GamepadAdded { which, .. } => {
                    if let Some(pads) = self.pads.as_mut() {
                        pads.add(which);
                    }
                    None
                }
                SdlEvent::GamepadRemoved { which, .. } => {
                    if let Some(pads) = self.pads.as_mut() {
                        pads.remove(which);
                    }
                    None
                }
                SdlEvent::GamepadButtonDown { button, .. } => {
                    Some(Event::Pad(pad::pad_code(button)))
                }
                SdlEvent::Quit { .. } => Some(Event::Quit),
                SdlEvent::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => Some(Event::Back),
                SdlEvent::KeyDown {
                    keycode: Some(keycode),
                    scancode,
                    repeat: false,
                    ..
                } => function_key(keycode)
                    .map(Event::FunctionKey)
                    .or_else(|| scancode.map(|scancode| Event::Key(scancode as u32))),
                _ => None,
            })
            .collect()
    }
}

/// The keys of the buttons, by the names [`key_name`] gives.
pub type KeyMap = Vec<(Button, String)>;

impl Sdl3Display {
    /// Gives the buttons the gamepad buttons of `map`, by the names
    /// [`pad_button_name`] gives; a button it leaves out keeps its default.
    pub fn set_pad_buttons(&mut self, map: &[(Button, String)]) {
        if let Some(pads) = self.pads.as_mut() {
            pads.set_buttons(map);
        }
    }

    /// The names of the gamepads connected.
    #[must_use]
    pub fn gamepads(&self) -> Vec<String> {
        self.pads.as_ref().map(pad::Pads::names).unwrap_or_default()
    }

    /// Shows the frame `scale` times its size in a window, or on the whole
    /// screen when `fullscreen`, scaled with `filter`: sharp keeps whole
    /// multiples, smooth fills the window keeping the frame's shape.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the window cannot change.
    pub fn set_video(
        &mut self,
        scale: u32,
        fullscreen: bool,
        filter: Filter,
    ) -> Result<(), PlatformError> {
        self.filter = filter;
        let (width, height) = (dimension(self.frame_width)?, dimension(self.frame_height)?);
        let presentation = match filter {
            Filter::Sharp => SDL_LOGICAL_PRESENTATION_INTEGER_SCALE,
            Filter::Smooth => SDL_LOGICAL_PRESENTATION_LETTERBOX,
        };
        self.canvas
            .set_logical_size(width, height, presentation)
            .map_err(backend_error)?;
        let window = self.canvas.window_mut();
        window.set_fullscreen(fullscreen).map_err(backend_error)?;
        if !fullscreen {
            let scale = scale.max(1);
            window
                .set_size(width * scale, height * scale)
                .map_err(backend_error)?;
            window.set_position(WindowPos::Centered, WindowPos::Centered);
        }
        Ok(())
    }

    /// Gives the buttons the keys of `map`; a button it leaves out or
    /// whose key has no known name keeps its default one.
    pub fn set_keys(&mut self, map: &[(Button, String)]) {
        self.keys = DEFAULT_KEYS
            .iter()
            .map(|&(default, button)| {
                let chosen = map
                    .iter()
                    .find(|(mapped, _)| *mapped == button)
                    .and_then(|(_, name)| Scancode::from_name(name));
                (chosen.unwrap_or(default), button)
            })
            .collect();
    }
}

/// The buttons' default keys, by name.
#[must_use]
pub fn default_keys() -> KeyMap {
    DEFAULT_KEYS
        .iter()
        .map(|&(scancode, button)| (button, scancode.name().to_owned()))
        .collect()
}

/// The name of the key [`Event::Key`] reports as `code`, as settings keep
/// it; `None` for a code with no key.
#[must_use]
pub fn key_name(code: u32) -> Option<String> {
    let code = i32::try_from(code).ok()?;
    let scancode = Scancode::from_i32(code)?;
    let name = scancode.name();
    (!name.is_empty()).then(|| name.to_owned())
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
