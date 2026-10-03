//! The on-screen pad on a touch screen, through SDL3: the fingers SDL
//! reports, the buttons they hold (laid out by [`TouchLayout`]) and the
//! controls drawn around the game's screen in the style of
//! [`platform::touch`]: translucent, brighter while held, with their
//! letters in a small pixel font of the port's own.
//!
//! Source of knowledge: this project's own design.

use platform::touch::{
    ARM_LENGTH, ARM_THICKNESS, BAR_LETTER_SCALE, Control, Fingers, GLYPH_ROWS, HELD_OPACITY,
    IDLE_OPACITY, LETTER_HEIGHT, MARK_OPACITY, Shape, label, label_cells,
};
use platform::{Button, Input, TouchLayout};
use sdl3::pixels::FColor;
use sdl3::render::{FPoint, FRect, Vertex, WindowCanvas};

/// Segments of a drawn circle.
const SEGMENTS: usize = 40;

/// The fingers on the screen and where the controls are.
pub(crate) struct TouchPad {
    fingers: Fingers,
    layout: TouchLayout,
    window: (u32, u32),
    frame: (f32, f32),
    /// The controls' size against their usual one, and their opacity from
    /// 0 to 1 of their usual one.
    size: f32,
    opacity: f32,
    /// Whether the fast forward is laid out: the enhanced mode's alone.
    fast_forward: bool,
}

impl TouchPad {
    /// A pad for game screens of `frame` (width, height), laid out once
    /// the window's size is known.
    pub(crate) fn new(frame: (usize, usize)) -> Self {
        let frame = (to_f32(frame.0), to_f32(frame.1));
        Self {
            fingers: Fingers::default(),
            layout: TouchLayout::new(frame, frame),
            window: (0, 0),
            frame,
            size: 1.0,
            opacity: 1.0,
            fast_forward: false,
        }
    }

    /// Lays the fast forward out, or leaves it out of the controls.
    pub(crate) fn set_fast_forward(&mut self, on: bool) {
        self.fast_forward = on;
        self.window = (0, 0);
    }

    /// Makes the controls `size` times their usual size and `opacity`
    /// (0 to 1) as opaque as usual.
    pub(crate) fn set_style(&mut self, size: f32, opacity: f32) {
        self.size = size;
        self.opacity = opacity.clamp(0.0, 1.0);
        self.window = (0, 0);
    }

    /// Lays the controls out again when the window is now `window` pixels.
    pub(crate) fn fit(&mut self, window: (u32, u32)) {
        if window != self.window {
            self.window = window;
            let window = (to_f32_u32(window.0), to_f32_u32(window.1));
            self.layout = TouchLayout::sized(window, self.frame, self.size);
            if !self.fast_forward {
                self.layout
                    .controls
                    .retain(|placed| placed.control != Control::Button(Button::FastForward));
            }
        }
    }

    /// Where the game's screen goes.
    pub(crate) fn screen(&self) -> FRect {
        let area = self.layout.screen;
        FRect::new(area.x, area.y, area.width, area.height)
    }

    /// A new batch of events is about to be read: the taps of the last one
    /// have had their frame.
    pub(crate) fn start_events(&mut self) {
        self.fingers.read();
    }

    /// Finger `id` is now at `at`, SDL's position across the window from
    /// 0 to 1, or has left the screen (`None`).
    pub(crate) fn finger(&mut self, id: u64, at: Option<(f32, f32)>) {
        let (width, height) = (to_f32_u32(self.window.0), to_f32_u32(self.window.1));
        self.fingers
            .set(id, at.map(|(x, y)| (x * width, y * height)));
    }

    /// Lets go of every finger, as when the app leaves the screen.
    pub(crate) fn release(&mut self) {
        self.fingers.release();
    }

    /// The buttons the fingers hold, and those of the taps just made.
    pub(crate) fn input(&self) -> Input {
        self.layout.input(&self.fingers.positions())
    }

    /// Draws the controls, those `held` brighter.
    pub(crate) fn draw(&self, canvas: &mut WindowCanvas, held: Input) -> Result<(), sdl3::Error> {
        let palette = Palette::new(self.opacity);
        for placed in &self.layout.controls {
            match (placed.control, placed.shape) {
                (Control::Cross, Shape::Circle { x, y, radius }) => {
                    draw_cross(canvas, (x, y), radius, held, &palette)?;
                }
                (Control::Button(button), Shape::Circle { x, y, radius }) => {
                    let color = palette.of(held.is_held(button));
                    disc(canvas, (x, y), radius, color)?;
                    let height = radius * 2.0 * LETTER_HEIGHT;
                    letters(canvas, label(button), (x, y), height, palette.mark)?;
                }
                (Control::Button(button), Shape::Pill(area)) => {
                    let color = palette.of(held.is_held(button));
                    let radius = area.height / 2.0;
                    let middle = (area.x + area.width / 2.0, area.y + radius);
                    let ends = (area.x + radius, area.x + area.width - radius);
                    fan(canvas, middle, &stadium(ends, middle.1, radius), color)?;
                    let height = area.height * LETTER_HEIGHT * BAR_LETTER_SCALE;
                    letters(canvas, label(button), middle, height, palette.mark)?;
                }
                (Control::Cross, Shape::Pill(_)) => {}
            }
        }
        Ok(())
    }
}

/// The colors of the controls at an opacity.
struct Palette {
    idle: FColor,
    held: FColor,
    mark: FColor,
}

impl Palette {
    fn new(opacity: f32) -> Self {
        let white = |alpha: f32| FColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: alpha * opacity,
        };
        Self {
            idle: white(IDLE_OPACITY),
            held: white(HELD_OPACITY),
            mark: white(MARK_OPACITY),
        }
    }

    fn of(&self, held: bool) -> FColor {
        if held { self.held } else { self.idle }
    }
}

