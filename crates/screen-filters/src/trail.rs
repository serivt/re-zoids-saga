//! The trail of the handheld's panel: its liquid crystals took longer
//! than a frame to change, so the picture before lingered under the new
//! one. Each picture shown here is half its own and half the one the game
//! drew before it; a picture the game alternates every other frame, as
//! some effects do to look see-through, shows as the mix of the two.
//!
//! Source of knowledge: this project's own design.

use platform::{Frame, Rgb};

/// The picture the game drew last, to mix the next one with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trail {
    previous: Vec<Rgb>,
}

impl Trail {
    /// Shows `frame` mixed half and half with the picture before it, and
    /// keeps it for the next; the first picture, or one of another size,
    /// shows as it is.
    pub fn apply(&mut self, frame: &mut Frame) {
        let pixels = frame.pixels_mut();
        if self.previous.len() == pixels.len() {
            for (pixel, before) in pixels.iter_mut().zip(self.previous.iter_mut()) {
                let drawn = *pixel;
                *pixel = mix(drawn, *before);
                *before = drawn;
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

    #[test]
    fn each_picture_mixes_with_the_one_drawn_before() {
        let mut trail = Trail::default();
        let black = Rgb::new(0, 0, 0);
        let white = Rgb::new(255, 255, 255);
        let mut frame = Frame::new(2, 1, white);
        trail.apply(&mut frame);
        assert_eq!(frame.pixel(0, 0), Some(white), "the first shows as it is");
        let mut frame = Frame::new(2, 1, black);
        trail.apply(&mut frame);
        assert_eq!(frame.pixel(1, 0), Some(Rgb::new(128, 128, 128)));
        let mut frame = Frame::new(2, 1, black);
        trail.apply(&mut frame);
        assert_eq!(
            frame.pixel(0, 0),
            Some(black),
            "the mix is not kept, the drawn is"
        );
        let mut other = Frame::new(3, 1, white);
        trail.apply(&mut other);
        assert_eq!(other.pixel(2, 0), Some(white), "a new size starts over");
    }
}
