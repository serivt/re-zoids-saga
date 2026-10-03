//! What the launcher remembers between runs: the ROM and the translation
//! last played, the window, the sound, the keys and gamepad buttons chosen
//! for the pad's buttons, the on-screen pad's size and opacity, the
//! picture's filter and colors, and the game mode with its enhancements,
//! one `key=value` line each, in the user's settings folder.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use game_core::{Enhancements, FAST_FORWARD_SPEEDS, PlayMode};
use platform::Button;
use platform_sdl3::Filter;
use screen_filters::{ColorProfile, TrailMode, Upscaler};

const ROM_KEY: &str = "rom";
const TRANSLATION_KEY: &str = "translation";
/// A button's key: `key.<button>=<key name>`, and its gamepad button:
/// `pad.<button>=<gamepad button name>`.
const BUTTON_PREFIX: &str = "key.";
const PAD_PREFIX: &str = "pad.";
const SCALE_KEY: &str = "scale";
const FULLSCREEN_KEY: &str = "fullscreen";
const FILTER_KEY: &str = "filter";
const VOLUME_KEY: &str = "volume";
const TOUCH_SIZE_KEY: &str = "touch-size";
const TOUCH_OPACITY_KEY: &str = "touch-opacity";
/// The game mode: `classic` or `enhanced`.
const MODE_KEY: &str = "mode";
const CLASSIC: &str = "classic";
const ENHANCED: &str = "enhanced";
/// The enhanced mode's battle animations, `1` shown or `0` skipped.
const BATTLE_ANIMATIONS_KEY: &str = "battle-animations";
/// The enhanced mode's damage numbers, `1` shown or `0` not.
const DAMAGE_NUMBERS_KEY: &str = "damage-numbers";
/// The enhanced mode's auto text, `1` on or `0` off.
const AUTO_TEXT_KEY: &str = "auto-text";
/// The enhanced mode's autosave, `1` on or `0` off.
const AUTOSAVE_KEY: &str = "autosave";
/// The enhanced mode's weapons' reach, `1` shown or `0` not.
const WEAPON_REACH_KEY: &str = "weapon-reach";
/// The enhanced mode's fast forward's speed, 2 to 4.
const FAST_FORWARD_KEY: &str = "fast-forward";
const SHARP: &str = "sharp";
const PIXEL_ART: &str = "pixel-art";
const SMOOTH: &str = "smooth";
const LCD: &str = "lcd";
const LCD_SOFT: &str = "lcd-soft";
const LCD_FINE: &str = "lcd-fine";
const SCANLINES: &str = "scanlines";
/// The LCD trail: `0` off, `1` the mix or `fade`.
const TRAIL_KEY: &str = "trail";
/// The pixel-art magnification: `none`, `scale2x` or `scale3x`.
const UPSCALE_KEY: &str = "upscale";
/// The colors the game is shown with: `original`, `gba` or `gba-sp`.
const COLOR_KEY: &str = "color";
/// The window's size in multiples of the screen, its default and the
/// sound's volume in percent.
pub const SCALES: std::ops::RangeInclusive<u32> = 1..=6;
/// See [`SCALES`].
pub const DEFAULT_SCALE: u32 = 3;
/// See [`SCALES`].
pub const FULL_VOLUME: u8 = 100;
/// The on-screen pad's size and opacity, in percent of their usual ones.
pub const TOUCH_SIZES: std::ops::RangeInclusive<u8> = 60..=140;
/// See [`TOUCH_SIZES`].
pub const TOUCH_OPACITIES: std::ops::RangeInclusive<u8> = 20..=100;
/// See [`TOUCH_SIZES`].
pub const USUAL_TOUCH: u8 = 100;
/// The settings folder's organization and program names, and the file in
/// it.
pub const ORGANIZATION: &str = "re-zoids-saga";
/// See [`ORGANIZATION`].
pub const APP: &str = "launcher";
/// See [`ORGANIZATION`].
pub const FILE_NAME: &str = "launcher.cfg";

