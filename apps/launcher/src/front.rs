//! The launcher's own screen, shown when no ROM is given on the command
//! line: the project's name and version, the ROM and the translation to
//! play with, chosen in the system's file dialog, the options (the
//! keyboard's keys, the gamepad's buttons, the window, the filter and the
//! volume), the screen about the port (its version, license and the
//! project's pages, opened in the web browser), and the lines that start
//! the game or quit.
//!
//! It is drawn with this project's Latin font and colors alone, since no
//! ROM has been read yet. Its texts are English until a translation is
//! chosen, and then that translation's (`port/launcher/...`, see
//! `docs/translation.md`).

use std::path::{Path, PathBuf};

use anyhow::Result;
use extraction::{Identification, IdentifyError, Title};
use game_core::port_text::{
    LAUNCHER_ABOUT, LAUNCHER_ABOUT_HELP, LAUNCHER_BACK, LAUNCHER_CONTROLS_HELP,
    LAUNCHER_DEFAULT_KEYS, LAUNCHER_DOWN, LAUNCHER_FILTER, LAUNCHER_FULLSCREEN, LAUNCHER_GAMEPAD,
    LAUNCHER_HELP, LAUNCHER_KEYBOARD, LAUNCHER_KEYS_CUSTOM, LAUNCHER_KEYS_DEFAULT, LAUNCHER_LEFT,
    LAUNCHER_LICENSE, LAUNCHER_NO_GAMEPAD, LAUNCHER_NO_ROM, LAUNCHER_NO_TRANSLATION, LAUNCHER_OFF,
    LAUNCHER_ON, LAUNCHER_OPENS_PAGE, LAUNCHER_OPTIONS, LAUNCHER_OPTIONS_HELP,
    LAUNCHER_PAGE_UNOPENED, LAUNCHER_PICK_ABOUT, LAUNCHER_PICK_OPTIONS, LAUNCHER_PICK_ROM,
    LAUNCHER_PICK_TRANSLATION, LAUNCHER_PLAY, LAUNCHER_PRESS_KEY, LAUNCHER_PRESS_PAD,
    LAUNCHER_PROJECT_PAGE, LAUNCHER_QUIT, LAUNCHER_READY, LAUNCHER_RIGHT, LAUNCHER_ROM,
    LAUNCHER_ROM_FIRST_RELEASE, LAUNCHER_ROM_OTHER, LAUNCHER_ROM_UNREADABLE,
    LAUNCHER_ROM_UNSUPPORTED, LAUNCHER_ROM_VERIFIED, LAUNCHER_SHARP, LAUNCHER_SMOOTH,
    LAUNCHER_SUBTITLE, LAUNCHER_TRANSLATION, LAUNCHER_TRANSLATION_READ,
    LAUNCHER_TRANSLATION_UNREADABLE, LAUNCHER_TRANSLATIONS_PAGE, LAUNCHER_UP, LAUNCHER_VERSION,
    LAUNCHER_VOLUME, LAUNCHER_WINDOW, default_text,
};
use game_core::{TextMetrics, Translation};
use platform::{Button, Display, Event, Frame, Input, Rgb};
use platform_sdl3::{
    FileChoice, Filter, KeyMap, Sdl3Display, default_keys, default_pad_buttons, key_name, open_url,
    pad_button_label, pad_button_name,
};

use crate::quit::QuitPrompt;
use crate::settings::{FULL_VOLUME, SCALES, Settings};

/// The project's name, never translated.
pub const PROJECT_NAME: &str = "Re:Zoids Saga";
/// The port's version, the workspace's.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The port's license, the workspace's.
const LICENSE: &str = env!("CARGO_PKG_LICENSE");
/// The project's pages: the port's repository and the translations'.
const PROJECT_URL: &str = "https://github.com/serivt/re-zoids-saga";
const TRANSLATIONS_URL: &str = "https://github.com/serivt/re-zoids-saga-translations";
/// What the pages' addresses show, their site left out to fit the panel.
const SITE_PREFIX: &str = "https://github.com/";
const ROM_FILTERS: [(&str, &str); 1] = [("Game Boy Advance ROM", "gba")];
const TRANSLATION_FILTERS: [(&str, &str); 1] = [("Translation (PO)", "po")];
/// Pixels kept clear along every edge of the screen.
const MARGIN: usize = 8;
const TITLE_SCALE: usize = 2;
const TITLE_Y: usize = 10;
const SUBTITLE_Y: usize = 32;
const PANEL: (usize, usize, usize, usize) = (MARGIN, 46, 240 - 2 * MARGIN, 82);
const FIRST_LINE_Y: usize = 53;
const LINE_HEIGHT: usize = 12;
const OPTIONS_FIRST_LINE_Y: usize = 51;
const OPTION_LINE_HEIGHT: usize = 10;
/// Rows the about screen's page lines skip, their address below them.
const PAGE_ROWS: usize = 2;
/// How far a page's address sits right of its label.
const ADDRESS_INDENT: usize = 8;
const KEY_LINE_HEIGHT: usize = 11;
const CURSOR_X: usize = MARGIN + 6;
const LABEL_X: usize = MARGIN + 14;
const VALUE_GAP: usize = 8;
const COLUMN_WIDTH: usize = 102;
const STATUS_Y: usize = 134;
/// The version's row, top right.
const VERSION_Y: usize = MARGIN;
const HELP_Y: usize = 144;
const VOLUME_STEP: u8 = 10;
const ELLIPSIS: &str = "...";
const BACKDROP_TOP: Rgb = Rgb::new(10, 16, 40);
const BACKDROP_BOTTOM: Rgb = Rgb::new(30, 44, 88);
const PANEL_FILL: Rgb = Rgb::new(16, 26, 58);
const PANEL_BORDER: Rgb = Rgb::new(96, 120, 176);
pub(crate) const TITLE_COLOR: Rgb = Rgb::new(248, 208, 72);
const SHADOW: Rgb = Rgb::new(4, 6, 16);
pub(crate) const TEXT: Rgb = Rgb::new(232, 236, 248);
pub(crate) const DIM: Rgb = Rgb::new(140, 152, 184);
const UNAVAILABLE: Rgb = Rgb::new(72, 84, 120);
const GOOD: Rgb = Rgb::new(120, 224, 136);
pub(crate) const WARNING: Rgb = Rgb::new(240, 200, 96);
const BAD: Rgb = Rgb::new(240, 112, 104);

/// What the launcher settled on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// The ROM to play.
    pub rom: PathBuf,
    /// The translation to play with.
    pub translation: Option<PathBuf>,
    /// The options to play with.
    pub settings: Settings,
}

/// The lines of the main screen, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    Rom,
    Translation,
    Options,
    About,
    Play,
    Quit,
}

const LINES: [Line; 6] = [
    Line::Rom,
    Line::Translation,
    Line::Options,
    Line::About,
    Line::Play,
    Line::Quit,
];

/// The lines of the about screen the cursor stops on, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Project,
    Translations,
    Back,
}

const PAGES: [Page; 3] = [Page::Project, Page::Translations, Page::Back];

/// The lines of the options screen, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Setting {
    Keyboard,
    Gamepad,
    Window,
    Fullscreen,
    Filter,
    Volume,
    Back,
}

const SETTINGS: [Setting; 7] = [
    Setting::Keyboard,
    Setting::Gamepad,
    Setting::Window,
    Setting::Fullscreen,
    Setting::Filter,
    Setting::Volume,
    Setting::Back,
];

/// What a binding screen binds the buttons to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Device {
    Keyboard,
    Gamepad,
}

/// A binding screen's entries: the buttons in two columns of five, then a
/// row with the defaults and the way back.
const BUTTON_ROWS: usize = 5;
const DEFAULTS_ENTRY: usize = Button::ALL.len();
const BACK_ENTRY: usize = DEFAULTS_ENTRY + 1;

