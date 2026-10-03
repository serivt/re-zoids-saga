//! The web version's on-screen pad: where the game's screen, each control
//! and the page's pause button sit in the window, and how the controls are
//! drawn. The layout is the web page's own design, not the Android app's;
//! which buttons a finger holds is still [`TouchLayout::input`]'s.
//!
//! Held upright, the screen fills the width at the top; under it L and R
//! sit at the sides with the pause button between them, then the cross on
//! the left and A over B on the right, and last a row of the fast forward,
//! SELECT, START and mute. Held sideways, the screen is in the middle, L,
//! the cross, SELECT and the fast forward on its left, R, A and B, mute
//! and START on its right, the pause button above it.
//!
//! The controls are translucent with a thin white edge, amber while held;
//! their letters are the system's monospaced bold, and the cross carries a
//! triangle on each arm.
//!
//! Source of knowledge: this project's own design (the web launcher's,
//! drafted with Claude Design).

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use platform::touch::{Area, Control, Placed, Shape};
use platform::{Button, Input, TouchLayout};
use web_sys::CanvasRenderingContext2d;

/// The sizes in the page's pixels at the usual size, from the design.
const EDGE: f32 = 16.0;
const SHOULDER: (f32, f32) = (120.0, 44.0);
const MENU_BAR: (f32, f32) = (92.0, 40.0);
const ROUND: f32 = 52.0;
const CROSS: f32 = 168.0;
const CROSS_SIDEWAYS: f32 = 150.0;
const FACE: f32 = 76.0;
const FACE_SIDEWAYS: f32 = 70.0;
/// Upright: the space above the screen and between it and the controls.
const ABOVE_SCREEN: f32 = 16.0;
const UNDER_SCREEN: f32 = 24.0;
/// Sideways: what the controls keep of the width, and the space the screen
/// leaves above and below.
const SIDES: f32 = 360.0;
const ABOVE_AND_BELOW: f32 = 56.0;
/// The pause button's distance from the window's top when sideways.
const MENU_TOP: f32 = 8.0;
/// The gap between the grid's cells.
const GAP: f32 = 8.0;

/// The colors, from the design: the controls' fill and edge, idle and held,
/// and their marks.
const FILL: &str = "rgba(255, 255, 255, 0.07)";
const EDGE_COLOR: &str = "rgba(255, 255, 255, 0.3)";
const MARK: &str = "rgba(255, 255, 255, 0.8)";
const HELD_FILL: &str = "rgba(245, 197, 66, 0.32)";
const HELD_EDGE: &str = "#f5c542";
const HELD_MARK: &str = "#fff";
const LINE: f64 = 1.5;
const FONT: &str = "ui-monospace, \"SF Mono\", \"Cascadia Mono\", Menlo, Consolas, monospace";

/// The pad laid out in a window: the controls (with the screen) and the
/// middle of the page's pause button.
#[derive(Debug, Clone, PartialEq)]
pub struct PadLayout {
    /// The screen and the controls, which tell the buttons held.
    pub touch: TouchLayout,
    /// Where the pause button's middle goes.
    pub menu: (f32, f32),
}

/// The pad in a window of `window` (width, height) for a game screen of
/// `frame`, its controls `size` times their usual size, with the fast
/// forward or without it.
#[must_use]
pub fn pad_layout(
    window: (f32, f32),
    frame: (f32, f32),
    size: f32,
    fast_forward: bool,
) -> PadLayout {
    let mut pad = if window.0 >= window.1 {
        sideways(window, frame, size)
    } else {
        upright(window, frame, size)
    };
    if !fast_forward {
        pad.touch
            .controls
            .retain(|placed| placed.control != Control::Button(Button::FastForward));
    }
    pad
}

