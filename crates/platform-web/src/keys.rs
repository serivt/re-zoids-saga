//! The pad's buttons from the keyboard and gamepads: the same keys as the
//! desktop's by default (arrows, X = A, Z = B, Return = START, Backspace =
//! SELECT, A = L, S = R, Space = the fast forward, M = mute), named by the
//! key's place (`KeyboardEvent.code`, so they hold on any layout), and any
//! gamepad in the browser's standard mapping: its right face button A, the
//! bottom one B, the shoulders L and R, Back SELECT, Start START, the
//! D-pad or the left stick the arrows, the right stick's click the fast
//! forward. A key the page takes for the game does nothing else there
//! (no scrolling, no going back).

use std::cell::Cell;
use std::rc::Rc;

use platform::{Button, Input, PlatformError};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Gamepad, GamepadButton, KeyboardEvent, Window};

use crate::web_error;

/// How far a stick must lean for its direction to count.
const STICK_DEAD_ZONE: f64 = 0.5;
/// The standard mapping's axes of the left stick.
const STICK_AXES: (u32, u32) = (0, 1);

/// The button the key at `code` stands for, if any.
#[must_use]
pub fn button_for_code(code: &str) -> Option<Button> {
    Some(match code {
        "ArrowUp" => Button::Up,
        "ArrowDown" => Button::Down,
        "ArrowLeft" => Button::Left,
        "ArrowRight" => Button::Right,
        "KeyX" => Button::A,
        "KeyZ" => Button::B,
        "Enter" => Button::Start,
        "Backspace" => Button::Select,
        "KeyA" => Button::L,
        "KeyS" => Button::R,
        "Space" => Button::FastForward,
        "KeyM" => Button::Mute,
        _ => return None,
    })
}

/// The button a gamepad's button `index` stands for in the standard
/// mapping, if any.
#[must_use]
pub fn button_for_pad(index: usize) -> Option<Button> {
    Some(match index {
        0 => Button::B,
        1 => Button::A,
        4 => Button::L,
        5 => Button::R,
        8 => Button::Select,
        9 => Button::Start,
        11 => Button::FastForward,
        12 => Button::Up,
        13 => Button::Down,
        14 => Button::Left,
        15 => Button::Right,
        _ => return None,
    })
}

/// The buttons a stick leaning `(x, y)` holds.
fn stick_buttons((x, y): (f64, f64)) -> Input {
    let mut input = Input::default();
    if x <= -STICK_DEAD_ZONE {
        input = input.with(Button::Left);
    }
    if x >= STICK_DEAD_ZONE {
        input = input.with(Button::Right);
    }
    if y <= -STICK_DEAD_ZONE {
        input = input.with(Button::Up);
    }
    if y >= STICK_DEAD_ZONE {
        input = input.with(Button::Down);
    }
    input
}

/// The keys held, kept by the page's key events, and the gamepads read
/// each frame.
pub struct WebInput {
    window: Window,
    keys: Rc<Cell<Input>>,
}

impl WebInput {
    /// Listens to `window`'s keys; a key the game takes does nothing else.
    /// Losing the focus lets every key go.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the listeners cannot be added.
    pub fn listen(window: Window) -> Result<Self, PlatformError> {
        let keys = Rc::new(Cell::new(Input::default()));
        for (kind, down) in [("keydown", true), ("keyup", false)] {
            let held = Rc::clone(&keys);
            let listener = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
                let Some(button) = button_for_code(&event.code()) else {
                    return;
                };
                event.prevent_default();
                let input = held.get();
                held.set(if down {
                    input.with(button)
                } else {
                    input.without(button)
                });
            });
            window
                .add_event_listener_with_callback(kind, listener.as_ref().unchecked_ref())
                .map_err(web_error)?;
            listener.forget();
        }
        let held = Rc::clone(&keys);
        let blur = Closure::<dyn FnMut()>::new(move || held.set(Input::default()));
        window
            .add_event_listener_with_callback("blur", blur.as_ref().unchecked_ref())
            .map_err(web_error)?;
        blur.forget();
        Ok(Self { window, keys })
    }

    /// The buttons held now: the keys' and every gamepad's.
    #[must_use]
    pub fn input(&self) -> Input {
        self.keys.get().union(self.pads())
    }

    /// The buttons the gamepads hold now.
    fn pads(&self) -> Input {
        let Ok(pads) = self.window.navigator().get_gamepads() else {
            return Input::default();
        };
        let mut input = Input::default();
        for pad in pads.iter().filter_map(|pad| pad.dyn_into::<Gamepad>().ok()) {
            for (index, button) in pad.buttons().iter().enumerate() {
                let pressed = button
                    .dyn_into::<GamepadButton>()
                    .is_ok_and(|button| button.pressed());
                if let Some(button) = button_for_pad(index).filter(|_| pressed) {
                    input = input.with(button);
                }
            }
            let leans = pad.axes();
            let lean = |index: u32| leans.get(index).as_f64().unwrap_or(0.0);
            input = input.union(stick_buttons((lean(STICK_AXES.0), lean(STICK_AXES.1))));
        }
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_are_the_desktop_s_by_their_place() {
        assert_eq!(button_for_code("KeyX"), Some(Button::A));
        assert_eq!(button_for_code("Enter"), Some(Button::Start));
        assert_eq!(button_for_code("KeyM"), Some(Button::Mute));
        assert_eq!(button_for_code("KeyQ"), None);
    }

    #[test]
    fn a_gamepad_follows_the_standard_mapping() {
        assert_eq!(button_for_pad(1), Some(Button::A));
        assert_eq!(button_for_pad(0), Some(Button::B));
        assert_eq!(button_for_pad(9), Some(Button::Start));
        assert_eq!(button_for_pad(3), None);
        let leaning = stick_buttons((-0.9, 0.7));
        assert!(leaning.is_held(Button::Left) && leaning.is_held(Button::Down));
        assert_eq!(
            stick_buttons((0.3, -0.2)),
            Input::default(),
            "inside the dead zone"
        );
    }
}