/// What the chosen ROM is. Only the verified dump of Zoids Saga (Japan,
/// Rev 1) plays: every table the port reads sits where that release keeps
/// it, and the first release (Rev 0) keeps its data elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomKind {
    Verified,
    FirstRelease,
    Unsupported,
    Other,
    Unreadable,
}

impl RomKind {
    /// Identifies the ROM at `path`: another game's header makes it
    /// [`Self::Other`], a file without a valid header [`Self::Unreadable`].
    fn of(path: &Path) -> Self {
        match std::fs::read(path).map(|bytes| extraction::identify(&bytes)) {
            Ok(Ok(found)) => Self::identified(&found),
            Ok(Err(IdentifyError::UnsupportedGame { .. })) => Self::Other,
            Ok(Err(_)) | Err(_) => Self::Unreadable,
        }
    }

    /// What an identified ROM is.
    pub fn identified(found: &Identification) -> Self {
        if found.title != Title::Saga {
            Self::Other
        } else if found.known_release.is_some() {
            Self::Verified
        } else if found.header.version == 0 {
            Self::FirstRelease
        } else {
            Self::Unsupported
        }
    }

    /// Whether the port plays it.
    pub fn playable(self) -> bool {
        self == Self::Verified
    }

    /// The key of the port's message that tells what it is.
    pub fn message(self) -> &'static str {
        match self {
            Self::Verified => LAUNCHER_ROM_VERIFIED,
            Self::FirstRelease => LAUNCHER_ROM_FIRST_RELEASE,
            Self::Unsupported => LAUNCHER_ROM_UNSUPPORTED,
            Self::Other => LAUNCHER_ROM_OTHER,
            Self::Unreadable => LAUNCHER_ROM_UNREADABLE,
        }
    }
}

/// Which screen is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Main,
    /// The options, with the line under the cursor.
    Options(usize),
    /// The screen about the port, with the line under the cursor.
    About(usize),
    /// The keys or the gamepad buttons of the pad's buttons, the entry
    /// under the cursor, and whether it waits for the button's new one.
    Bindings {
        device: Device,
        entry: usize,
        waiting: bool,
    },
}

/// What happened in a frame of the screen.
enum Step {
    Stay,
    Play,
    Quit,
}

/// The launcher's screen.
pub struct Front {
    rom: Option<(PathBuf, RomKind)>,
    translation: Option<(PathBuf, Option<Translation>)>,
    keys: KeyMap,
    pad_buttons: KeyMap,
    scale: u32,
    fullscreen: bool,
    filter: Filter,
    volume: u8,
    gamepads: Vec<String>,
    /// Whether a binding screen caught its key or gamepad button this
    /// frame, which the frame's buttons then leave alone.
    caught: bool,
    /// The question asked before closing, while open.
    quitting: Option<QuitPrompt>,
    /// Whether the last page asked for could not be opened.
    unopened: bool,
    /// Where the screen was last touched or clicked, until a frame takes
    /// it as the line under it chosen.
    tapped: Option<(i32, i32)>,
    line: usize,
    screen: Screen,
    choosing: Option<(Line, FileChoice)>,
    previous: Input,
    metrics: TextMetrics,
}

/// `defaults` with the entries `chosen` gives in place of theirs.
fn bound(defaults: KeyMap, chosen: &[(Button, String)]) -> KeyMap {
    defaults
        .into_iter()
        .map(|(button, name)| {
            let name = chosen
                .iter()
                .find(|(mapped, _)| *mapped == button)
                .map_or(name, |(_, chosen)| chosen.clone());
            (button, name)
        })
        .collect()
}

/// The entries of `map` that differ from `defaults`.
fn changed(map: &KeyMap, defaults: &KeyMap) -> KeyMap {
    map.iter()
        .filter(|entry| !defaults.contains(entry))
        .cloned()
        .collect()
}

impl Front {
    /// The screen with `settings`' choices, those still readable.
    #[must_use]
    pub fn new(settings: &Settings) -> Self {
        let mut front = Self {
            rom: None,
            translation: None,
            keys: bound(default_keys(), &settings.keys),
            pad_buttons: bound(default_pad_buttons(), &settings.pad_buttons),
            scale: settings.scale,
            fullscreen: settings.fullscreen,
            filter: settings.filter,
            volume: settings.volume,
            gamepads: Vec::new(),
            caught: false,
            quitting: None,
            tapped: None,
            unopened: false,
            line: 0,
            screen: Screen::Main,
            choosing: None,
            previous: Input::default(),
            metrics: TextMetrics::standard(),
        };
        if let Some(path) = settings.rom.as_deref().filter(|path| path.exists()) {
            front.take_rom(path.to_path_buf());
        }
        if let Some(path) = settings.translation.as_deref().filter(|path| path.exists()) {
            front.take_translation(path.to_path_buf());
        }
        if front.playable() {
            front.line = line_index(Line::Play);
        }
        front
    }

    /// The choices to remember: the keys and gamepad buttons only where
    /// they differ from the defaults.
    #[must_use]
    pub fn settings(&self) -> Settings {
        Settings {
            rom: self.rom.as_ref().map(|(path, _)| path.clone()),
            translation: self.translation.as_ref().map(|(path, _)| path.clone()),
            keys: changed(&self.keys, &default_keys()),
            pad_buttons: changed(&self.pad_buttons, &default_pad_buttons()),
            scale: self.scale,
            fullscreen: self.fullscreen,
            filter: self.filter,
            volume: self.volume,
        }
    }

    fn playable(&self) -> bool {
        self.rom.as_ref().is_some_and(|(_, kind)| kind.playable())
    }

    fn take_rom(&mut self, path: PathBuf) {
        let kind = RomKind::of(&path);
        self.rom = Some((path, kind));
    }

