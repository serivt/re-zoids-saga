//! SDL3 implementation of the platform traits, with the save kept as a
//! file and the system's dialogs.

mod dialog;
mod pad;
mod storage;
mod touch;

pub use dialog::{FileChoice, open_url, preferences_dir, read_file, write_file};
pub use pad::{default_pad_buttons, pad_button_label, pad_button_name};
pub use storage::{FileStorage, autosave_path, slot_path};

use platform::{AudioOut, Button, Display, Event, Frame, Input, PlatformError};
use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};
use sdl3::event::Event as SdlEvent;
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::mouse::MouseButton;
use sdl3::pixels::PixelFormat;
use sdl3::render::{BlendMode, FRect, ScaleMode, TextureCreator, WindowCanvas};
use sdl3::surface::Surface;
use sdl3::sys::render::{
    SDL_LOGICAL_PRESENTATION_DISABLED, SDL_LOGICAL_PRESENTATION_INTEGER_SCALE,
    SDL_LOGICAL_PRESENTATION_LETTERBOX,
};
use sdl3::video::{WindowContext, WindowPos};
use sdl3::{EventPump, Sdl};

const BACKEND: &str = "sdl3";
/// The hint that makes a renderer wait for the screen's vertical blank
/// before it shows a picture, so none shows half drawn (tearing).
const VSYNC_HINT: &str = "SDL_RENDER_VSYNC";
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
/// reads the keyboard, every gamepad connected and, with the on-screen pad
/// on, the fingers on a touch screen.
pub struct Sdl3Display {
    sdl: Sdl,
    keys: Vec<(Scancode, Button)>,
    pads: Option<pad::Pads>,
    touch: Option<touch::TouchPad>,
    /// The on-screen pad's size and opacity, against their usual ones.
    touch_style: (f32, f32),
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
        sdl3::hint::set(VSYNC_HINT, "1");
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
            touch: None,
            touch_style: (1.0, 1.0),
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
    /// Gives the window the icon `rgba`: `side` by `side` pixels, 8-bit
    /// RGBA, rows top to bottom.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the pixels do not make a square of
    /// `side`, or SDL cannot take them.
    pub fn set_icon(&mut self, rgba: &[u8], side: u32) -> Result<(), PlatformError> {
        let pitch = side * 4;
        let length = usize::try_from(pitch * side).map_err(backend_error)?;
        if rgba.len() != length {
            return Err(backend_error(format!(
                "an icon of {} bytes is not {side} by {side} RGBA pixels",
                rgba.len()
            )));
        }
        let mut pixels = rgba.to_vec();
        let surface = Surface::from_data(&mut pixels, side, side, pitch, PixelFormat::RGBA32)
            .map_err(backend_error)?;
        if self.canvas.window_mut().set_icon(surface) {
            Ok(())
        } else {
            Err(backend_error("the window refused its icon"))
        }
    }

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

impl Sdl3Display {
    /// Draws `frame`, and the on-screen pad when it is on, without showing
    /// them yet.
    fn draw(&mut self, frame: &Frame) -> Result<(), PlatformError> {
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
        self.canvas.set_draw_color(sdl3::pixels::Color::BLACK);
        self.canvas.clear();
        let held = self.input();
        let output = self.canvas.output_size().map_err(backend_error)?;
        let pad_hidden = self.has_gamepad();
        let frame_size = (self.frame_width, self.frame_height);
        let filter = self.filter;
        if let Some(touch) = self.touch.as_mut() {
            if pad_hidden {
                self.canvas
                    .copy(&texture, None, fitted(output, frame_size, filter))
                    .map_err(backend_error)?;
            } else {
                touch.fit(output);
                self.canvas
                    .copy(&texture, None, touch.screen())
                    .map_err(backend_error)?;
                touch.draw(&mut self.canvas, held).map_err(backend_error)?;
            }
        } else {
            self.canvas
                .copy(&texture, None, None)
                .map_err(backend_error)?;
        }
        Ok(())
    }

