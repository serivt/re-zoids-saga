//! The launcher's own screen, shown when no ROM is given on the command
//! line: the project's name, the ROM and the translation to play with,
//! chosen in the system's file dialog, the keys of the buttons, and the
//! lines that start the game or quit.
//!
//! It is drawn with this project's Latin font and colors alone, since no
//! ROM has been read yet. Its texts are English until a translation is
//! chosen, and then that translation's (`port/launcher/...`, see
//! `docs/translation.md`).

use std::path::{Path, PathBuf};

use anyhow::Result;
use extraction::Title;
use game_core::port_text::{
    LAUNCHER_BACK, LAUNCHER_CONTROLS, LAUNCHER_CONTROLS_HELP, LAUNCHER_DEFAULT_KEYS, LAUNCHER_DOWN,
    LAUNCHER_HELP, LAUNCHER_KEYS_CUSTOM, LAUNCHER_KEYS_DEFAULT, LAUNCHER_LEFT, LAUNCHER_NO_ROM,
    LAUNCHER_NO_TRANSLATION, LAUNCHER_PICK_CONTROLS, LAUNCHER_PICK_ROM, LAUNCHER_PICK_TRANSLATION,
    LAUNCHER_PLAY, LAUNCHER_PRESS_KEY, LAUNCHER_QUIT, LAUNCHER_READY, LAUNCHER_RIGHT, LAUNCHER_ROM,
    LAUNCHER_ROM_OTHER, LAUNCHER_ROM_UNREADABLE, LAUNCHER_ROM_UNVERIFIED, LAUNCHER_ROM_VERIFIED,
    LAUNCHER_SUBTITLE, LAUNCHER_TRANSLATION, LAUNCHER_TRANSLATION_READ,
    LAUNCHER_TRANSLATION_UNREADABLE, LAUNCHER_UP, default_text,
};
use game_core::{TextMetrics, Translation};
use platform::{Button, Display, Event, Frame, Input, Rgb};
use platform_sdl3::{FileChoice, KeyMap, Sdl3Display, default_keys, key_name};

use crate::settings::Settings;

/// The project's name, never translated.
pub const PROJECT_NAME: &str = "Re:Zoids Saga";
const ROM_FILTERS: [(&str, &str); 1] = [("Game Boy Advance ROM", "gba")];
const TRANSLATION_FILTERS: [(&str, &str); 1] = [("Translation (PO)", "po")];
/// Pixels kept clear along every edge of the screen.
const MARGIN: usize = 8;
const TITLE_SCALE: usize = 2;
const TITLE_Y: usize = 10;
const SUBTITLE_Y: usize = 32;
const PANEL: (usize, usize, usize, usize) = (MARGIN, 46, 240 - 2 * MARGIN, 82);
const FIRST_LINE_Y: usize = 53;
const LINE_HEIGHT: usize = 14;
const KEY_LINE_HEIGHT: usize = 11;
const CURSOR_X: usize = MARGIN + 6;
const LABEL_X: usize = MARGIN + 14;
const VALUE_GAP: usize = 8;
const COLUMN_WIDTH: usize = 102;
const STATUS_Y: usize = 134;
const HELP_Y: usize = 144;
const ELLIPSIS: &str = "...";
const BACKDROP_TOP: Rgb = Rgb::new(10, 16, 40);
const BACKDROP_BOTTOM: Rgb = Rgb::new(30, 44, 88);
const PANEL_FILL: Rgb = Rgb::new(16, 26, 58);
const PANEL_BORDER: Rgb = Rgb::new(96, 120, 176);
const TITLE_COLOR: Rgb = Rgb::new(248, 208, 72);
const SHADOW: Rgb = Rgb::new(4, 6, 16);
const TEXT: Rgb = Rgb::new(232, 236, 248);
const DIM: Rgb = Rgb::new(140, 152, 184);
const UNAVAILABLE: Rgb = Rgb::new(72, 84, 120);
const GOOD: Rgb = Rgb::new(120, 224, 136);
const WARNING: Rgb = Rgb::new(240, 200, 96);
const BAD: Rgb = Rgb::new(240, 112, 104);

/// What the launcher settled on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// The ROM to play.
    pub rom: PathBuf,
    /// The translation to play with.
    pub translation: Option<PathBuf>,
}

/// The lines of the main screen, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    Rom,
    Translation,
    Controls,
    Play,
    Quit,
}

const LINES: [Line; 5] = [
    Line::Rom,
    Line::Translation,
    Line::Controls,
    Line::Play,
    Line::Quit,
];

