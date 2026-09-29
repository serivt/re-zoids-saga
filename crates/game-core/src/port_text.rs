//! The port's own messages: the text of what the port adds to the game,
//! such as the save slots, which the ROM has no strings for.
//!
//! Source of knowledge: this project's own writing. Each message has a key
//! under `port/`, which a translation's PO file uses as it uses the name
//! entry's reserved keys, and a text shown when no extension answers:
//! Japanese for what the game shows, English for the launcher's screen,
//! which comes before any ROM is read. A `{name}` marker in a message stands for a value the port
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

/// The thanks at the end of the demo, in the story box.
pub const DEMO_THANKS: &str = "port/demo/thanks";
/// The question after the thanks.
pub const DEMO_QUESTION: &str = "port/demo/question";
/// The notice once the game is saved at the end of the demo.
pub const DEMO_SAVED: &str = "port/demo/saved";

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
pub const LAUNCHER_FILTER: &str = "port/launcher/filter";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_SHARP: &str = "port/launcher/sharp";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_SMOOTH: &str = "port/launcher/smooth";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_ON: &str = "port/launcher/on";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_OFF: &str = "port/launcher/off";
/// See [`LAUNCHER_KEYBOARD`].
pub const LAUNCHER_VOLUME: &str = "port/launcher/volume";
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
        LAUNCHER_FILTER,
        "Filter",
        "The line of how the picture is scaled",
    ),
    launcher(
        LAUNCHER_SHARP,
        "sharp",
        "Scaled by whole multiples, square pixels",
    ),
    launcher(LAUNCHER_SMOOTH, "smooth", "Scaled to fill, blended pixels"),
    launcher(LAUNCHER_ON, "on", "A setting that is on"),
    launcher(LAUNCHER_OFF, "off", "A setting that is off"),
    launcher(LAUNCHER_VOLUME, "Volume", "The line of the sound's volume"),
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
        "The translation screen's help",
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
        key: DEMO_THANKS,
        text: "あそんでくれて　ありがとう！\nこの体験版は　ここまでです。\nつづきは　これからのバージョンで！",
        note: "The thanks once chapter 1, the end of the demo, is over: up to 3 lines of 224 pixels in the story box",
        pixels: 224,
    },
    PortText {
        key: DEMO_QUESTION,
        text: "ここまでの記録を　セーブしますか？",
        note: "The question after the demo's thanks, over the original's yes/no window: up to 3 lines of 224 pixels",
        pixels: 224,
    },
    PortText {
        key: DEMO_SAVED,
        text: "セーブしました。",
        note: "The notice once the game is saved at the end of the demo: up to 3 lines of 224 pixels",
        pixels: 224,
    },
];

/// The widest value of each marker, to check a message's lines.
const WIDEST_VALUES: [(&str, u32); 4] = [
    ("level", 99),
    ("area", 10),
    ("money", 9_999_999),
    ("slot", 9),
];

/// The default text of the message `key`, if the port has one.
#[must_use]
pub fn default_text(key: &str) -> Option<&'static str> {
    all_texts()
        .find(|text| text.key == key)
        .map(|text| text.text)
}

/// The game's messages and the launcher's, in the template's order.
pub fn all_texts() -> impl Iterator<Item = &'static PortText> {
    PORT_TEXTS.iter().chain(LAUNCHER_TEXTS)
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
            let widest = fill(text, &WIDEST_VALUES)
                .lines()
                .map(|line| metrics.width(line))
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
