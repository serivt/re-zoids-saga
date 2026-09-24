//! The pause menu START opens on the field.
//!
//! The original assembles it from small scripts of one table (see
//! `extraction::saga::PAUSE_MENU_SCRIPTS`): script 46 opens the help line
//! and the six-item list, 64 and 44 the party panel and the money box,
//! whose values the game's code prints right-aligned, and 47 prints the
//! help text and runs the menu. Choices open the status submenu (48, 49),
//! the weapons screen (128, 129, 133), the message-speed setting (151–159)
//! or the save question (160, 61, 161, 162), or print a notice. The
//! screens this port does not have end in the table's "not done yet"
//! notice (63). Behind the windows a logo map drifts one pixel per frame
//! diagonally over a static texture.

use extraction::saga::{BootError, PauseWallpaper, SpriteSheet};
use extraction::saga_party::{self, UnitStatus};

use crate::data::GameData;
use gba_runtime::ppu::{FullPalette, draw_background_256};
use platform::{Frame, Input, Rgb};

use crate::script::{ScriptError, ScriptRunner};
use crate::translation::{NAME_TABLE, PAUSE_MENU_TABLE};
use crate::windows::ScriptWindows;
use crate::{ScriptHost, TextPainter, WindowPainter, draw_sprite};

const SCRIPT_WAIT_KEY: usize = 37;
const SCRIPT_MONEY_WINDOW: usize = 44;
const SCRIPT_MONEY_UNIT: usize = 45;
const SCRIPT_OPEN_MENU: usize = 46;
const SCRIPT_MENU: usize = 47;
const SCRIPT_STATUS_WINDOW: usize = 48;
const SCRIPT_STATUS_MENU: usize = 49;
const SCRIPT_NO_ITEMS: usize = 56;
const SCRIPT_NO_WEAPONS: usize = 57;
const SCRIPT_NO_ZI_DATA: usize = 58;
const SCRIPT_NO_ZI_ITEMS: usize = 60;
const SCRIPT_YES_NO: usize = 61;
const SCRIPT_NOT_DONE: usize = 63;
const SCRIPT_UNIT_LIST: usize = 68;
const SCRIPT_UNIT_EMPTY: usize = 69;
const SCRIPT_CHARACTER_WINDOWS: usize = 70;
const SCRIPT_STAT_LABELS: [usize; CHARACTER_STATS] = [71, 72, 73, 74, 75];
const SCRIPT_ZOID_HELP_BEFORE: usize = 76;
const SCRIPT_ZOID_HELP_AFTER: usize = 77;
const SCRIPT_LEAVE_HELP: usize = 78;
const SCRIPT_PANEL_WINDOW: usize = 64;
const SCRIPT_LEVEL_LABEL: usize = 65;
const SCRIPT_EXP_LABEL: usize = 66;
const SCRIPT_NEXT_LABEL: usize = 67;
const SCRIPT_BOOK_WINDOW: usize = 115;
const SCRIPT_BOOK_MENU: usize = 116;
const SCRIPT_WEAPONS_WINDOWS: usize = 128;
const SCRIPT_NO_ZOID: usize = 129;
const SCRIPT_WEAPONS_HELP: usize = 133;
const SCRIPT_NOT_BOARDED: usize = 134;
const SCRIPT_SPEED_WINDOW: usize = 151;
const SCRIPT_SPEED_LIST: usize = 152;
const SCRIPT_SPEED_MENU: usize = 153;
const SCRIPT_SPEED_VALUE: usize = 153;
const SCRIPT_SAVE_QUESTION: usize = 160;
const SCRIPT_SAVED: usize = 161;
const SCRIPT_SAVE_CANCELED: usize = 162;
const SCRIPT_CLEAR_HELP: usize = 0;
const SCRIPT_CLEAR_CHARACTER: usize = 1;
const SCRIPT_CLEAR_PORTRAIT: usize = 2;
const SCRIPT_CLEAR_MEMBERS: usize = 3;
const SCRIPT_PRESENT_ALL: usize = 24;
const SCRIPT_DRAW_HELP: usize = 25;
const SCRIPT_DRAW_CHARACTER: usize = 26;
const SCRIPT_DRAW_MEMBERS: usize = 28;
const SCRIPT_MEMBER_MENU: usize = 35;
const SCRIPT_DISABLED: usize = 87;
const SCRIPT_PORTRAITS: usize = 335;
const CHARACTER_NAMES: usize = 154;
const ZOID_NAMES: usize = 1;
const MEMBERS_PER_PAGE: usize = 6;
const UNIT_DISABLED: u16 = 0x800;
const HP_CELLS: usize = 4;
const EP_CELLS: usize = 3;
const CONFIRMED: u16 = 1;
const MOVED_UP: u16 = 0x20;
const MOVED_DOWN: u16 = 0x40;
const PAGE_LEFT: u16 = 2;
const PAGE_RIGHT: u16 = 4;
const MENU_MOVE_SOUND: u8 = 0x40;
const SCRIPT_ZOID_WINDOWS: usize = 80;
const SCRIPT_HP_LABEL: usize = 81;
const SCRIPT_EP_LABEL: usize = 82;
const SCRIPT_SP_LABEL: usize = 83;
const SCRIPT_DF_LABEL: usize = 84;
const SCRIPT_TRAINING_LABEL: usize = 85;
const SCRIPT_SLASH: usize = 86;
const SCRIPT_SIZES: usize = 88;
const SCRIPT_PERCENT: usize = 39;
const ZOID_WINDOW: u8 = 1;
/// Where the Zoid's picture is anchored: the call at `0x08052836`.
const ZOID_ANCHOR: (i32, i32) = (40, 88);
const LEAVE_SOUND: u8 = 0x3F;
const HELP_WINDOW: u8 = 0;
const MENU_WINDOW: u8 = 3;
const MONEY_WINDOW: u8 = 1;
const PANEL_WINDOW: u8 = 2;
const STATUS_WINDOW: u8 = 4;
const BOOK_WINDOW: u8 = 5;
const SPEED_WINDOW: u8 = 5;
const WEAPONS_ZOID_WINDOW: u8 = 1;
const WEAPONS_LIST_WINDOW: u8 = 3;
const PANEL_LABEL_CELLS: usize = 10;
const PANEL_CELLS: usize = 17;
const MONEY_CELLS: usize = 9;
const WALLPAPER_PALETTE_START: usize = 64;
const WALLPAPER_BACKDROP: u16 = 0x7240;
const MAP_PIXELS: i32 = 256;
const ITEM_STATUS: u16 = 0;
const ITEM_ITEMS: u16 = 1;
const ITEM_WEAPONS: u16 = 2;
const ITEM_CONFIG: u16 = 4;
const ITEM_SAVE: u16 = 5;
const STATUS_UNIT: u16 = 0;
const STATUS_CHARACTER: u16 = 1;
const STATUS_WEAPONS: u16 = 2;
const CHARACTER_WINDOW: u8 = 1;
const MEMBER_WINDOW: u8 = 3;
const UNIT_WINDOW: u8 = 1;
const STAT_VALUE_CELLS: usize = 3;
const STAT_SIGN_COLUMN: usize = 11;
const PLUS: char = '＋';
const MINUS: char = '－';
const PERCENT: char = '％';
const HP_COLUMN: usize = 11;
const EP_COLUMN: usize = 21;
const SLASH: char = '／';
const STATUS_ZI_DATA: u16 = 3;
const STATUS_ZI_ITEMS: u16 = 4;
const STATUS_BOOK: u16 = 5;
const SPEED_CHOICES: u16 = 5;
const MENU_CANCELABLE: bool = true;

