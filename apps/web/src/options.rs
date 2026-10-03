//! The page's options, given as the launcher's settings file writes them,
//! one `key=value` line each: `mode` (`classic` or `enhanced`), `color`,
//! `trail` and `upscale` (the display's, as the launcher's), `volume` (0 to
//! 100), `scaling` (`sharp`, `fill` or `smooth`), and the on-screen pad's
//! `touch` (`auto`, `on` or `off`), `touch-size` (60 to 140, percent) and
//! `touch-opacity` (20 to 100, percent), as Android's. Unknown keys and
//! values keep the defaults: the enhanced mode, the original colors, no
//! trail, no upscaler, full volume, sharp whole multiples, and the pad on
//! touch screens at its usual size and opacity.
//!
//! Source of knowledge: this project's own design (see
//! `apps/launcher/src/settings.rs`).

use game_core::{Enhancements, PlayMode};
use platform_web::{PadMode, Scaling};
use screen_filters::{ColorProfile, TrailMode, Upscaler};

/// The volume at its fullest, in percent.
pub const FULL_VOLUME: u8 = 100;
/// The pad's sizes and opacities, in percent of the usual ones, as
/// Android's.
pub const TOUCH_SIZES: std::ops::RangeInclusive<u8> = 60..=140;
/// See [`TOUCH_SIZES`].
pub const TOUCH_OPACITIES: std::ops::RangeInclusive<u8> = 20..=100;
/// See [`TOUCH_SIZES`].
pub const USUAL_TOUCH: u8 = 100;

/// What the page chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// How the game plays.
    pub mode: PlayMode,
    /// The colors the picture is shown with.
    pub color: ColorProfile,
    /// The trail each picture keeps.
    pub trail: TrailMode,
    /// How the picture is magnified.
    pub upscaler: Upscaler,
    /// The sound's volume, in percent.
    pub volume: u8,
    /// How the screen fills the window without the pad.
    pub scaling: Scaling,
    /// When the on-screen pad shows.
    pub touch: PadMode,
    /// The pad's size, in percent of the usual one.
    pub touch_size: u8,
    /// The pad's opacity, in percent of the usual one.
    pub touch_opacity: u8,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            mode: PlayMode::Enhanced(Enhancements::default()),
            color: ColorProfile::Original,
            trail: TrailMode::Off,
            upscaler: Upscaler::None,
            volume: FULL_VOLUME,
            scaling: Scaling::Sharp,
            touch: PadMode::Auto,
            touch_size: USUAL_TOUCH,
            touch_opacity: USUAL_TOUCH,
        }
    }
}

impl Options {
    /// The options `text` names.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut options = Self::default();
        for (key, value) in text
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.trim(), value.trim()))
        {
            match key {
                "mode" if value == "classic" => options.mode = PlayMode::Classic,
                "color" => options.color = ColorProfile::from_key(value).unwrap_or_default(),
                "trail" => options.trail = TrailMode::from_key(value).unwrap_or_default(),
                "upscale" => options.upscaler = Upscaler::from_key(value).unwrap_or_default(),
                "volume" => {
                    if let Ok(volume) = value.parse::<u8>() {
                        options.volume = volume.min(FULL_VOLUME);
                    }
                }
                "scaling" => options.scaling = Scaling::from_key(value).unwrap_or_default(),
                "touch" => options.touch = PadMode::from_key(value).unwrap_or_default(),
                "touch-size" => {
                    if let Some(size) = value.parse().ok().filter(|size| TOUCH_SIZES.contains(size))
                    {
                        options.touch_size = size;
                    }
                }
                "touch-opacity" => {
                    if let Some(opacity) = value
                        .parse()
                        .ok()
                        .filter(|opacity| TOUCH_OPACITIES.contains(opacity))
                    {
                        options.touch_opacity = opacity;
                    }
                }
                _ => {}
            }
        }
        options
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_lines_choose_and_the_rest_keeps_the_defaults() {
        let options = Options::parse("mode=classic\ncolor=gba\ntrail=fade\nvolume=250\nother=1\n");
        assert_eq!(options.mode, PlayMode::Classic);
        assert_eq!(
            (options.color, options.trail),
            (ColorProfile::Gba, TrailMode::Fade)
        );
        assert_eq!(options.volume, FULL_VOLUME);
        assert_eq!(Options::parse(""), Options::default());
        assert_eq!(
            Options::parse("upscale=scale2x").upscaler,
            Upscaler::Scale2x
        );
    }

    #[test]
    fn the_pad_and_the_scaling_take_their_lines_within_range() {
        let options =
            Options::parse("scaling=smooth\ntouch=on\ntouch-size=120\ntouch-opacity=40\n");
        assert_eq!(
            (options.scaling, options.touch),
            (Scaling::Smooth, PadMode::Always)
        );
        assert_eq!((options.touch_size, options.touch_opacity), (120, 40));
        let wrong = Options::parse("touch=maybe\ntouch-size=200\ntouch-opacity=5\n");
        assert_eq!(wrong.touch, PadMode::Auto);
        assert_eq!(
            (wrong.touch_size, wrong.touch_opacity),
            (USUAL_TOUCH, USUAL_TOUCH)
        );
    }
}
