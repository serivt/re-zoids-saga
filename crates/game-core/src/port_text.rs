//! The port's own messages: the text of what the port adds to the game,
//! such as the save slots, which the ROM has no strings for.
//!
//! Source of knowledge: this project's own writing. Each message has a key
//! under `port/`, which a translation's PO file uses as it uses the name
//! entry's reserved keys, and a text shown when no extension answers:
//! Japanese for what the game shows, English for the launcher's screen,
//! which comes before any ROM is read, and for the autosave's notice, which
//! draws with the port's small capitals alone. A `{name}` marker in a message stands for a value the port
//! fills in; numbers are printed with full-width digits, as the game's own
//! values are.

use crate::extension::SharedExtensions;
use crate::text::TextMetrics;

/// Prefix of every key of the port's own messages.
pub const PORT_PREFIX: &str = "port/";

/// One of the port's own messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortText {
    /// The key, under [`PORT_PREFIX`].
    pub key: &'static str,
    /// The Japanese text the port shows by default.
    pub text: &'static str,
    /// What the message is for, for translators.
    pub note: &'static str,
    /// Pixels each line may take once its markers are filled with their
    /// widest values.
    pub pixels: usize,
}

/// The level after a slot's name in the slot list.
pub const SLOT_LEVEL: &str = "port/save-slots/level";
/// A slot with no game in the slot list.
pub const SLOT_EMPTY: &str = "port/save-slots/empty";
/// A slot whose two copies are broken in the slot list.
pub const SLOT_BROKEN: &str = "port/save-slots/broken";
/// The help window's second line for a slot with a game.
pub const SLOT_DETAILS: &str = "port/save-slots/details";
/// The help line while choosing where to save.
pub const SLOT_SAVE_HELP: &str = "port/save-slots/save-help";
/// The help line while choosing the game to continue.
pub const SLOT_LOAD_HELP: &str = "port/save-slots/load-help";
/// The question before saving into an empty slot.
pub const SLOT_QUESTION: &str = "port/save-slots/question";
/// The question before saving over a slot's game.
pub const SLOT_OVERWRITE: &str = "port/save-slots/overwrite";
/// The autosave's mark in place of a slot's number, when continuing.
pub const SLOT_AUTOSAVE: &str = "port/save-slots/autosave";
/// The help line while the autosave is under the cursor.
pub const SLOT_AUTOSAVE_HELP: &str = "port/save-slots/autosave-help";

/// The pause menu's settings in the enhanced mode: the message speed's
/// line and help.
pub const OPTIONS_SPEED: &str = "port/options/speed";
/// See [`OPTIONS_SPEED`].
pub const OPTIONS_SPEED_HELP: &str = "port/options/speed-help";
/// The battle animations' line and help.
pub const OPTIONS_ANIMATIONS: &str = "port/options/battle-animations";
/// See [`OPTIONS_ANIMATIONS`].
pub const OPTIONS_ANIMATIONS_HELP: &str = "port/options/battle-animations-help";
/// The damage numbers' line and help.
pub const OPTIONS_DAMAGE_NUMBERS: &str = "port/options/damage-numbers";
/// See [`OPTIONS_DAMAGE_NUMBERS`].
pub const OPTIONS_DAMAGE_NUMBERS_HELP: &str = "port/options/damage-numbers-help";
/// The auto text's line and help.
pub const OPTIONS_AUTO_TEXT: &str = "port/options/auto-text";
/// See [`OPTIONS_AUTO_TEXT`].
pub const OPTIONS_AUTO_TEXT_HELP: &str = "port/options/auto-text-help";
/// The autosave's line and help.
pub const OPTIONS_AUTOSAVE: &str = "port/options/autosave";
/// See [`OPTIONS_AUTOSAVE`].
pub const OPTIONS_AUTOSAVE_HELP: &str = "port/options/autosave-help";
/// The weapons' reach's line and help.
pub const OPTIONS_WEAPON_REACH: &str = "port/options/weapon-reach";
/// See [`OPTIONS_WEAPON_REACH`].
pub const OPTIONS_WEAPON_REACH_HELP: &str = "port/options/weapon-reach-help";
/// The fast forward's speed: its line, its help and its value.
pub const OPTIONS_FAST_FORWARD: &str = "port/options/fast-forward";
/// See [`OPTIONS_FAST_FORWARD`].
pub const OPTIONS_FAST_FORWARD_HELP: &str = "port/options/fast-forward-help";
/// See [`OPTIONS_FAST_FORWARD`].
pub const OPTIONS_FAST_FORWARD_VALUE: &str = "port/options/fast-forward-value";
/// A setting that is on, and off.
pub const OPTIONS_ON: &str = "port/options/on";
/// See [`OPTIONS_ON`].
pub const OPTIONS_OFF: &str = "port/options/off";
/// The keys, on the help line's second line.
pub const OPTIONS_KEYS: &str = "port/options/keys";
/// The enhanced mode's main list's line that opens the statistics.
pub const MENU_STATS: &str = "port/menu/stats";
/// The enhanced mode's main list's line that opens the achievements (see
/// [`crate::achievements`]).
pub const MENU_ACHIEVEMENTS: &str = "port/menu/achievements";
/// The achievements' pages, shown on the help line with their number and
/// how many are unlocked.
pub const ACHIEVEMENT_PAGE: &str = "port/achievements/page";
/// The first line of the window that announces an achievement unlocked.
pub const ACHIEVEMENT_UNLOCKED: &str = "port/achievements/unlocked";
/// The window's second line when several were unlocked at once.
pub const ACHIEVEMENT_MANY: &str = "port/achievements/many";
/// The enhanced mode's main list's last line, which leaves the game for
/// the launcher, and the question it asks in the help line.
pub const MENU_QUIT: &str = "port/menu/quit";
/// See [`MENU_QUIT`].
pub const MENU_QUIT_QUESTION: &str = "port/menu/quit-question";
/// See [`MENU_QUIT`].
pub const MENU_QUIT_UNSAVED: &str = "port/menu/quit-unsaved";
/// The statistics' pages, shown on the help line with their number.
pub const STATS_PAGE_BATTLES: &str = "port/stats/page-battles";
/// See [`STATS_PAGE_BATTLES`].
pub const STATS_PAGE_COLLECTION: &str = "port/stats/page-collection";
/// See [`STATS_PAGE_BATTLES`].
pub const STATS_PAGE_RECORDS: &str = "port/stats/page-records";
/// The statistics' keys, on the help line's second line.
pub const STATS_KEYS: &str = "port/stats/keys";
/// The statistics' lines.
pub const STATS_WON: &str = "port/stats/won";
/// See [`STATS_WON`].
pub const STATS_LOST: &str = "port/stats/lost";
/// See [`STATS_WON`].
pub const STATS_RETREATED: &str = "port/stats/retreated";
/// See [`STATS_WON`].
pub const STATS_ENEMIES: &str = "port/stats/enemies";
/// See [`STATS_WON`].
pub const STATS_DESTROYED: &str = "port/stats/destroyed";
/// See [`STATS_WON`].
pub const STATS_MONEY: &str = "port/stats/money";
/// See [`STATS_WON`].
pub const STATS_ZI_DATA: &str = "port/stats/zi-data";
/// See [`STATS_WON`].
pub const STATS_ZOIDS: &str = "port/stats/zoids";
/// See [`STATS_WON`].
pub const STATS_CHARACTERS: &str = "port/stats/characters";
/// See [`STATS_WON`].
pub const STATS_COMMANDS: &str = "port/stats/commands";
/// See [`STATS_WON`].
pub const STATS_TIME: &str = "port/stats/time";
/// See [`STATS_WON`].
pub const STATS_BEST_HIT: &str = "port/stats/best-hit";
/// See [`STATS_WON`].
pub const STATS_LONGEST: &str = "port/stats/longest";
/// See [`STATS_WON`].
pub const STATS_STORY: &str = "port/stats/story";
/// The statistics' values: times, Zoids, money, a count of a total, the
/// time played, damage and rounds.
pub const STATS_TIMES: &str = "port/stats/times";
/// See [`STATS_TIMES`].
pub const STATS_UNITS: &str = "port/stats/units";
/// See [`STATS_TIMES`].
pub const STATS_GOLD: &str = "port/stats/gold";
/// See [`STATS_TIMES`].
pub const STATS_OF: &str = "port/stats/of";
/// See [`STATS_TIMES`].
pub const STATS_HOURS: &str = "port/stats/hours";
/// See [`STATS_TIMES`].
pub const STATS_DAMAGE: &str = "port/stats/damage";
/// See [`STATS_TIMES`].
pub const STATS_ROUNDS: &str = "port/stats/rounds";