fn upright(
    (width, height): (f32, f32),
    (frame_width, frame_height): (f32, f32),
    size: f32,
) -> PadLayout {
    let screen = Area {
        x: 0.0,
        y: ABOVE_SCREEN,
        width,
        height: width * frame_height / frame_width,
    };
    let top = screen.y + screen.height + UNDER_SCREEN;
    let bottom = height - UNDER_SCREEN;
    let (left, right) = (EDGE, width - EDGE);
    let shoulder = (SHOULDER.0 * size, SHOULDER.1 * size);
    let round = ROUND * size;
    let bar = (MENU_BAR.0 * size, MENU_BAR.1 * size);
    let row_middle = bottom - round / 2.0;
    let middle = f32::midpoint(top + shoulder.1, bottom - round);
    let cross = (CROSS * size)
        .min((right - left - GAP) / 2.0)
        .min(bottom - round - top - shoulder.1 - 2.0 * GAP);
    let face = cross * FACE / CROSS;
    let column = (right - left - 3.0 * GAP) / 4.0;
    let mut controls = vec![
        pill(Button::L, left, top, shoulder),
        pill(Button::R, right - shoulder.0, top, shoulder),
        cross_at(left + cross / 2.0, middle, cross),
    ];
    controls.extend(faces(right - cross, middle - cross / 2.0, cross, face));
    controls.extend([
        circle(Button::FastForward, left + round / 2.0, row_middle, round),
        pill(
            Button::Select,
            left + 2.0 * column + GAP - bar.0,
            row_middle - bar.1 / 2.0,
            bar,
        ),
        pill(
            Button::Start,
            left + 2.0 * (column + GAP),
            row_middle - bar.1 / 2.0,
            bar,
        ),
        circle(Button::Mute, right - round / 2.0, row_middle, round),
    ]);
    PadLayout {
        touch: TouchLayout { screen, controls },
        menu: (width / 2.0, top + shoulder.1 / 2.0),
    }
}

fn sideways(
    (width, height): (f32, f32),
    (frame_width, frame_height): (f32, f32),
    size: f32,
) -> PadLayout {
    let ratio = frame_width / frame_height;
    let screen_width = (width - SIDES * size)
        .min((height - ABOVE_AND_BELOW) * ratio)
        .max(0.0);
    let screen_height = screen_width / ratio;
    let screen = Area {
        x: (width - screen_width) / 2.0,
        y: (height - screen_height) / 2.0,
        width: screen_width,
        height: screen_height,
    };
    let (left, right) = (EDGE, width - EDGE);
    let (top, bottom) = (EDGE - 4.0, height - EDGE);
    let shoulder = (SHOULDER.0 * size, SHOULDER.1 * size);
    let round = ROUND * size;
    let bar = (MENU_BAR.0 * size, MENU_BAR.1 * size);
    let row_middle = bottom - round / 2.0;
    let middle = f32::midpoint(top + shoulder.1, bottom - round);
    let beside = ((screen.x - GAP) - left).max(0.0);
    let cross = (CROSS_SIDEWAYS * size)
        .min(beside)
        .min(bottom - round - top - shoulder.1 - 2.0 * GAP);
    let face = cross * FACE_SIDEWAYS / CROSS_SIDEWAYS;
    let column = (beside - GAP) / 2.0;
    let mut controls = vec![
        pill(Button::L, left, top, shoulder),
        pill(Button::R, right - shoulder.0, top, shoulder),
        cross_at(left + cross / 2.0, middle, cross),
    ];
    controls.extend(faces(right - cross, middle - cross / 2.0, cross, face));
    controls.extend([
        pill(Button::Select, left, row_middle - bar.1 / 2.0, bar),
        circle(
            Button::FastForward,
            left + 2.0 * column + GAP - round / 2.0,
            row_middle,
            round,
        ),
        circle(
            Button::Mute,
            right - 2.0 * column - GAP + round / 2.0,
            row_middle,
            round,
        ),
        pill(Button::Start, right - bar.0, row_middle - bar.1 / 2.0, bar),
    ]);
    PadLayout {
        touch: TouchLayout { screen, controls },
        menu: (width / 2.0, MENU_TOP + MENU_BAR.1 * size / 2.0),
    }
}

fn pill(button: Button, x: f32, y: f32, (width, height): (f32, f32)) -> Placed {
    Placed {
        control: Control::Button(button),
        shape: Shape::Pill(Area {
            x,
            y,
            width,
            height,
        }),
    }
}

fn circle(button: Button, x: f32, y: f32, diameter: f32) -> Placed {
    Placed {
        control: Control::Button(button),
        shape: Shape::Circle {
            x,
            y,
            radius: diameter / 2.0,
        },
    }
}

fn cross_at(x: f32, y: f32, side: f32) -> Placed {
    Placed {
        control: Control::Cross,
        shape: Shape::Circle {
            x,
            y,
            radius: side / 2.0,
        },
    }
}