    fn take_translation(&mut self, path: PathBuf) {
        let translation = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| Translation::from_po(&text).ok());
        self.translation = Some((path, translation));
    }

    fn map(&self, device: Device) -> &KeyMap {
        match device {
            Device::Keyboard => &self.keys,
            Device::Gamepad => &self.pad_buttons,
        }
    }

    /// Gives `button` the key or gamepad button `name` of `device`; a
    /// button that had it takes the button's old one, so nothing moves two
    /// buttons.
    fn bind(&mut self, device: Device, button: Button, name: &str) {
        let map = match device {
            Device::Keyboard => &mut self.keys,
            Device::Gamepad => &mut self.pad_buttons,
        };
        let old = map
            .iter()
            .find(|(bound, _)| *bound == button)
            .map(|(_, name)| name.clone());
        for entry in map.iter_mut() {
            if entry.0 == button {
                name.clone_into(&mut entry.1);
            } else if entry.1 == name
                && let Some(old) = &old
            {
                entry.1.clone_from(old);
            }
        }
    }

    /// A text of the screen in the chosen translation's language, else
    /// English.
    fn text(&self, key: &str) -> String {
        self.translation
            .as_ref()
            .and_then(|(_, translation)| translation.as_ref())
            .and_then(|translation| translation.port_text(key))
            .or_else(|| default_text(key))
            .unwrap_or_default()
            .to_owned()
    }

    /// The events of a frame: the key or gamepad button a button waits
    /// for, Escape to go back, or on the main screen to ask before closing
    /// and again to stay. Returns whether the launcher should close.
    fn events(&mut self, events: &[Event]) -> bool {
        for event in events {
            match (*event, self.screen) {
                (Event::Quit, _) => return true,
                (
                    Event::Back,
                    Screen::Bindings {
                        device,
                        entry,
                        waiting,
                    },
                ) => {
                    self.screen = if waiting {
                        Screen::Bindings {
                            device,
                            entry,
                            waiting: false,
                        }
                    } else {
                        device.options()
                    };
                }
                (Event::Back, Screen::Options(_) | Screen::About(_)) => {
                    self.screen = Screen::Main;
                }
                (Event::Back, Screen::Main) if self.choosing.is_none() => {
                    self.quitting = match self.quitting {
                        Some(_) => None,
                        None => Some(QuitPrompt::new(self.previous, false)),
                    };
                }
                (
                    Event::Key(code),
                    Screen::Bindings {
                        device: Device::Keyboard,
                        entry,
                        waiting: true,
                    },
                ) => self.caught(Device::Keyboard, entry, key_name(code)),
                (
                    Event::Pad(code),
                    Screen::Bindings {
                        device: Device::Gamepad,
                        entry,
                        waiting: true,
                    },
                ) => self.caught(Device::Gamepad, entry, pad_button_name(code)),
                (Event::Pointer { x, y }, _) => self.tapped = Some((x, y)),
                _ => {}
            }
        }
        false
    }

    /// The key or gamepad button `name` a binding screen waited for.
    fn caught(&mut self, device: Device, entry: usize, name: Option<String>) {
        if let (Some(button), Some(name)) = (Button::ALL.get(entry), name) {
            self.bind(device, *button, &name);
        }
        self.caught = true;
        self.screen = Screen::Bindings {
            device,
            entry,
            waiting: false,
        };
    }

    /// A frame of the buttons, or of a dialog's answer while one is open.
    fn update(&mut self, display: &Sdl3Display, input: Input) -> Result<Step> {
        let mut pressed = Button::ALL
            .into_iter()
            .filter(|&button| input.is_held(button) && !self.previous.is_held(button))
            .fold(Input::default(), Input::with);
        self.previous = input;
        let tapped = self.tapped.take();
        if std::mem::take(&mut self.caught) {
            return Ok(Step::Stay);
        }
        if let Some(prompt) = &mut self.quitting {
            match prompt.update(input) {
                Some(true) => return Ok(Step::Quit),
                Some(false) => self.quitting = None,
                None => {}
            }
            return Ok(Step::Stay);
        }
        if let Some((line, choice)) = &self.choosing {
            if let Some(answer) = choice.answer() {
                let line = *line;
                self.choosing = None;
                match (line, answer) {
                    (Line::Rom, Some(path)) => self.take_rom(keep(path, KEPT_ROM)),
                    (Line::Translation, Some(path)) => {
                        let name = kept_translation_name(&path);
                        self.take_translation(keep(path, &name));
                    }
                    _ => {}
                }
            }
            return Ok(Step::Stay);
        }
        if let Some((x, y)) = tapped {
            pressed = pressed.union(self.tap(x, y));
        }
        match self.screen {
            Screen::Main => return self.update_main(display, pressed),
            Screen::Options(line) => self.update_options(line, pressed),
            Screen::About(line) => self.update_about(line, pressed),
            Screen::Bindings {
                device,
                entry,
                waiting: false,
            } => self.update_bindings(device, entry, pressed),
            Screen::Bindings { .. } => {}
        }
        Ok(Step::Stay)
    }

    /// A touch or a click at (`x`, `y`) of the frame: the line under it
    /// becomes the selected one and is chosen, as X does; on the window's
    /// size and the volume, the left half of the screen lowers them and the
    /// right half raises them. Returns the buttons it stands for.
    fn tap(&mut self, x: i32, y: i32) -> Input {
        let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
            return Input::default();
        };
        if !(PANEL.0..PANEL.0 + PANEL.2).contains(&x) {
            return Input::default();
        }
        let chosen = Input::default().with(Button::A);
        match self.screen {
            Screen::Main => {
                let Some(line) = row_at(y, FIRST_LINE_Y, LINE_HEIGHT, LINES.len()) else {
                    return Input::default();
                };
                self.line = line;
                chosen
            }
            Screen::Options(_) => {
                let Some(line) =
                    row_at(y, OPTIONS_FIRST_LINE_Y, OPTION_LINE_HEIGHT, SETTINGS.len())
                else {
                    return Input::default();
                };
                self.screen = Screen::Options(line);
                match SETTINGS[line] {
                    Setting::Window | Setting::Volume if x < SCREEN_MIDDLE => {
                        Input::default().with(Button::Left)
                    }
                    Setting::Window | Setting::Volume => Input::default().with(Button::Right),
                    _ => chosen,
                }
            }
            Screen::About(_) => {
                let Some(line) = about_rows().iter().position(|&(top, rows)| {
                    (top..top + rows * OPTION_LINE_HEIGHT).contains(&(y + 1))
                }) else {
                    return Input::default();
                };
                if Screen::About(line) != self.screen {
                    self.unopened = false;
                }
                self.screen = Screen::About(line);
                chosen
            }
            Screen::Bindings { .. } => Input::default(),
        }
    }

    fn update_main(&mut self, display: &Sdl3Display, pressed: Input) -> Result<Step> {
        if pressed.is_held(Button::Up) {
            self.line = self.line.saturating_sub(1);
        }
        if pressed.is_held(Button::Down) {
            self.line = (self.line + 1).min(LINES.len() - 1);
        }
        let line = LINES[self.line];
        if pressed.is_held(Button::B) && line == Line::Translation {
            self.translation = None;
        }
        if pressed.is_held(Button::Start) && self.playable() {
            return Ok(Step::Play);
        }
        if !pressed.is_held(Button::A) {
            return Ok(Step::Stay);
        }
        match line {
            Line::Rom => {
                let location = self.rom.as_ref().and_then(|(path, _)| path.parent());
                let choice = display.choose_file(&ROM_FILTERS, location)?;
                self.choosing = Some((Line::Rom, choice));
            }
            Line::Translation => {
                let location = self
                    .translation
                    .as_ref()
                    .and_then(|(path, _)| path.parent())
                    .or_else(|| self.rom.as_ref().and_then(|(path, _)| path.parent()));
                let choice = display.choose_file(&TRANSLATION_FILTERS, location)?;
                self.choosing = Some((Line::Translation, choice));
            }
            Line::Options => self.screen = Screen::Options(0),
            Line::About => {
                self.unopened = false;
                self.screen = Screen::About(0);
            }
            Line::Play if self.playable() => return Ok(Step::Play),
            Line::Play => {}
            Line::Quit => return Ok(Step::Quit),
        }
        Ok(Step::Stay)
    }

    /// The options screen: up and down move, left and right change a value,
    /// X opens the keyboard's or the gamepad's buttons, switches the
    /// fullscreen and the filter, or goes back, and so does Z.
    fn update_options(&mut self, line: usize, pressed: Input) {
        if pressed.is_held(Button::B) {
            self.screen = Screen::Main;
            return;
        }
        let mut line = line;
        if pressed.is_held(Button::Up) {
            line = line.saturating_sub(1);
        }
        if pressed.is_held(Button::Down) {
            line = (line + 1).min(SETTINGS.len() - 1);
        }
        self.screen = Screen::Options(line);
        let less = pressed.is_held(Button::Left);
        let more = pressed.is_held(Button::Right);
        let chosen = pressed.is_held(Button::A);
        match SETTINGS[line] {
            Setting::Keyboard if chosen => self.screen = Device::Keyboard.bindings(),
            Setting::Gamepad if chosen => self.screen = Device::Gamepad.bindings(),
            Setting::Window if less && self.scale > *SCALES.start() => self.scale -= 1,
            Setting::Window if more && self.scale < *SCALES.end() => self.scale += 1,
            Setting::Fullscreen if less || more || chosen => self.fullscreen = !self.fullscreen,
            Setting::Filter if less || more || chosen => {
                self.filter = match self.filter {
                    Filter::Sharp => Filter::Smooth,
                    Filter::Smooth => Filter::Sharp,
                };
            }
            Setting::Volume if less => self.volume = self.volume.saturating_sub(VOLUME_STEP),
            Setting::Volume if more => {
                self.volume = self.volume.saturating_add(VOLUME_STEP).min(FULL_VOLUME);
            }
            Setting::Back if chosen => self.screen = Screen::Main,
            _ => {}
        }
    }

    /// The about screen: up and down move, X opens a page in the web
    /// browser or goes back, and so does Z.
    fn update_about(&mut self, line: usize, pressed: Input) {
        if pressed.is_held(Button::B) {
            self.screen = Screen::Main;
            return;
        }
        let previous = line;
        let mut line = line;
        if pressed.is_held(Button::Up) {
            line = line.saturating_sub(1);
        }
        if pressed.is_held(Button::Down) {
            line = (line + 1).min(PAGES.len() - 1);
        }
        if line != previous {
            self.unopened = false;
        }
        self.screen = Screen::About(line);
        if !pressed.is_held(Button::A) {
            return;
        }
        match PAGES[line] {
            Page::Back => self.screen = Screen::Main,
            page => {
                self.unopened = page.url().is_some_and(|url| open_url(url).is_err());
            }
        }
    }

    /// A binding screen: the cursor moves down each column and across, X
    /// waits for a button's key or gamepad button, takes the defaults or
    /// goes back, Z goes back.
    fn update_bindings(&mut self, device: Device, entry: usize, pressed: Input) {
        if pressed.is_held(Button::B) {
            self.screen = device.options();
            return;
        }
        let mut entry = entry;
        if pressed.is_held(Button::Up) {
            entry = match entry {
                DEFAULTS_ENTRY => BUTTON_ROWS - 1,
                BACK_ENTRY => DEFAULTS_ENTRY - 1,
                entry if entry % BUTTON_ROWS > 0 => entry - 1,
                entry => entry,
            };
        }
        if pressed.is_held(Button::Down) {
            entry = match entry {
                DEFAULTS_ENTRY | BACK_ENTRY => entry,
                entry if entry % BUTTON_ROWS + 1 < BUTTON_ROWS => entry + 1,
                entry if entry < BUTTON_ROWS => DEFAULTS_ENTRY,
                _ => BACK_ENTRY,
            };
        }
        if pressed.is_held(Button::Left) || pressed.is_held(Button::Right) {
            entry = match entry {
                DEFAULTS_ENTRY => BACK_ENTRY,
                BACK_ENTRY => DEFAULTS_ENTRY,
                entry => (entry + BUTTON_ROWS) % DEFAULTS_ENTRY,
            };
        }
        let chosen = pressed.is_held(Button::A);
        if chosen && entry == DEFAULTS_ENTRY {
            match device {
                Device::Keyboard => self.keys = default_keys(),
                Device::Gamepad => self.pad_buttons = default_pad_buttons(),
            }
        }
        if chosen && entry == BACK_ENTRY {
            self.screen = device.options();
            return;
        }
        self.screen = Screen::Bindings {
            device,
            entry,
            waiting: chosen && entry < DEFAULTS_ENTRY,
        };
    }

    /// Draws the screen.
    pub fn draw(&self, frame: &mut Frame) {
        draw_backdrop(frame);
        let title_width = self.metrics.plain_width(PROJECT_NAME, TITLE_SCALE);
        let title_x = frame.width().saturating_sub(title_width) / 2;
        self.metrics.draw_plain(
            frame,
            (title_x + 1, TITLE_Y + 2),
            PROJECT_NAME,
            SHADOW,
            TITLE_SCALE,
        );
        self.metrics.draw_plain(
            frame,
            (title_x, TITLE_Y),
            PROJECT_NAME,
            TITLE_COLOR,
            TITLE_SCALE,
        );
        let version = format!("v{VERSION}");
        let version_x = frame
            .width()
            .saturating_sub(MARGIN + self.metrics.plain_width(&version, 1));
        self.metrics
            .draw_plain(frame, (version_x, VERSION_Y), &version, DIM, 1);
        draw_panel(frame, PANEL);
        match self.screen {
            Screen::Main => {
                self.centered(frame, SUBTITLE_Y, &self.text(LAUNCHER_SUBTITLE), DIM);
                self.draw_main(frame);
                let (status, status_color) = self.status();
                self.centered(frame, STATUS_Y, &status, status_color);
                self.centered(frame, HELP_Y, &self.text(LAUNCHER_HELP), DIM);
            }
            Screen::Options(line) => {
                self.centered(frame, SUBTITLE_Y, &self.text(LAUNCHER_OPTIONS), DIM);
                self.draw_options(frame, line);
                self.centered(frame, HELP_Y, &self.text(LAUNCHER_OPTIONS_HELP), DIM);
            }
            Screen::About(line) => {
                self.centered(frame, SUBTITLE_Y, &self.text(LAUNCHER_ABOUT), DIM);
                self.draw_about(frame, line);
                let (status, color) = match PAGES[line] {
                    Page::Back => (String::new(), DIM),
                    _ if self.unopened => (self.text(LAUNCHER_PAGE_UNOPENED), BAD),
                    _ => (self.text(LAUNCHER_OPENS_PAGE), DIM),
                };
                self.centered(frame, STATUS_Y, &status, color);
                self.centered(frame, HELP_Y, &self.text(LAUNCHER_ABOUT_HELP), DIM);
            }
            Screen::Bindings {
                device,
                entry,
                waiting,
            } => {
                self.centered(frame, SUBTITLE_Y, &self.text(device.heading()), DIM);
                self.draw_bindings(frame, device, entry);
                if waiting {
                    let button = Button::ALL
                        .get(entry)
                        .map(|button| self.button_label(*button))
                        .unwrap_or_default();
                    let prompt = self.text(device.prompt()).replace("{button}", &button);
                    self.centered(frame, STATUS_Y, &prompt, WARNING);
                }
                self.centered(frame, HELP_Y, &self.text(LAUNCHER_CONTROLS_HELP), DIM);
            }
        }
        if let Some(prompt) = self.quitting {
            prompt.draw(frame, &self.metrics, &|key| self.text(key));
        }
    }

    /// The column where the values of lines labelled `labels` start.
    fn value_column(&self, labels: &[String]) -> usize {
        LABEL_X
            + labels
                .iter()
                .map(|label| self.metrics.plain_width(label, 1))
                .max()
                .unwrap_or(0)
            + VALUE_GAP
    }

    /// A line of the panel: its cursor when `selected`, its label and its
    /// value, cut to fit.
    fn draw_line(
        &self,
        frame: &mut Frame,
        (y, selected, color): (usize, bool, Rgb),
        label: &str,
        value: Option<(usize, String, Rgb)>,
    ) {
        if selected {
            self.metrics
                .draw_plain(frame, (CURSOR_X, y), ">", TITLE_COLOR, 1);
        }
        self.metrics
            .draw_plain(frame, (LABEL_X, y), label, color, 1);
        if let Some((x, value, value_color)) = value {
            let room = (PANEL.0 + PANEL.2).saturating_sub(x + MARGIN);
            let value = self.fitted(&value, room);
            self.metrics
                .draw_plain(frame, (x, y), &value, value_color, 1);
        }
    }

    fn draw_main(&self, frame: &mut Frame) {
        let labels = [
            self.text(LAUNCHER_ROM),
            self.text(LAUNCHER_TRANSLATION),
            self.text(LAUNCHER_OPTIONS),
        ];
        let value_x = self.value_column(&labels);
        for (index, line) in LINES.iter().enumerate() {
            let y = FIRST_LINE_Y + index * LINE_HEIGHT;
            let selected = index == self.line;
            let color = if *line == Line::Play && !self.playable() {
                UNAVAILABLE
            } else if selected {
                TEXT
            } else {
                DIM
            };
            let (label, value) = match line {
                Line::Rom | Line::Translation | Line::Options => {
                    let (value, value_color) = self.value(*line);
                    (labels[index].clone(), Some((value_x, value, value_color)))
                }
                Line::About => (self.text(LAUNCHER_ABOUT), None),
                Line::Play => (self.text(LAUNCHER_PLAY), None),
                Line::Quit => (self.text(LAUNCHER_QUIT), None),
            };
            self.draw_line(frame, (y, selected, color), &label, value);
        }
    }

    fn draw_options(&self, frame: &mut Frame, selected: usize) {
        let labels: Vec<String> = SETTINGS
            .iter()
            .map(|setting| self.text(setting.label()))
            .collect();
        let value_x = self.value_column(&labels);
        for (index, setting) in SETTINGS.iter().enumerate() {
            let y = OPTIONS_FIRST_LINE_Y + index * OPTION_LINE_HEIGHT;
            let is_selected = index == selected;
            let color = if is_selected { TEXT } else { DIM };
            let value = self
                .setting_value(*setting)
                .map(|(value, value_color)| (value_x, value, value_color));
            self.draw_line(frame, (y, is_selected, color), &labels[index], value);
        }
    }

    /// The about screen: the version and the license, then each page's
    /// line with its address below it, and the way back.
    fn draw_about(&self, frame: &mut Frame, selected: usize) {
        let facts = [
            (self.text(LAUNCHER_VERSION), VERSION),
            (self.text(LAUNCHER_LICENSE), LICENSE),
        ];
        let labels: Vec<String> = facts.iter().map(|(label, _)| label.clone()).collect();
        let value_x = self.value_column(&labels);
        let mut y = OPTIONS_FIRST_LINE_Y;
        for (label, value) in &facts {
            self.draw_line(
                frame,
                (y, false, DIM),
                label,
                Some((value_x, (*value).to_owned(), TEXT)),
            );
            y += OPTION_LINE_HEIGHT;
        }
        for ((index, page), (y, _)) in PAGES.iter().enumerate().zip(about_rows()) {
            let is_selected = index == selected;
            let color = if is_selected { TEXT } else { DIM };
            self.draw_line(
                frame,
                (y, is_selected, color),
                &self.text(page.label()),
                None,
            );
            if let Some(url) = page.url() {
                let address = url.strip_prefix(SITE_PREFIX).unwrap_or(url);
                let x = LABEL_X + ADDRESS_INDENT;
                let room = (PANEL.0 + PANEL.2).saturating_sub(x + MARGIN);
                let address = self.fitted(address, room);
                let address_color = if is_selected { GOOD } else { DIM };
                self.metrics.draw_plain(
                    frame,
                    (x, y + OPTION_LINE_HEIGHT),
                    &address,
                    address_color,
                    1,
                );
            }
        }
    }

    /// What a line of the options shows.
    fn setting_value(&self, setting: Setting) -> Option<(String, Rgb)> {
        let value = match setting {
            Setting::Keyboard if self.keys == default_keys() => {
                (self.text(LAUNCHER_KEYS_DEFAULT), DIM)
            }
            Setting::Keyboard => (self.text(LAUNCHER_KEYS_CUSTOM), WARNING),
            Setting::Gamepad => match self.gamepads.first() {
                Some(name) => (name.clone(), GOOD),
                None => (self.text(LAUNCHER_NO_GAMEPAD), DIM),
            },
            Setting::Window => (format!("x{}", self.scale), TEXT),
            Setting::Fullscreen => {
                let key = if self.fullscreen {
                    LAUNCHER_ON
                } else {
                    LAUNCHER_OFF
                };
                (self.text(key), TEXT)
            }
            Setting::Filter => (self.filter_name(), TEXT),
            Setting::Volume => (format!("{}%", self.volume), TEXT),
            Setting::Back => return None,
        };
        Some(value)
    }

    fn filter_name(&self) -> String {
        self.text(match self.filter {
            Filter::Sharp => LAUNCHER_SHARP,
            Filter::Smooth => LAUNCHER_SMOOTH,
        })
    }

    fn draw_bindings(&self, frame: &mut Frame, device: Device, selected: usize) {
        let map = self.map(device);
        let defaults = device.defaults();
        let columns: Vec<(usize, usize)> = map
            .chunks(BUTTON_ROWS)
            .enumerate()
            .map(|(column, entries)| {
                let x = LABEL_X + column * COLUMN_WIDTH;
                let widest = entries
                    .iter()
                    .map(|(button, _)| self.metrics.plain_width(&self.button_label(*button), 1))
                    .max()
                    .unwrap_or(0);
                (x, x + widest + VALUE_GAP)
            })
            .collect();
        let entry_at = |entry: usize| {
            let (column, row) = match entry {
                DEFAULTS_ENTRY => (0, BUTTON_ROWS),
                BACK_ENTRY => (1, BUTTON_ROWS),
                entry => (entry / BUTTON_ROWS, entry % BUTTON_ROWS),
            };
            let gap = if row == BUTTON_ROWS { 4 } else { 0 };
            (column, FIRST_LINE_Y + row * KEY_LINE_HEIGHT + gap)
        };
        let column_x = |column: usize| columns.get(column).map_or(LABEL_X, |(x, _)| *x);
        let cursor = |frame: &mut Frame, column: usize, y: usize| {
            let x = column_x(column) - (LABEL_X - CURSOR_X);
            self.metrics.draw_plain(frame, (x, y), ">", TITLE_COLOR, 1);
        };
        for (index, (button, name)) in map.iter().enumerate() {
            let (column, y) = entry_at(index);
            let (x, name_x) = columns[column];
            let is_selected = index == selected;
            if is_selected {
                cursor(frame, column, y);
            }
            let color = if is_selected { TEXT } else { DIM };
            self.metrics
                .draw_plain(frame, (x, y), &self.button_label(*button), color, 1);
            let is_default = defaults.contains(&(*button, name.clone()));
            let name_color = if is_default { GOOD } else { WARNING };
            let right = if column == 0 {
                x + COLUMN_WIDTH
            } else {
                PANEL.0 + PANEL.2
            };
            let name = self.fitted(
                &device.label(name),
                right.saturating_sub(name_x + VALUE_GAP),
            );
            self.metrics
                .draw_plain(frame, (name_x, y), &name, name_color, 1);
        }
        for (entry, text) in [
            (DEFAULTS_ENTRY, self.text(LAUNCHER_DEFAULT_KEYS)),
            (BACK_ENTRY, self.text(LAUNCHER_BACK)),
        ] {
            let (column, y) = entry_at(entry);
            let is_selected = entry == selected;
            if is_selected {
                cursor(frame, column, y);
            }
            let color = if is_selected { TEXT } else { DIM };
            let text = self.fitted(&text, COLUMN_WIDTH - VALUE_GAP);
            self.metrics
                .draw_plain(frame, (column_x(column), y), &text, color, 1);
        }
    }

    /// A button's name on the binding screens: the pad's directions in the
    /// screen's language, the others as the console marks them.
    fn button_label(&self, button: Button) -> String {
        match button {
            Button::Up => self.text(LAUNCHER_UP),
            Button::Down => self.text(LAUNCHER_DOWN),
            Button::Left => self.text(LAUNCHER_LEFT),
            Button::Right => self.text(LAUNCHER_RIGHT),
            Button::A => "A".to_owned(),
            Button::B => "B".to_owned(),
            Button::L => "L".to_owned(),
            Button::R => "R".to_owned(),
            Button::Start => "START".to_owned(),
            Button::Select => "SELECT".to_owned(),
        }
    }

    /// What a choice's line shows: the file's name, or that there is none;
    /// for the options, the window, the filter and the volume.
    fn value(&self, line: Line) -> (String, Rgb) {
        let name = |path: &Path| {
            path.file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
        };
        match line {
            Line::Rom => match &self.rom {
                Some((path, kind)) => (name(path), kind_color(*kind)),
                None => (self.text(LAUNCHER_NO_ROM), DIM),
            },
            Line::Options => {
                let window = if self.fullscreen {
                    self.text(LAUNCHER_FULLSCREEN)
                } else {
                    format!("x{}", self.scale)
                };
                let summary = format!("{window}, {}, {}%", self.filter_name(), self.volume);
                (summary, DIM)
            }
            _ => match &self.translation {
                Some((path, Some(_))) => (name(path), GOOD),
                Some((path, None)) => (name(path), BAD),
                None => (self.text(LAUNCHER_NO_TRANSLATION), DIM),
            },
        }
    }

    /// The line under the panel: about the line under the cursor.
    fn status(&self) -> (String, Rgb) {
        match LINES[self.line] {
            Line::Options => (self.text(LAUNCHER_PICK_OPTIONS), DIM),
            Line::About => (self.text(LAUNCHER_PICK_ABOUT), DIM),
            Line::Rom | Line::Play | Line::Quit => match &self.rom {
                None => (self.text(LAUNCHER_PICK_ROM), WARNING),
                Some((_, kind)) if LINES[self.line] == Line::Play && kind.playable() => {
                    (self.text(LAUNCHER_READY), GOOD)
                }
                Some((_, kind)) => (self.text(kind.message()), kind_color(*kind)),
            },
            Line::Translation => match &self.translation {
                Some((_, Some(translation))) => (
                    self.text(LAUNCHER_TRANSLATION_READ)
                        .replace("{count}", &translation.len().to_string()),
                    GOOD,
                ),
                Some((_, None)) => (self.text(LAUNCHER_TRANSLATION_UNREADABLE), BAD),
                None => (self.text(LAUNCHER_PICK_TRANSLATION), DIM),
            },
        }
    }

    /// `text` centered on row `y`, kept within the margins.
    fn centered(&self, frame: &mut Frame, y: usize, text: &str, color: Rgb) {
        let text = self.fitted(text, frame.width().saturating_sub(2 * MARGIN));
        let x = frame
            .width()
            .saturating_sub(self.metrics.plain_width(&text, 1))
            / 2;
        self.metrics.draw_plain(frame, (x, y), &text, color, 1);
    }

    /// `text`, cut short with an ellipsis to fit `room` pixels.
    fn fitted(&self, text: &str, room: usize) -> String {
        if self.metrics.plain_width(text, 1) <= room {
            return text.to_owned();
        }
        let mut cut: String = text.to_owned();
        while !cut.is_empty() && self.metrics.plain_width(&format!("{cut}{ELLIPSIS}"), 1) > room {
            cut.pop();
        }
        format!("{cut}{ELLIPSIS}")
    }
}

