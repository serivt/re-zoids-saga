//! The on-screen pad for touch screens: where the game's screen and each
//! control sit in a window, and which buttons the fingers on them hold.
//! Only layout and hit testing live here; the backend draws the controls
//! and reports the fingers, in window pixels.
//!
//! In landscape the screen fills the window's height in the middle, the
//! cross and Select on the left, A, B and Start on the right, L and R in
//! the top corners and the fast forward at the top in the middle, over the
//! screen's edge. In portrait the screen fills the width at the top and
//! the controls sit below it, in the same places, the fast forward between
//! L and R.
//!
//! Source of knowledge: this project's own design.

use crate::{Button, Input};

/// How large the controls can be made, against their usual size.
pub const SIZES: std::ops::RangeInclusive<f32> = 0.6..=1.4;
/// The cross's size and the buttons' against the window's short side.
const CROSS_RADIUS: f32 = 0.16;
const BUTTON_RADIUS: f32 = 0.075;
const SHOULDER_SIZE: (f32, f32) = (0.2, 0.09);
const MENU_SIZE: (f32, f32) = (0.16, 0.07);
/// How far past its edge a finger still presses a control, against the
/// control's radius or height, so a slightly missed press counts.
const SLACK: f32 = 0.3;
/// The cross's middle, against its radius, where no direction is held.
const DEAD_ZONE: f32 = 0.2;
/// A direction is held when the finger's offset along it is more than
/// this share of its distance from the middle (cos 67.5°): eight sectors
/// of 45°, the diagonals holding two directions.
const DIRECTION_SHARE: f32 = 0.383;

/// A rectangle in window pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Area {
    fn contains(&self, (x, y): (f32, f32), slack: f32) -> bool {
        x >= self.x - slack
            && x <= self.x + self.width + slack
            && y >= self.y - slack
            && y <= self.y + self.height + slack
    }
}

/// A control's shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// A disc around its middle.
    Circle {
        /// Middle, horizontally.
        x: f32,
        /// Middle, vertically.
        y: f32,
        /// Radius.
        radius: f32,
    },
    /// A rounded bar.
    Pill(Area),
}

/// What a control presses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// The cross: up, down, left and right, two at once on the diagonals.
    Cross,
    /// A single button.
    Button(Button),
}

/// A control and where it sits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// What it presses.
    pub control: Control,
    /// Where it sits.
    pub shape: Shape,
}

/// The game's screen and the controls in a window.
#[derive(Debug, Clone, PartialEq)]
pub struct TouchLayout {
    /// Where the game's screen is drawn.
    pub screen: Area,
    /// The controls, in drawing order.
    pub controls: Vec<Placed>,
}

impl TouchLayout {
    /// Lays the controls out in a window of `window` (width, height) for a
    /// game screen of `frame` (width, height), keeping its proportions.
    #[must_use]
    pub fn new(window: (f32, f32), frame: (f32, f32)) -> Self {
        Self::sized(window, frame, 1.0)
    }

    /// [`Self::new`] with the controls `size` times as large (and as far
    /// from each other and the edges), from [`SIZES`]' least to its most.
    #[must_use]
    pub fn sized(window: (f32, f32), frame: (f32, f32), size: f32) -> Self {
        let (width, height) = window;
        let size = size.clamp(*SIZES.start(), *SIZES.end());
        if width >= height {
            Self::landscape(width, height, frame, size)
        } else {
            Self::portrait(width, height, frame, size)
        }
    }