/// A and B in a square of `side` at (`x`, `y`): A at its top right, B at
/// its bottom left, as the design's.
fn faces(x: f32, y: f32, side: f32, face: f32) -> [Placed; 2] {
    let inset = (side - 2.0 * face) / 4.0;
    [
        circle(
            Button::A,
            x + side - face / 2.0,
            y + inset + face / 2.0,
            face,
        ),
        circle(
            Button::B,
            x + face / 2.0,
            y + side - inset - face / 2.0,
            face,
        ),
    ]
}

/// Draws the pad's controls on `context` (in the page's pixels), those
/// `held` amber, all `opacity` (0 to 1) as opaque as usual.
pub fn draw_pad(
    context: &CanvasRenderingContext2d,
    layout: &TouchLayout,
    held: Input,
    opacity: f32,
) {
    context.set_global_alpha(f64::from(opacity.clamp(0.0, 1.0)));
    context.set_line_width(LINE);
    context.set_text_align("center");
    context.set_text_baseline("middle");
    for placed in &layout.controls {
        match (placed.control, placed.shape) {
            (Control::Cross, Shape::Circle { x, y, radius }) => {
                draw_cross(
                    context,
                    (f64::from(x), f64::from(y)),
                    f64::from(radius),
                    held,
                );
            }
            (Control::Button(button), Shape::Circle { x, y, radius }) => {
                let on = held.is_held(button);
                let (x, y, radius) = (f64::from(x), f64::from(y), f64::from(radius));
                context.begin_path();
                let _ = context.arc(x, y, radius, 0.0, TAU);
                paint(context, on);
                mark(context, button, (x, y), radius * 2.0, on);
            }
            (Control::Button(button), Shape::Pill(area)) => {
                let on = held.is_held(button);
                pill_path(context, area);
                paint(context, on);
                let middle = (
                    f64::from(area.x + area.width / 2.0),
                    f64::from(area.y + area.height / 2.0),
                );
                mark(context, button, middle, f64::from(area.height), on);
            }
            (Control::Cross, Shape::Pill(_)) => {}
        }
    }
    context.set_global_alpha(1.0);
}

/// Fills and edges the current path, amber when `on`.
fn paint(context: &CanvasRenderingContext2d, on: bool) {
    context.set_fill_style_str(if on { HELD_FILL } else { FILL });
    context.fill();
    context.set_stroke_style_str(if on { HELD_EDGE } else { EDGE_COLOR });
    context.stroke();
}

/// The letters or icon of `button`, centered on `middle`, for a control
/// `tall` pixels high.
fn mark(
    context: &CanvasRenderingContext2d,
    button: Button,
    middle: (f64, f64),
    tall: f64,
    on: bool,
) {
    let color = if on { HELD_MARK } else { MARK };
    context.set_fill_style_str(color);
    context.set_stroke_style_str(color);
    let (text, share) = match button {
        Button::A | Button::B => (if button == Button::A { "A" } else { "B" }, 0.34),
        Button::L => ("L", 0.3),
        Button::R => ("R", 0.3),
        Button::Select => ("SELECT", 0.275),
        Button::Start => ("START", 0.275),
        Button::FastForward => {
            fast_forward_icon(context, middle, tall * 0.42);
            return;
        }
        Button::Mute => {
            speaker_icon(context, middle, tall * 0.42);
            return;
        }
        Button::Up | Button::Down | Button::Left | Button::Right => return,
    };
    context.set_font(&format!("700 {:.1}px {FONT}", tall * share));
    let _ = context.fill_text(text, middle.0, middle.1);
}

/// The cross, as the design's: three cells by three, the arms' outer
/// corners rounded, a triangle on each arm pointing out, the arms held
/// amber.
fn draw_cross(context: &CanvasRenderingContext2d, (x, y): (f64, f64), radius: f64, held: Input) {
    let side = radius * 2.0;
    let cell = side / 3.0;
    let round = cell * 0.18;
    let (left, top) = (x - radius, y - radius);
    cross_path(context, (left, top), cell, round);
    paint(context, false);
    for (button, column, row, angle) in [
        (Button::Up, 1.0, 0.0, 0.0),
        (Button::Right, 2.0, 1.0, FRAC_PI_2),
        (Button::Down, 1.0, 2.0, PI),
        (Button::Left, 0.0, 1.0, -FRAC_PI_2),
    ] {
        let on = held.is_held(button);
        let cell_middle = (left + (column + 0.5) * cell, top + (row + 0.5) * cell);
        if on {
            arm_path(context, cell_middle, cell, round, angle);
            paint(context, true);
        }
        context.set_fill_style_str(if on { HELD_MARK } else { MARK });
        triangle(context, cell_middle, cell * 0.32, angle);
    }
}