impl Page {
    fn label(self) -> &'static str {
        match self {
            Self::Project => LAUNCHER_PROJECT_PAGE,
            Self::Translations => LAUNCHER_TRANSLATIONS_PAGE,
            Self::Back => LAUNCHER_BACK,
        }
    }

    /// The page this line opens, if it opens one.
    fn url(self) -> Option<&'static str> {
        match self {
            Self::Project => Some(PROJECT_URL),
            Self::Translations => Some(TRANSLATIONS_URL),
            Self::Back => None,
        }
    }
}

impl Setting {
    fn label(self) -> &'static str {
        match self {
            Self::Keyboard => LAUNCHER_KEYBOARD,
            Self::Gamepad => LAUNCHER_GAMEPAD,
            Self::Window => LAUNCHER_WINDOW,
            Self::Fullscreen => LAUNCHER_FULLSCREEN,
            Self::Filter => LAUNCHER_FILTER,
            Self::Volume => LAUNCHER_VOLUME,
            Self::Back => LAUNCHER_BACK,
        }
    }
}

impl Device {
    /// The options screen with the cursor on this device's line.
    fn options(self) -> Screen {
        let setting = match self {
            Self::Keyboard => Setting::Keyboard,
            Self::Gamepad => Setting::Gamepad,
        };
        Screen::Options(
            SETTINGS
                .iter()
                .position(|shown| *shown == setting)
                .unwrap_or(0),
        )
    }

