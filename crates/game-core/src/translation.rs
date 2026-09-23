//! Translations kept outside the repository: a gettext PO file the player
//! downloads (from the project's Weblate) and hands to the launcher, keyed
//! by script table, string index and the message's offset in the string.
//!
//! The Japanese text is copyrighted ROM content, so the translation
//! template is generated from the player's own ROM by [`template`] rather
//! than shipped; the PO files translators produce hold that text as their
//! source strings and stay out of this repository too.

use std::collections::HashMap;
use std::fmt::Write as _;

use std::collections::HashSet;

use crate::data::GameData;
use crate::extension::{Extension, Rect};
use formats::script_ops::{Instruction, MessageStep, decode_instruction, decode_message_step};
use thiserror::Error;

use crate::boot::{
    NAME_GRID_ROWS, NAME_HELP, NAME_HELP_PIXELS, NAME_LABEL_PIXELS, NAME_PAGE_LABELS,
};
use crate::text::{CELL_WIDTH, TextMetrics};

/// Table name of the title menu script.
pub const TITLE_TABLE: &str = "title";
/// Table name of the name-entry scripts.
pub const NAME_ENTRY_TABLE: &str = "name-entry";
/// Table name of the pause-menu scripts.
pub const PAUSE_MENU_TABLE: &str = "pause-menu";
/// Table name of the dialogue strings.
pub const DIALOGUE_TABLE: &str = "dialogue";
/// Key prefix of the name entry's character pages: `name-entry/alphabet/N`.
pub const ALPHABET_PREFIX: &str = "name-entry/alphabet/";
/// Key of the name entry's help line.
pub const NAME_HELP_KEY: &str = "name-entry/help";
const EMPTY_CELL: char = '\u{3000}';
const STRING_LIMIT: usize = 0x1000;
const SCREEN_COLUMNS: usize = 30;
const SCREEN_ROWS: usize = 20;
const MENU_KIND: u8 = 1;
const MENU_MARGIN: usize = 2;
const TEXT_MARGIN: usize = 1;
const NAME_CELLS: usize = 8;
const MAX_CALL_DEPTH: usize = 4;
const TEMPLATE_HEADER: &str = "msgid \"\"\nmsgstr \"\"\n\"Project-Id-Version: re-zoids-saga\\n\"\n\"MIME-Version: 1.0\\n\"\n\"Content-Type: text/plain; charset=UTF-8\\n\"\n\"Content-Transfer-Encoding: 8bit\\n\"\n\"Language: \\n\"\n\"X-Source-Language: ja\\n\"\n\n";

/// Why a translation file could not be used.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TranslationError {
    /// A line is not part of the PO syntax this loader knows.
    #[error("line {line}: cannot read `{text}`")]
    Syntax {
        /// Line number, from 1.
        line: usize,
        /// The line.
        text: String,
    },
    /// A scope names a table this game does not have.
    #[error("no script table named `{0}`")]
    NoSuchTable(String),
    /// The ROM's tables could not be read.
    #[error("cannot read the script tables: {0}")]
    Rom(String),
}

/// A page of characters the name entry offers: its label and up to five
/// rows of up to thirteen characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlphabetPage {
    /// Shown in the label window.
    pub label: String,
    /// Characters by row; a full-width space is an empty cell.
    pub rows: Vec<Vec<char>>,
}

impl AlphabetPage {
    /// Parses a label line followed by one line per row.
    fn parse(text: &str) -> Self {
        let mut lines = text.lines();
        let label = lines.next().unwrap_or_default().to_owned();
        let rows = lines
            .take(NAME_GRID_ROWS)
            .map(|line| {
                line.chars()
                    .map(|ch| if ch == ' ' { EMPTY_CELL } else { ch })
                    .collect()
            })
            .collect();
        Self { label, rows }
    }
}

/// Translated messages keyed as [`key`] builds them, the windows they
/// need enlarged, and the name entry's pages and help line if the file
/// replaces them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Translation {
    messages: HashMap<String, String>,
    fits: HashMap<String, Fit>,
    alphabet: Vec<AlphabetPage>,
    name_entry_help: Option<String>,
}

/// The inner size a window must offer for its translated messages: the
/// widest line in pixels and, for a menu, the lines.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Fit {
    pixels: usize,
    rows: usize,
}

