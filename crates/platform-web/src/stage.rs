//! The game's screen in the page and the on-screen pad for touch screens.
//!
//! Without the pad the screen is centered in the window, as large as it
//! fits, in whole multiples for the sharp scaling. With it, the screen and
//! the controls sit where [`TouchLayout`] puts them, as on Android: the
//! screen in the middle and the controls on its sides when the window is
//! wider than tall, the screen on top and the controls below it otherwise.
//! The controls are drawn on a canvas over the whole window in the style
//! of [`platform::touch`], and the fingers come from the page's pointer
//! events, so several press at once.
//!
//! The pad shows on touch screens (a coarse pointer, or once the screen is
//! touched) and hides while a gamepad is connected; the page can also
//! always show it or never. The page's own menu button, if it has one, is
//! kept at the screen's top left corner, wherever the screen goes.
//!
//! An LCD's grid or a television's scan lines, when chosen, are drawn on a
//! canvas of their own over the screen, at the device's pixels, which the
//! page's style multiplies with the picture (white leaves it, the lines'
//! gray darkens it); the screen then takes whole multiples, as the
//! launcher's.

use std::cell::{Cell, RefCell};
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::rc::Rc;

use platform::touch::{
    ARM_LENGTH, ARM_THICKNESS, Area, BAR_LETTER_SCALE, Control, Fingers, GLYPH_ROWS, HELD_OPACITY,
    IDLE_OPACITY, LETTER_HEIGHT, MARK_OPACITY, Shape, label, label_cells,
};
use platform::{Button, Input, PlatformError, TouchLayout};
use screen_filters::Grid;
use screen_filters::grid::Rect;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{
    CanvasRenderingContext2d, Event, HtmlCanvasElement, HtmlElement, PointerEvent, Window,
};

use crate::web_error;

/// How far inside the screen's corner the page's menu button sits, in CSS
/// pixels.
const MENU_INSET: f32 = 8.0;

/// How the screen fills the window when the pad is not shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scaling {
    /// Sharp, in whole multiples of the screen when the window holds one.
    #[default]
    Sharp,
    /// Sharp, filling the window.
    Fill,
    /// Smooth, filling the window.
    Smooth,
}

impl Scaling {
    /// The scaling the page's settings name `key`, if any.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "sharp" => Some(Self::Sharp),
            "fill" => Some(Self::Fill),
            "smooth" => Some(Self::Smooth),
            _ => None,
        }
    }
}

/// When the on-screen pad shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PadMode {
    /// On touch screens, while no gamepad is connected.
    #[default]
    Auto,
    /// Always, as for trying it with a mouse.
    Always,
    /// Never.
    Never,
}

impl PadMode {
    /// The mode the page's settings name `key`, if any.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "auto" => Some(Self::Auto),
            "on" => Some(Self::Always),
            "off" => Some(Self::Never),
            _ => None,
        }
    }

    /// Whether the pad shows, on a touch screen or not (`touch`), with a
    /// gamepad connected or not.
    #[must_use]
    pub const fn shows(self, touch: bool, gamepad: bool) -> bool {
        match self {
            Self::Auto => touch && !gamepad,
            Self::Always => true,
            Self::Never => false,
        }
    }
}

/// How the pad shows and what it holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PadStyle {
    /// When it shows.
    pub mode: PadMode,
    /// Its controls' size against their usual one, within
    /// [`platform::touch::SIZES`].
    pub size: f32,
    /// Its controls' opacity against their usual one, from 0 to 1.
    pub opacity: f32,
    /// Whether it has the fast forward (the enhanced mode's alone).
    pub fast_forward: bool,
}

impl Default for PadStyle {
    fn default() -> Self {
        Self {
            mode: PadMode::Auto,
            size: 1.0,
            opacity: 1.0,
            fast_forward: false,
        }
    }
}

/// Where a screen of `frame` (width, height) goes in a window of `window`
/// without the pad: centered, as large as it fits keeping its proportions,
/// in whole multiples when `whole` and the window holds one.
#[must_use]
pub fn fit_screen(window: (f32, f32), frame: (f32, f32), whole: bool) -> Area {
    let scale = (window.0 / frame.0).min(window.1 / frame.1);
    let scale = if whole && scale >= 1.0 {
        scale.floor()
    } else {
        scale
    };
    let (width, height) = (frame.0 * scale, frame.1 * scale);
    Area {
        x: ((window.0 - width) / 2.0).floor(),
        y: ((window.1 - height) / 2.0).floor(),
        width,
        height,
    }
}