/// The controls screen's entries: the buttons in two columns of five,
/// then a row with the defaults and the way back.
const BUTTON_ROWS: usize = 5;
const DEFAULTS_ENTRY: usize = Button::ALL.len();
const BACK_ENTRY: usize = DEFAULTS_ENTRY + 1;

/// What the chosen ROM is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RomKind {
    Verified,
    Unverified,
    Other,
    Unreadable,
}

impl RomKind {
    /// Identifies the ROM at `path`.
    fn of(path: &Path) -> Self {
        let identification = std::fs::read(path)
            .ok()
            .and_then(|bytes| extraction::identify(&bytes).ok());
        match identification {
            Some(found) if found.title == Title::Saga && found.known_release.is_some() => {
                Self::Verified
            }
            Some(found) if found.title == Title::Saga => Self::Unverified,
            Some(_) => Self::Other,
            None => Self::Unreadable,
        }
    }

    fn playable(self) -> bool {
        matches!(self, Self::Verified | Self::Unverified)
    }
}

/// Which screen is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Main,
    /// The controls, the entry under the cursor, and whether it waits for
    /// the button's new key.
    Controls {
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
    line: usize,
    screen: Screen,
    choosing: Option<(Line, FileChoice)>,
    previous: Input,
    metrics: TextMetrics,
}