/// Stat bonuses a character shows, in the screen's order: 耐久, 攻撃,
/// 防御, 反応, 命中.
pub const CHARACTER_STATS: usize = 5;
/// Slots of the unit list: the formation's.
pub const UNIT_SLOTS: usize = saga_party::FORMATION_SLOTS;

/// What the party has; the values a new game starts with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Party {
    /// The party's level, which the panel shows.
    pub level: u32,
    /// Experience points.
    pub experience: u32,
    /// Money in G.
    pub money: u32,
    /// Battle message speed, 1 (fast) to 5 (slow).
    pub message_speed: u16,
}

impl Default for Party {
    fn default() -> Self {
        Self {
            level: 1,
            experience: 0,
            money: 0,
            message_speed: 3,
        }
    }
}

/// A member the character screen lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Member {
    /// The character: 0 is the player, the others name `name` table entry
    /// 154 + character and show portrait `character`.
    pub character: u8,
    /// Bonuses in percent, in the screen's order.
    pub bonuses: [i32; CHARACTER_STATS],
    /// The unit the character pilots.
    pub unit: Option<UnitStatus>,
}

/// Who is in the party and where they stand, as the game state holds it
/// when the menu opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roster {
    /// The members in the order the character screen lists them.
    pub members: Vec<Member>,
    /// The formation slots: the pilot and the unit.
    pub formation: [Option<(u8, UnitStatus)>; UNIT_SLOTS],
}

impl Default for Roster {
    fn default() -> Self {
        Self {
            members: vec![Member {
                character: 0,
                bonuses: [0; CHARACTER_STATS],
                unit: None,
            }],
            formation: [None; UNIT_SLOTS],
        }
    }
}

/// Where a notice returns to when dismissed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Return {
    Main,
    Status,
    Weapons,
    Character,
    Zoid,
}

/// What the menu needs after a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuStep {
    /// It is still open.
    Open,
    /// The player confirmed saving; the caller writes the save and reports
    /// with [`PauseMenu::finish_save`].
    Save,
    /// It closed.
    Closed,
}