/// The outline of a cross of three cells by three from `(left, top)`.
fn cross_path(context: &CanvasRenderingContext2d, (left, top): (f64, f64), cell: f64, round: f64) {
    let at = |column: f64, row: f64| (left + column * cell, top + row * cell);
    let corners = [
        (at(1.0, 0.0), true),
        (at(2.0, 0.0), true),
        (at(2.0, 1.0), false),
        (at(3.0, 1.0), true),
        (at(3.0, 2.0), true),
        (at(2.0, 2.0), false),
        (at(2.0, 3.0), true),
        (at(1.0, 3.0), true),
        (at(1.0, 2.0), false),
        (at(0.0, 2.0), true),
        (at(0.0, 1.0), true),
        (at(1.0, 1.0), false),
    ];
    rounded_polygon(context, &corners, round);
}

/// One arm of the cross, its outer end rounded: the cell at `middle`,
/// pointing out at `angle` (0 up, clockwise).
fn arm_path(
    context: &CanvasRenderingContext2d,
    middle: (f64, f64),
    cell: f64,
    round: f64,
    angle: f64,
) {
    let half = cell / 2.0;
    let turn = |(dx, dy): (f64, f64)| {
        let (sin, cos) = angle.sin_cos();
        (
            middle.0 + dx * cos - dy * sin,
            middle.1 + dx * sin + dy * cos,
        )
    };
    let corners = [
        (turn((-half, -half)), true),
        (turn((half, -half)), true),
        (turn((half, half)), false),
        (turn((-half, half)), false),
    ];
    rounded_polygon(context, &corners, round);
}

/// A closed path through `corners`, those marked rounded by `round`.
fn rounded_polygon(context: &CanvasRenderingContext2d, corners: &[((f64, f64), bool)], round: f64) {
    let count = corners.len();
    let last = corners[count - 1].0;
    let first = corners[0].0;
    context.begin_path();
    context.move_to(
        f64::midpoint(last.0, first.0),
        f64::midpoint(last.1, first.1),
    );
    for (at, (corner, rounded)) in corners.iter().enumerate() {
        let next = corners[(at + 1) % count].0;
        if *rounded {
            let _ = context.arc_to(corner.0, corner.1, next.0, next.1, round);
        } else {
            context.line_to(corner.0, corner.1);
        }
    }
    context.close_path();
}

/// A filled triangle `size` wide centered on `middle`, pointing out at
/// `angle` (0 up, clockwise).
fn triangle(context: &CanvasRenderingContext2d, middle: (f64, f64), size: f64, angle: f64) {
    let (sin, cos) = angle.sin_cos();
    let point = |(dx, dy): (f64, f64)| {
        (
            middle.0 + dx * cos - dy * sin,
            middle.1 + dx * sin + dy * cos,
        )
    };
    let tall = size * 0.7;
    let tip = point((0.0, -tall / 2.0));
    let (one, two) = (
        point((size / 2.0, tall / 2.0)),
        point((-size / 2.0, tall / 2.0)),
    );
    context.begin_path();
    context.move_to(tip.0, tip.1);
    context.line_to(one.0, one.1);
    context.line_to(two.0, two.1);
    context.close_path();
    context.fill();
}

/// The fast forward's two triangles, `size` wide, centered on `middle`.
fn fast_forward_icon(context: &CanvasRenderingContext2d, (x, y): (f64, f64), size: f64) {
    let half = size / 2.0;
    let tall = size * 0.5;
    for left in [x - half, x] {
        context.begin_path();
        context.move_to(left, y - tall);
        context.line_to(left + half, y);
        context.line_to(left, y + tall);
        context.close_path();
        context.fill();
    }
}