/// The page's elements the game plays in.
pub struct StageElements {
    /// The element over the whole window holding the others, which takes
    /// the fingers.
    pub stage: HtmlElement,
    /// The game's screen.
    pub screen: HtmlCanvasElement,
    /// The canvas the pad is drawn on, over the whole window.
    pub pad: HtmlCanvasElement,
    /// The page's menu button, kept at the screen's top left corner.
    pub menu: Option<HtmlElement>,
    /// The canvas the grid is drawn on, over the screen.
    pub grid: Option<HtmlCanvasElement>,
}

/// The window the screen and the pad were laid out for.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Laid {
    width: f64,
    height: f64,
    density: f64,
    shown: bool,
}

/// The game's screen and the pad in the page.
pub struct WebStage {
    window: Window,
    stage: HtmlElement,
    screen: HtmlCanvasElement,
    pad: HtmlCanvasElement,
    menu: Option<HtmlElement>,
    context: CanvasRenderingContext2d,
    /// The grid's canvas and its context, and the grid drawn, if any.
    lines: Option<(HtmlCanvasElement, CanvasRenderingContext2d)>,
    grid: Option<Grid>,
    fingers: Rc<RefCell<Fingers>>,
    /// Whether the screen is a touch screen: a coarse pointer, or a touch
    /// seen.
    touch: Rc<Cell<bool>>,
    scaling: Scaling,
    style: PadStyle,
    frame: (f32, f32),
    pixels: (usize, usize),
    /// The controls while the pad shows.
    layout: Option<TouchLayout>,
    laid: Option<Laid>,
    /// The buttons the pad was last drawn holding.
    drawn: Option<Input>,
}

impl WebStage {
    /// Places the game's screen and the page's menu button among the
    /// `elements`, for frames of `frame` (width, height); listens to the
    /// fingers on the stage.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the pad's canvas has no 2D context or
    /// the listeners cannot be added.
    pub fn new(
        window: Window,
        elements: StageElements,
        frame: (usize, usize),
        scaling: Scaling,
        style: PadStyle,
    ) -> Result<Self, PlatformError> {
        let StageElements {
            stage,
            screen,
            pad,
            menu,
            grid,
        } = elements;
        let context = context_2d(&pad)?;
        let lines = match grid {
            Some(canvas) => {
                let context = context_2d(&canvas)?;
                Some((canvas, context))
            }
            None => None,
        };
        let coarse = window
            .match_media("(pointer: coarse)")
            .ok()
            .flatten()
            .is_some_and(|query| query.matches());
        let fingers = Rc::new(RefCell::new(Fingers::default()));
        let touch = Rc::new(Cell::new(coarse));
        listen(&stage, &fingers, &touch)?;
        Ok(Self {
            window,
            stage,
            screen,
            pad,
            menu,
            context,
            lines,
            grid: None,
            fingers,
            touch,
            scaling,
            style,
            frame: (to_f32(frame.0), to_f32(frame.1)),
            pixels: frame,
            layout: None,
            laid: None,
            drawn: None,
        })
    }

    /// Fills the window as `scaling` asks while the pad is hidden.
    pub fn set_scaling(&mut self, scaling: Scaling) {
        self.scaling = scaling;
        self.laid = None;
    }

    /// Draws `grid` over the screen, or none.
    pub fn set_grid(&mut self, grid: Option<Grid>) {
        self.grid = grid;
        self.laid = None;
    }

    /// Shows the pad as `mode` asks.
    pub fn set_mode(&mut self, mode: PadMode) {
        self.style.mode = mode;
        self.laid = None;
    }

    /// Makes the pad `opacity` (0 to 1) as opaque as usual.
    pub fn set_opacity(&mut self, opacity: f32) {
        self.style.opacity = opacity.clamp(0.0, 1.0);
        self.drawn = None;
    }