/// The launcher's remembered choices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The ROM last played.
    pub rom: Option<PathBuf>,
    /// The translation last played with, if any.
    pub translation: Option<PathBuf>,
    /// The keys the player chose for buttons, by the backend's names; the
    /// others keep their defaults.
    pub keys: Vec<(Button, String)>,
    /// The gamepad buttons the player chose, by the backend's names.
    pub pad_buttons: Vec<(Button, String)>,
    /// The window's size in multiples of the screen.
    pub scale: u32,
    /// Whether the game fills the screen.
    pub fullscreen: bool,
    /// How the screen is scaled up.
    pub filter: Filter,
    /// The colors the game is shown with.
    pub color: ColorProfile,
    /// The trail each picture keeps of the one before, as the handheld's
    /// slow panel did.
    pub trail: TrailMode,
    /// How the picture is magnified before it is fitted to the window.
    pub upscaler: Upscaler,
    /// The sound's volume, in percent.
    pub volume: u8,
    /// The on-screen pad's size, in percent of its usual one.
    pub touch_size: u8,
    /// The on-screen pad's opacity, in percent of its usual one.
    pub touch_opacity: u8,
    /// The game mode and its enhancements.
    pub mode: GameMode,
}

/// The game mode the player chose, and the enhancements chosen for the
/// enhanced mode, kept in the classic mode too for when it comes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameMode {
    /// Whether the game plays in the enhanced mode.
    pub enhanced: bool,
    /// The enhancements the enhanced mode turns on, and its fast forward's
    /// speed.
    pub enhancements: Enhancements,
}

impl GameMode {
    /// How the game plays with this choice.
    #[must_use]
    pub fn play_mode(self) -> PlayMode {
        if self.enhanced {
            PlayMode::Enhanced(self.enhancements)
        } else {
            PlayMode::Classic
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rom: None,
            translation: None,
            keys: Vec::new(),
            pad_buttons: Vec::new(),
            scale: DEFAULT_SCALE,
            fullscreen: false,
            filter: Filter::Sharp,
            color: ColorProfile::Original,
            trail: TrailMode::Off,
            upscaler: Upscaler::None,
            volume: FULL_VOLUME,
            touch_size: USUAL_TOUCH,
            touch_opacity: USUAL_TOUCH,
            mode: GameMode::default(),
        }
    }
}

/// Sets `button`'s entry of `map` to `value`.
fn bind(map: &mut Vec<(Button, String)>, button: Button, value: &str) {
    if !value.is_empty() {
        map.retain(|(bound, _)| *bound != button);
        map.push((button, value.to_owned()));
    }
}