    /// This device's binding screen, the cursor on the first button.
    fn bindings(self) -> Screen {
        Screen::Bindings {
            device: self,
            entry: 0,
            waiting: false,
        }
    }

    fn defaults(self) -> KeyMap {
        match self {
            Self::Keyboard => default_keys(),
            Self::Gamepad => default_pad_buttons(),
        }
    }

    /// How a binding screen shows the key or gamepad button `name`.
    fn label(self, name: &str) -> String {
        match self {
            Self::Keyboard => name.to_owned(),
            Self::Gamepad => pad_button_label(name),
        }
    }

    fn heading(self) -> &'static str {
        match self {
            Self::Keyboard => LAUNCHER_KEYBOARD,
            Self::Gamepad => LAUNCHER_GAMEPAD,
        }
    }

    fn prompt(self) -> &'static str {
        match self {
            Self::Keyboard => LAUNCHER_PRESS_KEY,
            Self::Gamepad => LAUNCHER_PRESS_PAD,
        }
    }
}

/// The name the ROM is kept under in the app's own folder, when the
/// system's dialog hands it over as a document (see [`keep`]).
const KEPT_ROM: &str = "rom.gba";
const KEPT_TRANSLATION: &str = "translation";
const CONTENT_SCHEME: &str = "content://";