    /// Lays the screen and the pad out again when the window, its pixel
    /// density or whether the pad shows (with a `gamepad` connected or
    /// not) has changed since.
    pub fn fit(&mut self, gamepad: bool) {
        let bounds = self.stage.get_bounding_client_rect();
        let laid = Laid {
            width: bounds.width(),
            height: bounds.height(),
            density: self.window.device_pixel_ratio().max(1.0),
            shown: self.style.mode.shows(self.touch.get(), gamepad),
        };
        if self.laid == Some(laid) {
            return;
        }
        self.laid = Some(laid);
        self.drawn = None;
        let window = (to_f32_f64(laid.width), to_f32_f64(laid.height));
        let area = if laid.shown {
            let mut layout = TouchLayout::sized(window, self.frame, self.style.size);
            if !self.style.fast_forward {
                layout
                    .controls
                    .retain(|placed| placed.control != Control::Button(Button::FastForward));
            }
            let screen = layout.screen;
            self.layout = Some(layout);
            screen
        } else {
            self.layout = None;
            self.fingers.borrow_mut().release();
            let whole = self.scaling == Scaling::Sharp || self.grid.is_some();
            fit_screen(window, self.frame, whole)
        };
        place(&self.screen, area);
        self.draw_grid(area, laid.density);
        if let Some(menu) = &self.menu {
            let style = menu.style();
            let _ = style.set_property("left", &format!("{}px", area.x + MENU_INSET));
            let _ = style.set_property("top", &format!("{}px", area.y + MENU_INSET));
        }
        self.pad.set_hidden(!laid.shown);
        if laid.shown {
            self.pad.set_width(pixels(laid.width * laid.density));
            self.pad.set_height(pixels(laid.height * laid.density));
        }
    }

    /// Draws the grid over the screen at `area`, the device having
    /// `density` pixels for each of the page's, or hides its canvas.
    fn draw_grid(&self, area: Area, density: f64) {
        let Some((canvas, context)) = &self.lines else {
            return;
        };
        let Some(grid) = self.grid else {
            canvas.set_hidden(true);
            return;
        };
        canvas.set_hidden(false);
        place(canvas, area);
        let (width, height) = (f64::from(area.width), f64::from(area.height));
        canvas.set_width(pixels(width * density));
        canvas.set_height(pixels(height * density));
        let _ = context.set_transform(density, 0.0, 0.0, density, 0.0, 0.0);
        context.set_fill_style_str("#fff");
        context.fill_rect(0.0, 0.0, width, height);
        let shade = grid.shade;
        context.set_fill_style_str(&format!("rgb({shade}, {shade}, {shade})"));
        let whole = Rect {
            x: 0.0,
            y: 0.0,
            width: area.width,
            height: area.height,
        };
        for line in grid.lines(whole, self.pixels, to_f32_f64(density)) {
            context.fill_rect(
                f64::from(line.x),
                f64::from(line.y),
                f64::from(line.width),
                f64::from(line.height),
            );
        }
    }

    /// The buttons the fingers on the pad hold, and those of the taps since
    /// the last reading; nothing while the pad is hidden.
    pub fn input(&mut self) -> Input {
        let mut fingers = self.fingers.borrow_mut();
        let input = self
            .layout
            .as_ref()
            .map(|layout| layout.input(&fingers.positions()))
            .unwrap_or_default();
        fingers.read();
        input
    }

    /// Draws the pad, the controls `held` brighter, when it shows and has
    /// changed since it was last drawn.
    pub fn draw(&mut self, held: Input) {
        let (Some(layout), Some(laid)) = (&self.layout, self.laid) else {
            return;
        };
        if self.drawn == Some(held) {
            return;
        }
        self.drawn = Some(held);
        let context = &self.context;
        let _ = context.set_transform(laid.density, 0.0, 0.0, laid.density, 0.0, 0.0);
        context.clear_rect(0.0, 0.0, laid.width, laid.height);
        let opacity = f64::from(self.style.opacity.clamp(0.0, 1.0));
        let paint = Paint {
            idle: white(IDLE_OPACITY, opacity),
            held: white(HELD_OPACITY, opacity),
            mark: white(MARK_OPACITY, opacity),
        };
        for placed in &layout.controls {
            match (placed.control, placed.shape) {
                (Control::Cross, Shape::Circle { x, y, radius }) => {
                    draw_cross(context, (x, y), radius, held, &paint);
                }
                (Control::Button(button), Shape::Circle { x, y, radius }) => {
                    context.set_fill_style_str(paint.of(held.is_held(button)));
                    disc(context, (x, y), radius);
                    let height = radius * 2.0 * LETTER_HEIGHT;
                    letters(context, label(button), (x, y), height, &paint.mark);
                }
                (Control::Button(button), Shape::Pill(area)) => {
                    context.set_fill_style_str(paint.of(held.is_held(button)));
                    pill(context, area);
                    let middle = (area.x + area.width / 2.0, area.y + area.height / 2.0);
                    let height = area.height * LETTER_HEIGHT * BAR_LETTER_SCALE;
                    letters(context, label(button), middle, height, &paint.mark);
                }
                (Control::Cross, Shape::Pill(_)) => {}
            }
        }
    }
}

