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

use extraction::saga::{self, NAME_ENTRY_SCRIPTS, PAUSE_MENU_SCRIPTS, TITLE_MENU_SCRIPT_OFFSET};
use formats::script_ops::{Instruction, MessageStep, decode_instruction, decode_message_step};
use thiserror::Error;

/// Table name of the title menu script.
pub const TITLE_TABLE: &str = "title";
/// Table name of the name-entry scripts.
pub const NAME_ENTRY_TABLE: &str = "name-entry";
/// Table name of the pause-menu scripts.
pub const PAUSE_MENU_TABLE: &str = "pause-menu";
/// Table name of the dialogue strings.
pub const DIALOGUE_TABLE: &str = "dialogue";
const STRING_LIMIT: usize = 0x1000;
const TEMPLATE_HEADER: &str = "msgid \"\"\nmsgstr \"\"\n\"Content-Type: text/plain; charset=UTF-8\\n\"\n\"Language: ja\\n\"\n\n";

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

/// Translated messages keyed as [`key`] builds them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Translation {
    messages: HashMap<String, String>,
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
        Ok(Self { messages })
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
pub fn template(rom: &[u8], scopes: &[Scope]) -> Result<String, TranslationError> {
    let mut out = String::from(TEMPLATE_HEADER);
    for scope in scopes {
        let offsets = table_offsets(rom, &scope.table)?;
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
    }
    Ok(out)
}

fn table_offsets(rom: &[u8], table: &str) -> Result<Vec<usize>, TranslationError> {
    let rom_error = |error: &dyn std::fmt::Display| TranslationError::Rom(error.to_string());
    match table {
        TITLE_TABLE => Ok(vec![TITLE_MENU_SCRIPT_OFFSET]),
        NAME_ENTRY_TABLE => NAME_ENTRY_SCRIPTS
            .offsets(rom)
            .map_err(|error| rom_error(&error)),
        PAUSE_MENU_TABLE => PAUSE_MENU_SCRIPTS
            .offsets(rom)
            .map_err(|error| rom_error(&error)),
        DIALOGUE_TABLE => saga::string_table(DIALOGUE_TABLE)
            .ok_or_else(|| TranslationError::NoSuchTable(table.to_owned()))?
            .offsets(rom)
            .map_err(|error| rom_error(&error)),
        other => Err(TranslationError::NoSuchTable(other.to_owned())),
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
            template(&rom, &[Scope::parse("nowhere").unwrap()]),
            Err(TranslationError::NoSuchTable(_))
        ));
    }

    #[test]
    fn quotes_round_trip() {
        let text = "a\"b\\c\nd";
        assert_eq!(unquote(&quote(text)).unwrap(), text);
    }
}