/// The file to use for `path`, chosen in the system's dialog: a document
/// Android hands over as a `content://` URI is copied once into the app's
/// own folder under `name`, and that copy is read and remembered from then
/// on, as the URI stops working when the app closes; any other path is used
/// as it is. A copy that fails leaves the URI, which then reads as
/// unreadable.
fn keep(path: PathBuf, name: &str) -> PathBuf {
    if !path.to_string_lossy().starts_with(CONTENT_SCHEME) {
        return path;
    }
    let copied =
        platform_sdl3::preferences_dir(crate::settings::ORGANIZATION, crate::settings::APP)
            .ok()
            .and_then(|folder| {
                let bytes = platform_sdl3::read_file(&path).ok()?;
                let kept = folder.join(name);
                std::fs::write(&kept, bytes).ok()?;
                Some(kept)
            });
    copied.unwrap_or(path)
}

/// The name a translation handed over as a document is kept under: its
/// language (`Language: es` in the PO header gives `es.po`), so the
/// Translation line still says which it is.
fn kept_translation_name(path: &Path) -> String {
    let language = platform_sdl3::read_file(path)
        .ok()
        .and_then(|bytes| po_language(&String::from_utf8_lossy(&bytes)));
    format!(
        "{}.po",
        language.unwrap_or_else(|| KEPT_TRANSLATION.to_owned())
    )
}

/// The language code of a PO file's header, if it has one made of letters,
/// digits, `-` and `_` only.
fn po_language(text: &str) -> Option<String> {
    let start = text.find("Language:")? + "Language:".len();
    let code: String = text[start..]
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect();
    (!code.is_empty()).then_some(code)
}

/// Android shows the app on the whole screen, over its status and
/// navigation bars, whatever the settings say.
const ALWAYS_FULLSCREEN: bool = cfg!(target_os = "android");
/// The middle of the screen, which splits a tap on a value into lower and
/// raise.
const SCREEN_MIDDLE: usize = 120;
/// The facts the about screen shows above its pages.
const ABOUT_FACTS: usize = 2;

/// The row of a list whose first row's text starts at `first`, `height`
/// pixels apart, that pixel row `y` falls in (from a pixel above each
/// text), if one of its `count` rows.
fn row_at(y: usize, first: usize, height: usize, count: usize) -> Option<usize> {
    let row = (y + 1).checked_sub(first)? / height;
    (row < count).then_some(row)
}