/// Keeps the fingers on `stage` in `fingers`, in the window's CSS pixels,
/// and notes in `touch` a touch seen. The page does nothing else with them
/// (no scrolling, no zooming, no menu).
fn listen(
    stage: &HtmlElement,
    fingers: &Rc<RefCell<Fingers>>,
    touch: &Rc<Cell<bool>>,
) -> Result<(), PlatformError> {
    for kind in ["pointerdown", "pointermove", "pointerup", "pointercancel"] {
        let fingers = Rc::clone(fingers);
        let touch = Rc::clone(touch);
        let target = stage.clone();
        let listener = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            event.prevent_default();
            let id = u64::from(event.pointer_id().unsigned_abs());
            let at = (to_f32_i32(event.client_x()), to_f32_i32(event.client_y()));
            match kind {
                "pointerdown" => {
                    if event.pointer_type() == "touch" {
                        touch.set(true);
                    }
                    let _ = target.set_pointer_capture(event.pointer_id());
                    fingers.borrow_mut().set(id, Some(at));
                }
                "pointermove" if event.buttons() != 0 => fingers.borrow_mut().set(id, Some(at)),
                "pointermove" => {}
                _ => fingers.borrow_mut().set(id, None),
            }
        });
        stage
            .add_event_listener_with_callback(kind, listener.as_ref().unchecked_ref())
            .map_err(web_error)?;
        listener.forget();
    }
    let menu = Closure::<dyn FnMut(Event)>::new(|event: Event| event.prevent_default());
    stage
        .add_event_listener_with_callback("contextmenu", menu.as_ref().unchecked_ref())
        .map_err(web_error)?;
    menu.forget();
    Ok(())
}

/// `canvas`'s 2D context.
fn context_2d(canvas: &HtmlCanvasElement) -> Result<CanvasRenderingContext2d, PlatformError> {
    canvas
        .get_context("2d")
        .map_err(web_error)?
        .ok_or_else(|| web_error("no 2D context"))?
        .dyn_into::<CanvasRenderingContext2d>()
        .map_err(web_error)
}

/// Puts `canvas` over `area` of the window.
fn place(canvas: &HtmlCanvasElement, area: Area) {
    let style = canvas.style();
    for (property, value) in [
        ("left", area.x),
        ("top", area.y),
        ("width", area.width),
        ("height", area.height),
    ] {
        let _ = style.set_property(property, &format!("{value}px"));
    }
}

/// The colors of the controls.
struct Paint {
    idle: String,
    held: String,
    mark: String,
}

impl Paint {
    fn of(&self, held: bool) -> &str {
        if held { &self.held } else { &self.idle }
    }
}

/// White at `alpha` times `opacity`, as the canvas reads colors.
fn white(alpha: f32, opacity: f64) -> String {
    format!("rgba(255, 255, 255, {:.3})", f64::from(alpha) * opacity)
}