    /// Shows `frame` like [`Display::present`] and returns what the window
    /// shows, in its own pixels: for screenshots and the tests that
    /// compare them.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the backend cannot draw or read back.
    pub fn present_captured(&mut self, frame: &Frame) -> Result<Frame, PlatformError> {
        self.draw(frame)?;
        let surface = self
            .canvas
            .read_pixels(None)
            .and_then(|surface| surface.convert_format(PixelFormat::RGB24))
            .map_err(backend_error)?;
        let (width, height) = (surface.width() as usize, surface.height() as usize);
        let pitch = surface.pitch() as usize;
        let mut captured = Frame::new(width, height, platform::Rgb::default());
        surface.with_lock(|pixels| {
            for y in 0..height {
                for x in 0..width {
                    let at = y * pitch + x * BYTES_PER_PIXEL;
                    captured.set_pixel(
                        x,
                        y,
                        platform::Rgb::new(pixels[at], pixels[at + 1], pixels[at + 2]),
                    );
                }
            }
        });
        self.canvas.present();
        Ok(captured)
    }

    /// Resizes the window to `width` by `height` pixels.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the window cannot change.
    pub fn set_window_size(&mut self, width: u32, height: u32) -> Result<(), PlatformError> {
        self.canvas
            .window_mut()
            .set_size(width, height)
            .map_err(backend_error)
    }
}

impl Display for Sdl3Display {
    fn present(&mut self, frame: &Frame) -> Result<(), PlatformError> {
        self.draw(frame)?;
        self.canvas.present();
        Ok(())
    }

    fn input(&self) -> Input {
        let keys = self.event_pump.keyboard_state();
        let pads = self.pads.as_ref().map(pad::Pads::input).unwrap_or_default();
        let touch = self
            .touch
            .as_ref()
            .filter(|_| !self.has_gamepad())
            .map(touch::TouchPad::input)
            .unwrap_or_default();
        self.keys
            .iter()
            .filter(|(scancode, _)| keys.is_scancode_pressed(*scancode))
            .fold(pads.union(touch), |input, (_, button)| input.with(*button))
    }