/// Which list or notice the runner is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuState {
    Main,
    Status,
    Book,
    Weapons,
    Speed,
    Save,
    Saving,
    Unit,
    Character,
    Zoid,
    Notice(Return),
    Closed,
}

/// The pause menu with its wallpaper.
pub struct PauseMenu {
    wallpaper: PauseWallpaper,
    palette: FullPalette,
    runner: ScriptRunner,
    names: ScriptRunner,
    state: MenuState,
    scroll: i32,
    party: Party,
    roster: Roster,
    member: usize,
    zoid_sprites: Vec<(u16, SpriteSheet)>,
    shown_zoid: Option<u16>,
    held: Input,
    main_line: usize,
    status_line: usize,
    to_next: u32,
}

impl PauseMenu {
    /// Reads the wallpaper and scripts from `rom`.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when a block cannot be read.
    pub fn new(data: &GameData<'_>, party: Party, roster: Roster) -> Result<Self, BootError> {
        let wallpaper = data.pause_wallpaper()?;
        let mut palette = FullPalette::from_bgr555(&[WALLPAPER_BACKDROP]);
        palette.write(WALLPAPER_PALETTE_START, &wallpaper.palette);
        let scripts = data
            .script_offsets(PAUSE_MENU_TABLE)
            .ok()
            .flatten()
            .unwrap_or_default();
        let names = data
            .script_offsets(NAME_TABLE)
            .ok()
            .flatten()
            .unwrap_or_default();
        let mut zoid_sprites: Vec<(u16, SpriteSheet)> = Vec::new();
        for zoid in roster
            .members
            .iter()
            .filter_map(|member| member.unit.map(|unit| unit.zoid))
        {
            if zoid_sprites.iter().all(|(known, _)| *known != zoid) {
                if let Ok(sheet) = data.zoid_status_sprite(usize::from(zoid)) {
                    zoid_sprites.push((zoid, sheet));
                }
            }
        }
        let to_next = data
            .experience_to_next(usize::try_from(party.level).unwrap_or(0))
            .map_or(0, |needed| needed.saturating_sub(party.experience));
        Ok(Self {
            wallpaper,
            palette,
            runner: ScriptRunner::named(PAUSE_MENU_TABLE, scripts),
            names: ScriptRunner::named(NAME_TABLE, names),
            state: MenuState::Closed,
            scroll: 0,
            party,
            roster,
            member: 0,
            zoid_sprites,
            shown_zoid: None,
            held: Input::default(),
            main_line: 0,
            status_line: 0,
            to_next,
        })
    }

    /// Opens the menu on `windows`: the windows, the party panel and the
    /// money box, then the help line and the choice.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a script cannot run.
    pub fn open(&mut self, rom: &[u8], windows: &mut ScriptWindows<'_>) -> Result<(), ScriptError> {
        self.held = Input::default();
        self.main_line = 0;
        self.build(rom, windows)?;
        self.runner.start(SCRIPT_MENU)?;
        self.state = MenuState::Main;
        self.scroll = 0;
        Ok(())
    }

    fn build(&mut self, rom: &[u8], windows: &mut ScriptWindows<'_>) -> Result<(), ScriptError> {
        windows.close_window(None);
        self.run_now(rom, SCRIPT_OPEN_MENU, windows)?;
        self.run_now(rom, SCRIPT_PANEL_WINDOW, windows)?;
        self.print_panel(rom, windows)?;
        self.run_now(rom, SCRIPT_MONEY_WINDOW, windows)?;
        self.print_money(rom, windows)?;
        windows.set_cursor(MENU_WINDOW, Some(self.main_line));
        windows.set_cursor(MENU_WINDOW, None);
        Ok(())
    }

    /// The party as the menu leaves it.
    #[must_use]
    pub fn party(&self) -> Party {
        self.party.clone()
    }