/// The cross: a faint disc, and its four arms, each brighter while its
/// direction is held.
fn draw_cross(
    context: &CanvasRenderingContext2d,
    (x, y): (f32, f32),
    radius: f32,
    held: Input,
    paint: &Paint,
) {
    context.set_fill_style_str(&paint.idle);
    disc(context, (x, y), radius);
    let (x, y) = (f64::from(x), f64::from(y));
    let length = f64::from(radius * ARM_LENGTH);
    let thickness = f64::from(radius * ARM_THICKNESS);
    let half = thickness / 2.0;
    context.fill_rect(x - half, y - half, thickness, thickness);
    for (button, (left, top, width, height)) in [
        (Button::Up, (x - half, y - length, thickness, length - half)),
        (Button::Down, (x - half, y + half, thickness, length - half)),
        (
            Button::Left,
            (x - length, y - half, length - half, thickness),
        ),
        (
            Button::Right,
            (x + half, y - half, length - half, thickness),
        ),
    ] {
        context.set_fill_style_str(paint.of(held.is_held(button)));
        context.fill_rect(left, top, width, height);
    }
}

/// A filled circle in the current color.
fn disc(context: &CanvasRenderingContext2d, (x, y): (f32, f32), radius: f32) {
    context.begin_path();
    let _ = context.arc(f64::from(x), f64::from(y), f64::from(radius), 0.0, TAU);
    context.fill();
}

/// A filled bar with round ends over `area`, in the current color.
fn pill(context: &CanvasRenderingContext2d, area: Area) {
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
    context.fill();
}

/// `text` centered on `middle`, `height` pixels tall, in `color`.
fn letters(
    context: &CanvasRenderingContext2d,
    text: &str,
    middle: (f32, f32),
    height: f32,
    color: &str,
) {
    let (cells, columns) = label_cells(text);
    let cell = f64::from(height) / to_f64(GLYPH_ROWS);
    let left = f64::from(middle.0) - to_f64(columns) * cell / 2.0;
    let top = f64::from(middle.1 - height / 2.0);
    context.set_fill_style_str(color);
    for (column, row) in cells {
        context.fill_rect(
            left + to_f64(column) * cell,
            top + to_f64(row) * cell,
            cell,
            cell,
        );
    }
}

/// A canvas's size in device pixels for `value`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pixels(value: f64) -> u32 {
    value.round().clamp(1.0, f64::from(u32::MAX)) as u32
}

#[allow(clippy::cast_precision_loss)]
fn to_f32(value: usize) -> f32 {
    value as f32
}

#[allow(clippy::cast_precision_loss)]
fn to_f64(value: usize) -> f64 {
    value as f64
}

#[allow(clippy::cast_precision_loss)]
fn to_f32_i32(value: i32) -> f32 {
    value as f32
}

#[allow(clippy::cast_possible_truncation)]
fn to_f32_f64(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: (f32, f32) = (240.0, 160.0);

    #[test]
    fn the_screen_takes_whole_multiples_when_sharp_and_the_window_otherwise() {
        let sharp = fit_screen((1000.0, 700.0), FRAME, true);
        let near = |value: f32, expected: f32| (value - expected).abs() < 0.01;
        assert!(near(sharp.x, 20.0) && near(sharp.y, 30.0));
        assert!(near(sharp.width, 960.0) && near(sharp.height, 640.0));
        let fill = fit_screen((1000.0, 700.0), FRAME, false);
        assert!((fill.width - 1000.0).abs() < 0.01);
        assert!((fill.height - 1000.0 / 1.5).abs() < 0.01);
        let small = fit_screen((120.0, 100.0), FRAME, true);
        assert!((small.width - 120.0).abs() < 0.01, "below one, it fits");
    }

    #[test]
    fn the_pad_shows_on_touch_screens_without_a_gamepad_unless_chosen() {
        assert!(PadMode::Auto.shows(true, false));
        assert!(!PadMode::Auto.shows(true, true));
        assert!(!PadMode::Auto.shows(false, false));
        assert!(PadMode::Always.shows(false, true));
        assert!(!PadMode::Never.shows(true, false));
        assert_eq!(PadMode::from_key("on"), Some(PadMode::Always));
        assert_eq!(Scaling::from_key("fill"), Some(Scaling::Fill));
        assert_eq!(Scaling::from_key("other"), None);
        assert!(platform::touch::SIZES.contains(&PadStyle::default().size));
    }

    #[test]
    fn colors_fade_with_the_opacity() {
        assert_eq!(white(0.5, 1.0), "rgba(255, 255, 255, 0.500)");
        assert_eq!(white(0.5, 0.5), "rgba(255, 255, 255, 0.250)");
    }
}
