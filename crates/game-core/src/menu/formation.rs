//! The formation screen (部隊編成): the party's units on the battle field,
//! a list of the members, and a cursor on the field to put a unit in a slot
//! or take it out.
//!
//! The original runs it as a task of its own (`0x08037B84`) after the
//! pause menu darkens, over the battle module's field (see
//! `extraction::saga_formation`). Its windows and texts are scripts of the
//! `battle-menu` and `battle-text` tables: the help line (window 1), the
//! member list (window 2) with its menu (script 15, opcode `0x3C` in its
//! widest mode) and the Zoid of the member under the cursor (window 3).
//! Each pass of the task's loop runs one state's code, whose scripts cost
//! their frames, then waits a frame.

use extraction::saga::SpriteSheet;
use extraction::saga_battle::EffectSprite;
use extraction::saga_formation::{BattleField, FieldLayer, SLOTS};
use extraction::saga_party;
use gba_runtime::ppu::{FADE_STEPS, Palette, PaletteBank, darken, draw_background};
use platform::{Button, Frame, Input, Rgb};

use crate::battle::draw_piece;
use crate::data::GameData;
use crate::script::{ScriptError, ScriptRunner};
use crate::translation::{BATTLE_MENU_TABLE, BATTLE_TEXT_TABLE, NAME_TABLE};
use crate::windows::ScriptWindows;
use crate::{ScriptHost, TextPainter, WindowPainter, draw_sprite};

const SCRIPT_RESET: usize = 1;
const SCRIPT_HELP_WINDOW: usize = 2;
const SCRIPT_PRESENT_HELP: usize = 5;
const SCRIPT_DRAW_HELP: usize = 6;
const SCRIPT_CLEAR_HELP: usize = 7;
const SCRIPT_LIST_WINDOWS: usize = 8;
const SCRIPT_PRESENT_LIST: usize = 9;
const SCRIPT_PRESENT_HEADER: usize = 10;
const SCRIPT_CLEAR_LIST: usize = 11;
const SCRIPT_CLEAR_HEADER: usize = 12;
const SCRIPT_DRAW_LIST: usize = 13;
const SCRIPT_DRAW_HEADER: usize = 14;
const SCRIPT_LIST_MENU: usize = 15;
const TEXT_LINE_BREAK: usize = 0;
const TEXT_IN_FORMATION: usize = 0x11;
const TEXT_SIZES: usize = 0x12;
const TEXT_NO_ZOID: usize = 0x15;
const TEXT_NO_UNIT: usize = 0x16;
const TEXT_LIST_HELP: usize = 0x17;
const TEXT_PLACE_HELP: usize = 0x18;
const TEXT_REMOVE_HELP: usize = 0x19;
const TEXT_NO_SIZE: usize = 0x1C;
const TEXT_BROKEN: usize = 0x32;
const TEXT_SPACE: usize = 94 + 0xC;
const CHARACTER_NAMES: usize = 154;
const ZOID_NAMES: usize = 1;
const LIST_WINDOW: u8 = 2;
const HEADER_WINDOW: u8 = 3;
/// Members a page of the list shows (`0x080380E8`).
const PAGE_LINES: usize = 5;
/// The cell the size class is printed in, after the name.
const SIZE_COLUMN: usize = 10;
const LARGE: u8 = 2;
const UNIT_DISABLED: u16 = 0x800;
const PLACE_SOUND: u8 = 0x51;
const REMOVE_SOUND: u8 = 0x3E;
const BACK_SOUND: u8 = 0x3F;
const MOVE_SOUND: u8 = 0x40;
const KEY_A: u16 = 1;
const KEY_B: u16 = 2;
const KEY_RIGHT: u16 = 0x10;
const KEY_LEFT: u16 = 0x20;
const KEY_UP: u16 = 0x40;
const KEY_DOWN: u16 = 0x80;
const CONFIRMED: u16 = 1;
const PAGE_LEFT: u16 = 2;
const PAGE_RIGHT: u16 = 4;
const MOVED_LEFT: u16 = 8;
const MOVED_UP: u16 = 0x20;
const MOVED_DOWN: u16 = 0x40;
const STARTED: u16 = 0x80;
/// The slot a unit to be placed starts on.
const FIRST_PLACE: usize = 1;
/// How far the cursor's anchor lies from a slot's (`0x08038510`).
const CURSOR_OFFSET: (i32, i32) = (8, -16);
const FADE_TOP: u32 = 31;
/// Frames after the choice before the menu starts darkening.
const DARKEN_DELAY: u32 = 4;
/// Frames after the choice at which the screen is built, in the dark.
const BUILD_AT: u32 = 36;
/// Frames after the choice at which the screen starts brightening.
const BRIGHTEN_AT: u32 = 60;
/// Frames after the choice at which the help line is printed.
const HELP_AT: u32 = 95;
/// Frames after B or START before the screen starts darkening.
const LEAVE_DELAY: u32 = 4;
/// Frames after B or START at which the pause menu is rebuilt.
const LEAVE_FRAMES: u32 = 44;