/// Where each page's line of the about screen starts and how many rows
/// it takes (with its address below it), after the version and the
/// license.
fn about_rows() -> Vec<(usize, usize)> {
    let mut y = OPTIONS_FIRST_LINE_Y + ABOUT_FACTS * OPTION_LINE_HEIGHT + OPTION_LINE_HEIGHT / 5;
    PAGES
        .iter()
        .map(|page| {
            let rows = if page.url().is_some() { PAGE_ROWS } else { 1 };
            let top = y;
            y += rows * OPTION_LINE_HEIGHT;
            (top, rows)
        })
        .collect()
}

fn line_index(line: Line) -> usize {
    LINES.iter().position(|shown| *shown == line).unwrap_or(0)
}

fn kind_color(kind: RomKind) -> Rgb {
    match kind {
        RomKind::Verified => GOOD,
        RomKind::FirstRelease | RomKind::Unsupported | RomKind::Other | RomKind::Unreadable => BAD,
    }
}

/// The screen's background: a vertical blend of two blues.
fn draw_backdrop(frame: &mut Frame) {
    let height = frame.height().max(1);
    let blend = |top: u8, bottom: u8, y: usize| {
        let (top, bottom) = (usize::from(top), usize::from(bottom));
        let value = (top * (height - y) + bottom * y) / height;
        u8::try_from(value).unwrap_or(u8::MAX)
    };
    for y in 0..frame.height() {
        let color = Rgb::new(
            blend(BACKDROP_TOP.r, BACKDROP_BOTTOM.r, y),
            blend(BACKDROP_TOP.g, BACKDROP_BOTTOM.g, y),
            blend(BACKDROP_TOP.b, BACKDROP_BOTTOM.b, y),
        );
        for x in 0..frame.width() {
            frame.set_pixel(x, y, color);
        }
    }
}

/// A filled box with a one-pixel border.
pub(crate) fn draw_panel(frame: &mut Frame, (x, y, width, height): (usize, usize, usize, usize)) {
    for row in y..y + height {
        for column in x..x + width {
            let edge = row == y || row + 1 == y + height || column == x || column + 1 == x + width;
            frame.set_pixel(column, row, if edge { PANEL_BORDER } else { PANEL_FILL });
        }
    }
}

/// Writes `front`'s choices to `settings_path`, reporting a failure.
fn remember(front: &Front, settings_path: Option<&Path>) {
    if let Some(path) = settings_path
        && let Err(error) = front.settings().store(path)
    {
        eprintln!("Settings:   cannot write {}: {error}", path.display());
    }
}

/// Gives `display` the keys, the gamepad buttons, the window and the
/// filter of `settings`.
///
/// # Errors
///
/// Returns the display's error when the window cannot change.
pub fn apply(display: &mut Sdl3Display, settings: &Settings) -> Result<()> {
    display.set_keys(&settings.keys);
    display.set_pad_buttons(&settings.pad_buttons);
    let fullscreen = settings.fullscreen || ALWAYS_FULLSCREEN;
    display.set_video(settings.scale, fullscreen, settings.filter)?;
    Ok(())
}

/// Whether `before` and `after` show or control the game differently.
fn display_changed(before: &Settings, after: &Settings) -> bool {
    (
        &before.keys,
        &before.pad_buttons,
        before.scale,
        before.fullscreen,
        before.filter,
    ) != (
        &after.keys,
        &after.pad_buttons,
        after.scale,
        after.fullscreen,
        after.filter,
    )
}

