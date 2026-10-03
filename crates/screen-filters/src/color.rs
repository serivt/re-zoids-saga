//! The colors of the handheld's panels: the Game Boy Advance's own screen
//! showed the game darker and paler than a modern display does, and its
//! games were drawn bright to make up for it. A profile turns the
//! picture's colors into what such a panel showed.
//!
//! Each profile is a model of a panel: the GBA's colors are 5 bits a
//! channel, which the panel turns into light along a steeper curve than a
//! modern display's (its gamma); the channels bleed into each other (a
//! mix that keeps greys grey); the panel is dimmer than white and its
//! black lets some light through. The light is then written back for a
//! display of gamma 2.2. The picture's colors are the GBA's own (each
//! channel `c × 8 + c / 4`), so a table of the 32,768 colors does the
//! whole work, built once when the profile is chosen.
//!
//! Source of knowledge: this project's own design; the profiles' values
//! were chosen by eye against the panels' well-known look (the original
//! GBA's unlit reflective panel, darker and pale; the GBA SP's lit one,
//! closer to a modern screen), not taken from any other emulator.

use platform::{Frame, Rgb};

/// The colors a channel of 5 bits holds.
const LEVELS: usize = 32;
/// The colors the table covers, 5 bits a channel.
const COLORS: usize = LEVELS * LEVELS * LEVELS;
const LEVEL_MAX: f32 = 31.0;
/// The bits a channel of the picture keeps beyond the GBA's own.
const EXTRA_BITS: u32 = 3;
/// The gamma a modern display shows its colors with.
const DISPLAY_GAMMA: f32 = 2.2;
const CHANNEL_MAX: f32 = 255.0;

/// The colors the picture is shown with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorProfile {
    /// The colors as the game draws them, untouched.
    #[default]
    Original,
    /// The original Game Boy Advance's unlit panel: darker, paler.
    Gba,
    /// The Game Boy Advance SP's lit panel: a little paler than the
    /// original colors.
    GbaSp,
}

impl ColorProfile {
    /// Every profile, in the options' order.
    pub const ALL: [Self; 3] = [Self::Original, Self::Gba, Self::GbaSp];

    /// The name the settings keep it under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Gba => "gba",
            Self::GbaSp => "gba-sp",
        }
    }

    /// The profile the settings name `key`, if any.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|profile| profile.key() == key)
    }

    /// The next profile in the options' order, or the one before, round
    /// from the last to the first.
    #[must_use]
    pub fn step(self, forward: bool) -> Self {
        let count = Self::ALL.len();
        let at = Self::ALL
            .iter()
            .position(|&profile| profile == self)
            .unwrap_or(0);
        let next = if forward {
            (at + 1) % count
        } else {
            (at + count - 1) % count
        };
        Self::ALL[next]
    }

    /// The panel the profile models; `None` for the original colors.
    const fn panel(self) -> Option<Panel> {
        match self {
            Self::Original => None,
            Self::Gba => Some(Panel {
                gamma: 2.8,
                brightness: 0.93,
                black: 0.004,
                mix: [[0.80, 0.14, 0.06], [0.10, 0.80, 0.10], [0.06, 0.14, 0.80]],
            }),
            Self::GbaSp => Some(Panel {
                gamma: 2.4,
                brightness: 0.98,
                black: 0.002,
                mix: [[0.90, 0.07, 0.03], [0.05, 0.90, 0.05], [0.03, 0.07, 0.90]],
            }),
        }
    }
}

/// A panel's model.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Panel {
    /// The curve from a channel's level to its light.
    gamma: f32,
    /// The light of white, against a modern display's.
    brightness: f32,
    /// The light of black.
    black: f32,
    /// The light of each channel, out of the three's (rows sum to 1).
    mix: [[f32; 3]; 3],
}

impl Panel {
    /// The color the panel shows for the GBA color `levels` (0–31 each).
    fn show(&self, levels: [usize; 3]) -> Rgb {
        let light = levels.map(|level| (f32_of(level) / LEVEL_MAX).powf(self.gamma));
        let [r, g, b] = self.mix.map(|row| {
            let mixed = row[0] * light[0] + row[1] * light[1] + row[2] * light[2];
            let shown = self.black + (1.0 - self.black) * self.brightness * mixed;
            channel(shown.clamp(0.0, 1.0).powf(1.0 / DISPLAY_GAMMA))
        });
        Rgb::new(r, g, b)
    }
}