/// The row the weapons' reach is drawn from: the front, and the back.
pub const REACH_FRONT: &str = "port/reach/front";
/// See [`REACH_FRONT`].
pub const REACH_BACK: &str = "port/reach/back";

/// The notice over the field while the autosave is written.
pub const AUTOSAVE_NOTICE: &str = "port/autosave/notice";

/// The line under the launcher's title.
pub const LAUNCHER_SUBTITLE: &str = "port/launcher/subtitle";
/// The label of the launcher's ROM line.
pub const LAUNCHER_ROM: &str = "port/launcher/rom";
/// The label of the launcher's translation line.
pub const LAUNCHER_TRANSLATION: &str = "port/launcher/translation";
/// The launcher's line that starts the game.
pub const LAUNCHER_PLAY: &str = "port/launcher/play";
/// The launcher's line that closes it.
pub const LAUNCHER_QUIT: &str = "port/launcher/quit";
/// The ROM line before one is chosen.
pub const LAUNCHER_NO_ROM: &str = "port/launcher/no-rom";
/// The translation line without one.
pub const LAUNCHER_NO_TRANSLATION: &str = "port/launcher/no-translation";
/// The help for the ROM line before one is chosen.
pub const LAUNCHER_PICK_ROM: &str = "port/launcher/pick-rom";
/// The help for the translation line.
pub const LAUNCHER_PICK_TRANSLATION: &str = "port/launcher/pick-translation";
/// The translation's screen: its heading is [`LAUNCHER_TRANSLATION`]; the
/// line that opens a file, what a language's line shows once downloaded,
/// the help of the main screen's translation line, and the screen's
/// statuses and help.
pub const LAUNCHER_FROM_FILE: &str = "port/launcher/from-file";
/// See [`LAUNCHER_FROM_FILE`].
pub const LAUNCHER_DOWNLOADED: &str = "port/launcher/downloaded";
/// See [`LAUNCHER_FROM_FILE`].
pub const LAUNCHER_CHOOSE_TRANSLATION: &str = "port/launcher/choose-translation";
/// See [`LAUNCHER_FROM_FILE`].
pub const LAUNCHER_LOOKING_UP: &str = "port/launcher/looking-up";
/// See [`LAUNCHER_FROM_FILE`].
pub const LAUNCHER_OFFLINE: &str = "port/launcher/offline";
/// See [`LAUNCHER_FROM_FILE`].
pub const LAUNCHER_DOWNLOADS: &str = "port/launcher/downloads";
/// See [`LAUNCHER_FROM_FILE`].
pub const LAUNCHER_DOWNLOADING: &str = "port/launcher/downloading";
/// See [`LAUNCHER_FROM_FILE`].
pub const LAUNCHER_DOWNLOAD_FAILED: &str = "port/launcher/download-failed";
/// See [`LAUNCHER_FROM_FILE`].
pub const LAUNCHER_TRANSLATION_HELP: &str = "port/launcher/translation-help";
/// The options of the on-screen pad's size and opacity, on Android.
pub const LAUNCHER_TOUCH_SIZE: &str = "port/launcher/touch-size";
/// See [`LAUNCHER_TOUCH_SIZE`].
pub const LAUNCHER_TOUCH_OPACITY: &str = "port/launcher/touch-opacity";
/// The launcher's help lines and its options line's help on a touch
/// screen, where rows are tapped and the pad's A and B choose and go back:
/// those of the main screen, the options, the lists (translations and
/// saves) and the about screen.
pub const LAUNCHER_TOUCH_HELP: &str = "port/launcher/touch-help";
/// See [`LAUNCHER_TOUCH_HELP`].
pub const LAUNCHER_TOUCH_OPTIONS_HELP: &str = "port/launcher/touch-options-help";
/// See [`LAUNCHER_TOUCH_HELP`].
pub const LAUNCHER_TOUCH_LIST_HELP: &str = "port/launcher/touch-list-help";
/// See [`LAUNCHER_TOUCH_HELP`].
pub const LAUNCHER_TOUCH_ABOUT_HELP: &str = "port/launcher/touch-about-help";
/// See [`LAUNCHER_TOUCH_HELP`].
pub const LAUNCHER_PICK_TOUCH_OPTIONS: &str = "port/launcher/pick-touch-options";
/// The saves' screens, on Android: the options' line and heading, a
/// slot's line (`{slot}` its number) and what it holds, a slot's two
/// actions, and the statuses.
pub const LAUNCHER_SAVES: &str = "port/launcher/saves";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_SLOT: &str = "port/launcher/slot";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_SLOT_SAVED: &str = "port/launcher/slot-saved";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_SLOT_EMPTY: &str = "port/launcher/slot-empty";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_EXPORT: &str = "port/launcher/export";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_IMPORT: &str = "port/launcher/import";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_SAVES_HELP: &str = "port/launcher/saves-help";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_EXPORT_HELP: &str = "port/launcher/export-help";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_IMPORT_HELP: &str = "port/launcher/import-help";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_EXPORT_SRM: &str = "port/launcher/export-srm";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_EXPORT_STRICT: &str = "port/launcher/export-strict";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_EXPORT_SRM_HELP: &str = "port/launcher/export-srm-help";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_EXPORT_STRICT_HELP: &str = "port/launcher/export-strict-help";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_IMPORT_FILE: &str = "port/launcher/import-file";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_IMPORT_CHECK: &str = "port/launcher/import-check";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_SAVE_LEVEL: &str = "port/launcher/save-level";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_SAVE_MONEY: &str = "port/launcher/save-money";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_SAVE_PLAYED: &str = "port/launcher/save-played";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_REPLACE: &str = "port/launcher/replace";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_CANCEL: &str = "port/launcher/cancel";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_EXPORTED: &str = "port/launcher/exported";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_IMPORTED: &str = "port/launcher/imported";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_NOT_A_SAVE: &str = "port/launcher/not-a-save";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_COPY_FAILED: &str = "port/launcher/copy-failed";
/// See [`LAUNCHER_SAVES`].
pub const LAUNCHER_ROM_FIRST: &str = "port/launcher/rom-first";
/// The ROM is the known release.
pub const LAUNCHER_ROM_VERIFIED: &str = "port/launcher/rom-verified";
/// The ROM is the game's first release, which the port does not play.
pub const LAUNCHER_ROM_FIRST_RELEASE: &str = "port/launcher/rom-first-release";
/// The ROM is the game but not the known dump, which the port does not play.
pub const LAUNCHER_ROM_UNSUPPORTED: &str = "port/launcher/rom-unsupported";
/// The ROM is not a game the port plays.
pub const LAUNCHER_ROM_OTHER: &str = "port/launcher/rom-other";
/// The ROM cannot be opened or identified.
pub const LAUNCHER_ROM_UNREADABLE: &str = "port/launcher/rom-unreadable";
/// The translation was read, with its count of messages.
pub const LAUNCHER_TRANSLATION_READ: &str = "port/launcher/translation-read";
/// The translation cannot be opened or parsed.
pub const LAUNCHER_TRANSLATION_UNREADABLE: &str = "port/launcher/translation-unreadable";
/// The help for the play line.
pub const LAUNCHER_READY: &str = "port/launcher/ready";
/// The launcher's controls.
pub const LAUNCHER_HELP: &str = "port/launcher/help";