/// What the screen is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The pause menu darkens, then the screen is built and brightens;
    /// frames since the choice.
    Opening(u32),
    /// The member list's menu runs.
    List,
    /// The cursor is on the field.
    Slots(Aim),
    /// Darkening after B or START; frames since.
    Leaving(u32),
    Closed,
}

/// What the cursor on the field is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Aim {
    /// Placing the character's unit.
    Place(u8),
    /// Taking a unit out.
    Remove,
}

/// The formation screen.
pub(super) struct Formation {
    field: Option<BattleField>,
    cursor: Option<EffectSprite>,
    anchors: [(i32, i32); SLOTS],
    sprites: Vec<(u16, SpriteSheet)>,
    menu: ScriptRunner,
    text: ScriptRunner,
    names: ScriptRunner,
    phase: Phase,
    busy: u32,
    pages: Vec<Vec<u8>>,
    page: usize,
    line: usize,
    /// The Zoid the header shows; it is only redrawn when that changes.
    header: u16,
    slot: usize,
    cursor_shown: bool,
    held: Input,
}

impl Formation {
    /// The screen over `state`, opened on the frame 部隊編成 was chosen.
    pub(super) fn new(data: &GameData<'_>, state: &[u8]) -> Self {
        let offsets = |table| {
            data.script_offsets(table)
                .ok()
                .flatten()
                .unwrap_or_default()
        };
        let anchors = std::array::from_fn(|slot| data.slot_anchor(slot).unwrap_or_default());
        let mut sprites: Vec<(u16, SpriteSheet)> = Vec::new();
        for character in saga_party::members(state) {
            let Some(zoid) = unit_of(state, character).map(|unit| unit.zoid) else {
                continue;
            };
            if sprites.iter().all(|(known, _)| *known != zoid)
                && let Ok(sheet) = data.zoid_status_sprite(usize::from(zoid))
            {
                sprites.push((zoid, sheet));
            }
        }
        Self {
            field: data.battle_field(),
            cursor: data.slot_cursor(),
            anchors,
            sprites,
            menu: ScriptRunner::named(BATTLE_MENU_TABLE, offsets(BATTLE_MENU_TABLE)),
            text: ScriptRunner::named(BATTLE_TEXT_TABLE, offsets(BATTLE_TEXT_TABLE)),
            names: ScriptRunner::named(NAME_TABLE, offsets(NAME_TABLE)),
            phase: Phase::Opening(0),
            busy: 0,
            pages: pages(state),
            page: 0,
            line: 0,
            header: NO_ZOID,
            slot: 0,
            cursor_shown: false,
            held: Input::default(),
        }
    }

    /// Whether the screen has handed back to the pause menu.
    pub(super) fn is_closed(&self) -> bool {
        self.phase == Phase::Closed
    }

