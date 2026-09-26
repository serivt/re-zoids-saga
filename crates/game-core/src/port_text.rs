//! The port's own messages: the text of what the port adds to the game,
//! such as the save slots, which the ROM has no strings for.
//!
//! Source of knowledge: this project's own writing. Each message has a key
//! under `port/`, which a translation's PO file uses as it uses the name
//! entry's reserved keys, and a Japanese text shown when no extension
//! answers. A `{name}` marker in a message stands for a value the port
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
    PORT_TEXTS
        .iter()
        .find(|text| text.key == key)
        .map(|text| text.text)
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
            let limit = PORT_TEXTS.iter().find(|port| port.key == key)?.pixels;
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
        for text in PORT_TEXTS {
            assert!(text.key.starts_with(PORT_PREFIX), "{}", text.key);
        }
        let defaults = PORT_TEXTS.iter().map(|text| (text.key, text.text));
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