/// The label of the launcher's line to the screen about the port, and its
/// help.
pub const LAUNCHER_ABOUT: &str = "port/launcher/about";
/// See [`LAUNCHER_ABOUT`].
pub const LAUNCHER_PICK_ABOUT: &str = "port/launcher/pick-about";
/// The about screen's lines: the version, the license, the project's and
/// the translations' pages, their help and its controls.
pub const LAUNCHER_VERSION: &str = "port/launcher/version";
/// See [`LAUNCHER_VERSION`].
pub const LAUNCHER_LICENSE: &str = "port/launcher/license";
/// See [`LAUNCHER_VERSION`].
pub const LAUNCHER_PROJECT_PAGE: &str = "port/launcher/project-page";
/// See [`LAUNCHER_VERSION`].
pub const LAUNCHER_TRANSLATIONS_PAGE: &str = "port/launcher/translations-page";
/// See [`LAUNCHER_VERSION`].
pub const LAUNCHER_OPENS_PAGE: &str = "port/launcher/opens-page";
/// See [`LAUNCHER_VERSION`].
pub const LAUNCHER_PAGE_UNOPENED: &str = "port/launcher/page-unopened";
/// See [`LAUNCHER_VERSION`].
pub const LAUNCHER_ABOUT_HELP: &str = "port/launcher/about-help";

/// The label of the launcher's options line, and its help.
pub const LAUNCHER_OPTIONS: &str = "port/launcher/options";
/// See [`LAUNCHER_OPTIONS`].
pub const LAUNCHER_PICK_OPTIONS: &str = "port/launcher/pick-options";
/// The options screen's lines and values.
pub const LAUNCHER_KEYBOARD: &str = "port/launcher/keyboard";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_GAMEPAD: &str = "port/launcher/gamepad";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_NO_GAMEPAD: &str = "port/launcher/no-gamepad";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_WINDOW: &str = "port/launcher/window";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_FULLSCREEN: &str = "port/launcher/fullscreen";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_DISPLAY: &str = "port/launcher/display";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_PRESET: &str = "port/launcher/preset";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_CUSTOM: &str = "port/launcher/custom";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_FILTER: &str = "port/launcher/filter";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_SHARP: &str = "port/launcher/sharp";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_PIXEL_ART: &str = "port/launcher/pixel-art";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_SMOOTH: &str = "port/launcher/smooth";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_LCD: &str = "port/launcher/lcd";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_COLORS: &str = "port/launcher/colors";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_COLOR_ORIGINAL: &str = "port/launcher/color-original";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_COLOR_GBA: &str = "port/launcher/color-gba";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_COLOR_GBA_SP: &str = "port/launcher/color-gba-sp";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_TRAIL: &str = "port/launcher/trail";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_UPSCALER: &str = "port/launcher/upscaler";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_LCD_SOFT: &str = "port/launcher/lcd-soft";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_LCD_FINE: &str = "port/launcher/lcd-fine";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_SCANLINES: &str = "port/launcher/scanlines";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_COLOR_GBA_SP_FRONTLIT: &str = "port/launcher/color-gba-sp-frontlit";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_COLOR_MICRO: &str = "port/launcher/color-micro";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_COLOR_DS: &str = "port/launcher/color-ds";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_TRAIL_MIX: &str = "port/launcher/trail-mix";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_TRAIL_FADE: &str = "port/launcher/trail-fade";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_PRESET_GBA: &str = "port/launcher/preset-gba";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_PRESET_GBA_SP_FRONTLIT: &str = "port/launcher/preset-gba-sp-frontlit";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_PRESET_GBA_SP: &str = "port/launcher/preset-gba-sp";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_PRESET_MICRO: &str = "port/launcher/preset-micro";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_PRESET_DS: &str = "port/launcher/preset-ds";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_PRESET_PLAYER: &str = "port/launcher/preset-player";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_PRESET_MODERN: &str = "port/launcher/preset-modern";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_PRESET_SMOOTH_PIXEL_ART: &str = "port/launcher/preset-smooth-pixel-art";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_ON: &str = "port/launcher/on";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_OFF: &str = "port/launcher/off";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_VOLUME: &str = "port/launcher/volume";
/// The main screen's line of the game mode, its values, and the game
/// mode's screen: its enhancements and what each mode means.
pub const LAUNCHER_MODE: &str = "port/launcher/mode";
/// See [`LAUNCHER_MODE`].
pub const LAUNCHER_CLASSIC: &str = "port/launcher/classic";
/// See [`LAUNCHER_MODE`].
pub const LAUNCHER_ENHANCED: &str = "port/launcher/enhanced";
/// See [`LAUNCHER_MODE`].
pub const LAUNCHER_BATTLE_ANIMATIONS: &str = "port/launcher/battle-animations";
/// See [`LAUNCHER_MODE`].
pub const LAUNCHER_DAMAGE_NUMBERS: &str = "port/launcher/damage-numbers";
/// See [`LAUNCHER_MODE`].
pub const LAUNCHER_AUTO_TEXT: &str = "port/launcher/auto-text";
/// See [`LAUNCHER_MODE`].
pub const LAUNCHER_AUTOSAVE: &str = "port/launcher/autosave";
/// See [`LAUNCHER_MODE`].
pub const LAUNCHER_WEAPON_REACH: &str = "port/launcher/weapon-reach";
/// The main screen's line when a newer release is out, and its help.
pub const LAUNCHER_UPDATE: &str = "port/launcher/update";
/// See [`LAUNCHER_UPDATE`].
pub const LAUNCHER_UPDATE_HELP: &str = "port/launcher/update-help";
/// The game mode's line of the fast forward's speed.
pub const LAUNCHER_FAST_FORWARD: &str = "port/launcher/fast-forward";
/// See [`LAUNCHER_MODE`].
pub const LAUNCHER_PICK_MODE: &str = "port/launcher/pick-mode";
/// See [`LAUNCHER_MODE`].
pub const LAUNCHER_CLASSIC_NOTE: &str = "port/launcher/classic-note";
/// See [`LAUNCHER_MODE`].
pub const LAUNCHER_ENHANCED_NOTE: &str = "port/launcher/enhanced-note";
/// The options screen's help.
pub const LAUNCHER_OPTIONS_HELP: &str = "port/launcher/options-help";
/// The gamepad screen while it waits for the button of `{button}`.
pub const LAUNCHER_PRESS_PAD: &str = "port/launcher/press-pad";
/// The controls line while every button has its default key.
pub const LAUNCHER_KEYS_DEFAULT: &str = "port/launcher/keys-default";
/// The controls line once some button has another key.
pub const LAUNCHER_KEYS_CUSTOM: &str = "port/launcher/keys-custom";
/// The controls screen's help.
pub const LAUNCHER_CONTROLS_HELP: &str = "port/launcher/controls-help";
/// The controls screen while it waits for the key of `{button}`.
pub const LAUNCHER_PRESS_KEY: &str = "port/launcher/press-key";
/// The controls screen's line that gives every button its default key.
pub const LAUNCHER_DEFAULT_KEYS: &str = "port/launcher/default-keys";
/// The controls screen's line back to the launcher's.
pub const LAUNCHER_BACK: &str = "port/launcher/back";
/// The pad's directions on the controls screen.
pub const LAUNCHER_UP: &str = "port/launcher/up";
/// See [`LAUNCHER_UP`].
pub const LAUNCHER_DOWN: &str = "port/launcher/down";
/// See [`LAUNCHER_UP`].
pub const LAUNCHER_LEFT: &str = "port/launcher/left";
/// See [`LAUNCHER_UP`].
pub const LAUNCHER_RIGHT: &str = "port/launcher/right";
/// The question before closing, from the launcher's screen or the game.
pub const LAUNCHER_QUIT_QUESTION: &str = "port/launcher/quit-question";
/// The warning under the question while the game plays.
pub const LAUNCHER_QUIT_UNSAVED: &str = "port/launcher/quit-unsaved";
/// The answers to the question.
pub const LAUNCHER_YES: &str = "port/launcher/yes";
/// See [`LAUNCHER_YES`].
pub const LAUNCHER_NO: &str = "port/launcher/no";