    /// Whether the screen shows itself rather than the darkening menu.
    pub(super) fn covers(&self) -> bool {
        !matches!(self.phase, Phase::Opening(frames) if frames < BUILD_AT)
    }

    /// The game's fade level, 0 to 31.
    pub(super) fn fade(&self) -> u32 {
        match self.phase {
            Phase::Opening(frames) if frames < BRIGHTEN_AT => {
                frames.saturating_sub(DARKEN_DELAY).min(FADE_TOP)
            }
            Phase::Opening(frames) => FADE_TOP.saturating_sub(frames - BRIGHTEN_AT),
            Phase::Leaving(frames) => frames.saturating_sub(LEAVE_DELAY).min(FADE_TOP),
            Phase::Closed => FADE_TOP,
            Phase::List | Phase::Slots(_) => 0,
        }
    }

    /// Advances one frame; `state` is the game-state block the screen
    /// changes.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a script cannot run.
    pub(super) fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
        state: &mut [u8],
    ) -> Result<(), ScriptError> {
        let previous = self.held;
        self.held = input;
        let keys = [
            (Button::A, KEY_A),
            (Button::B, KEY_B),
            (Button::Up, KEY_UP),
            (Button::Down, KEY_DOWN),
            (Button::Left, KEY_LEFT),
            (Button::Right, KEY_RIGHT),
        ]
        .into_iter()
        .filter(|(button, _)| input.is_held(*button) && !previous.is_held(*button))
        .fold(0, |keys, (_, key)| keys | key);
        match self.phase {
            Phase::Opening(frames) => {
                let frames = frames + 1;
                self.phase = Phase::Opening(frames);
                if frames == BUILD_AT {
                    self.build(rom, windows, state)?;
                }
                if frames == HELP_AT {
                    self.busy = 0;
                    self.show_list(rom, windows)?;
                }
                return Ok(());
            }
            Phase::Leaving(frames) => {
                let frames = frames + 1;
                self.phase = Phase::Leaving(frames);
                if frames >= LEAVE_FRAMES {
                    self.run(rom, Table::Menu, SCRIPT_RESET, windows)?;
                    self.phase = Phase::Closed;
                }
                return Ok(());
            }
            Phase::Closed => return Ok(()),
            Phase::List | Phase::Slots(_) => {}
        }
        if self.busy > 0 {
            self.busy -= 1;
            return Ok(());
        }
        match self.phase {
            Phase::List => self.list_step(rom, input, windows, state),
            Phase::Slots(aim) => self.slots_step(rom, keys, aim, windows, state),
            _ => Ok(()),
        }
    }

    /// Builds the screen in the dark (`0x08037CD8`): the windows, the header
    /// and the first page of the list.
    fn build(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
        state: &[u8],
    ) -> Result<(), ScriptError> {
        self.pages = pages(state);
        self.page = 0;
        self.line = 0;
        self.header = NO_ZOID;
        for script in [
            SCRIPT_RESET,
            SCRIPT_HELP_WINDOW,
            SCRIPT_LIST_WINDOWS,
            SCRIPT_PRESENT_HELP,
            SCRIPT_PRESENT_LIST,
            SCRIPT_PRESENT_HEADER,
        ] {
            self.run(rom, Table::Menu, script, windows)?;
        }
        self.show_header(rom, windows, state)?;
        self.print_page(rom, windows, state)
    }

    /// The list's help and its menu (`0x08037D80`).
    fn show_list(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.help(rom, TEXT_LIST_HELP, windows)?;
        windows.set_cursor(LIST_WINDOW, Some(self.line));
        windows.set_cursor(LIST_WINDOW, None);
        self.menu.start(SCRIPT_LIST_MENU)?;
        self.phase = Phase::List;
        Ok(())
    }

    fn help(
        &mut self,
        rom: &[u8],
        text: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run(rom, Table::Menu, SCRIPT_DRAW_HELP, windows)?;
        self.run(rom, Table::Menu, SCRIPT_CLEAR_HELP, windows)?;
        self.print(rom, Table::Text, text, 1, windows)?;
        self.run(rom, Table::Menu, SCRIPT_PRESENT_HELP, windows)
    }

    /// One frame of the list's menu, and what its end asks for
    /// (`0x08037DA4`).
    fn list_step(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
        state: &mut [u8],
    ) -> Result<(), ScriptError> {
        if !self.menu.update(rom, input, windows)? {
            return Ok(());
        }
        let [code, line, ..] = *self.menu.vars();
        match code {
            MOVED_UP | MOVED_DOWN => {
                self.run(rom, Table::Menu, SCRIPT_DRAW_HEADER, windows)?;
                self.line = usize::from(line);
                self.show_header(rom, windows, state)?;
            }
            PAGE_LEFT | PAGE_RIGHT => {
                let last = self.pages.len().saturating_sub(1);
                let page = if code == PAGE_LEFT {
                    self.page.saturating_sub(1)
                } else {
                    (self.page + 1).min(last)
                };
                if page != self.page {
                    self.page = page;
                    self.print_page(rom, windows, state)?;
                    windows.set_cursor(LIST_WINDOW, Some(0));
                    self.run(rom, Table::Menu, SCRIPT_DRAW_HEADER, windows)?;
                    self.line = 0;
                    self.show_header(rom, windows, state)?;
                }
            }
            CONFIRMED => {
                if let Some(aim) = self.choose(state) {
                    return self.enter_slots(rom, aim, windows);
                }
            }
            MOVED_LEFT => {
                self.slot = (0..SLOTS)
                    .find(|&slot| saga_party::formation(state)[slot].is_some())
                    .unwrap_or(0);
                return self.enter_slots(rom, Aim::Remove, windows);
            }
            STARTED => {
                self.leave();
                return Ok(());
            }
            0 if input.is_held(Button::B) => {
                self.leave();
                return Ok(());
            }
            _ => {}
        }
        self.busy += 1;
        self.menu.start(SCRIPT_LIST_MENU)
    }

    /// What A on the member under the cursor starts: taking its unit out
    /// when it is in the formation (the cursor on its slot), placing it
    /// otherwise; nothing for a member without a unit or with a broken
    /// one (`0x08037E48`).
    fn choose(&mut self, state: &[u8]) -> Option<Aim> {
        let character = self.member()?;
        let unit = unit_of(state, character)?;
        if let Some(slot) = saga_party::formation_slot(state, character) {
            self.slot = slot;
            return Some(Aim::Remove);
        }
        if unit.flags & UNIT_DISABLED != 0 {
            return None;
        }
        self.slot = FIRST_PLACE;
        Some(Aim::Place(character))
    }

    /// Shows the cursor on the field with its help (`0x08037F20`,
    /// `0x08037FE8`).
    fn enter_slots(
        &mut self,
        rom: &[u8],
        aim: Aim,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.cursor_shown = true;
        let help = if matches!(aim, Aim::Place(_)) {
            TEXT_PLACE_HELP
        } else {
            TEXT_REMOVE_HELP
        };
        self.help(rom, help, windows)?;
        self.phase = Phase::Slots(aim);
        Ok(())
    }

    /// One frame of the cursor on the field (`0x08037F52`, `0x08038024`):
    /// the pad moves it, A places or takes out, B goes back to the list.
    fn slots_step(
        &mut self,
        rom: &[u8],
        keys: u16,
        aim: Aim,
        windows: &mut ScriptWindows<'_>,
        state: &mut [u8],
    ) -> Result<(), ScriptError> {
        let large = match aim {
            Aim::Place(character) => {
                unit_of(state, character).is_some_and(|unit| unit.size == LARGE)
            }
            Aim::Remove => false,
        };
        let moved = next_slot(self.slot, large, keys);
        if moved != self.slot {
            windows.play_sound(MOVE_SOUND);
            self.slot = moved;
        }
        if keys & KEY_A != 0 {
            if let Aim::Place(character) = aim {
                windows.play_sound(PLACE_SOUND);
                saga_party::join_formation(state, self.slot, character);
            } else {
                windows.play_sound(REMOVE_SOUND);
                saga_party::leave_formation(state, self.slot);
            }
        } else if keys & KEY_B != 0 {
            windows.play_sound(BACK_SOUND);
        } else {
            return Ok(());
        }
        self.busy += 1;
        self.back_to_list(rom, windows, state)
    }

    /// Hides the cursor and prints the page again (`0x08037FC8`), then the
    /// list's help and menu.
    fn back_to_list(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
        state: &[u8],
    ) -> Result<(), ScriptError> {
        self.cursor_shown = false;
        self.run(rom, Table::Menu, SCRIPT_DRAW_LIST, windows)?;
        self.print_page(rom, windows, state)?;
        self.busy += 1;
        self.show_list(rom, windows)
    }

    fn leave(&mut self) {
        self.cursor_shown = false;
        self.phase = Phase::Leaving(0);
    }

    /// The character under the list's cursor.
    fn member(&self) -> Option<u8> {
        self.pages.get(self.page)?.get(self.line).copied()
    }

    /// Prints the Zoid of the member under the cursor in the header when it
    /// differs from the one shown (`0x08038314`).
    fn show_header(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
        state: &[u8],
    ) -> Result<(), ScriptError> {
        let zoid = self
            .member()
            .and_then(|character| unit_of(state, character))
            .map_or(NO_ZOID, |unit| unit.zoid);
        if zoid == self.header {
            return Ok(());
        }
        self.header = zoid;
        self.run(rom, Table::Menu, SCRIPT_CLEAR_HEADER, windows)?;
        self.run(rom, Table::Menu, SCRIPT_PRESENT_HEADER, windows)?;
        if zoid == NO_ZOID {
            self.print(rom, Table::Text, TEXT_NO_ZOID, HEADER_WINDOW, windows)?;
        } else {
            let name = ZOID_NAMES + usize::from(zoid);
            self.print(rom, Table::Names, name, HEADER_WINDOW, windows)?;
        }
        self.run(rom, Table::Menu, SCRIPT_PRESENT_HEADER, windows)
    }

    /// Prints the page of the list (`0x08038194`): per member a mark (★ in
    /// the formation, × without a unit, 壊 for a broken one), the name,
    /// then from cell 10 the size class, or － without a unit.
    fn print_page(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
        state: &[u8],
    ) -> Result<(), ScriptError> {
        let last = self.pages.len().saturating_sub(1);
        windows.set_scroll_marks(LIST_WINDOW, (self.page > 0, self.page < last));
        self.run(rom, Table::Menu, SCRIPT_CLEAR_LIST, windows)?;
        self.run(rom, Table::Menu, SCRIPT_PRESENT_LIST, windows)?;
        let members = self.pages.get(self.page).cloned().unwrap_or_default();
        for (index, character) in members.into_iter().enumerate() {
            if index > 0 {
                self.print(rom, Table::Text, TEXT_LINE_BREAK, LIST_WINDOW, windows)?;
            }
            self.print_member(rom, character, windows, state)?;
        }
        self.run(rom, Table::Menu, SCRIPT_PRESENT_LIST, windows)
    }

    fn print_member(
        &mut self,
        rom: &[u8],
        character: u8,
        windows: &mut ScriptWindows<'_>,
        state: &[u8],
    ) -> Result<(), ScriptError> {
        let unit = unit_of(state, character);
        let mark = if saga_party::formation_slot(state, character).is_some() {
            TEXT_IN_FORMATION
        } else {
            match unit {
                None => TEXT_NO_UNIT,
                Some(unit) if unit.flags & UNIT_DISABLED != 0 => TEXT_BROKEN,
                Some(_) => TEXT_SPACE,
            }
        };
        self.print(rom, Table::Text, mark, LIST_WINDOW, windows)?;
        if character == 0 {
            for ch in windows.player_name().chars() {
                windows.put_char(LIST_WINDOW, ch);
            }
        } else {
            let name = CHARACTER_NAMES + usize::from(character);
            self.print(rom, Table::Names, name, LIST_WINDOW, windows)?;
        }
        windows.pad_to(LIST_WINDOW, SIZE_COLUMN);
        let size = unit.map_or(TEXT_NO_SIZE, |unit| {
            TEXT_SIZES + usize::from(if unit.size <= LARGE { unit.size } else { 1 })
        });
        self.print(rom, Table::Text, size, LIST_WINDOW, windows)
    }

    /// Runs `script` of `table` to its end at once, counting the frames the
    /// original spends on it. The runner sees the frame's keys, so a key
    /// held on does not count as pressed again when a menu starts.
    fn run(
        &mut self,
        rom: &[u8],
        table: Table,
        script: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let runner = match table {
            Table::Menu => &mut self.menu,
            Table::Text => &mut self.text,
            Table::Names => &mut self.names,
        };
        runner.start(script)?;
        while !runner.update(rom, self.held, windows)? {
            if runner.is_waiting_for_key() {
                break;
            }
            self.busy += 1;
        }
        Ok(())
    }

    /// Prints `script` of `table` in `window`.
    fn print(
        &mut self,
        rom: &[u8],
        table: Table,
        script: usize,
        window: u8,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        match table {
            Table::Menu => self.menu.select_window(window),
            Table::Text => self.text.select_window(window),
            Table::Names => self.names.select_window(window),
        }
        self.run(rom, table, script, windows)
    }

    /// Draws the field, the units, the windows and the cursor, darkened by
    /// the fade.
    pub(super) fn draw(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
        state: &[u8],
    ) {
        frame.fill(Rgb::default());
        if let Some(field) = &self.field {
            draw_layer(frame, &field.grid);
            draw_layer(frame, &field.platform);
        }
        self.draw_units(frame, state);
        windows.draw(frame, skin, painter);
        self.draw_cursor(frame);
        let level = self.fade().min(u32::from(FADE_STEPS));
        darken(frame, u8::try_from(level).unwrap_or(FADE_STEPS));
    }

    /// The units in their slots, the nearer (lower) ones over the others,
    /// as the entities' order in OAM puts them.
    fn draw_units(&self, frame: &mut Frame, state: &[u8]) {
        let mut units: Vec<((i32, i32), u16)> = saga_party::formation(state)
            .iter()
            .zip(self.anchors)
            .filter_map(|(slot, anchor)| {
                let (unit, _) = (*slot)?;
                Some((anchor, saga_party::unit_status(state, unit)?.zoid))
            })
            .collect();
        units.sort_by_key(|((_, y), _)| *y);
        for ((x, y), zoid) in units {
            let Some((_, sheet)) = self.sprites.iter().find(|(known, _)| *known == zoid) else {
                continue;
            };
            if let (Some(sprite), Some(image)) = (sheet.frames.first(), sheet.frame_image(0)) {
                draw_sprite(
                    frame,
                    x + i32::from(sprite.x),
                    y + i32::from(sprite.y),
                    &image,
                    &sheet.palette,
                    sprite.mirrored,
                );
            }
        }
    }

    /// The cursor on its slot. The screen draws the entities without
    /// running their animations, so it keeps its first frame.
    fn draw_cursor(&self, frame: &mut Frame) {
        let Some(cursor) = self.cursor.as_ref().filter(|_| self.cursor_shown) else {
            return;
        };
        let (x, y) = self.anchors[self.slot.min(SLOTS - 1)];
        let anchor = (x + CURSOR_OFFSET.0, y + CURSOR_OFFSET.1);
        let shown = cursor.animation.first().map_or(0, |step| step.frame);
        let palette = Palette::new(cursor.palette.map(Palette::from_bgr555));
        let (width, height) = (frame.width(), frame.height());
        let mut layer = vec![None; width * height];
        for piece in cursor.frames.get(shown).into_iter().flatten() {
            draw_piece(&mut layer, cursor, &palette, piece, anchor, false);
        }
        for (index, color) in layer.into_iter().enumerate() {
            if let Some(color) = color {
                frame.set_pixel(index % width, index / width, color);
            }
        }
    }
}