impl Front {
    /// The screen with `settings`' choices, those still readable.
    #[must_use]
    pub fn new(settings: &Settings) -> Self {
        let mut keys = default_keys();
        for (button, key) in &settings.keys {
            if let Some(entry) = keys.iter_mut().find(|(bound, _)| bound == button) {
                entry.1.clone_from(key);
            }
        }
        let mut front = Self {
            rom: None,
            translation: None,
            keys,
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

    /// The choices to remember: the keys only where they differ from the
    /// defaults.
    #[must_use]
    pub fn settings(&self) -> Settings {
        let defaults = default_keys();
        Settings {
            rom: self.rom.as_ref().map(|(path, _)| path.clone()),
            translation: self.translation.as_ref().map(|(path, _)| path.clone()),
            keys: self
                .keys
                .iter()
                .filter(|entry| !defaults.contains(entry))
                .cloned()
                .collect(),
        }
    }

    /// The keys of the buttons.
    #[must_use]
    pub fn keys(&self) -> &KeyMap {
        &self.keys
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

    /// Gives `button` the key `key`; a button that had it takes the
    /// button's old key, so no key moves two buttons.
    fn bind(&mut self, button: Button, key: &str) {
        let old = self
            .keys
            .iter()
            .find(|(bound, _)| *bound == button)
            .map(|(_, key)| key.clone());
        for entry in &mut self.keys {
            if entry.0 == button {
                key.clone_into(&mut entry.1);
            } else if entry.1 == key
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

    /// The events of a frame: the key a button waits for, Escape to go
    /// back. Returns whether the launcher should close.
    fn events(&mut self, events: &[Event]) -> bool {
        for event in events {
            match (*event, self.screen) {
                (Event::Quit, _) => return true,
                (Event::Back, Screen::Controls { entry, waiting }) => {
                    self.screen = if waiting {
                        Screen::Controls {
                            entry,
                            waiting: false,
                        }
                    } else {
                        Screen::Main
                    };
                }
                (Event::Back, Screen::Main) if self.choosing.is_none() => return true,
                (
                    Event::Key(code),
                    Screen::Controls {
                        entry,
                        waiting: true,
                    },
                ) => {
                    if let (Some(button), Some(name)) = (Button::ALL.get(entry), key_name(code)) {
                        self.bind(*button, &name);
                    }
                    self.screen = Screen::Controls {
                        entry,
                        waiting: false,
                    };
                }
                _ => {}
            }
        }
        false
    }

    /// A frame of the buttons, or of a dialog's answer while one is open.
    fn update(&mut self, display: &Sdl3Display, input: Input) -> Result<Step> {
        let pressed = Button::ALL
            .into_iter()
            .filter(|&button| input.is_held(button) && !self.previous.is_held(button))
            .fold(Input::default(), Input::with);
        self.previous = input;
        if let Some((line, choice)) = &self.choosing {
            if let Some(answer) = choice.answer() {
                let line = *line;
                self.choosing = None;
                match (line, answer) {
                    (Line::Rom, Some(path)) => self.take_rom(path),
                    (Line::Translation, Some(path)) => self.take_translation(path),
                    _ => {}
                }
            }
            return Ok(Step::Stay);
        }
        match self.screen {
            Screen::Main => self.update_main(display, pressed),
            Screen::Controls { entry, waiting } => {
                if !waiting {
                    self.update_controls(entry, pressed);
                }
                Ok(Step::Stay)
            }
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
            Line::Controls => {
                self.screen = Screen::Controls {
                    entry: 0,
                    waiting: false,
                };
            }
            Line::Play if self.playable() => return Ok(Step::Play),
            Line::Play => {}
            Line::Quit => return Ok(Step::Quit),
        }
        Ok(Step::Stay)
    }

    /// The controls screen: the cursor moves down each column and across,
    /// X waits for a button's key, takes the defaults or goes back, Z goes
    /// back.
    fn update_controls(&mut self, entry: usize, pressed: Input) {
        if pressed.is_held(Button::B) {
            self.screen = Screen::Main;
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
        if pressed.is_held(Button::A) && entry == DEFAULTS_ENTRY {
            self.keys = default_keys();
        }
        if pressed.is_held(Button::A) && entry == BACK_ENTRY {
            self.screen = Screen::Main;
            return;
        }
        self.screen = Screen::Controls {
            entry,
            waiting: pressed.is_held(Button::A) && entry < DEFAULTS_ENTRY,
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
        draw_panel(frame, PANEL);
        match self.screen {
            Screen::Main => {
                self.centered(frame, SUBTITLE_Y, &self.text(LAUNCHER_SUBTITLE), DIM);
                self.draw_main(frame);
                let (status, status_color) = self.status();
                self.centered(frame, STATUS_Y, &status, status_color);
                self.centered(frame, HELP_Y, &self.text(LAUNCHER_HELP), DIM);
            }
            Screen::Controls { entry, waiting } => {
                self.centered(frame, SUBTITLE_Y, &self.text(LAUNCHER_CONTROLS), DIM);
                self.draw_controls(frame, entry);
                if waiting {
                    let button = Button::ALL
                        .get(entry)
                        .map(|button| self.button_label(*button))
                        .unwrap_or_default();
                    let prompt = self.text(LAUNCHER_PRESS_KEY).replace("{button}", &button);
                    self.centered(frame, STATUS_Y, &prompt, WARNING);
                }
                self.centered(frame, HELP_Y, &self.text(LAUNCHER_CONTROLS_HELP), DIM);
            }
        }
    }

    fn draw_main(&self, frame: &mut Frame) {
        let labels = [
            self.text(LAUNCHER_ROM),
            self.text(LAUNCHER_TRANSLATION),
            self.text(LAUNCHER_CONTROLS),
        ];
        let value_x = LABEL_X
            + labels
                .iter()
                .map(|label| self.metrics.plain_width(label, 1))
                .max()
                .unwrap_or(0)
            + VALUE_GAP;
        for (index, line) in LINES.iter().enumerate() {
            let y = FIRST_LINE_Y + index * LINE_HEIGHT;
            let selected = index == self.line;
            let color = if selected { TEXT } else { DIM };
            if selected {
                self.metrics
                    .draw_plain(frame, (CURSOR_X, y), ">", TITLE_COLOR, 1);
            }
            let label = match line {
                Line::Rom | Line::Translation | Line::Controls => {
                    let (value, value_color) = self.value(*line);
                    let room = (PANEL.0 + PANEL.2).saturating_sub(value_x + MARGIN);
                    let value = self.fitted(&value, room);
                    self.metrics
                        .draw_plain(frame, (value_x, y), &value, value_color, 1);
                    labels[index].clone()
                }
                Line::Play => self.text(LAUNCHER_PLAY),
                Line::Quit => self.text(LAUNCHER_QUIT),
            };
            let color = if *line == Line::Play && !self.playable() {
                UNAVAILABLE
            } else {
                color
            };
            self.metrics
                .draw_plain(frame, (LABEL_X, y), &label, color, 1);
        }
    }

    fn draw_controls(&self, frame: &mut Frame, selected: usize) {
        let defaults = default_keys();
        let columns: Vec<(usize, usize)> = self
            .keys
            .chunks(BUTTON_ROWS)
            .enumerate()
            .map(|(column, keys)| {
                let x = LABEL_X + column * COLUMN_WIDTH;
                let widest = keys
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
        for (index, (button, key)) in self.keys.iter().enumerate() {
            let (column, y) = entry_at(index);
            let (x, key_x) = columns[column];
            let is_selected = index == selected;
            if is_selected {
                cursor(frame, column, y);
            }
            let color = if is_selected { TEXT } else { DIM };
            self.metrics
                .draw_plain(frame, (x, y), &self.button_label(*button), color, 1);
            let changed = !defaults.contains(&(*button, key.clone()));
            let key_color = if changed { WARNING } else { GOOD };
            let right = if column == 0 {
                x + COLUMN_WIDTH
            } else {
                PANEL.0 + PANEL.2
            };
            let key = self.fitted(key, right.saturating_sub(key_x + VALUE_GAP));
            self.metrics
                .draw_plain(frame, (key_x, y), &key, key_color, 1);
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

    /// A button's name on the controls screen: the pad's directions in the
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
    /// for the controls, whether any key was changed.
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
            Line::Controls if self.keys == default_keys() => {
                (self.text(LAUNCHER_KEYS_DEFAULT), DIM)
            }
            Line::Controls => (self.text(LAUNCHER_KEYS_CUSTOM), WARNING),
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
            Line::Controls => (self.text(LAUNCHER_PICK_CONTROLS), DIM),
            Line::Rom | Line::Play | Line::Quit => match &self.rom {
                None => (self.text(LAUNCHER_PICK_ROM), WARNING),
                Some((_, kind)) if LINES[self.line] == Line::Play && kind.playable() => {
                    (self.text(LAUNCHER_READY), GOOD)
                }
                Some((_, kind)) => {
                    let key = match kind {
                        RomKind::Verified => LAUNCHER_ROM_VERIFIED,
                        RomKind::Unverified => LAUNCHER_ROM_UNVERIFIED,
                        RomKind::Other => LAUNCHER_ROM_OTHER,
                        RomKind::Unreadable => LAUNCHER_ROM_UNREADABLE,
                    };
                    (self.text(key), kind_color(*kind))
                }
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

fn line_index(line: Line) -> usize {
    LINES.iter().position(|shown| *shown == line).unwrap_or(0)
}

fn kind_color(kind: RomKind) -> Rgb {
    match kind {
        RomKind::Verified => GOOD,
        RomKind::Unverified => WARNING,
        RomKind::Other | RomKind::Unreadable => BAD,
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
fn draw_panel(frame: &mut Frame, (x, y, width, height): (usize, usize, usize, usize)) {
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

/// Shows the screen until the player starts the game, with what they
/// chose, or leaves (`None`); the keys chosen take hold in `display` at
/// once, and every choice is remembered in `settings_path` when the game
/// starts or the launcher closes.
///
/// # Errors
///
/// Returns the display's errors.
pub fn run(display: &mut Sdl3Display, settings_path: Option<&Path>) -> Result<Option<Choice>> {
    let settings = settings_path.map(Settings::load).unwrap_or_default();
    let mut front = Front::new(&settings);
    display.set_keys(front.keys());
    let mut frame = Frame::new(
        gba_runtime::ppu::SCREEN_WIDTH,
        gba_runtime::ppu::SCREEN_HEIGHT,
        Rgb::default(),
    );
    loop {
        let started = std::time::Instant::now();
        let events = display.poll_events();
        let before = front.keys().clone();
        if front.events(&events) {
            remember(&front, settings_path);
            return Ok(None);
        }
        let step = front.update(display, display.input())?;
        if *front.keys() != before {
            display.set_keys(front.keys());
        }
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

    fn key_of(front: &Front, button: Button) -> String {
        front
            .keys
            .iter()
            .find(|(bound, _)| *bound == button)
            .map(|(_, key)| key.clone())
            .expect("bound")
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
                keys: Vec::new(),
            }
        );
        let mut frame = Frame::new(240, 160, Rgb::default());
        front.draw(&mut frame);
        assert_eq!(frame.pixel(0, 0), Some(BACKDROP_TOP));
    }

    #[test]
    fn a_key_taken_from_another_button_swaps_them() {
        let mut front = Front::new(&Settings::default());
        let (a, b) = (key_of(&front, Button::A), key_of(&front, Button::B));
        front.bind(Button::A, &b);
        assert_eq!(
            (key_of(&front, Button::A), key_of(&front, Button::B)),
            (b, a)
        );
        front.bind(Button::Start, "Space");
        let settings = front.settings();
        assert_eq!(settings.keys.len(), 3);
        assert_eq!(Front::new(&settings).keys, front.keys);
    }

    #[test]
    fn escape_cancels_a_wait_then_leaves_the_controls() {
        let mut front = Front::new(&Settings::default());
        front.screen = Screen::Controls {
            entry: 4,
            waiting: true,
        };
        assert!(!front.events(&[Event::Back]));
        assert_eq!(
            front.screen,
            Screen::Controls {
                entry: 4,
                waiting: false
            }
        );
        assert!(!front.events(&[Event::Back]));
        assert_eq!(front.screen, Screen::Main);
        assert!(front.events(&[Event::Back]));
    }
}