    fn landscape(
        width: f32,
        height: f32,
        (frame_width, frame_height): (f32, f32),
        size: f32,
    ) -> Self {
        let scale = (height / frame_height).min(width / frame_width);
        let screen = Area {
            x: (width - frame_width * scale) / 2.0,
            y: (height - frame_height * scale) / 2.0,
            width: frame_width * scale,
            height: frame_height * scale,
        };
        let unit = height * size;
        let margin = screen.x.max(unit * (CROSS_RADIUS + 0.04));
        let left = (margin / 2.0).max(unit * (CROSS_RADIUS + 0.04));
        let right = width - left;
        let buttons_y = height * 0.5 - unit * 0.04;
        let controls = vec![
            cross(left, height * 0.6, unit),
            button(Button::A, right + unit * 0.065, buttons_y, unit),
            button(
                Button::B,
                right - unit * 0.075,
                buttons_y + unit * 0.18,
                unit,
            ),
            pill(Button::L, unit * 0.03, unit * 0.03, SHOULDER_SIZE, unit),
            pill(
                Button::R,
                width - unit * (0.03 + SHOULDER_SIZE.0),
                unit * 0.03,
                SHOULDER_SIZE,
                unit,
            ),
            centered_pill(Button::Select, left, height - unit * 0.08, unit),
            centered_pill(Button::Start, right, height - unit * 0.08, unit),
            centered_pill(
                Button::FastForward,
                width * 0.5,
                unit * (0.03 + MENU_SIZE.1 / 2.0),
                unit,
            ),
        ];
        Self { screen, controls }
    }

    fn portrait(
        width: f32,
        height: f32,
        (frame_width, frame_height): (f32, f32),
        size: f32,
    ) -> Self {
        let scale = width / frame_width;
        let screen = Area {
            x: 0.0,
            y: 0.0,
            width,
            height: frame_height * scale,
        };
        let unit = width * size;
        let top = screen.height;
        let middle = top + (height - top) * 0.5;
        let controls = vec![
            cross(unit * (CROSS_RADIUS + 0.09), middle, unit),
            button(
                Button::A,
                width - unit * (BUTTON_RADIUS + 0.085),
                middle - unit * 0.05,
                unit,
            ),
            button(
                Button::B,
                width - unit * (BUTTON_RADIUS + 0.265),
                middle + unit * 0.07,
                unit,
            ),
            pill(
                Button::L,
                unit * 0.03,
                top + unit * 0.04,
                SHOULDER_SIZE,
                unit,
            ),
            pill(
                Button::R,
                width - unit * (0.03 + SHOULDER_SIZE.0),
                top + unit * 0.04,
                SHOULDER_SIZE,
                unit,
            ),
            centered_pill(
                Button::Select,
                width * 0.5 - unit * 0.1,
                height - unit * 0.08,
                unit,
            ),
            centered_pill(
                Button::Start,
                width * 0.5 + unit * 0.1,
                height - unit * 0.08,
                unit,
            ),
            centered_pill(
                Button::FastForward,
                width * 0.5,
                top + unit * (0.04 + SHOULDER_SIZE.1 / 2.0),
                unit,
            ),
        ];
        Self { screen, controls }
    }

    /// The buttons the fingers at `fingers` (window pixels) hold.
    #[must_use]
    pub fn input(&self, fingers: &[(f32, f32)]) -> Input {
        let mut input = Input::default();
        for &finger in fingers {
            for placed in &self.controls {
                input = press(input, placed, finger);
            }
        }
        input
    }
}

fn cross(x: f32, y: f32, unit: f32) -> Placed {
    Placed {
        control: Control::Cross,
        shape: Shape::Circle {
            x,
            y,
            radius: unit * CROSS_RADIUS,
        },
    }
}

fn button(button: Button, x: f32, y: f32, unit: f32) -> Placed {
    Placed {
        control: Control::Button(button),
        shape: Shape::Circle {
            x,
            y,
            radius: unit * BUTTON_RADIUS,
        },
    }
}

fn pill(button: Button, x: f32, y: f32, (width, height): (f32, f32), unit: f32) -> Placed {
    Placed {
        control: Control::Button(button),
        shape: Shape::Pill(Area {
            x,
            y,
            width: unit * width,
            height: unit * height,
        }),
    }
}

