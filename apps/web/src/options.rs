//! The page's options, given as the launcher's settings file writes them,
//! one `key=value` line each: `mode` (`classic` or `enhanced`) and the
//! enhanced mode's `battle-animations`, `damage-numbers`, `auto-text`,
//! `autosave`, `weapon-reach` (`1` or `0`) and `fast-forward` (2 to 4), as
//! the launcher's, `color`,
//! `trail` and `upscale` (the display's, as the launcher's), `volume` (0 to
//! 100), `filter` (the launcher's `lcd`, `lcd-soft`, `lcd-fine` or
//! `scanlines` draw its grid; anything else none), `muted` (`1` to start
//! without sound), `scaling` (`sharp`, `fill`
//! or `smooth`), and the on-screen pad's
//! `touch` (`auto`, `on` or `off`), `touch-size` (60 to 140, percent) and
//! `touch-opacity` (20 to 100, percent), as Android's. Unknown keys and
//! values keep the defaults: the classic mode (as the launcher's), the
//! enhanced mode's own defaults, the original colors, no trail, no
//! upscaler, full volume, the window filled sharp (the page's Modern
//! preset), and the pad on touch screens at its usual size and opacity.
//!
//! Source of knowledge: this project's own design (see
//! `apps/launcher/src/settings.rs`).

use game_core::play_mode::FAST_FORWARD_SPEEDS;
use game_core::{Enhancements, PlayMode};
use platform_web::{PadMode, Scaling};
use screen_filters::{ColorProfile, Grid, TrailMode, Upscaler};

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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// How the game plays.
    pub mode: PlayMode,
    /// The colors the picture is shown with.
    pub color: ColorProfile,
    /// The trail each picture keeps.
    pub trail: TrailMode,
    /// How the picture is magnified.
    pub upscaler: Upscaler,
    /// The grid drawn over the picture, if any.
    pub grid: Option<Grid>,
    /// The sound's volume, in percent.
    pub volume: u8,
    /// Whether the game starts without sound.
    pub muted: bool,
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
            mode: PlayMode::Classic,
            color: ColorProfile::Original,
            trail: TrailMode::Off,
            upscaler: Upscaler::None,
            grid: None,
            volume: FULL_VOLUME,
            muted: false,
            scaling: Scaling::Fill,
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
        let mut enhanced = false;
        let mut enhancements = Enhancements::default();
        for (key, value) in text
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.trim(), value.trim()))
        {
            match key {
                "mode" => enhanced = value == "enhanced",
                "battle-animations" => enhancements.battle_animations = value != "0",
                "damage-numbers" => enhancements.damage_numbers = value == "1",
                "auto-text" => enhancements.auto_text = value == "1",
                "autosave" => enhancements.autosave = value != "0",
                "weapon-reach" => enhancements.weapon_reach = value != "0",
                "fast-forward" => {
                    if let Some(speed) = value
                        .parse()
                        .ok()
                        .filter(|speed| FAST_FORWARD_SPEEDS.contains(speed))
                    {
                        enhancements.fast_forward = speed;
                    }
                }
                "color" => options.color = ColorProfile::from_key(value).unwrap_or_default(),
                "trail" => options.trail = TrailMode::from_key(value).unwrap_or_default(),
                "upscale" => options.upscaler = Upscaler::from_key(value).unwrap_or_default(),
                "filter" => options.grid = Grid::from_key(value),
                "volume" => {
                    if let Ok(volume) = value.parse::<u8>() {
                        options.volume = volume.min(FULL_VOLUME);
                    }
                }
                "muted" => options.muted = value == "1",
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
        options.mode = if enhanced {
            PlayMode::Enhanced(enhancements)
        } else {
            PlayMode::Classic
        };
        options
    }
}

/// The enhanced mode's settings as the page keeps them, one line each, as
/// [`Options::parse`] reads them.
#[must_use]
pub fn enhancement_lines(enhancements: Enhancements) -> String {
    let flag = |on: bool| if on { "1" } else { "0" };
    format!(
        "battle-animations={}\ndamage-numbers={}\nauto-text={}\nautosave={}\nweapon-reach={}\nfast-forward={}\n",
        flag(enhancements.battle_animations),
        flag(enhancements.damage_numbers),
        flag(enhancements.auto_text),
        flag(enhancements.autosave),
        flag(enhancements.weapon_reach),
        enhancements.fast_forward,
    )
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
    fn the_enhanced_mode_s_settings_come_and_go_as_lines() {
        let chosen = Enhancements {
            battle_animations: false,
            damage_numbers: true,
            auto_text: true,
            autosave: false,
            weapon_reach: false,
            fast_forward: 4,
        };
        let options = Options::parse(&format!("mode=enhanced\n{}", enhancement_lines(chosen)));
        assert_eq!(options.mode, PlayMode::Enhanced(chosen));
        let odd = Options::parse("fast-forward=9\nbattle-animations=x\nmode=classic\n");
        assert_eq!(odd.mode, PlayMode::Classic);
        assert_eq!(
            Options::parse("mode=enhanced\nfast-forward=9\n").mode,
            PlayMode::Enhanced(Enhancements::default())
        );
        assert_eq!(Options::parse("mode=other\n").mode, PlayMode::Classic);
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
        assert!(Options::parse("muted=1").muted && !Options::parse("muted=0").muted);
        assert_eq!(Options::parse("filter=lcd-soft").grid, Some(Grid::LCD_SOFT));
        assert_eq!(Options::parse("filter=pixel-art").grid, None);
        let wrong = Options::parse("touch=maybe\ntouch-size=200\ntouch-opacity=5\n");
        assert_eq!(wrong.touch, PadMode::Auto);
        assert_eq!(
            (wrong.touch_size, wrong.touch_opacity),
            (USUAL_TOUCH, USUAL_TOUCH)
        );
    }
}
