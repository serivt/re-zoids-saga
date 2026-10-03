//! Pixel-art magnification: the picture made two or three times larger
//! with its diagonal edges rounded off and nothing blurred, before the
//! backend fits it to the window. Each new pixel copies the original
//! pixel or one of its neighbors, chosen by which of the neighbors are
//! equal, so flat areas and straight edges stay as drawn while a stair of
//! single pixels becomes a smoother line.
//!
//! Source of knowledge: the Scale2x and Scale3x algorithms (Andrea
//! Mazzoleni, of the `AdvanceMAME` project), written here from the public
//! description of their rules, not from any implementation.

use platform::{Frame, Rgb};

/// How the picture is magnified before it is fitted to the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Upscaler {
    /// Not at all: the backend scales the game's own pixels.
    #[default]
    None,
    /// Scale2x: twice as large.
    Scale2x,
    /// Scale3x: three times as large.
    Scale3x,
}

impl Upscaler {
    /// Every upscaler, in the options' order.
    pub const ALL: [Self; 3] = [Self::None, Self::Scale2x, Self::Scale3x];

    /// The name the settings keep it under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Scale2x => "scale2x",
            Self::Scale3x => "scale3x",
        }
    }

    /// The upscaler the settings name `key`, if any.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|upscaler| upscaler.key() == key)
    }

    /// The next upscaler in the options' order, or the one before, round
    /// from the last to the first.
    #[must_use]
    pub fn step(self, forward: bool) -> Self {
        let count = Self::ALL.len();
        let at = Self::ALL
            .iter()
            .position(|&upscaler| upscaler == self)
            .unwrap_or(0);
        Self::ALL[if forward {
            (at + 1) % count
        } else {
            (at + count - 1) % count
        }]
    }

    /// How many times larger the picture comes out.
    #[must_use]
    pub const fn factor(self) -> usize {
        match self {
            Self::None => 1,
            Self::Scale2x => 2,
            Self::Scale3x => 3,
        }
    }

    /// `frame` magnified, or `None` when the upscaler leaves it as it is.
    #[must_use]
    pub fn scale(self, frame: &Frame) -> Option<Frame> {
        match self {
            Self::None => None,
            Self::Scale2x => Some(magnify(frame, 2, scale2x)),
            Self::Scale3x => Some(magnify(frame, 3, scale3x)),
        }
    }
}

/// A pixel's neighborhood, row by row: `a b c`, `d e f`, `g h i`, with
/// `e` the pixel itself; past the frame's edge a pixel stands for its
/// missing neighbor.
#[derive(Debug, Clone, Copy)]
struct Around {
    a: Rgb,
    b: Rgb,
    c: Rgb,
    d: Rgb,
    e: Rgb,
    f: Rgb,
    g: Rgb,
    h: Rgb,
    i: Rgb,
}

/// `frame` made `factor` times larger, each pixel's block of
/// `factor × factor` written row by row by `block`.
fn magnify(frame: &Frame, factor: usize, block: fn(&Around) -> [Rgb; 9]) -> Frame {
    let (width, height) = (frame.width(), frame.height());
    let pixels = frame.pixels();
    let at = |x: usize, y: usize| pixels[y * width + x];
    let mut out = Frame::new(width * factor, height * factor, Rgb::default());
    let out_width = width * factor;
    let target = out.pixels_mut();
    for y in 0..height {
        let (up, down) = (y.saturating_sub(1), (y + 1).min(height - 1));
        for x in 0..width {
            let (left, right) = (x.saturating_sub(1), (x + 1).min(width - 1));
            let around = Around {
                a: at(left, up),
                b: at(x, up),
                c: at(right, up),
                d: at(left, y),
                e: at(x, y),
                f: at(right, y),
                g: at(left, down),
                h: at(x, down),
                i: at(right, down),
            };
            let colors = block(&around);
            for row in 0..factor {
                let start = (y * factor + row) * out_width + x * factor;
                target[start..start + factor]
                    .copy_from_slice(&colors[row * factor..row * factor + factor]);
            }
        }
    }
    out
}