    fn poll_events(&mut self) -> Vec<Event> {
        if let Some(touch) = self.touch.as_mut() {
            touch.start_events();
        }
        let events: Vec<SdlEvent> = self.event_pump.poll_iter().collect();
        events
            .into_iter()
            .filter_map(|event| match event {
                SdlEvent::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    ..
                } if self.touch.is_none() => match event.get_converted_coords(&self.canvas) {
                    Some(SdlEvent::MouseButtonDown { x, y, .. }) => Some(Event::Pointer {
                        x: pixel(x),
                        y: pixel(y),
                    }),
                    _ => None,
                },
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
                SdlEvent::FingerDown {
                    finger_id, x, y, ..
                }
                | SdlEvent::FingerMotion {
                    finger_id, x, y, ..
                } => {
                    if let Some(touch) = self.touch.as_mut() {
                        touch.finger(finger_id, Some((x, y)));
                    }
                    None
                }
                SdlEvent::FingerUp { finger_id, .. }
                | SdlEvent::FingerCanceled { finger_id, .. } => {
                    if let Some(touch) = self.touch.as_mut() {
                        touch.finger(finger_id, None);
                    }
                    None
                }
                SdlEvent::AppWillEnterBackground { .. } => {
                    if let Some(touch) = self.touch.as_mut() {
                        touch.release();
                    }
                    None
                }
                SdlEvent::Quit { .. } => Some(Event::Quit),
                SdlEvent::KeyDown {
                    keycode: Some(Keycode::Escape | Keycode::AcBack),
                    repeat: false,
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
        self.present_frames()?;
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

    /// Shows the on-screen pad for touch screens, or hides it. With it the
    /// frame fills the window's height in landscape, or its width at the
    /// top in portrait, and the controls sit around it.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the renderer cannot change.
    pub fn set_touch_pad(&mut self, on: bool) -> Result<(), PlatformError> {
        self.touch = on.then(|| {
            let mut pad = touch::TouchPad::new((self.frame_width, self.frame_height));
            pad.set_style(self.touch_style.0, self.touch_style.1);
            pad
        });
        self.canvas.set_blend_mode(if on {
            BlendMode::Blend
        } else {
            BlendMode::None
        });
        self.present_frames()
    }

    /// Whether the on-screen pad is shown.
    #[must_use]
    pub fn has_touch_pad(&self) -> bool {
        self.touch.is_some()
    }

    /// Makes the on-screen pad's controls `size` percent of their usual
    /// size (from 60 to 140) and `opacity` percent as opaque as usual.
    pub fn set_touch_style(&mut self, size: u8, opacity: u8) {
        self.touch_style = (f32::from(size) / 100.0, f32::from(opacity) / 100.0);
        if let Some(touch) = self.touch.as_mut() {
            touch.set_style(self.touch_style.0, self.touch_style.1);
        }
    }

    /// Whether a gamepad is connected, which hides the on-screen pad and
    /// leaves the whole window to the frame.
    fn has_gamepad(&self) -> bool {
        self.pads
            .as_ref()
            .is_some_and(|pads| !pads.names().is_empty())
    }

    /// Lets SDL scale the frame to the window, unless the on-screen pad
    /// places it itself.
    fn present_frames(&mut self) -> Result<(), PlatformError> {
        let (width, height) = (dimension(self.frame_width)?, dimension(self.frame_height)?);
        let presentation = match (self.touch.is_some(), self.filter) {
            (true, _) => SDL_LOGICAL_PRESENTATION_DISABLED,
            (false, Filter::Sharp) => SDL_LOGICAL_PRESENTATION_INTEGER_SCALE,
            (false, Filter::Smooth) => SDL_LOGICAL_PRESENTATION_LETTERBOX,
        };
        self.canvas
            .set_logical_size(width, height, presentation)
            .map_err(backend_error)
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

/// Where a frame of `frame` pixels goes in an output of `output` pixels:
/// centered, as large as it fits, in whole multiples with the sharp filter.
fn fitted(output: (u32, u32), frame: (usize, usize), filter: Filter) -> FRect {
    let (width, height) = (f32_of(output.0), f32_of(output.1));
    let (frame_width, frame_height) = (f32_of_usize(frame.0), f32_of_usize(frame.1));
    let scale = (width / frame_width).min(height / frame_height);
    let scale = match filter {
        Filter::Sharp if scale >= 1.0 => scale.floor(),
        _ => scale,
    };
    let (shown_width, shown_height) = (frame_width * scale, frame_height * scale);
    FRect::new(
        (width - shown_width) / 2.0,
        (height - shown_height) / 2.0,
        shown_width,
        shown_height,
    )
}

#[allow(clippy::cast_precision_loss)]
fn f32_of(value: u32) -> f32 {
    value as f32
}

#[allow(clippy::cast_precision_loss)]
fn f32_of_usize(value: usize) -> f32 {
    value as f32
}

/// A position in the frame, in whole pixels (a pointer outside it gives a
/// negative or too large one).
#[allow(clippy::cast_possible_truncation)]
fn pixel(position: f32) -> i32 {
    position.floor() as i32
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fitted_frame_is_centered_and_sharp_in_whole_multiples() {
        let sharp = fitted((2400, 1080), (240, 160), Filter::Sharp);
        assert_eq!((sharp.w, sharp.h), (1440.0, 960.0));
        assert_eq!((sharp.x, sharp.y), (480.0, 60.0));
        let smooth = fitted((2400, 1080), (240, 160), Filter::Smooth);
        assert_eq!((smooth.w, smooth.h), (1620.0, 1080.0));
        let small = fitted((200, 100), (240, 160), Filter::Sharp);
        assert!(small.w <= 200.0 && small.h <= 100.0);
    }
}
