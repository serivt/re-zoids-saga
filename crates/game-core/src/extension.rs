//! The contract mods and the engine's own features share: events the
//! game raises and hooks whose first answer wins (see
//! `docs/extensibility.md`). Extensions are asked in the order they were
//! added; the engine's built-in answer comes after all of them.

use std::cell::RefCell;
use std::rc::Rc;

use crate::translation::AlphabetPage;

/// A window rectangle in tiles: x, y, width, height.
pub type Rect = (u8, u8, u8, u8);

/// A sound the game asks for by meaning rather than by song number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameSound {
    /// Music of the title screen.
    TitleMusic,
    /// Music behind the name entry.
    NameEntryMusic,
    /// Music of the opening cutscene.
    OpeningMusic,
    /// Music of the first room once control begins.
    FirstRoomMusic,
    /// START pressed on the title.
    TitleStart,
    /// A door taken.
    Door,
}

/// Something that happened in the game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// One update ran; the frame number counts from the start.
    Frame(u64),
    /// The title screen appeared.
    TitleShown,
    /// The player confirmed a name.
    NameConfirmed(String),
    /// A map was entered.
    RoomEntered {
        /// Map number.
        map: usize,
        /// Arrival metatile.
        cell: (usize, usize),
    },
    /// An exit was taken.
    ExitTaken {
        /// Map left.
        map: usize,
        /// Exit index on it.
        exit: usize,
        /// Map arrived at.
        destination: usize,
    },
    /// The player talked to a character.
    Talk {
        /// Character index on the map.
        npc: usize,
        /// Dialogue string.
        dialogue: usize,
    },
    /// A script string started running.
    ScriptStarted {
        /// Script table.
        table: String,
        /// String index.
        index: usize,
    },
    /// A script string finished.
    ScriptEnded {
        /// Script table.
        table: String,
        /// String index.
        index: usize,
    },
    /// A message began printing.
    MessageShown {
        /// Script table.
        table: String,
        /// String index.
        index: usize,
        /// Offset of the message in the string.
        offset: usize,
        /// Window it prints in.
        window: u8,
    },
    /// A window opened.
    WindowOpened {
        /// Window id.
        id: u8,
        /// Rectangle in tiles.
        rect: Rect,
        /// Kind byte.
        kind: u8,
    },
    /// A window closed, or all of them.
    WindowClosed(Option<u8>),
    /// A song or sound effect was requested.
    SoundRequested(usize),
    /// A game flag changed.
    FlagChanged {
        /// The flag.
        flag: u16,
        /// Its new value.
        set: bool,
    },
    /// The pause menu opened.
    MenuOpened,
    /// The pause menu closed.
    MenuClosed,
}

/// A part that watches the game and may answer its questions. Every method
/// has a default: watch nothing, answer nothing.
pub trait Extension {
    /// A short unique name; adding an extension with a name in use replaces
    /// the old one.
    fn name(&self) -> &str;

    /// Sees every event, in order.
    fn on_event(&mut self, event: &Event) {
        let _ = event;
    }

    /// Text to show for a message instead of the ROM's.
    fn translate_message(&self, table: &str, index: usize, offset: usize) -> Option<String> {
        let _ = (table, index, offset);
        None
    }

    /// The rectangle a window should take instead of the one its script
    /// gives.
    fn fit_window(&self, table: &str, index: usize, id: u8, kind: u8, rect: Rect) -> Option<Rect> {
        let _ = (table, index, id, kind, rect);
        None
    }

    /// The song a map should play on entering.
    fn music_for_map(&self, map: usize) -> Option<usize> {
        let _ = map;
        None
    }

    /// The name entry's character pages.
    fn alphabet_pages(&self) -> Option<Vec<AlphabetPage>> {
        None
    }

    /// The name entry's help line.
    fn name_entry_help(&self) -> Option<String> {
        None
    }

    /// The song number for a game sound.
    fn sound_for(&self, sound: GameSound) -> Option<usize> {
        let _ = sound;
        None
    }
}

/// The extensions in order, and the questions the engine puts to them.
#[derive(Default)]
pub struct Extensions {
    list: Vec<Box<dyn Extension>>,
}

impl Extensions {
    /// Adds `extension`, replacing one of the same name in its place.
    pub fn insert(&mut self, extension: Box<dyn Extension>) {
        match self
            .list
            .iter()
            .position(|other| other.name() == extension.name())
        {
            Some(index) => self.list[index] = extension,
            None => self.list.push(extension),
        }
    }

    /// Removes the extension called `name`, if any.
    pub fn remove(&mut self, name: &str) {
        self.list.retain(|extension| extension.name() != name);
    }

    /// Number of extensions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Tells every extension about `event`.
    pub fn emit(&mut self, event: &Event) {
        for extension in &mut self.list {
            extension.on_event(event);
        }
    }

    fn first<T>(&self, ask: impl Fn(&dyn Extension) -> Option<T>) -> Option<T> {
        self.list
            .iter()
            .find_map(|extension| ask(extension.as_ref()))
    }

    /// The first translation of a message.
    #[must_use]
    pub fn translate_message(&self, table: &str, index: usize, offset: usize) -> Option<String> {
        self.first(|extension| extension.translate_message(table, index, offset))
    }

    /// The first rectangle offered for a window.
    #[must_use]
    pub fn fit_window(
        &self,
        table: &str,
        index: usize,
        id: u8,
        kind: u8,
        rect: Rect,
    ) -> Option<Rect> {
        self.first(|extension| extension.fit_window(table, index, id, kind, rect))
    }

    /// The first song offered for a map.
    #[must_use]
    pub fn music_for_map(&self, map: usize) -> Option<usize> {
        self.first(|extension| extension.music_for_map(map))
    }

    /// The first character pages offered for the name entry.
    #[must_use]
    pub fn alphabet_pages(&self) -> Option<Vec<AlphabetPage>> {
        self.first(|extension| extension.alphabet_pages())
    }

    /// The first help line offered for the name entry.
    #[must_use]
    pub fn name_entry_help(&self) -> Option<String> {
        self.first(|extension| extension.name_entry_help())
    }

    /// The first song number offered for a game sound.
    #[must_use]
    pub fn sound_for(&self, sound: GameSound) -> Option<usize> {
        self.first(|extension| extension.sound_for(sound))
    }
}

/// Extensions shared by the game and the parts that raise events or ask
/// questions from inside a frame; the game is single-threaded and hooks
/// never raise events, so the cell is never borrowed twice.
pub type SharedExtensions = Rc<RefCell<Extensions>>;

#[cfg(test)]
mod tests {
    use super::*;

    struct Counter {
        seen: Vec<Event>,
        song: usize,
    }

    impl Extension for Counter {
        fn name(&self) -> &'static str {
            "counter"
        }
        fn on_event(&mut self, event: &Event) {
            self.seen.push(event.clone());
        }
        fn music_for_map(&self, map: usize) -> Option<usize> {
            (map == 4).then_some(self.song)
        }
    }

    #[test]
    fn extensions_see_events_in_order_and_the_first_answer_wins() {
        let mut extensions = Extensions::default();
        extensions.insert(Box::new(Counter {
            seen: Vec::new(),
            song: 7,
        }));
        extensions.insert(Box::new(Counter {
            seen: Vec::new(),
            song: 9,
        }));
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions.music_for_map(4), Some(9));
        assert_eq!(extensions.music_for_map(5), None);
        assert_eq!(extensions.translate_message("dialogue", 40, 12), None);
        extensions.emit(&Event::TitleShown);
        extensions.emit(&Event::Frame(1));
        extensions.remove("counter");
        assert!(extensions.is_empty());
    }
}