/// Scale2x's four pixels for `p`, in the first four places: a corner takes
/// the neighbors' color where the two neighbors beside it are equal and
/// the other two differ from them.
fn scale2x(p: &Around) -> [Rgb; 9] {
    let (up, left, right, down, own) = (p.b, p.d, p.f, p.h, p.e);
    let blocked = up == down || left == right;
    let pick = |near: Rgb, other: Rgb| if !blocked && near == other { near } else { own };
    [
        pick(left, up),
        pick(up, right),
        pick(left, down),
        pick(down, right),
        own,
        own,
        own,
        own,
        own,
    ]
}

/// Scale3x's nine pixels for `e`: the corners as Scale2x's, and each side
/// where a corner beside it changes direction against the pixel.
fn scale3x(p: &Around) -> [Rgb; 9] {
    let Around {
        a,
        b,
        c,
        d,
        e,
        f,
        g,
        h,
        i,
    } = *p;
    if b == h || d == f {
        return [e; 9];
    }
    let top_left = d == b;
    let top_right = b == f;
    let bottom_left = d == h;
    let bottom_right = h == f;
    [
        if top_left { d } else { e },
        if (top_left && e != c) || (top_right && e != a) {
            b
        } else {
            e
        },
        if top_right { f } else { e },
        if (top_left && e != g) || (bottom_left && e != a) {
            d
        } else {
            e
        },
        e,
        if (top_right && e != i) || (bottom_right && e != c) {
            f
        } else {
            e
        },
        if bottom_left { d } else { e },
        if (bottom_left && e != i) || (bottom_right && e != g) {
            h
        } else {
            e
        },
        if bottom_right { f } else { e },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const INK: Rgb = Rgb::new(255, 255, 255);
    const PAPER: Rgb = Rgb::new(0, 0, 0);

    /// A frame from rows of `#` (ink) and `.` (paper).
    fn frame(rows: &[&str]) -> Frame {
        let mut frame = Frame::new(rows[0].len(), rows.len(), PAPER);
        for (y, row) in rows.iter().enumerate() {
            for (x, cell) in row.chars().enumerate() {
                if cell == '#' {
                    frame.set_pixel(x, y, INK);
                }
            }
        }
        frame
    }

    fn rows(frame: &Frame) -> Vec<String> {
        (0..frame.height())
            .map(|y| {
                (0..frame.width())
                    .map(|x| {
                        if frame.pixel(x, y) == Some(INK) {
                            '#'
                        } else {
                            '.'
                        }
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn scale2x_rounds_a_diagonal_and_keeps_flat_areas() {
        let stair = frame(&["#..", ".#.", "..#"]);
        let scaled = Upscaler::Scale2x
            .scale(&stair)
            .unwrap_or_else(|| stair.clone());
        assert_eq!(
            rows(&scaled),
            ["##....", "#.#...", ".###..", "..###.", "...#.#", "....##"]
        );
        let flat = frame(&["##", "##"]);
        assert_eq!(
            rows(
                &Upscaler::Scale2x
                    .scale(&flat)
                    .unwrap_or_else(|| flat.clone())
            ),
            vec!["####"; 4]
        );
    }

    #[test]
    fn scale3x_fills_the_corners_of_a_diagonal() {
        let stair = frame(&["#..", ".#.", "..#"]);
        let scaled = Upscaler::Scale3x
            .scale(&stair)
            .unwrap_or_else(|| stair.clone());
        assert_eq!((scaled.width(), scaled.height()), (9, 9));
        assert_eq!(
            rows(&scaled),
            [
                "###......",
                "##.#.....",
                "#..#.....",
                ".#####...",
                "...###...",
                "...#####.",
                ".....#..#",
                ".....#.##",
                "......###",
            ]
        );
        let single = frame(&["...", ".#.", "..."]);
        let dot = rows(
            &Upscaler::Scale3x
                .scale(&single)
                .unwrap_or_else(|| single.clone()),
        );
        assert_eq!(&dot[3..6], ["...###...", "...###...", "...###..."]);
    }

    #[test]
    fn none_leaves_the_frame_and_the_upscalers_go_round() {
        assert!(Upscaler::None.scale(&frame(&["#"])).is_none());
        assert_eq!(Upscaler::Scale3x.factor(), 3);
        assert_eq!(Upscaler::None.step(true), Upscaler::Scale2x);
        assert_eq!(Upscaler::None.step(false), Upscaler::Scale3x);
        for upscaler in Upscaler::ALL {
            assert_eq!(Upscaler::from_key(upscaler.key()), Some(upscaler));
        }
    }
}