/// A window as a script opens it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Opened {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    kind: u8,
    opener: usize,
}

impl Opened {
    fn margin(&self) -> usize {
        if self.kind & MENU_KIND != 0 {
            MENU_MARGIN
        } else {
            TEXT_MARGIN
        }
    }

    fn columns(&self) -> usize {
        self.width.saturating_sub(2 * self.margin())
    }

    fn rows(&self) -> usize {
        self.height.saturating_sub(2) / 2
    }
}

/// Where a message lands: its window, and the other windows open with it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Placement {
    id: u8,
    window: Opened,
    others: Vec<Opened>,
}

impl Opened {
    /// Whether two windows share any cell.
    fn overlaps(&self, other: &Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

/// The key of a message: table, string index and the message's offset from
/// the string's start.
#[must_use]
pub fn key(table: &str, index: usize, offset: usize) -> String {
    format!("{table}/{index}/{offset:#x}")
}

impl Translation {
    /// Loads the entries of a PO file whose `msgctxt` lines hold keys.
    ///
    /// # Errors
    ///
    /// Returns [`TranslationError::Syntax`] on a line it cannot read.
    pub fn from_po(text: &str) -> Result<Self, TranslationError> {
        let mut messages = HashMap::new();
        let mut entry = PoEntry::default();
        let mut last: Option<PoField> = None;
        for (number, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() {
                entry.store(&mut messages);
                entry = PoEntry::default();
                last = None;
                continue;
            }
            if line.starts_with('#') {
                continue;
            }
            let (field, rest) = if let Some(rest) = line.strip_prefix("msgctxt ") {
                (PoField::Context, rest)
            } else if let Some(rest) = line.strip_prefix("msgid ") {
                (PoField::Id, rest)
            } else if let Some(rest) = line.strip_prefix("msgstr ") {
                (PoField::Text, rest)
            } else if line.starts_with('"') {
                let Some(field) = last else {
                    return Err(syntax(number, raw));
                };
                (field, line)
            } else {
                return Err(syntax(number, raw));
            };
            let value = unquote(rest).ok_or_else(|| syntax(number, raw))?;
            entry.append(field, &value);
            last = Some(field);
        }
        entry.store(&mut messages);
        let name_entry_help = messages.remove(NAME_HELP_KEY);
        let mut pages: Vec<(usize, AlphabetPage)> = messages
            .iter()
            .filter_map(|(context, text)| {
                let number = context.strip_prefix(ALPHABET_PREFIX)?.parse().ok()?;
                Some((number, AlphabetPage::parse(text)))
            })
            .collect();
        pages.sort_by_key(|(number, _)| *number);
        messages.retain(|context, _| !context.starts_with(ALPHABET_PREFIX));
        Ok(Self {
            messages,
            fits: HashMap::new(),
            alphabet: pages.into_iter().map(|(_, page)| page).collect(),
            name_entry_help,
        })
    }

    /// The name entry's character pages, if the file replaces them.
    #[must_use]
    pub fn alphabet(&self) -> &[AlphabetPage] {
        &self.alphabet
    }

    /// The name entry's help line, if the file replaces it.
    #[must_use]
    pub fn name_entry_help(&self) -> Option<&str> {
        self.name_entry_help.as_deref()
    }

    /// Works out, with `metrics`, which windows the translated messages
    /// overflow and remembers how much to enlarge them; returns one line
    /// per message that cannot fit even a screen-wide window.
    ///
    /// # Errors
    ///
    /// Returns [`TranslationError`] when a table cannot be read.
    pub fn fit(
        &mut self,
        data: &GameData<'_>,
        metrics: &TextMetrics,
    ) -> Result<Vec<String>, TranslationError> {
        self.fits.clear();
        let mut problems = Vec::new();
        let mut placements = HashMap::new();
        let mut visited = HashSet::new();
        for context in self.messages.keys() {
            let Some((table, index, _)) = split_key(context) else {
                continue;
            };
            if !visited.insert((table.to_owned(), index)) {
                continue;
            }
            let offsets = table_offsets(data, table)?;
            let mut walker = Walker::new(data.bytes(), table, &offsets);
            walker.walk(index, 0);
            placements.extend(walker.placements);
        }
        for (context, text) in &self.messages {
            let Some(placement) = placements.get(context) else {
                continue;
            };
            let (pixels, rows) = needed(text, metrics);
            let window = placement.window;
            let fit_key = key(
                placement_table(context),
                window.opener,
                usize::from(placement.id),
            );
            let is_menu = window.kind & MENU_KIND != 0;
            let rows_needed = if is_menu { rows } else { 0 };
            if pixels <= window.columns() * CELL_WIDTH && rows_needed <= window.rows() {
                continue;
            }
            let columns = pixels.div_ceil(CELL_WIDTH);
            if columns + 2 * window.margin() > SCREEN_COLUMNS || 2 + 2 * rows_needed > SCREEN_ROWS {
                problems.push(format!(
                    "{context}: needs {pixels} pixels, a window can hold {}",
                    (SCREEN_COLUMNS - 2 * window.margin()) * CELL_WIDTH
                ));
                continue;
            }
            let grown = grown_window(&window, pixels, rows_needed);
            if let Some(other) = placement
                .others
                .iter()
                .filter(|other| !window.overlaps(other))
                .find(|other| grown.overlaps(other))
            {
                problems.push(format!(
                    "{context}: needs {pixels} pixels, but a larger window would cover the one at ({}, {})",
                    other.x, other.y
                ));
                continue;
            }
            let entry = self.fits.entry(fit_key).or_default();
            entry.pixels = entry.pixels.max(pixels);
            entry.rows = entry.rows.max(rows_needed);
        }
        if let Some(help) = &self.name_entry_help
            && metrics.width(help) > NAME_HELP_PIXELS
        {
            problems.push(format!(
                "{NAME_HELP_KEY}: needs {} pixels, the help line holds {NAME_HELP_PIXELS}",
                metrics.width(help)
            ));
        }
        for (number, page) in self.alphabet.iter().enumerate() {
            let width = metrics.width(&page.label);
            if width > NAME_LABEL_PIXELS {
                problems.push(format!(
                    "{ALPHABET_PREFIX}{number}: the label needs {width} pixels, the window holds {NAME_LABEL_PIXELS}"
                ));
            }
        }
        problems.sort();
        Ok(problems)
    }

    /// The rectangle window `id`, opened by string `index` of `table` as
    /// `(x, y, width, height)`, should take so its translations fit.
    #[must_use]
    pub fn fit_window(
        &self,
        table: &str,
        index: usize,
        id: u8,
        kind: u8,
        rect: (u8, u8, u8, u8),
    ) -> (u8, u8, u8, u8) {
        let Some(fit) = self.fits.get(&key(table, index, usize::from(id))) else {
            return rect;
        };
        let margin = if kind & MENU_KIND != 0 {
            MENU_MARGIN
        } else {
            TEXT_MARGIN
        };
        let width = usize::from(rect.2).max(fit.pixels.div_ceil(CELL_WIDTH) + 2 * margin);
        let height = usize::from(rect.3).max(if fit.rows > 0 { 2 + 2 * fit.rows } else { 0 });
        let x = centered(
            usize::from(rect.0),
            usize::from(rect.2),
            width,
            SCREEN_COLUMNS,
        );
        let y = centered(
            usize::from(rect.1),
            usize::from(rect.3),
            height,
            SCREEN_ROWS,
        );
        let cell = |value: usize| u8::try_from(value).unwrap_or(u8::MAX);
        (cell(x), cell(y), cell(width), cell(height))
    }

    /// Windows this translation enlarges.
    #[must_use]
    pub fn enlarged_windows(&self) -> usize {
        self.fits.len()
    }

    /// The translation of a message, if the file has one.
    #[must_use]
    pub fn get(&self, table: &str, index: usize, offset: usize) -> Option<&str> {
        self.messages
            .get(&key(table, index, offset))
            .map(String::as_str)
    }

    /// Number of translated messages.
    #[must_use]
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Whether the file had no translated messages.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

/// Where a window that grew from `size` to `grown` starts so its center
/// stays where it was, kept within `limit` cells of the screen.
fn centered(start: usize, size: usize, grown: usize, limit: usize) -> usize {
    let center = start + size / 2;
    center
        .saturating_sub(grown / 2)
        .min(limit.saturating_sub(grown))
}

/// `window` enlarged to `pixels` wide and, if not zero, `rows` tall, as
/// [`Translation::fit_window`] will place it.
fn grown_window(window: &Opened, pixels: usize, rows: usize) -> Opened {
    let width = window
        .width
        .max(pixels.div_ceil(CELL_WIDTH) + 2 * window.margin());
    let height = window.height.max(if rows > 0 { 2 + 2 * rows } else { 0 });
    Opened {
        x: centered(window.x, window.width, width, SCREEN_COLUMNS),
        y: centered(window.y, window.height, height, SCREEN_ROWS),
        width,
        height,
        ..*window
    }
}

fn split_key(context: &str) -> Option<(&str, usize, usize)> {
    let mut parts = context.splitn(3, '/');
    let table = parts.next()?;
    let index = parts.next()?.parse().ok()?;
    let offset = parts.next()?.strip_prefix("0x")?;
    Some((table, index, usize::from_str_radix(offset, 16).ok()?))
}

fn placement_table(context: &str) -> &str {
    context.split('/').next().unwrap_or_default()
}

/// Cells the widest line of translated text needs, and its line count.
fn needed(text: &str, metrics: &TextMetrics) -> (usize, usize) {
    let mut widest = 0;
    let mut lines = 0;
    let mut current = 0;
    for step in steps(text) {
        match step {
            MessageStep::Character(ch) => {
                if current == 0 {
                    current += metrics.inset(ch);
                }
                current += metrics.advance(ch);
            }
            MessageStep::PlayerName => current += NAME_CELLS * CELL_WIDTH,
            MessageStep::Variable { digits, .. } => {
                current += usize::from(digits.max(1)) * CELL_WIDTH;
            }
            MessageStep::LineBreak => {
                widest = widest.max(current);
                lines += 1;
                current = 0;
            }
            _ => {}
        }
    }
    if current > 0 {
        widest = widest.max(current);
        lines += 1;
    }
    (widest, lines)
}

/// Walks a string and the strings it calls in order, tracking the windows
/// they open, to find the window each message is printed in.
struct Walker<'a> {
    rom: &'a [u8],
    table: &'a str,
    offsets: &'a [usize],
    windows: HashMap<u8, Opened>,
    current: u8,
    placements: HashMap<String, Placement>,
}

impl<'a> Walker<'a> {
    fn new(rom: &'a [u8], table: &'a str, offsets: &'a [usize]) -> Self {
        Self {
            rom,
            table,
            offsets,
            windows: HashMap::new(),
            current: 0,
            placements: HashMap::new(),
        }
    }

    fn walk(&mut self, index: usize, depth: usize) {
        let Some(start) = self.offsets.get(index).copied().filter(|start| *start != 0) else {
            return;
        };
        let next_string = self
            .offsets
            .iter()
            .copied()
            .filter(|offset| *offset > start)
            .min();
        let end = next_string
            .unwrap_or(start + STRING_LIMIT)
            .min(self.rom.len());
        let mut at = start;
        while at < end {
            let Ok((instruction, next)) = decode_instruction(self.rom, at) else {
                break;
            };
            at = next;
            match instruction {
                Instruction::End if next_string.is_none() => break,
                Instruction::OpenWindow {
                    id,
                    kind,
                    x,
                    y,
                    width,
                    height,
                    ..
                } => {
                    self.windows.insert(
                        id,
                        Opened {
                            x: usize::from(x),
                            y: usize::from(y),
                            width: usize::from(width),
                            height: usize::from(height),
                            kind,
                            opener: index,
                        },
                    );
                    self.current = id;
                }
                Instruction::CloseWindow { id: Some(id) } => {
                    self.windows.remove(&id);
                }
                Instruction::CloseWindow { id: None } => self.windows.clear(),
                Instruction::Present { id: Some(id) } => self.current = id,
                Instruction::Present { id: None } => {
                    if let Some(id) = self.windows.keys().max() {
                        self.current = *id;
                    }
                }
                Instruction::Call(callee) if depth < MAX_CALL_DEPTH => {
                    self.walk(usize::from(callee), depth + 1);
                }
                Instruction::Message => {
                    let offset = at - 1 - start;
                    at = self.place(index, offset, at);
                }
                _ => {}
            }
        }
    }

    fn place(&mut self, index: usize, offset: usize, mut at: usize) -> usize {
        let mut id = self.current;
        let mut leading = true;
        loop {
            let Ok((step, next)) = decode_message_step(self.rom, at) else {
                return at;
            };
            at = next;
            match step {
                MessageStep::SwitchWindow(target) if leading => id = target,
                MessageStep::End => break,
                _ => leading = false,
            }
        }
        if let Some(window) = self.windows.get(&id) {
            let others = self
                .windows
                .iter()
                .filter(|(other, _)| **other != id)
                .map(|(_, other)| *other)
                .collect();
            self.placements.insert(
                key(self.table, index, offset),
                Placement {
                    id,
                    window: *window,
                    others,
                },
            );
        }
        at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PoField {
    Context,
    Id,
    Text,
}

#[derive(Debug, Default)]
struct PoEntry {
    context: String,
    id: String,
    text: String,
}

impl PoEntry {
    fn append(&mut self, field: PoField, value: &str) {
        match field {
            PoField::Context => self.context.push_str(value),
            PoField::Id => self.id.push_str(value),
            PoField::Text => self.text.push_str(value),
        }
    }

    fn store(&self, messages: &mut HashMap<String, String>) {
        if !self.context.is_empty() && !self.text.is_empty() {
            messages.insert(self.context.clone(), self.text.clone());
        }
    }
}

fn syntax(number: usize, raw: &str) -> TranslationError {
    TranslationError::Syntax {
        line: number + 1,
        text: raw.to_owned(),
    }
}

/// The content of a quoted PO string, escapes resolved.
fn unquote(quoted: &str) -> Option<String> {
    let inner = quoted.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next()? {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            other => out.push(other),
        }
    }
    Some(out)
}

fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// One table, or a range of its strings, to put in a template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    /// Table name.
    pub table: String,
    /// First and last string index, or the whole table.
    pub range: Option<(usize, usize)>,
}

impl Scope {
    /// Parses `table` or `table:first-last`.
    ///
    /// # Errors
    ///
    /// Returns [`TranslationError::Syntax`] when the range is malformed.
    pub fn parse(text: &str) -> Result<Self, TranslationError> {
        let bad = || TranslationError::Syntax {
            line: 0,
            text: text.to_owned(),
        };
        let Some((table, range)) = text.split_once(':') else {
            return Ok(Self {
                table: text.to_owned(),
                range: None,
            });
        };
        let (first, last) = range.split_once('-').ok_or_else(bad)?;
        let first = first.parse().map_err(|_| bad())?;
        let last = last.parse().map_err(|_| bad())?;
        Ok(Self {
            table: table.to_owned(),
            range: Some((first, last)),
        })
    }
}

/// Writes a PO template with every message of the given scopes: the
/// Japanese text as the source string and an empty translation.
///
/// # Errors
///
/// Returns [`TranslationError`] when a scope names no table or the ROM's
/// tables cannot be read.
pub fn template(data: &GameData<'_>, scopes: &[Scope]) -> Result<String, TranslationError> {
    let rom = data.bytes();
    let mut out = String::from(TEMPLATE_HEADER);
    for scope in scopes {
        let offsets = table_offsets(data, &scope.table)?;
        let (first, last) = scope.range.unwrap_or((0, offsets.len().saturating_sub(1)));
        let mut sorted: Vec<usize> = offsets.iter().copied().filter(|o| *o != 0).collect();
        sorted.sort_unstable();
        let last = last.min(offsets.len().saturating_sub(1));
        for (index, start) in offsets.iter().copied().enumerate() {
            if index < first || index > last || start == 0 {
                continue;
            }
            let end = sorted
                .iter()
                .copied()
                .find(|offset| *offset > start)
                .unwrap_or(start + STRING_LIMIT)
                .min(rom.len());
            for (offset, text) in messages(rom, start, end) {
                let _ = writeln!(out, "#: {}+{offset:#x}", scope.table);
                let _ = writeln!(out, "msgctxt {}", quote(&key(&scope.table, index, offset)));
                let _ = writeln!(out, "msgid {}", quote(&text));
                let _ = writeln!(out, "msgstr \"\"\n");
            }
        }
        if scope.table == NAME_ENTRY_TABLE {
            name_entry_entries(data, &mut out)?;
        }
    }
    Ok(out)
}

/// The name entry's help line and character pages: the pages are keyed
/// `name-entry/alphabet/N`, their text a label line then one line per row
/// of characters (a space is an empty cell); a translation may have any
/// number of pages.
fn name_entry_entries(data: &GameData<'_>, out: &mut String) -> Result<(), TranslationError> {
    let table = data
        .kana_table()
        .map_err(|error| TranslationError::Rom(error.to_string()))?;
    let _ = writeln!(out, "#. The help line of the name entry, 20 cells");
    let _ = writeln!(out, "msgctxt {}", quote(NAME_HELP_KEY));
    let _ = writeln!(out, "msgid {}", quote(NAME_HELP));
    let _ = writeln!(out, "msgstr \"\"\n");
    for (number, label) in NAME_PAGE_LABELS.iter().enumerate() {
        let mut text = (*label).to_owned();
        for row in table
            .iter()
            .skip(number * NAME_GRID_ROWS)
            .take(NAME_GRID_ROWS)
        {
            text.push('\n');
            text.extend(
                row.iter()
                    .map(|ch| if *ch == EMPTY_CELL { ' ' } else { *ch }),
            );
        }
        let _ = writeln!(
            out,
            "#. Page {number} of the name entry: a label of up to 7 cells, then up to 5 rows of up to 13 characters"
        );
        let _ = writeln!(
            out,
            "msgctxt {}",
            quote(&format!("{ALPHABET_PREFIX}{number}"))
        );
        let _ = writeln!(out, "msgid {}", quote(&text));
        let _ = writeln!(out, "msgstr \"\"\n");
    }
    Ok(())
}

fn table_offsets(data: &GameData<'_>, table: &str) -> Result<Vec<usize>, TranslationError> {
    data.script_offsets(table)
        .map_err(|error| TranslationError::Rom(error.to_string()))?
        .ok_or_else(|| TranslationError::NoSuchTable(table.to_owned()))
}

/// A loaded translation as the engine's extension: it answers for the
/// messages, windows and name-entry pages its file covers.
pub struct TranslationExtension {
    translation: Translation,
}

impl TranslationExtension {
    /// Wraps a fitted translation.
    #[must_use]
    pub fn new(translation: Translation) -> Self {
        Self { translation }
    }
}

impl Extension for TranslationExtension {
    fn name(&self) -> &'static str {
        "translation"
    }

    fn translate_message(&self, table: &str, index: usize, offset: usize) -> Option<String> {
        self.translation
            .get(table, index, offset)
            .map(str::to_owned)
    }

    fn fit_window(&self, table: &str, index: usize, id: u8, kind: u8, rect: Rect) -> Option<Rect> {
        let fitted = self.translation.fit_window(table, index, id, kind, rect);
        (fitted != rect).then_some(fitted)
    }

    fn alphabet_pages(&self) -> Option<Vec<AlphabetPage>> {
        let pages = self.translation.alphabet();
        (!pages.is_empty()).then(|| pages.to_vec())
    }

    fn name_entry_help(&self) -> Option<String> {
        self.translation.name_entry_help().map(str::to_owned)
    }
}

/// The messages of the string at `start`, as (offset from `start`, text
/// with the markers translations use), reading up to `end`.
fn messages(rom: &[u8], start: usize, end: usize) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut at = start;
    while at < end {
        let Ok((instruction, next)) = decode_instruction(rom, at) else {
            break;
        };
        if instruction != Instruction::Message {
            at = next;
            continue;
        }
        let (text, after) = message_text(rom, next);
        if !text.is_empty() {
            found.push((at - start, text));
        }
        at = after;
    }
    found
}