    /// Whether the menu has closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state == MenuState::Closed
    }

    fn run_now(
        &mut self,
        rom: &[u8],
        script: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.runner.start(script)?;
        while !self.runner.update(rom, self.held, windows)? {}
        Ok(())
    }

    fn print_panel(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let name = windows.player_name();
        let (level, experience) = (self.party.level, self.party.experience);
        let to_next = self.to_next;
        for ch in name.chars() {
            windows.put_char(PANEL_WINDOW, ch);
        }
        self.run_now(rom, SCRIPT_LEVEL_LABEL, windows)?;
        let used = name.chars().count() + label_len(rom, self.script_offset(SCRIPT_LEVEL_LABEL));
        print_number(
            windows,
            PANEL_WINDOW,
            level,
            PANEL_CELLS.saturating_sub(used),
        );
        windows.line_break(PANEL_WINDOW);
        self.run_now(rom, SCRIPT_EXP_LABEL, windows)?;
        print_number(
            windows,
            PANEL_WINDOW,
            experience,
            PANEL_CELLS - PANEL_LABEL_CELLS,
        );
        windows.line_break(PANEL_WINDOW);
        self.run_now(rom, SCRIPT_NEXT_LABEL, windows)?;
        print_number(
            windows,
            PANEL_WINDOW,
            to_next,
            PANEL_CELLS - PANEL_LABEL_CELLS,
        );
        Ok(())
    }

    fn print_money(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        print_number(windows, MONEY_WINDOW, self.party.money, MONEY_CELLS - 1);
        self.run_now(rom, SCRIPT_MONEY_UNIT, windows)
    }

    fn script_offset(&self, script: usize) -> usize {
        self.runner.string_offset(script).unwrap_or(0)
    }

    /// Advances one frame.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a script cannot run.
    pub fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<MenuStep, ScriptError> {
        self.scroll += 1;
        self.held = input;
        match self.state {
            MenuState::Closed => return Ok(MenuStep::Closed),
            MenuState::Saving => return Ok(MenuStep::Save),
            _ => {}
        }
        if !self.runner.update(rom, input, windows)? {
            return Ok(MenuStep::Open);
        }
        let [code, choice, ..] = *self.runner.vars();
        let confirmed = code != 0;
        match self.state {
            MenuState::Main => self.main_choice(rom, confirmed, choice, windows)?,
            MenuState::Status => self.status_choice(rom, confirmed, choice, windows)?,
            MenuState::Book => {
                windows.close_window(Some(BOOK_WINDOW));
                if confirmed {
                    self.placeholder(rom, Return::Status, windows)?;
                } else {
                    self.return_to(rom, Return::Status, windows)?;
                }
            }
            MenuState::Weapons => {
                if confirmed {
                    self.run_now(rom, SCRIPT_NOT_BOARDED, windows)?;
                    self.notice(SCRIPT_WAIT_KEY, Return::Weapons)?;
                } else {
                    self.build(rom, windows)?;
                    self.return_to(rom, Return::Main, windows)?;
                }
            }
            MenuState::Speed => {
                if confirmed && choice < SPEED_CHOICES {
                    self.party.message_speed = choice + 1;
                }
                if confirmed && choice >= SPEED_CHOICES {
                    self.placeholder(rom, Return::Main, windows)?;
                } else {
                    self.return_to(rom, Return::Main, windows)?;
                }
            }
            MenuState::Save if confirmed && choice == 0 => {
                self.state = MenuState::Saving;
                return Ok(MenuStep::Save);
            }
            MenuState::Save => self.notice(SCRIPT_SAVE_CANCELED, Return::Main)?,
            MenuState::Unit => self.rebuild_status(rom, windows)?,
            MenuState::Character => self.member_choice(rom, code, choice, windows)?,
            MenuState::Zoid if confirmed => self.placeholder(rom, Return::Zoid, windows)?,
            MenuState::Zoid => self.return_to(rom, Return::Character, windows)?,
            MenuState::Notice(back) => self.return_to(rom, back, windows)?,
            MenuState::Saving | MenuState::Closed => {}
        }
        Ok(if self.state == MenuState::Closed {
            MenuStep::Closed
        } else {
            MenuStep::Open
        })
    }

    /// Tells the player whether the save was written: セーブしました, or
    /// セーブを中止しました when it could not be.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when the notice cannot run.
    pub fn finish_save(&mut self, written: bool) -> Result<(), ScriptError> {
        let script = if written {
            SCRIPT_SAVED
        } else {
            SCRIPT_SAVE_CANCELED
        };
        self.notice(script, Return::Main)
    }

    /// Rebuilds the main menu and the status list under a screen that
    /// replaced their windows, keeping the status cursor where it was.
    fn rebuild_status(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.build(rom, windows)?;
        self.run_now(rom, SCRIPT_STATUS_WINDOW, windows)?;
        windows.set_cursor(STATUS_WINDOW, Some(self.status_line));
        windows.set_cursor(STATUS_WINDOW, None);
        self.return_to(rom, Return::Status, windows)
    }

    /// The unit list (`0x0804EC2C`): a header, then per formation slot the
    /// pilot's name and the unit's hit and energy points, or 配置なし;
    /// then a key wait.
    fn open_units(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        close_status_windows(windows);
        self.run_now(rom, SCRIPT_UNIT_LIST, windows)?;
        for slot in self.roster.formation {
            let Some((character, unit)) = slot else {
                self.run_now(rom, SCRIPT_UNIT_EMPTY, windows)?;
                continue;
            };
            self.print_character_name(rom, character, UNIT_WINDOW, windows)?;
            if unit.flags & UNIT_DISABLED == 0 {
                put_number_at(windows, UNIT_WINDOW, HP_COLUMN, HP_CELLS, unit.hp.0);
            } else {
                windows.pad_to(UNIT_WINDOW, HP_COLUMN);
                self.runner.select_window(UNIT_WINDOW);
                self.run_now(rom, SCRIPT_DISABLED, windows)?;
            }
            windows.put_at(UNIT_WINDOW, HP_COLUMN + HP_CELLS, SLASH);
            put_number_at(
                windows,
                UNIT_WINDOW,
                HP_COLUMN + HP_CELLS + 1,
                HP_CELLS,
                unit.hp.1,
            );
            put_number_at(windows, UNIT_WINDOW, EP_COLUMN, EP_CELLS, unit.ep.0);
            windows.put_at(UNIT_WINDOW, EP_COLUMN + EP_CELLS, SLASH);
            put_number_at(
                windows,
                UNIT_WINDOW,
                EP_COLUMN + EP_CELLS + 1,
                EP_CELLS,
                unit.ep.1,
            );
            windows.line_break(UNIT_WINDOW);
        }
        windows.present(None);
        self.runner.start(SCRIPT_WAIT_KEY)?;
        self.state = MenuState::Unit;
        Ok(())
    }

    /// The character screen (`0x0804ED5C`): the member under the cursor
    /// with its bonuses, portrait and boarded Zoid, and the member list,
    /// six to a page, which the move-reporting menu of script 35 drives.
    fn open_character(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        close_status_windows(windows);
        self.run_now(rom, SCRIPT_CHARACTER_WINDOWS, windows)?;
        self.member = 0;
        self.show_member(rom, true, windows)
    }

    /// Redraws the character screen for the member under the cursor, the
    /// page's names too when `page_changed`, and waits on the list again.
    fn show_member(
        &mut self,
        rom: &[u8],
        page_changed: bool,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(member) = self.roster.members.get(self.member).copied() else {
            return self.rebuild_status(rom, windows);
        };
        let page = self.member / MEMBERS_PER_PAGE;
        if page_changed {
            self.run_now(rom, SCRIPT_CLEAR_MEMBERS, windows)?;
            let shown = self.roster.members.iter().skip(page * MEMBERS_PER_PAGE);
            let characters: Vec<u8> = shown
                .take(MEMBERS_PER_PAGE)
                .map(|other| other.character)
                .collect();
            for (line, character) in characters.iter().enumerate() {
                if line > 0 {
                    windows.line_break(MEMBER_WINDOW);
                }
                self.print_character_name(rom, *character, MEMBER_WINDOW, windows)?;
            }
        }
        self.run_now(rom, SCRIPT_CLEAR_CHARACTER, windows)?;
        self.run_now(rom, SCRIPT_CLEAR_PORTRAIT, windows)?;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        self.runner.select_window(HELP_WINDOW);
        match member.unit {
            Some(unit) => {
                self.run_now(rom, SCRIPT_ZOID_HELP_BEFORE, windows)?;
                self.print_zoid_name(rom, unit.zoid, HELP_WINDOW, windows)?;
                self.run_now(rom, SCRIPT_ZOID_HELP_AFTER, windows)?;
            }
            None => self.run_now(rom, SCRIPT_LEAVE_HELP, windows)?,
        }
        self.run_now(rom, SCRIPT_DRAW_CHARACTER, windows)?;
        self.print_character_name(rom, member.character, CHARACTER_WINDOW, windows)?;
        for (label, bonus) in SCRIPT_STAT_LABELS.iter().zip(member.bonuses) {
            windows.line_break(CHARACTER_WINDOW);
            self.runner.select_window(CHARACTER_WINDOW);
            self.run_now(rom, *label, windows)?;
            windows.put_at(
                CHARACTER_WINDOW,
                STAT_SIGN_COLUMN,
                if bonus < 0 { MINUS } else { PLUS },
            );
            put_number_at(
                windows,
                CHARACTER_WINDOW,
                STAT_SIGN_COLUMN + 1,
                STAT_VALUE_CELLS,
                bonus.unsigned_abs(),
            );
            windows.put_at(
                CHARACTER_WINDOW,
                STAT_SIGN_COLUMN + 1 + STAT_VALUE_CELLS,
                PERCENT,
            );
        }
        self.run_now(
            rom,
            SCRIPT_PORTRAITS + usize::from(member.character),
            windows,
        )?;
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.run_now(rom, SCRIPT_DRAW_MEMBERS, windows)?;
        windows.set_cursor(MEMBER_WINDOW, Some(self.member % MEMBERS_PER_PAGE));
        windows.set_cursor(MEMBER_WINDOW, None);
        self.runner.start(SCRIPT_MEMBER_MENU)?;
        self.state = MenuState::Character;
        Ok(())
    }

    /// What the member list's menu ended with: a cursor move or a page
    /// turn redraws, A on a member with a Zoid shows the Zoid, B leaves.
    fn member_choice(
        &mut self,
        rom: &[u8],
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let count = self.roster.members.len();
        let page = self.member / MEMBERS_PER_PAGE;
        let pages = count.div_ceil(MEMBERS_PER_PAGE).max(1);
        let turned = match code {
            MOVED_UP | MOVED_DOWN => {
                self.member = page * MEMBERS_PER_PAGE + usize::from(line);
                return self.show_member(rom, false, windows);
            }
            PAGE_LEFT if page > 0 => page - 1,
            PAGE_RIGHT if page + 1 < pages => page + 1,
            PAGE_LEFT | PAGE_RIGHT => return self.show_member(rom, false, windows),
            CONFIRMED => {
                return match self.roster.members.get(self.member).and_then(|m| m.unit) {
                    Some(_) => self.show_zoid(rom, windows),
                    None => self.show_member(rom, false, windows),
                };
            }
            _ => {
                windows.play_sound(LEAVE_SOUND);
                return self.rebuild_status(rom, windows);
            }
        };
        windows.play_sound(MENU_MOVE_SOUND);
        let line = self.member % MEMBERS_PER_PAGE;
        let last = count
            .saturating_sub(turned * MEMBERS_PER_PAGE)
            .min(MEMBERS_PER_PAGE);
        self.member = turned * MEMBERS_PER_PAGE + line.min(last.saturating_sub(1));
        self.show_member(rom, true, windows)
    }

    /// The character screen again, after a screen that replaced its
    /// windows.
    fn reopen_character(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.shown_zoid = None;
        close_status_windows(windows);
        self.run_now(rom, SCRIPT_CHARACTER_WINDOWS, windows)?;
        self.show_member(rom, true, windows)
    }

    /// The status of the Zoid the member under the cursor pilots
    /// (`0x08052724`): name and size, hit, energy and SP points, DF,
    /// training and its picture; A would show the weapons, B goes back.
    fn show_zoid(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(unit) = self.roster.members.get(self.member).and_then(|m| m.unit) else {
            return self.reopen_character(rom, windows);
        };
        close_status_windows(windows);
        self.run_now(rom, SCRIPT_ZOID_WINDOWS, windows)?;
        self.run_now(rom, SCRIPT_DRAW_CHARACTER, windows)?;
        self.print_zoid_name(rom, unit.zoid, ZOID_WINDOW, windows)?;
        self.runner.select_window(ZOID_WINDOW);
        self.run_now(rom, SCRIPT_SIZES + usize::from(unit.size), windows)?;
        let rows: [(usize, Option<u32>, u32, usize); 5] = [
            (SCRIPT_HP_LABEL, Some(unit.hp.1), unit.hp.0, HP_CELLS),
            (SCRIPT_EP_LABEL, Some(unit.ep.1), unit.ep.0, HP_CELLS),
            (SCRIPT_SP_LABEL, None, positive(unit.sp), HP_CELLS),
            (SCRIPT_DF_LABEL, None, positive(unit.df), EP_CELLS),
            (
                SCRIPT_TRAINING_LABEL,
                None,
                u32::from(unit.training),
                EP_CELLS,
            ),
        ];
        for (label, full, value, cells) in rows {
            windows.line_break(ZOID_WINDOW);
            self.runner.select_window(ZOID_WINDOW);
            self.run_now(rom, label, windows)?;
            if label == SCRIPT_HP_LABEL && unit.flags & UNIT_DISABLED != 0 {
                self.run_now(rom, SCRIPT_DISABLED, windows)?;
            } else {
                print_number(windows, ZOID_WINDOW, value, cells);
            }
            if let Some(full) = full {
                self.run_now(rom, SCRIPT_SLASH, windows)?;
                print_number(windows, ZOID_WINDOW, full, cells);
            }
            if label == SCRIPT_DF_LABEL {
                self.run_now(rom, SCRIPT_PERCENT, windows)?;
            }
        }
        self.shown_zoid = Some(unit.zoid);
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.runner.start(SCRIPT_WAIT_KEY)?;
        self.state = MenuState::Zoid;
        Ok(())
    }

    /// Prints `character`'s name at the text position of `window`: the
    /// player's own, or the `name` table's (`0x08032818`).
    fn print_character_name(
        &mut self,
        rom: &[u8],
        character: u8,
        window: u8,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if character == 0 {
            for ch in windows.player_name().chars() {
                windows.put_char(window, ch);
            }
            return Ok(());
        }
        self.print_name(
            rom,
            CHARACTER_NAMES + usize::from(character),
            window,
            windows,
        )
    }

    /// Prints Zoid `zoid`'s name (`0x08032800`).
    fn print_zoid_name(
        &mut self,
        rom: &[u8],
        zoid: u16,
        window: u8,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.print_name(rom, ZOID_NAMES + usize::from(zoid), window, windows)
    }

    fn print_name(
        &mut self,
        rom: &[u8],
        index: usize,
        window: u8,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.names.select_window(window);
        self.names.start(index)?;
        while !self.names.update(rom, self.held, windows)? {}
        Ok(())
    }

    fn main_choice(
        &mut self,
        rom: &[u8],
        confirmed: bool,
        choice: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if !confirmed {
            windows.close_window(None);
            self.state = MenuState::Closed;
            return Ok(());
        }
        self.main_line = usize::from(choice);
        match choice {
            ITEM_STATUS => {
                self.run_now(rom, SCRIPT_STATUS_WINDOW, windows)?;
                self.return_to(rom, Return::Status, windows)
            }
            ITEM_ITEMS => self.notice(SCRIPT_NO_ITEMS, Return::Main),
            ITEM_WEAPONS => {
                self.run_now(rom, SCRIPT_WEAPONS_WINDOWS, windows)?;
                self.runner.select_window(WEAPONS_ZOID_WINDOW);
                self.run_now(rom, SCRIPT_NO_ZOID, windows)?;
                for ch in windows.player_name().chars() {
                    windows.put_char(WEAPONS_LIST_WINDOW, ch);
                }
                windows.present(None);
                self.return_to(rom, Return::Weapons, windows)
            }
            ITEM_CONFIG => {
                windows.clear_window(HELP_WINDOW);
                self.run_now(rom, SCRIPT_SPEED_WINDOW, windows)?;
                let value = SCRIPT_SPEED_VALUE + usize::from(self.party.message_speed);
                self.run_now(rom, value, windows)?;
                self.run_now(rom, SCRIPT_SPEED_LIST, windows)?;
                windows.present(None);
                let line = usize::from(self.party.message_speed.saturating_sub(1));
                windows.set_cursor(SPEED_WINDOW, Some(line));
                self.runner.start(SCRIPT_SPEED_MENU)?;
                self.state = MenuState::Speed;
                Ok(())
            }
            ITEM_SAVE => {
                windows.clear_window(HELP_WINDOW);
                self.run_now(rom, SCRIPT_SAVE_QUESTION, windows)?;
                self.runner.start(SCRIPT_YES_NO)?;
                self.state = MenuState::Save;
                Ok(())
            }
            _ => self.placeholder(rom, Return::Main, windows),
        }
    }

    fn status_choice(
        &mut self,
        rom: &[u8],
        confirmed: bool,
        choice: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if !confirmed {
            windows.close_window(Some(STATUS_WINDOW));
            return self.return_to(rom, Return::Main, windows);
        }
        self.status_line = usize::from(choice);
        match choice {
            STATUS_UNIT => self.open_units(rom, windows),
            STATUS_CHARACTER => self.open_character(rom, windows),
            STATUS_WEAPONS => self.notice(SCRIPT_NO_WEAPONS, Return::Status),
            STATUS_ZI_DATA => self.notice(SCRIPT_NO_ZI_DATA, Return::Status),
            STATUS_ZI_ITEMS => self.notice(SCRIPT_NO_ZI_ITEMS, Return::Status),
            STATUS_BOOK => {
                self.run_now(rom, SCRIPT_BOOK_WINDOW, windows)?;
                windows.clear_window(HELP_WINDOW);
                self.runner.start(SCRIPT_BOOK_MENU)?;
                self.state = MenuState::Book;
                Ok(())
            }
            _ => self.placeholder(rom, Return::Status, windows),
        }
    }

    fn notice(&mut self, script: usize, back: Return) -> Result<(), ScriptError> {
        self.runner.start(script)?;
        self.state = MenuState::Notice(back);
        Ok(())
    }

    fn placeholder(
        &mut self,
        rom: &[u8],
        back: Return,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.clear_window(HELP_WINDOW);
        self.run_now(rom, SCRIPT_NOT_DONE, windows)?;
        self.notice(SCRIPT_WAIT_KEY, back)
    }

    fn return_to(
        &mut self,
        rom: &[u8],
        back: Return,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        windows.clear_window(HELP_WINDOW);
        match back {
            Return::Main => {
                self.runner.start(SCRIPT_MENU)?;
                self.state = MenuState::Main;
            }
            Return::Status => {
                self.runner.start(SCRIPT_STATUS_MENU)?;
                self.state = MenuState::Status;
            }
            Return::Character => return self.reopen_character(rom, windows),
            Return::Zoid => return self.show_zoid(rom, windows),
            Return::Weapons => {
                self.run_now(rom, SCRIPT_WEAPONS_HELP, windows)?;
                self.runner
                    .run_menu(WEAPONS_LIST_WINDOW, MENU_CANCELABLE, windows);
                self.state = MenuState::Weapons;
            }
        }
        Ok(())
    }

    /// Draws the wallpaper and the windows.
    pub fn draw(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
    ) {
        frame.fill(Rgb::default());
        draw_background_256(
            frame,
            |x, y| self.wallpaper.texture.wrapping(x, y),
            |index| self.wallpaper.tiles.tile(index),
            &self.palette,
            (0, 0),
            false,
        );
        let scroll_x =
            usize::try_from((MAP_PIXELS - self.scroll % MAP_PIXELS) % MAP_PIXELS).unwrap_or(0);
        let scroll_y = usize::try_from(self.scroll % MAP_PIXELS).unwrap_or(0);
        draw_background_256(
            frame,
            |x, y| self.wallpaper.logo.wrapping(x, y),
            |index| self.wallpaper.tiles.tile(index),
            &self.palette,
            (scroll_x, scroll_y),
            true,
        );
        windows.draw(frame, skin, painter);
        let sheet = self
            .shown_zoid
            .and_then(|zoid| self.zoid_sprites.iter().find(|(known, _)| *known == zoid));
        if let Some((_, sheet)) = sheet
            && let (Some(sprite), Some(image)) = (sheet.frames.first(), sheet.frame_image(0))
        {
            draw_sprite(
                frame,
                ZOID_ANCHOR.0 + i32::from(sprite.x),
                ZOID_ANCHOR.1 + i32::from(sprite.y),
                &image,
                &sheet.palette,
                sprite.mirrored,
            );
        }
    }
}