/// Which table a script belongs to.
#[derive(Debug, Clone, Copy)]
enum Table {
    Menu,
    Text,
    Names,
}

/// The Zoid id the header starts with, which also stands for no Zoid.
const NO_ZOID: u16 = 0xFF;

/// Where the pad moves the cursor from `slot` (`0x080383C0`): up and down a
/// column, across between the columns' matching slots; an L unit only
/// moves between the columns' middles. `keys` are the keys pressed this
/// frame, as the game's bits.
fn next_slot(slot: usize, large: bool, keys: u16) -> usize {
    let (up, down) = (keys & KEY_UP != 0, keys & KEY_DOWN != 0);
    let (left, right) = (keys & KEY_LEFT != 0, keys & KEY_RIGHT != 0);
    if large {
        return match slot {
            1 if right => 4,
            4 if left => 1,
            _ => slot,
        };
    }
    match slot {
        0 if down => 1,
        0 if right => 3,
        1 if down => 2,
        1 if up => 0,
        1 if right => 4,
        2 if up => 1,
        2 if right => 5,
        3 if down => 4,
        3 if left => 0,
        4 if down => 5,
        4 if up => 3,
        4 if left => 1,
        5 if up => 4,
        5 if left => 2,
        _ => slot,
    }
}

/// The party's members in pages of five, in the order the game lists
/// characters (`0x080380E8`).
fn pages(state: &[u8]) -> Vec<Vec<u8>> {
    let members = saga_party::members(state);
    let pages: Vec<Vec<u8>> = members.chunks(PAGE_LINES).map(<[u8]>::to_vec).collect();
    if pages.is_empty() {
        vec![Vec::new()]
    } else {
        pages
    }
}