/// Shows the screen until the player starts the game, with what they
/// chose, or leaves (`None`); the options chosen take hold in `display` at
/// once, and every choice is remembered in `settings_path` as soon as it
/// is made (Android may close the app at any moment) and when the game
/// starts or the launcher closes.
///
/// # Errors
///
/// Returns the display's errors.
pub fn run(display: &mut Sdl3Display, settings_path: Option<&Path>) -> Result<Option<Choice>> {
    let settings = settings_path.map(Settings::load).unwrap_or_default();
    let mut front = Front::new(&settings);
    let mut applied = front.settings();
    apply(display, &applied)?;
    let mut frame = Frame::new(
        gba_runtime::ppu::SCREEN_WIDTH,
        gba_runtime::ppu::SCREEN_HEIGHT,
        Rgb::default(),
    );
    loop {
        let started = std::time::Instant::now();
        let events = display.poll_events();
        front.gamepads = display.gamepads();
        if front.events(&events) {
            remember(&front, settings_path);
            return Ok(None);
        }
        let step = front.update(display, display.input())?;
        let current = front.settings();
        if display_changed(&applied, &current) {
            apply(display, &current)?;
        }
        if current != applied {
            remember(&front, settings_path);
        }
        applied = current;
        match step {
            Step::Stay => {}
            Step::Quit => {
                remember(&front, settings_path);
                return Ok(None);
            }
            Step::Play => {
                remember(&front, settings_path);
                let Some((rom, _)) = front.rom.clone() else {
                    continue;
                };
                return Ok(Some(Choice {
                    rom,
                    translation: front
                        .translation
                        .as_ref()
                        .filter(|(_, translation)| translation.is_some())
                        .map(|(path, _)| path.clone()),
                    settings: applied,
                }));
            }
        }
        front.draw(&mut frame);
        display.present(&frame)?;
        std::thread::sleep(crate::FRAME_DURATION.saturating_sub(started.elapsed()));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("re-zoids-front-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temporary directory");
        dir.join(name)
    }

    fn name_of(front: &Front, device: Device, button: Button) -> String {
        front
            .map(device)
            .iter()
            .find(|(bound, _)| *bound == button)
            .map(|(_, name)| name.clone())
            .expect("bound")
    }

    fn press(button: Button) -> Input {
        Input::default().with(button)
    }

    fn line_of(setting: Setting) -> usize {
        SETTINGS
            .iter()
            .position(|shown| *shown == setting)
            .expect("listed")
    }

    /// A header-only image with `game_code` and software version `version`.
    fn synthetic_rom(name: &str, game_code: [u8; 4], version: u8) -> PathBuf {
        use formats::rom_header::{HEADER_LEN, complement};
        let mut rom = vec![0u8; HEADER_LEN + 16];
        rom[0xA0..0xA4].copy_from_slice(b"TEST");
        rom[0xAC..0xB0].copy_from_slice(&game_code);
        rom[0xB0..0xB2].copy_from_slice(b"DA");
        rom[0xB2] = 0x96;
        rom[0xBC] = version;
        rom[0xBD] = complement(&rom);
        let path = scratch(name);
        std::fs::write(&path, rom).expect("writable");
        path
    }

    #[test]
    fn only_the_verified_dump_plays() {
        let kind =
            |name, code: &[u8; 4], version| RomKind::of(&synthetic_rom(name, *code, version));
        assert_eq!(kind("rev0.gba", b"ATZJ", 0), RomKind::FirstRelease);
        assert_eq!(kind("rev1.gba", b"ATZJ", 1), RomKind::Unsupported);
        assert_eq!(kind("fuzors.gba", b"BZFJ", 0), RomKind::Other);
        assert_eq!(kind("other.gba", b"AXYZ", 0), RomKind::Other);
        for kind in [
            RomKind::FirstRelease,
            RomKind::Unsupported,
            RomKind::Other,
            RomKind::Unreadable,
        ] {
            assert!(!kind.playable());
            assert!(default_text(kind.message()).is_some());
        }
        assert!(RomKind::Verified.playable());
    }

    #[test]
    fn a_path_that_is_not_a_document_is_used_as_it_is() {
        let path = PathBuf::from("/games/zoids.gba");
        assert_eq!(keep(path.clone(), KEPT_ROM), path);
    }

    #[test]
    fn a_kept_translation_is_named_after_its_language() {
        assert_eq!(
            po_language("msgid \"\"\nmsgstr \"\"\n\"Language: es\\n\"\n"),
            Some("es".to_owned())
        );
        assert_eq!(
            po_language("\"Language: pt_BR\\n\""),
            Some("pt_BR".to_owned())
        );
        assert_eq!(po_language("\"Language: \\n\""), None);
        assert_eq!(po_language("no header"), None);
    }

    #[test]
    fn the_texts_follow_the_translation_chosen() {
        let po = scratch("es.po");
        std::fs::write(
            &po,
            "msgctxt \"port/launcher/play\"\nmsgid \"Play\"\nmsgstr \"Jugar\"\n",
        )
        .expect("writable");
        let rom = scratch("not-a-rom.gba");
        std::fs::write(&rom, [0u8; 16]).expect("writable");
        let mut front = Front::new(&Settings::default());
        assert_eq!(front.text(LAUNCHER_PLAY), "Play");
        assert!(!front.playable());
        front.take_translation(po.clone());
        front.take_rom(rom.clone());
        assert_eq!(front.text(LAUNCHER_PLAY), "Jugar");
        assert_eq!(front.text(LAUNCHER_QUIT), "Quit");
        assert_eq!(
            front.rom.as_ref().map(|(_, kind)| *kind),
            Some(RomKind::Unreadable)
        );
        assert!(!front.playable());
        assert_eq!(
            front.settings(),
            Settings {
                rom: Some(rom),
                translation: Some(po),
                ..Settings::default()
            }
        );
        let mut frame = Frame::new(240, 160, Rgb::default());
        front.draw(&mut frame);
        assert_eq!(frame.pixel(0, 0), Some(BACKDROP_TOP));
    }

    #[test]
    fn a_key_taken_from_another_button_swaps_them() {
        let mut front = Front::new(&Settings::default());
        let (a, b) = (
            name_of(&front, Device::Keyboard, Button::A),
            name_of(&front, Device::Keyboard, Button::B),
        );
        front.bind(Device::Keyboard, Button::A, &b);
        assert_eq!(
            (
                name_of(&front, Device::Keyboard, Button::A),
                name_of(&front, Device::Keyboard, Button::B)
            ),
            (b, a)
        );
        front.bind(Device::Keyboard, Button::Start, "Space");
        front.bind(Device::Gamepad, Button::Select, "y");
        let settings = front.settings();
        assert_eq!(settings.keys.len(), 3);
        assert_eq!(settings.pad_buttons, [(Button::Select, "y".to_owned())]);
        let again = Front::new(&settings);
        assert_eq!(
            (again.keys, again.pad_buttons),
            (front.keys, front.pad_buttons)
        );
    }

    #[test]
    fn escape_cancels_a_wait_goes_back_then_asks_before_closing() {
        let mut front = Front::new(&Settings::default());
        front.screen = Screen::Bindings {
            device: Device::Gamepad,
            entry: 4,
            waiting: true,
        };
        assert!(!front.events(&[Event::Key(4)]));
        assert!(matches!(
            front.screen,
            Screen::Bindings { waiting: true, .. }
        ));
        assert!(!front.events(&[Event::Back]));
        assert!(matches!(
            front.screen,
            Screen::Bindings { waiting: false, .. }
        ));
        assert!(!front.events(&[Event::Back]));
        assert_eq!(front.screen, Screen::Options(line_of(Setting::Gamepad)));
        assert!(!front.events(&[Event::Back]));
        assert_eq!(front.screen, Screen::Main);
        assert!(!front.events(&[Event::Back]));
        assert!(front.quitting.is_some());
        assert!(!front.events(&[Event::Back]));
        assert!(front.quitting.is_none());
        assert!(!front.events(&[Event::Back]));
        let prompt = front.quitting.as_mut().expect("asked");
        assert_eq!(prompt.update(press(Button::Left)), None);
        assert_eq!(prompt.update(press(Button::A)), Some(true));
        assert!(front.events(&[Event::Quit]));
    }

    #[test]
    fn the_options_change_the_window_the_filter_and_the_volume() {
        let mut front = Front::new(&Settings::default());
        let window = line_of(Setting::Window);
        for _ in 0..10 {
            front.update_options(window, press(Button::Right));
        }
        assert_eq!(front.scale, *SCALES.end());
        front.update_options(line_of(Setting::Filter), press(Button::A));
        front.update_options(line_of(Setting::Fullscreen), press(Button::Left));
        let volume = line_of(Setting::Volume);
        front.update_options(volume, press(Button::Left));
        front.update_options(volume, press(Button::Left));
        let settings = front.settings();
        assert_eq!(
            (settings.filter, settings.fullscreen, settings.volume),
            (Filter::Smooth, true, 80)
        );
        front.update_options(volume, press(Button::B));
        assert_eq!(front.screen, Screen::Main);
    }

    #[test]
    fn the_about_screen_lists_the_pages_and_goes_back() {
        let mut front = Front::new(&Settings::default());
        assert_eq!(LINES[line_index(Line::About)], Line::About);
        front.screen = Screen::About(0);
        front.update_about(0, press(Button::Down));
        assert_eq!(front.screen, Screen::About(1));
        front.update_about(1, press(Button::Down));
        front.update_about(2, press(Button::Down));
        assert_eq!(front.screen, Screen::About(2));
        assert_eq!(PAGES[2].url(), None);
        assert_eq!(
            PAGES.map(Page::url)[..2],
            [Some(PROJECT_URL), Some(TRANSLATIONS_URL)]
        );
        front.update_about(2, press(Button::A));
        assert_eq!(front.screen, Screen::Main);
        front.screen = Screen::About(1);
        front.update_about(1, press(Button::B));
        assert_eq!(front.screen, Screen::Main);
        front.screen = Screen::About(0);
        assert!(!front.events(&[Event::Back]));
        assert_eq!(front.screen, Screen::Main);
        let mut frame = Frame::new(240, 160, Rgb::default());
        front.screen = Screen::About(1);
        front.draw(&mut frame);
        assert_eq!(frame.pixel(0, 0), Some(BACKDROP_TOP));
    }

    /// The pixel row in the middle of row `row` of a list.
    fn middle_of(first: usize, height: usize, row: usize) -> i32 {
        i32::try_from(first + row * height + height / 2).unwrap_or_default()
    }

    #[test]
    fn a_tap_selects_and_chooses_the_line_under_it() {
        let mut front = Front::new(&Settings::default());
        let x = i32::try_from(LABEL_X).unwrap_or_default();
        let options = line_index(Line::Options);
        let pressed = front.tap(x, middle_of(FIRST_LINE_Y, LINE_HEIGHT, options));
        assert_eq!(front.line, options);
        assert_eq!(pressed, press(Button::A));
        assert_eq!(front.tap(x, 2), Input::default(), "above the list");
        assert_eq!(front.tap(-4, 60), Input::default(), "outside the panel");
        assert_eq!(front.line, options);

        front.screen = Screen::Options(0);
        let volume = line_of(Setting::Volume);
        let y = middle_of(OPTIONS_FIRST_LINE_Y, OPTION_LINE_HEIGHT, volume);
        assert_eq!(front.tap(40, y), press(Button::Left));
        assert_eq!(front.tap(200, y), press(Button::Right));
        assert_eq!(front.screen, Screen::Options(volume));
        let back = line_of(Setting::Back);
        let y = middle_of(OPTIONS_FIRST_LINE_Y, OPTION_LINE_HEIGHT, back);
        assert_eq!(front.tap(x, y), press(Button::A));

        front.screen = Screen::About(0);
        let (top, _) = about_rows()[2];
        let y = i32::try_from(top + 2).unwrap_or_default();
        assert_eq!(front.tap(x, y), press(Button::A));
        assert_eq!(front.screen, Screen::About(2));
    }

    #[test]
    fn a_pointer_event_waits_for_the_next_frame() {
        let mut front = Front::new(&Settings::default());
        assert!(!front.events(&[Event::Pointer { x: 30, y: 60 }]));
        assert_eq!(front.tapped, Some((30, 60)));
    }
}