/// A profile ready to use: the color each GBA color is shown with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorCorrection {
    table: Option<Vec<Rgb>>,
}

impl ColorCorrection {
    /// Builds the table of `profile`.
    #[must_use]
    pub fn new(profile: ColorProfile) -> Self {
        let table = profile.panel().map(|panel| {
            (0..COLORS)
                .map(|index| {
                    panel.show([
                        index / (LEVELS * LEVELS),
                        index / LEVELS % LEVELS,
                        index % LEVELS,
                    ])
                })
                .collect()
        });
        Self { table }
    }

    /// Shows `frame` with the profile's colors.
    pub fn apply(&self, frame: &mut Frame) {
        let Some(table) = &self.table else {
            return;
        };
        for pixel in frame.pixels_mut() {
            *pixel = table[index(*pixel)];
        }
    }
}

/// The table's entry of `color`: its GBA levels, 5 bits each.
fn index(color: Rgb) -> usize {
    let level = |channel: u8| usize::from(channel >> EXTRA_BITS);
    (level(color.r) * LEVELS + level(color.g)) * LEVELS + level(color.b)
}

/// A channel of light 0–1 as a byte.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn channel(value: f32) -> u8 {
    (value * CHANNEL_MAX).round().clamp(0.0, CHANNEL_MAX) as u8
}

#[allow(clippy::cast_precision_loss)]
fn f32_of(value: usize) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The picture's color of GBA levels `r`, `g`, `b`.
    fn gba(r: u8, g: u8, b: u8) -> Rgb {
        let expand = |level: u8| level << EXTRA_BITS | level >> 2;
        Rgb::new(expand(r), expand(g), expand(b))
    }

    fn shown(profile: ColorProfile, color: Rgb) -> Rgb {
        let mut frame = Frame::new(1, 1, color);
        ColorCorrection::new(profile).apply(&mut frame);
        frame.pixel(0, 0).unwrap_or_default()
    }

    fn spread(color: Rgb) -> u8 {
        color.r.max(color.g).max(color.b) - color.r.min(color.g).min(color.b)
    }

    #[test]
    fn the_original_colors_are_left_alone() {
        let color = gba(31, 7, 12);
        assert_eq!(shown(ColorProfile::Original, color), color);
    }

    #[test]
    fn the_panels_show_greys_grey_and_darker_and_colors_paler() {
        for profile in [ColorProfile::Gba, ColorProfile::GbaSp] {
            let grey = shown(profile, gba(16, 16, 16));
            assert_eq!((grey.r, grey.g), (grey.g, grey.b), "{profile:?}");
            assert!(grey.r < gba(16, 16, 16).r, "{profile:?}");
            let white = shown(profile, gba(31, 31, 31));
            assert!(white.r > 235, "{profile:?}");
            let black = shown(profile, gba(0, 0, 0));
            assert!(black.r < 30, "{profile:?}");
            let red = gba(31, 0, 0);
            assert!(spread(shown(profile, red)) < spread(red), "{profile:?}");
        }
        let original = shown(ColorProfile::Gba, gba(10, 20, 31));
        let lit = shown(ColorProfile::GbaSp, gba(10, 20, 31));
        assert!(spread(original) < spread(lit), "the unlit panel is paler");
    }

    #[test]
    fn the_profiles_go_round_and_keep_their_keys() {
        assert_eq!(ColorProfile::Original.step(true), ColorProfile::Gba);
        assert_eq!(ColorProfile::GbaSp.step(true), ColorProfile::Original);
        assert_eq!(ColorProfile::Original.step(false), ColorProfile::GbaSp);
        for profile in ColorProfile::ALL {
            assert_eq!(ColorProfile::from_key(profile.key()), Some(profile));
        }
        assert_eq!(ColorProfile::from_key("sepia"), None);
    }
}