/// The launcher's screen, in English until a translation is chosen; one
/// line each: up to 208 pixels in its panel, 232 across the screen (the
/// subtitle, the status and the help).
pub const LAUNCHER_TEXTS: &[PortText] = &[
    launcher(LAUNCHER_OPTIONS, "Options", "The label of the options line"),
    launcher_line(
        LAUNCHER_PICK_OPTIONS,
        "Window, sound and controls.",
        "Help for the options line",
    ),
    launcher(
        LAUNCHER_KEYBOARD,
        "Keyboard",
        "The line of the keyboard's keys, and that screen's heading",
    ),
    launcher(
        LAUNCHER_GAMEPAD,
        "Gamepad",
        "The line of the gamepad's buttons, and that screen's heading",
    ),
    launcher(
        LAUNCHER_NO_GAMEPAD,
        "none connected",
        "The gamepad line without a gamepad",
    ),
    launcher(
        LAUNCHER_WINDOW,
        "Window size",
        "The line of the window's size",
    ),
    launcher(
        LAUNCHER_FULLSCREEN,
        "Fullscreen",
        "The line that fills the screen",
    ),
    launcher(
        LAUNCHER_DISPLAY,
        "Display",
        "The line that opens the display's options (the filter, the colors, the LCD trail), and that screen's heading; their summary follows",
    ),
    launcher(
        LAUNCHER_PRESET,
        "Preset",
        "The line that sets the display's options together as a handheld's screen showed the game (GBA or GBA SP, the names of the colors' line), or shows they are custom",
    ),
    launcher(
        LAUNCHER_CUSTOM,
        "custom",
        "The preset line's value when the display's options are not one of the handhelds'",
    ),
    launcher(
        LAUNCHER_FILTER,
        "Filter",
        "The line of how the picture is scaled",
    ),
    launcher(
        LAUNCHER_SHARP,
        "sharp",
        "Scaled by whole multiples, square pixels",
    ),
    launcher(
        LAUNCHER_PIXEL_ART,
        "pixel art",
        "Scaled to fill, square pixels blended only at their edges",
    ),
    launcher(LAUNCHER_SMOOTH, "smooth", "Scaled to fill, blended pixels"),
    launcher(
        LAUNCHER_LCD,
        "LCD",
        "Scaled by whole multiples, with the dark lines between the handheld screen's pixels",
    ),
    launcher(
        LAUNCHER_COLORS,
        "Colors",
        "The line of the colors the game is shown with",
    ),
    launcher(
        LAUNCHER_COLOR_ORIGINAL,
        "original",
        "The game's colors as it draws them",
    ),
    launcher(
        LAUNCHER_COLOR_GBA,
        "GBA",
        "The colors of the original Game Boy Advance's unlit screen: darker and paler",
    ),
    launcher(
        LAUNCHER_COLOR_GBA_SP,
        "GBA SP AGS-101",
        "The colors of the later Game Boy Advance SP's backlit screen: a little paler",
    ),
    launcher(
        LAUNCHER_LCD_SOFT,
        "LCD soft",
        "The filter's value: the LCD grid with lighter lines, as on a lit screen",
    ),
    launcher(
        LAUNCHER_LCD_FINE,
        "LCD fine",
        "The filter's value: the LCD grid with thin, light lines, as on a small dense screen",
    ),
    launcher(
        LAUNCHER_SCANLINES,
        "scan lines",
        "The filter's value: whole multiples with a dark band under each row, as on a television",
    ),
    launcher(
        LAUNCHER_COLOR_GBA_SP_FRONTLIT,
        "GBA SP AGS-001",
        "The colors of the first Game Boy Advance SP's front-lit screen: washed out and cool",
    ),
    launcher(
        LAUNCHER_COLOR_MICRO,
        "Micro",
        "The colors of the Game Boy Micro's backlit screen: vivid",
    ),
    launcher(
        LAUNCHER_COLOR_DS,
        "DS",
        "The colors of the Nintendo DS's screens, which play the cartridges in their slot: bright",
    ),
    launcher(
        LAUNCHER_TRAIL_MIX,
        "mix",
        "The LCD trail's value: each picture mixed with the one before",
    ),
    launcher(
        LAUNCHER_TRAIL_FADE,
        "fade",
        "The LCD trail's value: a trail that fades over two or three frames",
    ),
    launcher(
        LAUNCHER_PRESET_GBA,
        "GBA",
        "The preset of the original Game Boy Advance's screen",
    ),
    launcher(
        LAUNCHER_PRESET_GBA_SP_FRONTLIT,
        "GBA SP AGS-001",
        "The preset of the first, front-lit Game Boy Advance SP's screen",
    ),
    launcher(
        LAUNCHER_PRESET_GBA_SP,
        "GBA SP AGS-101",
        "The preset of the later, backlit Game Boy Advance SP's screen",
    ),
    launcher(
        LAUNCHER_PRESET_MICRO,
        "Game Boy Micro",
        "The preset of the Game Boy Micro's screen",
    ),
    launcher(
        LAUNCHER_PRESET_DS,
        "Nintendo DS",
        "The preset of the Nintendo DS's screen, which plays the cartridges in its slot",
    ),
    launcher(
        LAUNCHER_PRESET_PLAYER,
        "Game Boy Player",
        "The preset of the Game Boy Player, which showed the game on a television",
    ),
    launcher(
        LAUNCHER_PRESET_MODERN,
        "modern",
        "The preset of today's screens: the window filled, sharp, the original colors",
    ),
    launcher(
        LAUNCHER_PRESET_SMOOTH_PIXEL_ART,
        "smooth pixel art",
        "The preset of today's screens with the Scale3x upscaler's rounded edges",
    ),
    launcher(
        LAUNCHER_UPSCALER,
        "Upscaler",
        "The line of the pixel-art magnification (Scale2x, Scale3x, or off), which rounds diagonal edges without blurring",
    ),
    launcher(
        LAUNCHER_TRAIL,
        "LCD trail",
        "The line that lets each picture linger a frame, as the handheld's slow screen did; on or off follows",
    ),
    launcher(LAUNCHER_ON, "on", "A setting that is on"),
    launcher(LAUNCHER_OFF, "off", "A setting that is off"),
    launcher(LAUNCHER_VOLUME, "Volume", "The line of the sound's volume"),
    launcher(
        LAUNCHER_MODE,
        "Game mode",
        "The line of the game mode: as the original, or with the port's conveniences",
    ),
    launcher(
        LAUNCHER_CLASSIC,
        "Classic",
        "The game mode that plays exactly as the original",
    ),
    launcher(
        LAUNCHER_ENHANCED,
        "Enhanced",
        "The game mode with the port's conveniences",
    ),
    launcher(
        LAUNCHER_BATTLE_ANIMATIONS,
        "Battle animations",
        "The enhanced mode's line that shows or skips the battles' attack scenes",
    ),
    launcher(
        LAUNCHER_DAMAGE_NUMBERS,
        "Damage numbers",
        "The enhanced mode's line that shows the damage each unit takes as a number under it",
    ),
    launcher(
        LAUNCHER_AUTO_TEXT,
        "Auto text",
        "The enhanced mode's line that lets the text boxes go on by themselves, turned on or off with SELECT while one shows",
    ),
    launcher(
        LAUNCHER_AUTOSAVE,
        "Autosave",
        "The enhanced mode's line that saves the game on each change of map, into a slot of its own listed first when continuing",
    ),
    launcher(
        LAUNCHER_UPDATE,
        "Update available",
        "The main screen's line, after Quit, when a newer release of the port is out; the new version (v0.4.0) follows it",
    ),
    launcher_line(
        LAUNCHER_UPDATE_HELP,
        "A new version is out: open its page.",
        "The status line under the update's line: choosing it opens the releases page in the web browser",
    ),
    launcher(
        LAUNCHER_FAST_FORWARD,
        "Fast forward",
        "The enhanced mode's line of the fast forward's speed (2x to 4x), at which the game plays while its button (Space by default) is held",
    ),
    launcher(
        LAUNCHER_WEAPON_REACH,
        "Weapon reach",
        "The enhanced mode's line that draws, in the shops and the pause menu, the cells of the battle's grid the weapon under the cursor reaches",
    ),
    launcher_line(
        LAUNCHER_PICK_MODE,
        "As the original, or with conveniences.",
        "Help for the game mode's line",
    ),
    launcher_line(
        LAUNCHER_CLASSIC_NOTE,
        "Plays exactly as the original.",
        "What the classic mode means, on the game mode's screen",
    ),
    launcher_line(
        LAUNCHER_ENHANCED_NOTE,
        "Adds the conveniences turned on here.",
        "What the enhanced mode means, on the game mode's screen",
    ),
    launcher(
        LAUNCHER_TOUCH_SIZE,
        "Pad size",
        "The line of the on-screen pad's size, on Android",
    ),
    launcher(
        LAUNCHER_TOUCH_OPACITY,
        "Pad opacity",
        "The line of the on-screen pad's opacity, on Android",
    ),
    launcher_line(
        LAUNCHER_TOUCH_HELP,
        "Tap or A: choose   B: clear",
        "The controls, at the bottom, on Android",
    ),
    launcher_line(
        LAUNCHER_TOUCH_OPTIONS_HELP,
        "Left/Right: change   A: choose   B: back",
        "The options screen's help, on Android",
    ),
    launcher_line(
        LAUNCHER_TOUCH_LIST_HELP,
        "Tap or A: choose   B: back",
        "The help of the translation's and the saves' screens, on Android",
    ),
    launcher_line(
        LAUNCHER_TOUCH_ABOUT_HELP,
        "Tap or A: open   B: back",
        "The about screen's help, on Android",
    ),
    launcher_line(
        LAUNCHER_PICK_TOUCH_OPTIONS,
        "Sound, the pad and saves.",
        "Help for the options line, on Android",
    ),
    launcher_line(
        LAUNCHER_OPTIONS_HELP,
        "Left/Right: change   X: choose   Z: back",
        "The options screen's help",
    ),
    launcher_line(
        LAUNCHER_PRESS_PAD,
        "Press a button for {button}. Esc cancels.",
        "Waiting for a gamepad button; {button} is the pad's button",
    ),
    launcher(
        LAUNCHER_KEYS_DEFAULT,
        "default keys",
        "The controls line when no key was changed",
    ),
    launcher(
        LAUNCHER_KEYS_CUSTOM,
        "your keys",
        "The controls line once a key was changed",
    ),
    launcher_line(
        LAUNCHER_CONTROLS_HELP,
        "X: change   Z: back",
        "The controls screen's help",
    ),
    launcher_line(
        LAUNCHER_PRESS_KEY,
        "Press a key for {button}. Esc cancels.",
        "Waiting for a key; {button} is the button's name",
    ),
    launcher(
        LAUNCHER_DEFAULT_KEYS,
        "Defaults",
        "The line that gives every button its default key or gamepad button",
    ),
    launcher(
        LAUNCHER_BACK,
        "Back",
        "The line back to the launcher's screen",
    ),
    launcher(LAUNCHER_UP, "Up", "The pad's up; up to 40 pixels"),
    launcher(LAUNCHER_DOWN, "Down", "The pad's down; up to 40 pixels"),
    launcher(LAUNCHER_LEFT, "Left", "The pad's left; up to 40 pixels"),
    launcher(LAUNCHER_RIGHT, "Right", "The pad's right; up to 40 pixels"),
    launcher(
        LAUNCHER_QUIT_QUESTION,
        "Quit Re:Zoids Saga?",
        "The question before closing, asked on Esc",
    ),
    launcher(
        LAUNCHER_QUIT_UNSAVED,
        "Progress not saved will be lost.",
        "Under the question while the game plays",
    ),
    launcher(
        LAUNCHER_YES,
        "Yes",
        "The answer that closes; up to 60 pixels",
    ),
    launcher(LAUNCHER_NO, "No", "The answer that stays; up to 60 pixels"),
    launcher_line(
        LAUNCHER_SUBTITLE,
        "A free port of Zoids Saga (GBA)",
        "The line under the title",
    ),
    launcher(LAUNCHER_ROM, "ROM", "The label of the ROM's line"),
    launcher(
        LAUNCHER_TRANSLATION,
        "Translation",
        "The label of the translation's line",
    ),
    launcher(LAUNCHER_PLAY, "Play", "The line that starts the game"),
    launcher(LAUNCHER_QUIT, "Quit", "The line that closes the launcher"),
    launcher(
        LAUNCHER_NO_ROM,
        "none",
        "The ROM's line before one is chosen",
    ),
    launcher(
        LAUNCHER_NO_TRANSLATION,
        "none (Japanese)",
        "The translation's line without one",
    ),
    launcher_line(
        LAUNCHER_PICK_ROM,
        "Choose your Zoids Saga ROM.",
        "Help for the ROM's line before one is chosen",
    ),
    launcher_line(
        LAUNCHER_PICK_TRANSLATION,
        "Choose a translation file (.po), or none.",
        "Help for the translation's line",
    ),
    launcher(
        LAUNCHER_FROM_FILE,
        "From a file...",
        "The translation screen's line that opens a PO file",
    ),
    launcher(
        LAUNCHER_DOWNLOADED,
        "downloaded",
        "Beside a language already downloaded, which choosing downloads again",
    ),
    launcher_line(
        LAUNCHER_CHOOSE_TRANSLATION,
        "Download a translation, or open a file.",
        "Help for the translation's line of the main screen",
    ),
    launcher_line(
        LAUNCHER_LOOKING_UP,
        "Looking for translations online...",
        "While the list of languages downloads",
    ),
    launcher_line(
        LAUNCHER_OFFLINE,
        "The translations cannot be reached online.",
        "The list of languages could not be downloaded",
    ),
    launcher_line(
        LAUNCHER_DOWNLOADS,
        "Downloads it from the translations' site.",
        "Help for a language's line",
    ),
    launcher_line(
        LAUNCHER_DOWNLOADING,
        "Downloading {name}...",
        "While a translation downloads; {name} is the language's name",
    ),
    launcher_line(
        LAUNCHER_DOWNLOAD_FAILED,
        "The download failed.",
        "A translation could not be downloaded",
    ),
    launcher_line(
        LAUNCHER_TRANSLATION_HELP,
        "X: choose   Z: back",
        "The help of the translation's and the saves' screens",
    ),
    launcher(
        LAUNCHER_SAVES,
        "Saves",
        "The options' line of the saves, and its screen's heading",
    ),
    launcher(
        LAUNCHER_SLOT,
        "Slot {slot}",
        "A save slot's line; {slot} is its number",
    ),
    launcher(
        LAUNCHER_SLOT_SAVED,
        "saved",
        "Beside a slot that holds a save",
    ),
    launcher(
        LAUNCHER_SLOT_EMPTY,
        "empty",
        "Beside a slot that holds none",
    ),
    launcher(
        LAUNCHER_EXPORT,
        "Export .sav...",
        "Writes a copy of the save where the player chooses, for an emulator or a flash cart",
    ),
    launcher(
        LAUNCHER_IMPORT,
        "Import...",
        "Replaces the slot's save with a file the player chooses",
    ),
    launcher_line(
        LAUNCHER_SAVES_HELP,
        "Copy saves to or from an emulator.",
        "Help for the saves' screen",
    ),
    launcher_line(
        LAUNCHER_EXPORT_HELP,
        "For an emulator or a flash cart.",
        "Help for Export .sav",
    ),
    launcher(
        LAUNCHER_EXPORT_SRM,
        "Export .srm...",
        "Writes a copy of the save under RetroArch's extension, the same bytes",
    ),
    launcher(
        LAUNCHER_EXPORT_STRICT,
        "Export as the cartridge...",
        "Writes a copy of the save without the port's notes (the name in full, the records), as the cartridge would hold it",
    ),
    launcher_line(
        LAUNCHER_EXPORT_SRM_HELP,
        "For RetroArch: the same save, as .srm.",
        "Help for Export .srm",
    ),
    launcher_line(
        LAUNCHER_EXPORT_STRICT_HELP,
        "Without the port's name and records.",
        "Help for Export as the cartridge",
    ),
    launcher(
        LAUNCHER_IMPORT_FILE,
        "File",
        "The import's confirmation: the line of what the chosen file holds",
    ),
    launcher_line(
        LAUNCHER_IMPORT_CHECK,
        "Replace? The slot's save is kept as .bak.",
        "The import's confirmation's help: what the file and the slot hold are shown above",
    ),
    launcher(
        LAUNCHER_SAVE_LEVEL,
        "Lv {level}, area {area}",
        "What a save holds, first line: the party's level and the area it was saved in",
    ),
    launcher(
        LAUNCHER_SAVE_MONEY,
        "{money} G",
        "Second line: its money; the time played follows when the port counted it",
    ),
    launcher(
        LAUNCHER_SAVE_PLAYED,
        "{hours}:{minutes} played",
        "The time played, hours and minutes",
    ),
    launcher(
        LAUNCHER_REPLACE,
        "Replace the slot's save",
        "The import's confirmation: replaces the slot's save with the file's",
    ),
    launcher(
        LAUNCHER_CANCEL,
        "Cancel",
        "The import's confirmation: leaves the slot as it is",
    ),
    launcher_line(
        LAUNCHER_IMPORT_HELP,
        "Replaces this save (kept as .bak).",
        "Help for Import",
    ),
    launcher_line(LAUNCHER_EXPORTED, "The save was exported.", "Export worked"),
    launcher_line(LAUNCHER_IMPORTED, "The save was imported.", "Import worked"),
    launcher_line(
        LAUNCHER_NOT_A_SAVE,
        "That file is not a Zoids Saga save.",
        "The file chosen to import is not a save of the game",
    ),
    launcher_line(
        LAUNCHER_COPY_FAILED,
        "The save could not be copied.",
        "Export or import failed",
    ),
    launcher_line(
        LAUNCHER_ROM_FIRST,
        "Choose your ROM first.",
        "The saves live beside the ROM, which is not chosen yet",
    ),
    launcher_line(
        LAUNCHER_ROM_VERIFIED,
        "Zoids Saga, a verified dump.",
        "The ROM is the known release",
    ),
    launcher_line(
        LAUNCHER_ROM_FIRST_RELEASE,
        "Original release: the port needs Rev 1.",
        "The ROM is the game's first release (Rev 0), which the port does not play",
    ),
    launcher_line(
        LAUNCHER_ROM_UNSUPPORTED,
        "Unknown dump: the port needs Rev 1.",
        "The ROM looks like the game but is not the known dump, which the port does not play",
    ),
    launcher_line(
        LAUNCHER_ROM_OTHER,
        "This ROM is not Zoids Saga.",
        "Another game, or a Zoids title the port does not play",
    ),
    launcher_line(
        LAUNCHER_ROM_UNREADABLE,
        "The ROM cannot be read.",
        "The file could not be opened or identified",
    ),
    launcher_line(
        LAUNCHER_TRANSLATION_READ,
        "{count} translated messages.",
        "The translation was read; {count} is its number of messages in plain digits",
    ),
    launcher_line(
        LAUNCHER_TRANSLATION_UNREADABLE,
        "The translation cannot be read.",
        "The file could not be opened or parsed",
    ),
    launcher_line(LAUNCHER_READY, "Ready to play.", "Help for the play line"),
    launcher(
        LAUNCHER_ABOUT,
        "About",
        "The line to the screen with the version, the license and the project's pages",
    ),
    launcher_line(
        LAUNCHER_PICK_ABOUT,
        "Version, license and the project's pages.",
        "Help for the about line",
    ),
    launcher(
        LAUNCHER_VERSION,
        "Version",
        "The label of the port's version on the about screen",
    ),
    launcher(
        LAUNCHER_LICENSE,
        "License",
        "The label of the port's license on the about screen",
    ),
    launcher(
        LAUNCHER_PROJECT_PAGE,
        "Project on GitHub",
        "The line of the port's repository, whose address follows on the next line",
    ),
    launcher(
        LAUNCHER_TRANSLATIONS_PAGE,
        "Translations on GitHub",
        "The line of the translations' repository, whose address follows on the next line",
    ),
    launcher_line(
        LAUNCHER_OPENS_PAGE,
        "Opens the page in your web browser.",
        "Help for a page's line on the about screen",
    ),
    launcher_line(
        LAUNCHER_PAGE_UNOPENED,
        "The web browser could not be opened.",
        "The page could not be opened",
    ),
    launcher_line(
        LAUNCHER_ABOUT_HELP,
        "X: open   Z: back",
        "The about screen's help",
    ),
    launcher_line(
        LAUNCHER_HELP,
        "Arrows: move   X: choose   Z: clear",
        "The controls, at the bottom",
    ),
];