/// The text of the message whose steps start at `at`, and where it ends.
/// Leading window switches are part of the structure, not the text.
fn message_text(rom: &[u8], mut at: usize) -> (String, usize) {
    let mut text = String::new();
    loop {
        let Ok((step, next)) = decode_message_step(rom, at) else {
            return (text, at);
        };
        at = next;
        match step {
            MessageStep::Character(ch) => text.push(ch),
            MessageStep::LineBreak => text.push('\n'),
            MessageStep::PlayerName => text.push_str("{name}"),
            MessageStep::Variable { var, digits, .. } => {
                let _ = write!(text, "{{var{var}:{digits}}}");
            }
            MessageStep::SwitchWindow(id) if !text.is_empty() => {
                let _ = write!(text, "{{window:{id}}}");
            }
            MessageStep::SwitchWindow(_) | MessageStep::Ignored(_) => {}
            MessageStep::End => return (text, at),
        }
    }
}

/// Turns translated text back into message steps: `\n` breaks the line,
/// `{name}` prints the player's name, `{varN:D}` a variable and
/// `{window:N}` moves to another window.
#[must_use]
pub fn steps(text: &str) -> Vec<MessageStep> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(ch) = rest.chars().next() {
        if ch == '{' {
            if let Some(close) = rest.find('}') {
                let marker = &rest[1..close];
                if let Some(step) = marker_step(marker) {
                    out.push(step);
                    rest = &rest[close + 1..];
                    continue;
                }
            }
        }
        out.push(match ch {
            '\n' => MessageStep::LineBreak,
            other => MessageStep::Character(other),
        });
        rest = &rest[ch.len_utf8()..];
    }
    out
}