impl Settings {
    /// Reads `text`; unknown keys and malformed lines are ignored.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if let Some(button) = key.strip_prefix(BUTTON_PREFIX).and_then(Button::from_name) {
                bind(&mut settings.keys, button, value);
                continue;
            }
            if let Some(button) = key.strip_prefix(PAD_PREFIX).and_then(Button::from_name) {
                bind(&mut settings.pad_buttons, button, value);
                continue;
            }
            let path = (!value.is_empty()).then(|| PathBuf::from(value));
            match key {
                ROM_KEY => settings.rom = path,
                TRANSLATION_KEY => settings.translation = path,
                SCALE_KEY => {
                    if let Some(scale) = value.parse().ok().filter(|scale| SCALES.contains(scale)) {
                        settings.scale = scale;
                    }
                }
                FULLSCREEN_KEY => settings.fullscreen = value == "1",
                FILTER_KEY if value == SMOOTH => settings.filter = Filter::Smooth,
                FILTER_KEY if value == PIXEL_ART => settings.filter = Filter::PixelArt,
                FILTER_KEY if value == LCD => settings.filter = Filter::Lcd,
                FILTER_KEY if value == LCD_SOFT => settings.filter = Filter::LcdSoft,
                FILTER_KEY if value == LCD_FINE => settings.filter = Filter::LcdFine,
                FILTER_KEY if value == SCANLINES => settings.filter = Filter::Scanlines,
                FILTER_KEY => settings.filter = Filter::Sharp,
                COLOR_KEY => settings.color = ColorProfile::from_key(value).unwrap_or_default(),
                TRAIL_KEY => settings.trail = TrailMode::from_key(value).unwrap_or_default(),
                UPSCALE_KEY => settings.upscaler = Upscaler::from_key(value).unwrap_or_default(),
                VOLUME_KEY => {
                    if let Ok(volume) = value.parse::<u8>() {
                        settings.volume = volume.min(FULL_VOLUME);
                    }
                }
                TOUCH_SIZE_KEY => {
                    if let Some(size) = value.parse().ok().filter(|size| TOUCH_SIZES.contains(size))
                    {
                        settings.touch_size = size;
                    }
                }
                TOUCH_OPACITY_KEY => {
                    if let Some(opacity) = value
                        .parse()
                        .ok()
                        .filter(|opacity| TOUCH_OPACITIES.contains(opacity))
                    {
                        settings.touch_opacity = opacity;
                    }
                }
                MODE_KEY => settings.mode.enhanced = value == ENHANCED,
                BATTLE_ANIMATIONS_KEY => {
                    settings.mode.enhancements.battle_animations = value != "0";
                }
                DAMAGE_NUMBERS_KEY => settings.mode.enhancements.damage_numbers = value == "1",
                AUTO_TEXT_KEY => settings.mode.enhancements.auto_text = value == "1",
                AUTOSAVE_KEY => settings.mode.enhancements.autosave = value != "0",
                WEAPON_REACH_KEY => settings.mode.enhancements.weapon_reach = value != "0",
                FAST_FORWARD_KEY => {
                    if let Some(speed) = value
                        .parse()
                        .ok()
                        .filter(|speed| FAST_FORWARD_SPEEDS.contains(speed))
                    {
                        settings.mode.enhancements.fast_forward = speed;
                    }
                }
                _ => {}
            }
        }
        settings
    }

    /// The settings as the file holds them.
    #[must_use]
    pub fn to_text(&self) -> String {
        let path = |path: &Option<PathBuf>| {
            path.as_deref()
                .map(Path::to_string_lossy)
                .unwrap_or_default()
                .into_owned()
        };
        let mut text = format!(
            "{ROM_KEY}={}\n{TRANSLATION_KEY}={}\n",
            path(&self.rom),
            path(&self.translation)
        );
        let filter = match self.filter {
            Filter::Sharp => SHARP,
            Filter::PixelArt => PIXEL_ART,
            Filter::Smooth => SMOOTH,
            Filter::Lcd => LCD,
            Filter::LcdSoft => LCD_SOFT,
            Filter::LcdFine => LCD_FINE,
            Filter::Scanlines => SCANLINES,
        };
        let _ = writeln!(
            text,
            "{SCALE_KEY}={}\n{FULLSCREEN_KEY}={}\n{FILTER_KEY}={filter}\n{COLOR_KEY}={}\n{TRAIL_KEY}={}\n{VOLUME_KEY}={}",
            self.scale,
            u8::from(self.fullscreen),
            self.color.key(),
            self.trail.key(),
            self.volume
        );
        let _ = writeln!(text, "{UPSCALE_KEY}={}", self.upscaler.key());
        let _ = writeln!(
            text,
            "{TOUCH_SIZE_KEY}={}\n{TOUCH_OPACITY_KEY}={}",
            self.touch_size, self.touch_opacity
        );
        let mode = if self.mode.enhanced {
            ENHANCED
        } else {
            CLASSIC
        };
        let _ = writeln!(
            text,
            "{MODE_KEY}={mode}\n{BATTLE_ANIMATIONS_KEY}={}\n{DAMAGE_NUMBERS_KEY}={}\n{AUTO_TEXT_KEY}={}\n{AUTOSAVE_KEY}={}\n{WEAPON_REACH_KEY}={}\n{FAST_FORWARD_KEY}={}",
            u8::from(self.mode.enhancements.battle_animations),
            u8::from(self.mode.enhancements.damage_numbers),
            u8::from(self.mode.enhancements.auto_text),
            u8::from(self.mode.enhancements.autosave),
            u8::from(self.mode.enhancements.weapon_reach),
            self.mode.enhancements.fast_forward
        );
        for (button, key) in &self.keys {
            let _ = writeln!(text, "{BUTTON_PREFIX}{}={key}", button.name());
        }
        for (button, pad_button) in &self.pad_buttons {
            let _ = writeln!(text, "{PAD_PREFIX}{}={pad_button}", button.name());
        }
        text
    }

    /// Reads the file at `path`; nothing is remembered when it is missing.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .map(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    /// Writes the file at `path`.
    ///
    /// # Errors
    ///
    /// Returns the error of the write.
    pub fn store(&self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, self.to_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_the_rom_and_the_translation() {
        let settings = Settings {
            rom: Some(PathBuf::from("/games/Zoids Saga.gba")),
            translation: None,
            keys: vec![
                (Button::A, "Space".to_owned()),
                (Button::Up, "W".to_owned()),
            ],
            pad_buttons: vec![(Button::B, "a".to_owned())],
            scale: 4,
            fullscreen: true,
            filter: Filter::LcdSoft,
            color: ColorProfile::GbaSpFrontlit,
            trail: TrailMode::Fade,
            upscaler: Upscaler::Scale3x,
            volume: 70,
            touch_size: 120,
            touch_opacity: 40,
            mode: GameMode {
                enhanced: true,
                enhancements: Enhancements {
                    battle_animations: false,
                    damage_numbers: true,
                    auto_text: true,
                    autosave: false,
                    weapon_reach: false,
                    fast_forward: 4,
                },
            },
        };
        assert_eq!(Settings::parse(&settings.to_text()), settings);
        let read = Settings::parse("# notes\ntranslation=es.po\nrom=a=b.gba\nother=1\n");
        assert_eq!(read.rom, Some(PathBuf::from("a=b.gba")));
        assert_eq!(read.translation, Some(PathBuf::from("es.po")));
        assert_eq!(Settings::parse(""), Settings::default());
        let keys = Settings::parse("key.b=Q\nkey.b=E\nkey.turbo=T\nkey.a=\n").keys;
        assert_eq!(keys, [(Button::B, "E".to_owned())]);
        let odd = Settings::parse(
            "scale=40\nvolume=250\nfilter=blurry\nfullscreen=yes\ntouch-size=300\ntouch-opacity=5\ncolor=sepia\n",
        );
        assert_eq!(
            (odd.touch_size, odd.touch_opacity),
            (USUAL_TOUCH, USUAL_TOUCH)
        );
        assert_eq!(
            (odd.scale, odd.volume, odd.filter, odd.fullscreen),
            (DEFAULT_SCALE, FULL_VOLUME, Filter::Sharp, false)
        );
        assert_eq!(odd.color, ColorProfile::Original);
        assert_eq!(odd.trail, TrailMode::Off);
        assert_eq!(Settings::parse("trail=1\n").trail, TrailMode::Mix);
        assert_eq!(
            Settings::parse("filter=scanlines\n").filter,
            Filter::Scanlines
        );
        assert_eq!(Settings::parse("upscale=hq9x\n").upscaler, Upscaler::None);
        assert_eq!(
            Settings::parse("filter=pixel-art\n").filter,
            Filter::PixelArt
        );
    }

    #[test]
    fn the_game_mode_is_classic_until_the_enhanced_one_is_chosen() {
        let speed = |text: &str| Settings::parse(text).mode.enhancements.fast_forward;
        assert_eq!(speed("fast-forward=9\n"), 2);
        assert_eq!(speed("fast-forward=3\n"), 3);
        assert_eq!(Settings::default().mode.play_mode(), PlayMode::Classic);
        let odd = Settings::parse("mode=turbo\nbattle-animations=0\n");
        assert_eq!(odd.mode.play_mode(), PlayMode::Classic);
        assert!(!odd.mode.enhancements.battle_animations);
        assert!(!odd.mode.enhancements.damage_numbers);
        assert!(odd.mode.enhancements.autosave);
        assert!(odd.mode.enhancements.weapon_reach);
        let enhanced = Settings::parse(
            "mode=enhanced\nbattle-animations=0\ndamage-numbers=1\nauto-text=1\nautosave=0\n",
        );
        assert_eq!(
            enhanced.mode.play_mode(),
            PlayMode::Enhanced(Enhancements {
                battle_animations: false,
                damage_numbers: true,
                auto_text: true,
                autosave: false,
                weapon_reach: true,
                fast_forward: 2,
            })
        );
    }
}
