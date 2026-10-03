//! The page's options, given as the launcher's settings file writes them,
//! one `key=value` line each: `mode` (`classic` or `enhanced`), `color`,
//! `trail` and `upscale` (the display's, as the launcher's), and `volume`
//! (0 to 100). Unknown keys and values keep the defaults: the enhanced
//! mode, the original colors, no trail, no upscaler, full volume.
//!
//! Source of knowledge: this project's own design (see
//! `apps/launcher/src/settings.rs`).

use game_core::{Enhancements, PlayMode};
use screen_filters::{ColorProfile, TrailMode, Upscaler};

/// The volume at its fullest, in percent.
pub const FULL_VOLUME: u8 = 100;

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
}

impl Default for Options {
    fn default() -> Self {
        Self {
            mode: PlayMode::Enhanced(Enhancements::default()),
            color: ColorProfile::Original,
            trail: TrailMode::Off,
            upscaler: Upscaler::None,
            volume: FULL_VOLUME,
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
}