/// Pixels a line of the launcher's panel may take, and one across its
/// screen.
const LAUNCHER_PIXELS: usize = 208;
const LAUNCHER_LINE_PIXELS: usize = 232;

const fn launcher_line(key: &'static str, text: &'static str, note: &'static str) -> PortText {
    PortText {
        key,
        text,
        note,
        pixels: LAUNCHER_LINE_PIXELS,
    }
}

const fn launcher(key: &'static str, text: &'static str, note: &'static str) -> PortText {
    PortText {
        key,
        text,
        note,
        pixels: LAUNCHER_PIXELS,
    }
}

/// Every message of the port, in the order the template lists them.
pub const PORT_TEXTS: &[PortText] = &[
    PortText {
        key: SLOT_LEVEL,
        text: "Ｌｖ{level}",
        note: "The party level after a save slot's name, 4 cells (32 pixels); {level} is up to 2 digits",
        pixels: 32,
    },
    PortText {
        key: SLOT_EMPTY,
        text: "データなし",
        note: "A save slot with no game, after its number: 14 cells (112 pixels)",
        pixels: 112,
    },
    PortText {
        key: SLOT_BROKEN,
        text: "こわれたデータ",
        note: "A save slot whose data is broken, after its number: 14 cells (112 pixels)",
        pixels: 112,
    },
    PortText {
        key: SLOT_DETAILS,
        text: "エリア{area}　所持金{money}Ｇ",
        note: "The second help line for a save slot's game: {area} is its area (1-10), {money} up to 7 digits; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: SLOT_SAVE_HELP,
        text: "どのスロットにセーブしますか？",
        note: "The help line while choosing the save slot to save into; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: SLOT_LOAD_HELP,
        text: "どのデータからつづけますか？",
        note: "The help line while choosing the save slot to continue; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: SLOT_QUESTION,
        text: "スロット{slot}にセーブしますか？",
        note: "The question before saving into an empty slot, {slot} its number; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: SLOT_OVERWRITE,
        text: "スロット{slot}に上書きしますか？",
        note: "The question before saving over a slot's game, {slot} its number; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: SLOT_AUTOSAVE,
        text: "Ａ",
        note: "The autosave's mark in the list of games to continue, where the other slots show their number: up to 2 cells (16 pixels)",
        pixels: 16,
    },
    PortText {
        key: SLOT_AUTOSAVE_HELP,
        text: "マップ移動時のオートセーブです",
        note: "The help line while the autosave, the game saved on each change of map, is under the cursor; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: OPTIONS_SPEED,
        text: "メッセージ速度",
        note: "The pause menu's settings, enhanced mode: the battle message speed's line, before its value at cell 13 (96 pixels)",
        pixels: 96,
    },
    PortText {
        key: OPTIONS_SPEED_HELP,
        text: "戦闘メッセージの速さ　１が速い",
        note: "The help line for the message speed, 1 fastest and 5 slowest; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: OPTIONS_ANIMATIONS,
        text: "戦闘アニメ",
        note: "The pause menu's settings, enhanced mode: the battle animations' line, before its value at cell 13 (96 pixels)",
        pixels: 96,
    },
    PortText {
        key: OPTIONS_ANIMATIONS_HELP,
        text: "ＯＦＦで攻撃シーンを省略",
        note: "The help line for the battle animations: off skips the attack scenes; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: OPTIONS_DAMAGE_NUMBERS,
        text: "ダメージ表示",
        note: "The pause menu's settings, enhanced mode: the damage numbers' line, before its value at cell 13 (96 pixels)",
        pixels: 96,
    },
    PortText {
        key: OPTIONS_DAMAGE_NUMBERS_HELP,
        text: "受けたダメージを数字で表示",
        note: "The help line for the damage numbers: on shows each unit's damage as a number under it; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: OPTIONS_AUTO_TEXT,
        text: "オート送り",
        note: "The pause menu's settings, enhanced mode: the auto text's line, before its value at cell 13 (96 pixels)",
        pixels: 96,
    },
    PortText {
        key: OPTIONS_AUTO_TEXT_HELP,
        text: "ＯＮで会話が自動で進む　ＳＥＬＥＣＴでも切替",
        note: "The help line for the auto text: on, the text boxes go on by themselves; SELECT turns it on or off while one shows; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: OPTIONS_AUTOSAVE,
        text: "オートセーブ",
        note: "The pause menu's settings, enhanced mode: the autosave's line, before its value at cell 13 (96 pixels)",
        pixels: 96,
    },
    PortText {
        key: OPTIONS_AUTOSAVE_HELP,
        text: "マップ移動時に専用のスロットへ記録",
        note: "The help line for the autosave: on, the game is saved into a slot of its own on each change of map, listed first when continuing; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: OPTIONS_WEAPON_REACH,
        text: "射程表示",
        note: "The pause menu's settings, enhanced mode: the weapons' reach's line, before its value at cell 13 (96 pixels)",
        pixels: 96,
    },
    PortText {
        key: OPTIONS_WEAPON_REACH_HELP,
        text: "武器の射程をマスで表示　ＳＥＬＥＣＴで列を切替",
        note: "The help line for the weapons' reach: on, the shops and the pause menu draw the cells of the battle's grid a weapon reaches; SELECT turns from the front row to the back; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: OPTIONS_FAST_FORWARD,
        text: "早送り",
        note: "The pause menu's settings, enhanced mode: the fast forward's speed's line, before its value at cell 13 (96 pixels)",
        pixels: 96,
    },
    PortText {
        key: OPTIONS_FAST_FORWARD_HELP,
        text: "早送りボタンを押している間の速さ",
        note: "The help line for the fast forward: how fast the game plays while its button (Space, or the on-screen pad's >>) is held; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: OPTIONS_FAST_FORWARD_VALUE,
        text: "{count}倍",
        note: "The fast forward's speed as its line shows it, {count} from 2 to 4; 48 pixels",
        pixels: 48,
    },
    PortText {
        key: OPTIONS_ON,
        text: "ＯＮ",
        note: "A setting that is on, in the pause menu's settings; 48 pixels",
        pixels: 48,
    },
    PortText {
        key: OPTIONS_OFF,
        text: "ＯＦＦ",
        note: "A setting that is off, in the pause menu's settings; 48 pixels",
        pixels: 48,
    },
    PortText {
        key: MENU_QUIT,
        text: "終了",
        note: "The enhanced mode's last line of the pause menu's main list, which leaves the game for the launcher (a port feature); as wide as the list's other lines, 56 pixels",
        pixels: 56,
    },
    PortText {
        key: MENU_QUIT_QUESTION,
        text: "ランチャーにもどりますか？",
        note: "The question on the help line's first line after choosing the quit line, over the original's yes/no window; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: MENU_QUIT_UNSAVED,
        text: "セーブしていない進行は失われます",
        note: "The help line's second line under that question: what was not saved is lost; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: MENU_STATS,
        text: "記録",
        note: "The enhanced mode's line of the pause menu's main list, after Save, that opens the statistics of the game played (a port feature); as wide as the list's other lines, 56 pixels",
        pixels: 56,
    },
    PortText {
        key: MENU_ACHIEVEMENTS,
        text: "実績",
        note: "The enhanced mode's line of the pause menu's main list, after the statistics, that opens the achievements (a port feature); as wide as the list's other lines, 56 pixels",
        pixels: 56,
    },
    PortText {
        key: ACHIEVEMENT_PAGE,
        text: "実績　{page}／{pages}　達成　{count}／{total}",
        note: "The achievements' first help line, with the page's number of the pages and the achievements unlocked of all of them; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: ACHIEVEMENT_UNLOCKED,
        text: "実績解除！",
        note: "The first line of the window that slides down at the field's top when an achievement is unlocked; the achievement's name follows on the second line; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: ACHIEVEMENT_MANY,
        text: "{count}件の実績",
        note: "That window's second line when several achievements were unlocked at once, as a game saved before is continued; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: STATS_PAGE_BATTLES,
        text: "記録　{page}／{pages}　戦い",
        note: "The statistics' first help line on the battles' page, with its number of the pages; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: STATS_PAGE_COLLECTION,
        text: "記録　{page}／{pages}　収集",
        note: "The same on the collection's page; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: STATS_PAGE_RECORDS,
        text: "記録　{page}／{pages}　その他",
        note: "The same on the page of the time played and the records; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: STATS_KEYS,
        text: "左右：ページ　Ｂ：もどる",
        note: "The statistics' second help line: left/right turn the page, B goes back; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: STATS_WON,
        text: "勝利",
        note: "Battles won: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_LOST,
        text: "敗北",
        note: "Battles lost: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_RETREATED,
        text: "退却",
        note: "Battles the party retreated from: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_ENEMIES,
        text: "倒した敵ゾイド",
        note: "Enemy Zoids destroyed: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_DESTROYED,
        text: "破壊された味方",
        note: "The party's Zoids destroyed: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_MONEY,
        text: "戦闘で得たお金",
        note: "Money the won battles brought: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_ZI_DATA,
        text: "Ｚｉデータ",
        note: "The Zi data held, of all the Zoids' ones: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_ZOIDS,
        text: "ゾイドの種類",
        note: "The kinds of Zoid the party has units of, of all the kinds: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_CHARACTERS,
        text: "人物図鑑",
        note: "The characters in the character guide, of all of them: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_COMMANDS,
        text: "デッキコマンド",
        note: "The deck commands learned, of all of them: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_TIME,
        text: "プレイ時間",
        note: "The time played: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_BEST_HIT,
        text: "最大ダメージ",
        note: "The most damage one blow of the party dealt: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_LONGEST,
        text: "最長の戦闘",
        note: "The most rounds a battle lasted: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_STORY,
        text: "ストーリー戦闘",
        note: "The story battles won, of all of them: the label of a line of the statistics, its value after it on the same line; 120 pixels",
        pixels: 120,
    },
    PortText {
        key: STATS_TIMES,
        text: "{count}回",
        note: "Times (battles): a value of the statistics, after its label; 96 pixels",
        pixels: 96,
    },
    PortText {
        key: STATS_UNITS,
        text: "{count}体",
        note: "Zoids (destroyed): a value of the statistics, after its label; 96 pixels",
        pixels: 96,
    },
    PortText {
        key: STATS_GOLD,
        text: "{money}Ｇ",
        note: "Money: a value of the statistics, after its label; 96 pixels",
        pixels: 96,
    },
    PortText {
        key: STATS_OF,
        text: "{count}／{total}",
        note: "A count of a total: a value of the statistics, after its label; 96 pixels",
        pixels: 96,
    },
    PortText {
        key: STATS_HOURS,
        text: "{hours}時間{minutes}分",
        note: "The time played, in hours and minutes: a value of the statistics, after its label; 96 pixels",
        pixels: 96,
    },
    PortText {
        key: STATS_DAMAGE,
        text: "{count}",
        note: "Damage: a value of the statistics, after its label; 96 pixels",
        pixels: 96,
    },
    PortText {
        key: STATS_ROUNDS,
        text: "{count}ターン",
        note: "The rounds of a battle: a value of the statistics, after its label; 96 pixels",
        pixels: 96,
    },
    PortText {
        key: OPTIONS_KEYS,
        text: "左右：変更　Ｂ：もどる",
        note: "The second help line of the pause menu's settings: left/right change, B goes back; 224 pixels",
        pixels: 224,
    },
    PortText {
        key: REACH_FRONT,
        text: "FRONT",
        note: "Over the grid of a weapon's reach: its user stands in the front row. In the port's small capitals alone (lower case shows as capitals); the plate widens past 27 pixels, up to 36",
        pixels: 36,
    },
    PortText {
        key: REACH_BACK,
        text: "BACK",
        note: "Over the grid of a weapon's reach: its user stands in the back row. In the port's small capitals alone; the plate widens past 27 pixels, up to 36",
        pixels: 36,
    },
    PortText {
        key: AUTOSAVE_NOTICE,
        text: "Autosaving",
        note: "The small notice at the field's top left while the enhanced mode's autosave is written, in the port's small capitals (lower case shows as capitals; characters they lack are skipped), followed by one to three dots the port adds; one line of 120 pixels",
        pixels: 120,
    },
];