/// A speaker, `size` wide, centered on `middle`: its box and cone filled,
/// one wave beside it.
fn speaker_icon(context: &CanvasRenderingContext2d, (x, y): (f64, f64), size: f64) {
    let unit = size / 20.0;
    let left = x - 10.0 * unit;
    context.begin_path();
    context.move_to(left, y - 3.0 * unit);
    context.line_to(left + 4.0 * unit, y - 3.0 * unit);
    context.line_to(left + 9.0 * unit, y - 7.5 * unit);
    context.line_to(left + 9.0 * unit, y + 7.5 * unit);
    context.line_to(left + 4.0 * unit, y + 3.0 * unit);
    context.line_to(left, y + 3.0 * unit);
    context.close_path();
    context.fill();
    context.set_line_width(LINE * 1.2);
    context.begin_path();
    let _ = context.arc(left + 11.0 * unit, y, 5.0 * unit, -0.9, 0.9);
    context.stroke();
    context.set_line_width(LINE);
}

/// A bar with round ends over `area`, as the current path.
fn pill_path(context: &CanvasRenderingContext2d, area: Area) {
    let radius = f64::from(area.height) / 2.0;
    let middle = f64::from(area.y) + radius;
    let (left, right) = (
        f64::from(area.x) + radius,
        f64::from(area.x + area.width) - radius,
    );
    context.begin_path();
    let _ = context.arc(right, middle, radius, -FRAC_PI_2, FRAC_PI_2);
    let _ = context.arc(left, middle, radius, FRAC_PI_2, PI + FRAC_PI_2);
    context.close_path();
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: (f32, f32) = (240.0, 160.0);

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

    fn inside(area: Area, window: (f32, f32)) -> bool {
        area.x >= -0.01
            && area.y >= -0.01
            && area.x + area.width <= window.0 + 0.01
            && area.y + area.height <= window.1 + 0.01
    }

    fn overlap(one: Area, two: Area) -> bool {
        one.x < two.x + two.width
            && two.x < one.x + one.width
            && one.y < two.y + two.height
            && two.y < one.y + one.height
    }

    #[test]
    fn every_control_fits_the_window_and_leaves_the_screen_clear() {
        for window in [
            (390.0, 844.0),
            (360.0, 740.0),
            (844.0, 390.0),
            (1024.0, 768.0),
        ] {
            let pad = pad_layout(window, FRAME, 1.0, true);
            assert!(inside(pad.touch.screen, window), "{window:?}");
            assert!((pad.touch.screen.width / pad.touch.screen.height - 1.5).abs() < 0.01);
            assert_eq!(pad.touch.controls.len(), 9, "{window:?}");
            for placed in &pad.touch.controls {
                let area = bounds(placed.shape);
                assert!(inside(area, window), "{window:?} {placed:?}");
                assert!(!overlap(area, pad.touch.screen), "{window:?} {placed:?}");
            }
            for (at, one) in pad.touch.controls.iter().enumerate() {
                for two in &pad.touch.controls[at + 1..] {
                    assert!(
                        !overlap(bounds(one.shape), bounds(two.shape)),
                        "{window:?} {one:?} {two:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn upright_puts_the_screen_on_top_and_sideways_in_the_middle() {
        let upright = pad_layout((390.0, 844.0), FRAME, 1.0, true);
        assert!((upright.touch.screen.y - ABOVE_SCREEN).abs() < 0.01);
        assert!((upright.touch.screen.width - 390.0).abs() < 0.01);
        assert!(upright.menu.1 > upright.touch.screen.y + upright.touch.screen.height);
        let sideways = pad_layout((844.0, 390.0), FRAME, 1.0, true);
        let screen = sideways.touch.screen;
        assert!((screen.x + screen.width / 2.0 - 422.0).abs() < 0.01);
        assert!(
            sideways.menu.1 < screen.y,
            "the pause button sits above the screen"
        );
    }

    #[test]
    fn the_fast_forward_is_left_out_when_asked_and_the_buttons_answer() {
        let pad = pad_layout((390.0, 844.0), FRAME, 1.0, false);
        assert!(
            !pad.touch
                .controls
                .iter()
                .any(|placed| placed.control == Control::Button(Button::FastForward))
        );
        for placed in &pad.touch.controls {
            if let Control::Button(button) = placed.control {
                let area = bounds(placed.shape);
                let middle = (area.x + area.width / 2.0, area.y + area.height / 2.0);
                assert!(pad.touch.input(&[middle]).is_held(button), "{button:?}");
            }
        }
    }
}