/// A half-word statistic as the screen prints it, never below zero.
fn positive(value: i16) -> u32 {
    u32::try_from(value).unwrap_or(0)
}

/// Closes the menu's windows but the help line, which the status screens
/// replace (`0x0804EBDC`), and clears the help line.
fn close_status_windows(windows: &mut ScriptWindows<'_>) {
    for id in (1..=STATUS_WINDOW).rev() {
        windows.close_window(Some(id));
    }
    windows.clear_window(HELP_WINDOW);
}

/// Length in characters of the message a label script prints.
fn label_len(rom: &[u8], offset: usize) -> usize {
    use formats::script_ops::{Instruction, MessageStep, decode_instruction, decode_message_step};
    let Ok((Instruction::Message, mut at)) = decode_instruction(rom, offset) else {
        return 0;
    };
    let mut count = 0;
    while let Ok((step, next)) = decode_message_step(rom, at) {
        at = next;
        match step {
            MessageStep::Character(_) => count += 1,
            MessageStep::End => break,
            _ => {}
        }
    }
    count
}

/// Puts `value` right-aligned in `cells` cells from `column`, with
/// full-width digits, the way the game's code places numbers.
fn put_number_at(
    windows: &mut ScriptWindows<'_>,
    window: u8,
    column: usize,
    cells: usize,
    value: u32,
) {
    let digits: Vec<char> = value
        .to_string()
        .chars()
        .map(|digit| char::from_u32(0xFF10 + u32::from(digit as u8 - b'0')).unwrap_or(digit))
        .collect();
    let start = column + cells.saturating_sub(digits.len());
    for (index, digit) in digits.into_iter().enumerate() {
        windows.put_at(window, start + index, digit);
    }
}