/// The messages drawn in the port's small capitals, measured in them.
const SMALL_TEXTS: [&str; 3] = [REACH_FRONT, REACH_BACK, AUTOSAVE_NOTICE];

/// The widest value of each marker, to check a message's lines.
const WIDEST_VALUES: [(&str, u32); 10] = [
    ("level", 99),
    ("area", 10),
    ("money", 9_999_999),
    ("slot", 9),
    ("count", 4),
    ("total", 153),
    ("page", 3),
    ("pages", 3),
    ("hours", 999),
    ("minutes", 59),
];

/// The default text of the message `key`, if the port has one.
#[must_use]
pub fn default_text(key: &str) -> Option<&'static str> {
    all_texts()
        .find(|text| text.key == key)
        .map(|text| text.text)
}

/// The game's messages, the achievements' and the launcher's, in the
/// template's order.
pub fn all_texts() -> impl Iterator<Item = &'static PortText> {
    PORT_TEXTS
        .iter()
        .chain(crate::achievements::texts())
        .chain(LAUNCHER_TEXTS)
}

/// The text of the message `key`: an extension's answer, or the port's
/// Japanese text.
#[must_use]
pub fn port_text(extensions: &SharedExtensions, key: &str) -> String {
    extensions
        .borrow()
        .port_text(key)
        .or_else(|| default_text(key).map(str::to_owned))
        .unwrap_or_default()
}

