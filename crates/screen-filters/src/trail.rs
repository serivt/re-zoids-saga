//! The trail of the handheld's panel: its liquid crystals took longer
//! than a frame to change, so the picture before lingered under the new
//! one. Two kinds:
//!
//! - **Mix:** each picture shown is half its own and half the one the game
//!   drew before it; a picture the game alternates every other frame, as
//!   some effects do to look see-through, shows as the mix of the two.
//! - **Fade:** each picture shown is half its own and half the one shown
//!   before it, so what moves leaves a trail that fades over two or three
//!   frames (a half, a quarter, an eighth), as the original GBA's slow
//!   panel did.
//!
//! Source of knowledge: this project's own design.

use platform::{Frame, Rgb};

/// Which trail the pictures leave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrailMode {
    /// None: each picture shows as it is.
    #[default]
    Off,
    /// Each picture mixed with the one drawn before.
    Mix,
    /// Each picture mixed with the one shown before, a fading trail.
    Fade,
}

impl TrailMode {
    /// Every trail, in the options' order.
    pub const ALL: [Self; 3] = [Self::Off, Self::Mix, Self::Fade];

    /// The name the settings keep it under: `0`, `1` (the mix) or `fade`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Off => "0",
            Self::Mix => "1",
            Self::Fade => "fade",
        }
    }

    /// The trail the settings name `key`, if any.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.key() == key)
    }

    /// The next trail in the options' order, or the one before, round
    /// from the last to the first.
    #[must_use]
    pub fn step(self, forward: bool) -> Self {
        let count = Self::ALL.len();
        let at = Self::ALL.iter().position(|&mode| mode == self).unwrap_or(0);
        Self::ALL[if forward {
            (at + 1) % count
        } else {
            (at + count - 1) % count
        }]
    }
}

/// The picture to mix the next one with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trail {
    mode: TrailMode,
    previous: Vec<Rgb>,
}

impl Trail {
    /// A trail of `mode`, before any picture.
    #[must_use]
    pub const fn new(mode: TrailMode) -> Self {
        Self {
            mode,
            previous: Vec::new(),
        }
    }

    /// Shows `frame` mixed half and half with the picture before it (the
    /// one drawn for a mix, the one shown for a fade), and keeps what the
    /// next is mixed with; the first picture, or one of another size,
    /// shows as it is.
    pub fn apply(&mut self, frame: &mut Frame) {
        if self.mode == TrailMode::Off {
            return;
        }
        let fade = self.mode == TrailMode::Fade;
        let pixels = frame.pixels_mut();
        if self.previous.len() == pixels.len() {
            for (pixel, before) in pixels.iter_mut().zip(self.previous.iter_mut()) {
                let drawn = *pixel;
                *pixel = mix(drawn, *before);
                *before = if fade { *pixel } else { drawn };
            }
        } else {
            self.previous = pixels.to_vec();
        }
    }
}

/// The color halfway between `a` and `b`, rounded up.
fn mix(a: Rgb, b: Rgb) -> Rgb {
    let half =
        |x: u8, y: u8| u8::try_from((u16::from(x) + u16::from(y)).div_ceil(2)).unwrap_or(u8::MAX);
    Rgb::new(half(a.r, b.r), half(a.g, b.g), half(a.b, b.b))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: Rgb = Rgb::new(0, 0, 0);
    const WHITE: Rgb = Rgb::new(255, 255, 255);

    fn shown(trail: &mut Trail, color: Rgb) -> Rgb {
        let mut frame = Frame::new(2, 1, color);
        trail.apply(&mut frame);
        frame.pixel(1, 0).unwrap_or_default()
    }

    #[test]
    fn a_mix_takes_half_of_the_picture_drawn_before() {
        let mut trail = Trail::new(TrailMode::Mix);
        assert_eq!(shown(&mut trail, WHITE), WHITE, "the first shows as it is");
        assert_eq!(shown(&mut trail, BLACK), Rgb::new(128, 128, 128));
        assert_eq!(shown(&mut trail, BLACK), BLACK, "the mix is not kept");
        let mut other = Frame::new(3, 1, WHITE);
        trail.apply(&mut other);
        assert_eq!(other.pixel(2, 0), Some(WHITE), "a new size starts over");
    }

    #[test]
    fn a_fade_takes_half_of_the_picture_shown_before() {
        let mut trail = Trail::new(TrailMode::Fade);
        shown(&mut trail, WHITE);
        let levels: Vec<u8> = (0..3).map(|_| shown(&mut trail, BLACK).r).collect();
        assert_eq!(levels, [128, 64, 32]);
    }

    #[test]
    fn off_leaves_the_picture_and_the_trails_go_round() {
        let mut trail = Trail::new(TrailMode::Off);
        shown(&mut trail, WHITE);
        assert_eq!(shown(&mut trail, BLACK), BLACK);
        assert_eq!(TrailMode::Off.step(true), TrailMode::Mix);
        assert_eq!(TrailMode::Off.step(false), TrailMode::Fade);
        for mode in TrailMode::ALL {
            assert_eq!(TrailMode::from_key(mode.key()), Some(mode));
        }
    }
}