fn marker_step(marker: &str) -> Option<MessageStep> {
    if marker == "name" {
        return Some(MessageStep::PlayerName);
    }
    if let Some(id) = marker.strip_prefix("window:") {
        return Some(MessageStep::SwitchWindow(id.parse().ok()?));
    }
    let (var, digits) = marker.strip_prefix("var")?.split_once(':')?;
    Some(MessageStep::Variable {
        var: var.parse().ok()?,
        digits: digits.parse().ok()?,
        mode: 0,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use extraction::saga::TITLE_MENU_SCRIPT_OFFSET;

    use super::*;

    #[test]
    fn reads_po_entries_with_continuations_and_escapes() {
        let po = "# comment\nmsgid \"\"\nmsgstr \"\"\n\"Language: es\\n\"\n\nmsgctxt \"dialogue/40/0x1a\"\nmsgid \"こんにちは\"\nmsgstr \"Hola,\"\n\" {name}\\n\"\n\"\\\"bien\\\"\"\n\nmsgctxt \"dialogue/40/0x30\"\nmsgid \"x\"\nmsgstr \"\"\n";
        let translation = Translation::from_po(po).unwrap();
        assert_eq!(translation.len(), 1);
        assert_eq!(
            translation.get("dialogue", 40, 0x1a),
            Some("Hola, {name}\n\"bien\"")
        );
        assert_eq!(translation.get("dialogue", 40, 0x30), None);
        assert_eq!(
            Translation::from_po("msgid \"a\"\nnonsense\n"),
            Err(TranslationError::Syntax {
                line: 2,
                text: "nonsense".to_owned()
            })
        );
    }

    #[test]
    fn turns_text_into_steps_and_back() {
        let parsed = steps("Hi {name}\n{var2:3}{window:1}!{oops}");
        assert_eq!(
            parsed,
            [
                MessageStep::Character('H'),
                MessageStep::Character('i'),
                MessageStep::Character(' '),
                MessageStep::PlayerName,
                MessageStep::LineBreak,
                MessageStep::Variable {
                    var: 2,
                    digits: 3,
                    mode: 0
                },
                MessageStep::SwitchWindow(1),
                MessageStep::Character('!'),
                MessageStep::Character('{'),
                MessageStep::Character('o'),
                MessageStep::Character('o'),
                MessageStep::Character('p'),
                MessageStep::Character('s'),
                MessageStep::Character('}'),
            ]
        );
    }

    #[test]
    fn extracts_messages_with_their_offsets_and_markers() {
        let mut rom = vec![
            0x01, 0, 0x10, 0, 0, 8, 8, 0, 0x20, 0x1C, 1, 0xA0, 0x82, 0x0D, 0x1E, 0x1D, 0x04, 0xFF,
        ];
        rom.extend([0x20, 0x1F, 2, 0x03, 0x1C, 0, 0xA2, 0x82, 0x1D, 0x22]);
        let found = messages(&rom, 0, rom.len());
        assert_eq!(
            found,
            [
                (8, "あ\n{name}".to_owned()),
                (18, "{var2:3}{window:0}い".to_owned())
            ]
        );
        assert_eq!(
            Scope::parse("dialogue:30-41").unwrap().range,
            Some((30, 41))
        );
        assert!(Scope::parse("dialogue:x").is_err());
        assert!(matches!(
            template(&GameData::new(&rom), &[Scope::parse("nowhere").unwrap()]),
            Err(TranslationError::NoSuchTable(_))
        ));
    }

    #[test]
    fn enlarges_windows_the_translated_lines_overflow() {
        let mut script = vec![0x01, 0, 0x21, 10, 10, 9, 8, 4, 0x20];
        script.extend([0xA0, 0x82, 0x1D, 0x04, 0xFF, 0x22]);
        let po = "msgctxt \"title/0/0x8\"\nmsgid \"\"\nmsgstr \"Nueva partida\\nContinuar\\nOpciones\\nExtra\"\n";
        let mut translation = Translation::from_po(po).unwrap();
        let mut rom = vec![0; 0x6C_0500 + 32];
        rom[TITLE_MENU_SCRIPT_OFFSET..TITLE_MENU_SCRIPT_OFFSET + script.len()]
            .copy_from_slice(&script);
        let metrics = TextMetrics::default();
        let problems = translation.fit(&GameData::new(&rom), &metrics).unwrap();
        assert!(problems.is_empty());
        assert_eq!(translation.enlarged_windows(), 1);
        assert_eq!(
            translation.fit_window("title", 0, 0, 0x21, (10, 10, 9, 8)),
            (6, 9, 17, 10)
        );
        assert_eq!(centered(24, 6, 12, 30), 18);
        assert_eq!(centered(0, 4, 10, 30), 0);
        assert_eq!(
            translation.fit_window("title", 0, 1, 0x21, (10, 10, 9, 8)),
            (10, 10, 9, 8)
        );
        assert_eq!(needed("ab{name}\ncd", &metrics), (80, 2));
        let wide = Translation::from_po(
            "msgctxt \"title/0/0x8\"\nmsgid \"\"\nmsgstr \"abcdefghijklmnopqrstuvwxyzabcdefg\"\n",
        );
        let problems = wide.unwrap().fit(&GameData::new(&rom), &metrics).unwrap();
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn reads_the_name_entry_pages_and_help() {
        let po = "msgctxt \"name-entry/alphabet/1\"\nmsgid \"\"\nmsgstr \"abc\\nabcdefghijklm\\nn o\"\n\nmsgctxt \"name-entry/alphabet/0\"\nmsgid \"\"\nmsgstr \"ABC\\nABC\"\n\nmsgctxt \"name-entry/help\"\nmsgid \"\"\nmsgstr \"START: done\"\n";
        let mut translation = Translation::from_po(po).unwrap();
        assert!(translation.is_empty());
        assert_eq!(translation.name_entry_help(), Some("START: done"));
        let long = "msgctxt \"name-entry/alphabet/0\"\nmsgid \"\"\nmsgstr \"a label far too long\\nabc\"\n\nmsgctxt \"name-entry/help\"\nmsgid \"\"\nmsgstr \"a help line that is far too long for the window\"\n";
        let mut long = Translation::from_po(long).unwrap();
        assert!(
            translation
                .fit(&GameData::new(&[]), &TextMetrics::default())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            long.fit(&GameData::new(&[]), &TextMetrics::default())
                .unwrap()
                .len(),
            2
        );
        let pages = translation.alphabet();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].label, "ABC");
        assert_eq!(pages[1].rows[1], ['n', EMPTY_CELL, 'o']);
        assert_eq!(pages[1].rows[0].len(), 13);
    }

    #[test]
    fn the_last_string_of_a_table_ends_at_its_end_instruction() {
        let mut rom = vec![
            0x22, 0x01, 0, 0x10, 0, 0, 8, 8, 0, 0x20, 0xA0, 0x82, 0x1D, 0x22,
        ];
        rom.extend([0x01, 5, 0x10, 0, 0, 4, 4, 0, 0x20, 0xA2, 0x82, 0x1D]);
        let offsets = [1];
        let mut walker = Walker::new(&rom, "pause-menu", &offsets);
        walker.walk(0, 0);
        assert_eq!(walker.placements.len(), 1);
        assert_eq!(walker.placements["pause-menu/0/0x8"].id, 0);
    }

    #[test]
    fn quotes_round_trip() {
        let text = "a\"b\\c\nd";
        assert_eq!(unquote(&quote(text)).unwrap(), text);
    }
}
