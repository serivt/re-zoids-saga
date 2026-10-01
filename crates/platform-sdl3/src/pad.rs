//! Gamepads through SDL3: every one connected drives the pad's buttons,
//! by a map the player can change, and its left stick moves like the
//! D-pad.

use platform::{Button, Input};
use sdl3::GamepadSubsystem;
use sdl3::Sdl;
use sdl3::gamepad::{Axis, Button as PadButton, Gamepad};
use sdl3::joystick::JoystickId;

/// How far a stick must lean to count as a direction.
const STICK_DEADZONE: i16 = 16_000;

/// The gamepad buttons the pad's buttons have unless the player chose
/// others: the D-pad, A on the right face button and B on the bottom one,
/// as the console's are placed.
const DEFAULT_PAD_BUTTONS: [(PadButton, Button); 11] = [
    (PadButton::DPadUp, Button::Up),
    (PadButton::DPadDown, Button::Down),
    (PadButton::DPadLeft, Button::Left),
    (PadButton::DPadRight, Button::Right),
    (PadButton::East, Button::A),
    (PadButton::South, Button::B),
    (PadButton::LeftShoulder, Button::L),
    (PadButton::RightShoulder, Button::R),
    (PadButton::Start, Button::Start),
    (PadButton::Back, Button::Select),
    (PadButton::RightStick, Button::FastForward),
];

/// The gamepads connected and the map of their buttons.
pub(crate) struct Pads {
    subsystem: GamepadSubsystem,
    open: Vec<Gamepad>,
    buttons: Vec<(PadButton, Button)>,
}

impl Pads {
    /// Opens the gamepad subsystem and every gamepad already connected.
    pub(crate) fn open(sdl: &Sdl) -> Option<Self> {
        let subsystem = sdl.gamepad().ok()?;
        let open = subsystem
            .gamepads()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| subsystem.open(id).ok())
            .collect();
        Some(Self {
            subsystem,
            open,
            buttons: DEFAULT_PAD_BUTTONS.to_vec(),
        })
    }

    /// A gamepad was connected.
    pub(crate) fn add(&mut self, id: JoystickId) {
        if self.open.iter().all(|pad| pad.id().ok() != Some(id))
            && let Ok(pad) = self.subsystem.open(id)
        {
            self.open.push(pad);
        }
    }

    /// A gamepad was disconnected.
    pub(crate) fn remove(&mut self, id: JoystickId) {
        self.open.retain(|pad| pad.id().ok() != Some(id));
    }

    /// The names of the gamepads connected.
    pub(crate) fn names(&self) -> Vec<String> {
        self.open.iter().filter_map(Gamepad::name).collect()
    }

    /// Gives the buttons the gamepad buttons of `map`, by name; a button
    /// it leaves out or names wrongly keeps its default.
    pub(crate) fn set_buttons(&mut self, map: &[(Button, String)]) {
        self.buttons = DEFAULT_PAD_BUTTONS
            .iter()
            .map(|&(default, button)| {
                let chosen = map
                    .iter()
                    .find(|(mapped, _)| *mapped == button)
                    .and_then(|(_, name)| PadButton::from_string(name));
                (chosen.unwrap_or(default), button)
            })
            .collect();
    }

    /// The buttons the gamepads hold.
    pub(crate) fn input(&self) -> Input {
        let mut input = Input::default();
        for pad in &self.open {
            for &(pad_button, button) in &self.buttons {
                if pad.button(pad_button) {
                    input = input.with(button);
                }
            }
            let (x, y) = (pad.axis(Axis::LeftX), pad.axis(Axis::LeftY));
            for (leans, button) in [
                (x < -STICK_DEADZONE, Button::Left),
                (x > STICK_DEADZONE, Button::Right),
                (y < -STICK_DEADZONE, Button::Up),
                (y > STICK_DEADZONE, Button::Down),
            ] {
                if leans {
                    input = input.with(button);
                }
            }
        }
        input
    }
}

/// The code [`platform::Event::Pad`] reports for `button`.
pub(crate) fn pad_code(button: PadButton) -> u32 {
    u32::try_from(button as i32).unwrap_or(u32::MAX)
}

/// The gamepad buttons' defaults, by name.
#[must_use]
pub fn default_pad_buttons() -> Vec<(Button, String)> {
    DEFAULT_PAD_BUTTONS
        .iter()
        .map(|&(pad_button, button)| (button, pad_button.string()))
        .collect()
}

/// The name of the gamepad button [`platform::Event::Pad`] reports as
/// `code`, as settings keep it.
#[must_use]
pub fn pad_button_name(code: u32) -> Option<String> {
    let code = i32::try_from(code).ok()?;
    let button = PadButton::from_ll(sdl3::sys::gamepad::SDL_GamepadButton(code))?;
    Some(button.string())
}

/// A short label for the gamepad button named `name`, to show: the face
/// buttons by their place, the others by the usual marks.
#[must_use]
pub fn pad_button_label(name: &str) -> String {
    match name {
        "a" => "South",
        "b" => "East",
        "x" => "West",
        "y" => "North",
        "back" => "Back",
        "start" => "Start",
        "guide" => "Guide",
        "leftshoulder" => "LB",
        "rightshoulder" => "RB",
        "leftstick" => "LS",
        "rightstick" => "RS",
        "dpup" => "D-Up",
        "dpdown" => "D-Down",
        "dpleft" => "D-Left",
        "dpright" => "D-Right",
        other => other,
    }
    .to_owned()
}