fn centered_pill(button: Button, x: f32, y: f32, unit: f32) -> Placed {
    let (width, height) = MENU_SIZE;
    pill(
        button,
        x - unit * width / 2.0,
        y - unit * height / 2.0,
        MENU_SIZE,
        unit,
    )
}

/// `input` with what a finger at `finger` presses on `placed` added.
fn press(input: Input, placed: &Placed, finger: (f32, f32)) -> Input {
    match (placed.control, placed.shape) {
        (Control::Cross, Shape::Circle { x, y, radius }) => {
            let (dx, dy) = (finger.0 - x, finger.1 - y);
            let distance = dx.hypot(dy);
            if distance > radius * (1.0 + SLACK) || distance < radius * DEAD_ZONE {
                return input;
            }
            let share = distance * DIRECTION_SHARE;
            [
                (dx > share, Button::Right),
                (dx < -share, Button::Left),
                (dy > share, Button::Down),
                (dy < -share, Button::Up),
            ]
            .into_iter()
            .filter(|(held, _)| *held)
            .fold(input, |input, (_, button)| input.with(button))
        }
        (Control::Button(button), Shape::Circle { x, y, radius }) => {
            let distance = (finger.0 - x).hypot(finger.1 - y);
            if distance <= radius * (1.0 + SLACK) {
                input.with(button)
            } else {
                input
            }
        }
        (Control::Button(button), Shape::Pill(area)) => {
            if area.contains(finger, area.height * SLACK) {
                input.with(button)
            } else {
                input
            }
        }
        (Control::Cross, Shape::Pill(_)) => input,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    const FRAME: (f32, f32) = (240.0, 160.0);

    fn circle(layout: &TouchLayout, control: Control) -> (f32, f32, f32) {
        layout
            .controls
            .iter()
            .find_map(|placed| match placed.shape {
                Shape::Circle { x, y, radius } if placed.control == control => Some((x, y, radius)),
                _ => None,
            })
            .expect("placed as a circle")
    }

    fn held(input: Input) -> Vec<Button> {
        Button::ALL
            .into_iter()
            .filter(|button| input.is_held(*button))
            .collect()
    }

    fn inside(area: Area, window: (f32, f32)) -> bool {
        area.x >= 0.0
            && area.y >= 0.0
            && area.x + area.width <= window.0 + 0.01
            && area.y + area.height <= window.1 + 0.01
    }

    fn bounds(shape: Shape) -> Area {
        match shape {
            Shape::Circle { x, y, radius } => Area {
                x: x - radius,
                y: y - radius,
                width: radius * 2.0,
                height: radius * 2.0,
            },
            Shape::Pill(area) => area,
        }
    }

    #[test]
    fn every_control_fits_the_window_in_both_orientations() {
        for window in [
            (2400.0, 1080.0),
            (1920.0, 1080.0),
            (1080.0, 2400.0),
            (800.0, 1280.0),
        ] {
            let layout = TouchLayout::new(window, FRAME);
            assert!(inside(layout.screen, window), "{window:?}");
            assert!((layout.screen.width / layout.screen.height - 1.5).abs() < 0.001);
            for placed in &layout.controls {
                assert!(
                    inside(bounds(placed.shape), window),
                    "{window:?} {placed:?}"
                );
            }
            let buttons: Vec<Control> = layout.controls.iter().map(|p| p.control).collect();
            for button in [
                Button::A,
                Button::B,
                Button::L,
                Button::R,
                Button::Start,
                Button::Select,
            ] {
                assert!(buttons.contains(&Control::Button(button)));
            }
            assert!(buttons.contains(&Control::Cross));
        }
    }

    #[test]
    fn every_size_fits_and_grows_the_controls() {
        for window in [
            (2400.0, 1080.0),
            (1920.0, 1080.0),
            (1080.0, 2400.0),
            (800.0, 1280.0),
        ] {
            let mut last = 0.0;
            for size in [0.6, 0.8, 1.0, 1.2, 1.4, 3.0] {
                let layout = TouchLayout::sized(window, FRAME, size);
                for placed in &layout.controls {
                    assert!(
                        inside(bounds(placed.shape), window),
                        "{window:?} {size} {placed:?}"
                    );
                }
                let (_, _, radius) = circle(&layout, Control::Cross);
                assert!(radius >= last, "{window:?} {size}");
                last = radius;
            }
        }
    }

    #[test]
    fn landscape_centers_the_screen_and_portrait_puts_it_on_top() {
        let wide = TouchLayout::new((2400.0, 1080.0), FRAME);
        assert!((wide.screen.height - 1080.0).abs() < 0.001);
        assert!((wide.screen.x - (2400.0 - 1620.0) / 2.0).abs() < 0.001);
        let tall = TouchLayout::new((1080.0, 2400.0), FRAME);
        assert!(tall.screen.y.abs() < 0.001);
        assert!((tall.screen.width - 1080.0).abs() < 0.001);
        assert!((tall.screen.height - 720.0).abs() < 0.001);
        let (_, cross_y, _) = circle(&tall, Control::Cross);
        assert!(cross_y > tall.screen.height);
    }

    #[test]
    fn the_cross_holds_one_direction_or_two_on_the_diagonals() {
        let layout = TouchLayout::new((2400.0, 1080.0), FRAME);
        let (x, y, radius) = circle(&layout, Control::Cross);
        let at = |dx: f32, dy: f32| held(layout.input(&[(x + dx * radius, y + dy * radius)]));
        assert_eq!(at(0.8, 0.0), [Button::Right]);
        assert_eq!(at(-0.8, 0.1), [Button::Left]);
        assert_eq!(at(0.0, -0.8), [Button::Up]);
        assert_eq!(at(0.05, 0.9), [Button::Down]);
        assert_eq!(at(0.6, 0.6), [Button::Down, Button::Right]);
        assert_eq!(at(-0.6, -0.6), [Button::Up, Button::Left]);
        assert!(at(0.1, 0.0).is_empty(), "the middle holds nothing");
        assert!(at(2.0, 0.0).is_empty(), "far outside holds nothing");
        assert_eq!(
            at(1.2, 0.0),
            [Button::Right],
            "just past the edge still counts"
        );
    }

    #[test]
    fn several_fingers_hold_several_buttons() {
        let layout = TouchLayout::new((1080.0, 2400.0), FRAME);
        let (cx, cy, radius) = circle(&layout, Control::Cross);
        let (ax, ay, _) = circle(&layout, Control::Button(Button::A));
        let (bx, by, _) = circle(&layout, Control::Button(Button::B));
        let input = layout.input(&[(cx - radius * 0.7, cy), (ax, ay), (bx, by)]);
        assert_eq!(held(input), [Button::Left, Button::A, Button::B]);
        assert_eq!(layout.input(&[]), Input::default());
    }

    #[test]
    fn the_shoulders_and_the_menu_buttons_answer_to_their_bars() {
        for window in [(2400.0, 1080.0), (1080.0, 2400.0)] {
            let layout = TouchLayout::new(window, FRAME);
            for placed in &layout.controls {
                if let (Control::Button(button), Shape::Pill(area)) = (placed.control, placed.shape)
                {
                    let middle = (area.x + area.width / 2.0, area.y + area.height / 2.0);
                    assert_eq!(held(layout.input(&[middle])), [button], "{window:?}");
                }
            }
        }
    }

    #[test]
    fn a_touch_on_the_screen_itself_holds_nothing() {
        let layout = TouchLayout::new((2400.0, 1080.0), FRAME);
        let middle = (
            layout.screen.x + layout.screen.width / 2.0,
            layout.screen.y + layout.screen.height / 2.0,
        );
        assert_eq!(layout.input(&[middle]), Input::default());
    }
}