/// Prints `value` right-aligned in `cells` cells with full-width digits.
fn print_number(windows: &mut ScriptWindows<'_>, window: u8, value: u32, cells: usize) {
    let digits: String = value
        .to_string()
        .chars()
        .map(|digit| char::from_u32(0xFF10 + u32::from(digit as u8 - b'0')).unwrap_or(digit))
        .collect();
    let padding = cells.saturating_sub(digits.chars().count());
    for _ in 0..padding {
        windows.put_char(window, '\u{3000}');
    }
    for ch in digits.chars() {
        windows.put_char(window, ch);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn numbers_are_right_aligned_with_full_width_digits() {
        let mut windows = ScriptWindows::new(&[], "X");
        windows.open_window(2, 0x20, (11, 0, 19, 8), 4);
        print_number(&mut windows, 2, 14, 4);
        assert_eq!(
            windows.windows()[2].as_ref().map(|w| w.lines.clone()),
            Some(vec!["\u{3000}\u{3000}１４".to_owned()])
        );
        let (bytes, _) = ([0x20u8, 0x41, 0x83, 0x0D, 0x1D, 0x22], 0);
        assert_eq!(label_len(&bytes, 0), 1);
        assert_eq!(label_len(&[0x22], 0), 0);
    }

    #[test]
    fn a_new_party_starts_with_the_player_alone_at_level_one() {
        let party = Party::default();
        assert_eq!((party.level, party.experience, party.money), (1, 0, 0));
        let roster = Roster::default();
        assert_eq!(roster.members.len(), 1);
        assert_eq!(roster.members[0].character, 0);
        assert_eq!(roster.members[0].bonuses, [0; CHARACTER_STATS]);
        assert!(roster.members[0].unit.is_none());
        assert!(roster.formation.iter().all(Option::is_none));
    }

    #[test]
    fn values_are_placed_by_cell_after_padding() {
        let mut windows = ScriptWindows::new(&[], "X");
        windows.open_window(1, 0x20, (0, 0, 30, 16), 4);
        put_number_at(&mut windows, 1, 2, HP_CELLS, 85);
        windows.put_at(1, 2 + HP_CELLS, SLASH);
        windows.pad_to(1, 9);
        windows.put_char(1, 'a');
        assert_eq!(
            windows.windows()[1].as_ref().map(|w| w.lines.clone()),
            Some(vec![
                "\u{3000}\u{3000}\u{3000}\u{3000}８５／\u{3000}\u{3000}a".to_owned()
            ])
        );
    }
}