/// `text` with each `{marker}` of `values` replaced by its number in
/// full-width digits.
#[must_use]
pub fn fill(text: &str, values: &[(&str, u32)]) -> String {
    values
        .iter()
        .fold(text.to_owned(), |text, (marker, value)| {
            text.replace(&format!("{{{marker}}}"), &full_width(*value))
        })
}

/// `value` in full-width digits.
#[must_use]
pub fn full_width(value: u32) -> String {
    value
        .to_string()
        .chars()
        .filter_map(|digit| char::from_u32(u32::from(digit) - u32::from('0') + u32::from('０')))
        .collect()
}

/// The messages of `texts` (key and text) whose lines are wider than
/// their message allows, measured with `metrics`, as problems to report.
#[must_use]
pub fn problems<'a>(
    texts: impl IntoIterator<Item = (&'a str, &'a str)>,
    metrics: &TextMetrics,
) -> Vec<String> {
    texts
        .into_iter()
        .filter_map(|(key, text)| {
            let limit = all_texts().find(|port| port.key == key)?.pixels;
            let small = SMALL_TEXTS.contains(&key);
            let widest = fill(text, &WIDEST_VALUES)
                .lines()
                .map(|line| {
                    if small {
                        metrics.small_width(line)
                    } else {
                        metrics.width(line)
                    }
                })
                .max()
                .unwrap_or(0);
            (widest > limit)
                .then(|| format!("{key}: needs {widest} pixels, the port allows {limit}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::{Extension, Extensions};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Spanish;

    impl Extension for Spanish {
        fn name(&self) -> &'static str {
            "spanish"
        }

        fn port_text(&self, key: &str) -> Option<String> {
            (key == SLOT_EMPTY).then(|| "Vacía".to_owned())
        }
    }

    #[test]
    fn every_key_is_under_the_prefix_and_its_default_fits() {
        let metrics = TextMetrics::standard();
        for text in all_texts() {
            assert!(text.key.starts_with(PORT_PREFIX), "{}", text.key);
        }
        let defaults = all_texts().map(|text| (text.key, text.text));
        assert_eq!(problems(defaults, &metrics), Vec::<String>::new());
    }

    #[test]
    fn an_extension_answers_before_the_default() {
        let extensions: SharedExtensions = Rc::new(RefCell::new(Extensions::default()));
        assert_eq!(port_text(&extensions, SLOT_EMPTY), "データなし");
        extensions.borrow_mut().insert(Box::new(Spanish));
        assert_eq!(port_text(&extensions, SLOT_EMPTY), "Vacía");
        assert_eq!(port_text(&extensions, SLOT_BROKEN), "こわれたデータ");
        assert_eq!(port_text(&extensions, "port/none"), "");
    }

    #[test]
    fn markers_take_full_width_numbers() {
        assert_eq!(fill("Ｌｖ{level}", &[("level", 12)]), "Ｌｖ１２");
        assert_eq!(
            fill("{slot}: {money}", &[("slot", 2), ("money", 305)]),
            "２: ３０５"
        );
    }

    #[test]
    fn a_line_too_wide_is_reported() {
        let metrics = TextMetrics::standard();
        let found = problems([(SLOT_LEVEL, "Level {level}")], &metrics);
        assert_eq!(found.len(), 1);
        assert!(found[0].starts_with(SLOT_LEVEL));
    }
}