fn unit_of(state: &[u8], character: u8) -> Option<saga_party::UnitStatus> {
    saga_party::character_unit(state, character)
        .and_then(|unit| saga_party::unit_status(state, unit))
}

/// Draws a field layer over the frame: its tile 0 is clear.
fn draw_layer(frame: &mut Frame, layer: &FieldLayer) {
    let mut palettes = [[0u16; 16]; 16];
    palettes[usize::from(layer.bank & 0x0F)] = layer.palette;
    let bank = PaletteBank::from_bgr555(&palettes);
    draw_background(
        frame,
        |column, row| layer.entry(column, row),
        |tile| layer.tiles.tile(tile),
        &bank,
        (0, 0),
        true,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_moves_within_and_across_the_columns() {
        let (down, up, left, right) = (KEY_DOWN, KEY_UP, KEY_LEFT, KEY_RIGHT);
        assert_eq!(next_slot(0, false, down), 1);
        assert_eq!(next_slot(2, false, down), 2);
        assert_eq!(next_slot(1, false, right), 4);
        assert_eq!(next_slot(4, false, right), 4);
        assert_eq!(next_slot(5, false, left), 2);
        assert_eq!(next_slot(3, false, up), 3);
        assert_eq!(next_slot(1, true, down), 1);
        assert_eq!(next_slot(1, true, right), 4);
        assert_eq!(next_slot(4, true, left), 1);
    }

    #[test]
    fn members_fill_pages_of_five() {
        let mut state = vec![0; formats::progress::STATE_LEN];
        for character in 0..7usize {
            state[0x34A4 + character * 4] = 2;
        }
        assert_eq!(pages(&state), [vec![0, 1, 2, 3, 4], vec![5, 6]]);
        assert_eq!(pages(&[0; 16]), [Vec::<u8>::new()]);
    }
}