/// The cross: a faint disc, and its four arms, each brighter while its
/// direction is held.
fn draw_cross(
    canvas: &mut WindowCanvas,
    (x, y): (f32, f32),
    radius: f32,
    held: Input,
    palette: &Palette,
) -> Result<(), sdl3::Error> {
    disc(canvas, (x, y), radius, palette.idle)?;
    let length = radius * ARM_LENGTH;
    let thickness = radius * ARM_THICKNESS;
    let half = thickness / 2.0;
    let arms = [
        (
            Button::Up,
            FRect::new(x - half, y - length, thickness, length - half),
        ),
        (
            Button::Down,
            FRect::new(x - half, y + half, thickness, length - half),
        ),
        (
            Button::Left,
            FRect::new(x - length, y - half, length - half, thickness),
        ),
        (
            Button::Right,
            FRect::new(x + half, y - half, length - half, thickness),
        ),
    ];
    fill(
        canvas,
        FRect::new(x - half, y - half, thickness, thickness),
        palette.idle,
    )?;
    for (button, arm) in arms {
        fill(canvas, arm, palette.of(held.is_held(button)))?;
    }
    Ok(())
}

fn fill(canvas: &mut WindowCanvas, rect: FRect, color: FColor) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(color);
    canvas.fill_rect(rect)
}

/// A filled circle.
fn disc(
    canvas: &mut WindowCanvas,
    (x, y): (f32, f32),
    radius: f32,
    color: FColor,
) -> Result<(), sdl3::Error> {
    let outline: Vec<FPoint> = (0..SEGMENTS)
        .map(|step| {
            let angle = std::f32::consts::TAU * to_f32(step) / to_f32(SEGMENTS);
            FPoint::new(x + radius * angle.cos(), y + radius * angle.sin())
        })
        .collect();
    fan(canvas, (x, y), &outline, color)
}

/// The outline of a bar with round ends around the middles `ends` of
/// its end circles, on the row `y`.
fn stadium((left, right): (f32, f32), y: f32, radius: f32) -> Vec<FPoint> {
    let half = SEGMENTS / 2;
    let arc = |middle: f32, from: f32| {
        (0..=half).map(move |step| {
            let angle = from + std::f32::consts::PI * to_f32(step) / to_f32(half);
            FPoint::new(middle + radius * angle.cos(), y + radius * angle.sin())
        })
    };
    let quarter = std::f32::consts::FRAC_PI_2;
    arc(right, -quarter).chain(arc(left, quarter)).collect()
}

/// A convex shape filled as a fan of triangles from `middle` to its
/// `outline`, drawn in one go so its translucency is even.
fn fan(
    canvas: &mut WindowCanvas,
    (x, y): (f32, f32),
    outline: &[FPoint],
    color: FColor,
) -> Result<(), sdl3::Error> {
    let vertex = |position: FPoint| Vertex {
        position,
        color,
        tex_coord: FPoint::new(0.0, 0.0),
    };
    let mut vertices = vec![vertex(FPoint::new(x, y))];
    vertices.extend(outline.iter().copied().map(vertex));
    let count = outline.len();
    let indices: Vec<[u16; 3]> = (0..count)
        .filter_map(|step| {
            let from = u16::try_from(step + 1).ok()?;
            let to = u16::try_from((step + 1) % count + 1).ok()?;
            Some([0, from, to])
        })
        .collect();
    canvas.render_geometry(&vertices, None, indices.as_slice())
}

/// `text` centered on `middle`, `height` pixels tall, in `color`.
fn letters(
    canvas: &mut WindowCanvas,
    text: &str,
    middle: (f32, f32),
    height: f32,
    color: FColor,
) -> Result<(), sdl3::Error> {
    let cell = height / to_f32(GLYPH_ROWS);
    let (marks, columns) = label_cells(text);
    let left = middle.0 - to_f32(columns) * cell / 2.0;
    let top = middle.1 - height / 2.0;
    let cells: Vec<FRect> = marks
        .into_iter()
        .map(|(column, row)| {
            FRect::new(
                left + to_f32(column) * cell,
                top + to_f32(row) * cell,
                cell,
                cell,
            )
        })
        .collect();
    canvas.set_draw_color(color);
    canvas.fill_rects(&cells)
}

#[allow(clippy::cast_precision_loss)]
fn to_f32(value: usize) -> f32 {
    value as f32
}

#[allow(clippy::cast_precision_loss)]
fn to_f32_u32(value: u32) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingers_follow_the_window_and_lift() {
        let mut pad = TouchPad::new((240, 160));
        pad.fit((2400, 1080));
        let a = pad
            .layout
            .controls
            .iter()
            .find_map(|placed| match placed.shape {
                Shape::Circle { x, y, .. } if placed.control == Control::Button(Button::A) => {
                    Some((x / 2400.0, y / 1080.0))
                }
                _ => None,
            })
            .unwrap_or_default();
        pad.finger(7, Some(a));
        assert!(pad.input().is_held(Button::A));
        pad.start_events();
        pad.finger(7, None);
        assert_eq!(pad.input(), Input::default(), "a held finger lifts at once");
        pad.finger(9, Some(a));
        pad.finger(9, None);
        assert!(
            pad.input().is_held(Button::A),
            "a quick tap holds for a frame"
        );
        pad.start_events();
        assert_eq!(pad.input(), Input::default());
        pad.finger(8, Some(a));
        pad.release();
        assert_eq!(pad.input(), Input::default());
    }
}
