//! The pause menu START opens on the field.
//!
//! The original assembles it from small scripts of one table (see
//! `extraction::saga::PAUSE_MENU_SCRIPTS`): script 46 opens the help line
//! and the six-item list, 64 and 44 the party panel and the money box,
//! whose values the game's code prints right-aligned, and 47 prints the
//! help text and runs the menu. Choices open the status submenu (48, 49)
//! with the unit list, the character screen, each Zoid's status and parts
//! pages (68–102, 164–218) and the stocked weapons (103–107), the
//! equipment screen (128–150, in `equipment`), the formation screen (a
//! task of its own over the battle field, in `formation`), the
//! message-speed setting (151–159) or the save question (160, 61, 161, 162), or print a notice. The
//! screens this port does not have end in the table's "not done yet"
//! notice (63). Behind the windows a logo map drifts one pixel per frame
//! diagonally over a static texture.

use extraction::saga::{BootError, PauseWallpaper, SpriteSheet};
use extraction::saga_battle::{self, BattleImage, EffectSprite};
use extraction::saga_party::{self, PART_SLOTS, PartSlot, UnitStatus};

use crate::data::GameData;
use gba_runtime::ppu::{FADE_STEPS, FullPalette, Palette, darken, draw_background_256};
use platform::{Frame, Input, Rgb};

use crate::battle::draw_piece;
use crate::guide::{Cover, Guide, GuideError, GuideKind};
use crate::script::{ScriptError, ScriptRunner};
use crate::translation::{ITEM_TABLE, NAME_TABLE, PART_TABLE, PAUSE_MENU_TABLE};
use crate::windows::ScriptWindows;
use crate::{ScriptHost, TextPainter, WindowPainter, draw_sprite};

mod equipment;
pub(crate) mod formation;
mod items;
mod parts;
mod shop;

pub use shop::Shop;

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
/// The help line alone, window 0 at (0, 14) 30×6, for the battle's
/// character screen.
const SCRIPT_BATTLE_HELP_WINDOW: usize = 79;
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
const PANEL_LABEL_CELLS: usize = 10;
const PANEL_CELLS: usize = 17;
const MONEY_CELLS: usize = 9;
const WALLPAPER_PALETTE_START: usize = 64;
const WALLPAPER_BACKDROP: u16 = 0x7240;
const MAP_PIXELS: i32 = 256;
const ITEM_STATUS: u16 = 0;
const ITEM_ITEMS: u16 = 1;
const ITEM_WEAPONS: u16 = 2;
const ITEM_FORMATION: u16 = 3;
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
const EQUIP_IMAGE_PIXELS: usize = 128;
const GUIDE_RETURN_FRAMES: u32 = 45;
/// Frames from the menu's building until its first choice runs.
const INTRO_FRAMES: u32 = 43;
const INTRO_STILL: u32 = 5;
const INTRO_HOLD: u32 = 8;
const INTRO_STALL: u32 = 10;
const INTRO_STALL_FRAMES: u32 = 2;
/// Frames from B on the main list until the field returns, the screen
/// darkening a level a frame from the fourth.
const CLOSE_FRAMES: u32 = 34;
/// Frames from the building of a shop until its welcome shows: the fade
/// task brightens it after eight frames and the task waits for it to end;
/// a welcome that overruns its frame shows later (see
/// `ShopSession::welcome_lag`).
const SHOP_WELCOME_FRAME: u32 = 42;
const SHOP_INTRO_STILL: u32 = 6;
const SHOP_INTRO_HOLD: u32 = 9;
/// Frames from B on a shop's choice until the field returns: the shop
/// darkens as the menu does, then the map is loaded again in the dark.
const SHOP_CLOSE_FRAMES: u32 = 50;
/// The same for the lab, whose way out takes three frames more in the
/// dark (measured in Arcana, with and without the keeper's word).
const LAB_CLOSE_FRAMES: u32 = 53;
const SHOP_CLOSE_DELAY: u32 = 3;
const CLOSE_DELAY: u32 = 4;
const GUIDE_FADE_TOP: u32 = 31;
const GUIDE_FADE_HOLD: u32 = 12;
/// Frames from the formation screen's end until the main list's menu
/// runs again (`0x0805203C`, then the menu's state 1 at `0x0804E938`).
const FORMATION_RETURN_FRAMES: u32 = 38;
/// The frames after the rebuild at which the wallpaper stands still while
/// the menu's scripts draw, and the fade level with it.
const FORMATION_RETURN_STALL: (u32, u32) = (6, 7);
const FORMATION_RETURN_HOLD: u32 = 3;
const SCREEN_WIDTH: usize = 240;
const SCREEN_HEIGHT: usize = 160;
const EQUIP_IMAGE_TILES: usize = 16;
const TILE_PIXELS: usize = 8;
/// Window 4's pixels, where the original's window 0 hides sprites while
/// the rack list is open (`WIN0H` `0x0897`, `WIN0V` `0x205F`).
const RACK_SPRITE_MASK: (usize, usize, usize, usize) = (8, 32, 151, 95);
const SCRIPT_CLOSE: usize = 8;
const SCRIPT_PRESENT: usize = 16;
const SCRIPT_RACK_PAGES: usize = 91;
const SCRIPT_FIXED_PAGES: usize = 98;
const SCRIPT_ARMS_HELP: usize = 101;
const SCRIPT_ARMS_LAST_HELP: usize = 102;
const SCRIPT_ATTACK: usize = 94;
const SCRIPT_ACCURACY: usize = 95;
const SCRIPT_COST: usize = 96;
const SCRIPT_RANGE: usize = 97;
const SCRIPT_RANGES: usize = 164;
const SCRIPT_REACHES: usize = 170;
const SCRIPT_SUPPORT: usize = 175;
const SCRIPT_NO_EFFECT: usize = 207;
const SCRIPT_EFFECTS: usize = 208;
const SCRIPT_NO_RACK: usize = 212;
const SCRIPT_NO_PART: usize = 213;
const SCRIPT_RACK_KINDS: usize = 213;
const SCRIPT_FIXED_KIND: usize = 218;
const SCRIPT_COLON: usize = 41;
const SCRIPT_SPACE: usize = 42;
const SCRIPT_DOT: usize = 43;
const RACKS: usize = 3;
const LAST_ARMS_PAGE: usize = PART_SLOTS - 1;
const EFFECT_BITS: [u32; 4] = [0x400, 0x800, 0x1000, 0x2000];
const MOST_EFFECTS_FIRST_LINE: usize = 2;
const WEAPON: u32 = 1;
const MAX_SHOWN: i32 = 999;
const COST_COLUMN: usize = 22;
const NUMBER_DIGITS: usize = 7;
const LEFT_ALIGNED: u8 = 1;
const ZERO_PADDED: u8 = 2;
const SIGNED: u8 = 4;
const SCRIPT_STOCK_WINDOWS: usize = 103;
const SCRIPT_STOCK_LABELS: usize = 104;
const SCRIPT_TIMES: usize = 40;
const SCRIPT_EFFECTS_LABEL: usize = 206;
const SCRIPT_PART_TEXTS: usize = 422;
const SCRIPT_CLEAR_STOCK: usize = 2;
const SCRIPT_DRAW_STOCK: usize = 27;
const STOCK_WINDOW: u8 = 1;
const STOCK_LIST_WINDOW: u8 = 2;
const STOCK_MASK: u32 = 0xF;
const STOCK_PAGE: usize = 6;
const STOCK_NAME_CELLS: usize = 8;
const EMPTY_SOUND: u8 = 0x4F;
const EMPTY_BACK_SOUND: u8 = 0x41;
const SUPPORT_KINDS: u32 = 0xE;

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
    /// That unit's part slots.
    pub parts: Option<[PartSlot; PART_SLOTS]>,
    /// Whether the equipment screen refuses to change them.
    pub keeps_equipment: bool,
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
                parts: None,
                keeps_equipment: false,
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
    /// The status list after a notice that its list is empty.
    EmptyList,
    /// The equipment screen's member list.
    Weapons,
    /// The equipment screen's rack list.
    Racks,
    Character,
    /// The main list after notice 56, with sound `0x41`.
    EmptyMain,
    /// The main menu built again.
    MainRebuilt,
    /// The item list after a refusal.
    ItemList,
    /// The members after a refusal.
    ItemTarget,
    /// Out of the members after an item's message.
    ItemUsed,
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
    /// A page of the Zoid's parts: the racks, then the fixed weapons.
    Arms(usize),
    /// The stocked weapons and support parts.
    Stock,
    /// The party's items.
    Items,
    /// The member whose Zoid gets the item.
    ItemTarget,
    /// The equipment screen's racks.
    Racks,
    /// The parts a rack can take.
    Equip,
    /// Whether to throw away a part the stock has no room for.
    Discard,
    Notice(Return),
    /// A shop's list, question or notice.
    Shop(shop::ShopStep),
    /// Darkening after B on the main list, frames since.
    Closing(u32),
    Closed,
}

/// The pause menu with its wallpaper.
pub struct PauseMenu {
    wallpaper: PauseWallpaper,
    palette: FullPalette,
    runner: ScriptRunner,
    names: ScriptRunner,
    parts: ScriptRunner,
    items: ScriptRunner,
    state: MenuState,
    scroll: i32,
    guide: Option<Box<Guide>>,
    formation: Option<Box<formation::Formation>>,
    /// Frames since the formation screen handed back, while the main menu
    /// brightens.
    formation_return: Option<u32>,
    book_line: usize,
    returning: Option<u32>,
    /// Frames since the menu was built, while it brightens.
    intro: Option<u32>,
    /// The shop the menu is, when a keeper opened it.
    shop: Option<shop::ShopSession>,
    /// Frames since the shop was built, until its welcome.
    shop_intro: Option<u32>,
    /// Frames the original would still spend drawing what the port drew
    /// at once: its interpreter blocks the game on each window it opens,
    /// clears or presents, so the wallpaper and the blinking stand still
    /// and keys go unread.
    busy: u32,
    /// A sound the original plays once the scripts before it are done:
    /// the frames left, and the sound.
    delayed_sound: Option<(u32, u8)>,
    party: Party,
    roster: Roster,
    game_state: Vec<u8>,
    stock: Vec<u16>,
    stock_page: usize,
    stock_shown: Option<usize>,
    item_menu: items::ItemMenu,
    equipment: equipment::Equipment,
    equip_image: Option<(u16, BattleImage)>,
    weapon_sprites: Vec<((u16, usize), Option<EffectSprite>)>,
    weapon_mounts: [(i32, i32); RACKS],
    member: usize,
    zoid_sprites: Vec<(u16, SpriteSheet)>,
    shown_zoid: Option<u16>,
    held: Input,
    main_line: usize,
    status_line: usize,
    to_next: u32,
    /// Whether the battle's ステータス opened it: the character screen alone,
    /// over black, which B leaves.
    battle: bool,
}

impl PauseMenu {
    /// Reads the wallpaper and scripts from `rom`.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when a block cannot be read.
    pub fn new(data: &GameData<'_>, party: Party, state: Vec<u8>) -> Result<Self, BootError> {
        let roster = data.roster(&state);
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
        let part_names = data
            .script_offsets(PART_TABLE)
            .ok()
            .flatten()
            .unwrap_or_default();
        let items = data
            .script_offsets(ITEM_TABLE)
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
            parts: ScriptRunner::named(PART_TABLE, part_names),
            items: ScriptRunner::named(ITEM_TABLE, items),
            state: MenuState::Closed,
            scroll: 0,
            guide: None,
            formation: None,
            formation_return: None,
            book_line: 0,
            returning: None,
            intro: None,
            shop: None,
            shop_intro: None,
            busy: 0,
            delayed_sound: None,
            party,
            roster,
            game_state: state,
            stock: Vec::new(),
            stock_page: 0,
            stock_shown: None,
            item_menu: items::ItemMenu::default(),
            equipment: equipment::Equipment::default(),
            equip_image: None,
            weapon_sprites: Vec::new(),
            weapon_mounts: [(0, 0); RACKS],
            member: 0,
            zoid_sprites,
            shown_zoid: None,
            held: Input::default(),
            main_line: 0,
            status_line: 0,
            to_next,
            battle: false,
        })
    }

    /// The character screen the battle's ステータス shows (`0x0805224C`).
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when a block cannot be read.
    pub fn battle_status(data: &GameData<'_>, state: Vec<u8>) -> Result<Self, BootError> {
        let mut menu = Self::new(data, Party::default(), state)?;
        menu.battle = true;
        Ok(menu)
    }

    /// Builds the battle's character screen on `windows` (`0x0804E4A8`):
    /// the help line, the first member, and the list's menu.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a script cannot run.
    pub fn open_battle_status(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.held = Input::default();
        windows.close_window(None);
        self.run_now(rom, SCRIPT_BATTLE_HELP_WINDOW, windows)?;
        self.open_character(rom, windows)?;
        self.busy = 0;
        Ok(())
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
        windows.present(None);
        self.busy = 0;
        self.intro = Some(0);
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

    /// The game-state block as the menu leaves it: the equipment screen
    /// changes the units' parts and the stock.
    #[must_use]
    pub fn state(&self) -> &[u8] {
        &self.game_state
    }

    /// Whether the menu has closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state == MenuState::Closed
    }

    /// Runs `script` to its end at once, counting the frames the original
    /// would spend on it (see [`PauseMenu::busy`]). None of the scripts run
    /// this way waits for a key, but a translated label may not fit its
    /// window and wait for the page to turn; its text then stops there.
    fn run_now(
        &mut self,
        rom: &[u8],
        script: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.runner.start(script)?;
        while !self.runner.update(rom, self.held, windows)? {
            if self.runner.is_waiting_for_key() {
                break;
            }
            self.busy += 1;
        }
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
    /// Returns [`GuideError`] when a script, or the guide 図鑑 opens,
    /// cannot run.
    pub fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<MenuStep, GuideError> {
        if let Some(step) = self.update_phases(rom, input, windows)? {
            return Ok(step);
        }
        if let Some((frames, sound)) = self.delayed_sound {
            let left = frames.saturating_sub(1);
            if left == 0 {
                windows.play_sound(sound);
                self.delayed_sound = None;
            } else {
                self.delayed_sound = Some((left, sound));
            }
        }
        if self.busy > 0 {
            self.busy -= 1;
            if let Some(session) = self.shop.as_mut() {
                session.forget_keys(input);
                self.scroll += 1;
            }
            return Ok(MenuStep::Open);
        }
        self.scroll += 1;
        self.held = input;
        if self.state == MenuState::Shop(shop::ShopStep::Quantity) {
            self.shop_frame(rom, input, windows)?;
            return Ok(MenuStep::Open);
        }
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
            MenuState::Book if confirmed => {
                let kind = if choice == 0 {
                    GuideKind::Zoids
                } else {
                    GuideKind::Characters
                };
                self.book_line = usize::from(choice);
                let data = GameData::new(rom);
                let state = self.game_state.clone();
                self.guide = Some(Box::new(Guide::new(&data, kind, state, Cover::PauseMenu)?));
            }
            MenuState::Book => {
                windows.close_window(Some(BOOK_WINDOW));
                self.return_to(rom, Return::Status, windows)?;
            }
            MenuState::Weapons => {
                self.equipment_member_choice(rom, code, choice, windows)?;
            }
            MenuState::Racks => self.rack_choice(rom, code, choice, windows)?,
            MenuState::Equip => self.rack_part_choice(rom, code, choice, windows)?,
            MenuState::Discard => self.discard_choice(rom, code, choice, windows)?,
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
            MenuState::Zoid if confirmed => self.show_arms(rom, 0, windows)?,
            MenuState::Arms(page) if confirmed && page < LAST_ARMS_PAGE => {
                self.show_arms(rom, page + 1, windows)?;
            }
            MenuState::Arms(_) if confirmed => self.return_to(rom, Return::Character, windows)?,
            MenuState::Zoid | MenuState::Arms(_) => {
                windows.play_sound(LEAVE_SOUND);
                self.return_to(rom, Return::Character, windows)?;
            }
            MenuState::Stock => self.stock_choice(rom, code, choice, windows)?,
            MenuState::Items => self.item_choice(rom, code, choice, windows)?,
            MenuState::ItemTarget => self.target_choice(rom, code, choice, windows)?,
            MenuState::Notice(back) => self.return_to(rom, back, windows)?,
            MenuState::Shop(step) => self.shop_choice(rom, step, code, choice, windows)?,
            MenuState::Saving | MenuState::Closing(_) | MenuState::Closed => {}
        }
        self.load_equip_image(rom);
        Ok(if self.state == MenuState::Closed {
            MenuStep::Closed
        } else {
            MenuStep::Open
        })
    }

    /// The frames the menu spends outside its menus: darkening to close,
    /// brightening after it was built, the guide and the way back from it.
    fn update_phases(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<Option<MenuStep>, GuideError> {
        if let MenuState::Closing(frames) = self.state {
            self.scroll += 1;
            let close = match self.shop_kind() {
                Some(Shop::Lab(_)) => LAB_CLOSE_FRAMES,
                Some(_) => SHOP_CLOSE_FRAMES,
                None => CLOSE_FRAMES,
            };
            if frames + 1 < close {
                self.state = MenuState::Closing(frames + 1);
                return Ok(Some(MenuStep::Open));
            }
            windows.close_window(None);
            self.state = MenuState::Closed;
            return Ok(Some(MenuStep::Closed));
        }
        if let Some(frames) = self.shop_intro {
            let frames = frames + 1;
            let lag = self.shop.as_ref().map_or(0, shop::ShopSession::welcome_lag);
            self.scroll = shop_intro_scroll(frames, lag);
            if frames < SHOP_WELCOME_FRAME + lag {
                self.shop_intro = Some(frames);
            } else {
                self.shop_intro = None;
                self.welcome(rom, windows)?;
                self.busy = 0;
            }
            return Ok(Some(MenuStep::Open));
        }
        if let Some(frames) = self.intro {
            let frames = frames + 1;
            if frames < INTRO_FRAMES {
                self.scroll = intro_scroll(frames);
            }
            self.intro = (frames < INTRO_FRAMES).then_some(frames);
            return Ok(Some(MenuStep::Open));
        }
        if let Some(formation) = &mut self.formation {
            formation.update(rom, input, windows, &mut self.game_state)?;
            if formation.is_closed() {
                self.formation = None;
                self.return_from_formation(rom, windows)?;
            }
            return Ok(Some(MenuStep::Open));
        }
        if let Some(frames) = self.formation_return {
            let frames = frames + 1;
            if frames < FORMATION_RETURN_FRAMES {
                self.scroll = formation_return_scroll(frames);
                self.formation_return = Some(frames);
            } else {
                self.formation_return = None;
                self.return_to(rom, Return::Main, windows)?;
            }
            return Ok(Some(MenuStep::Open));
        }
        if let Some(guide) = &mut self.guide {
            guide.update(&GameData::new(rom), input, windows)?;
            if guide.is_closed() {
                self.guide = None;
                self.return_from_guide(rom, windows)?;
            }
            return Ok(Some(MenuStep::Open));
        }
        if let Some(frames) = self.returning {
            self.scroll += 1;
            if frames + 1 < GUIDE_RETURN_FRAMES {
                self.returning = Some(frames + 1);
                return Ok(Some(MenuStep::Open));
            }
            self.returning = None;
            windows.clear_window(HELP_WINDOW);
            self.runner.start(SCRIPT_BOOK_MENU)?;
            self.state = MenuState::Book;
            return Ok(Some(MenuStep::Open));
        }
        Ok(None)
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

    /// Back from the guide (`0x08050090`): the main menu, the status list
    /// and the guide choice rebuilt with the cursor on the guide left, in
    /// the dark; they brighten from the 25th frame over 16 and the choice
    /// runs again 45 frames after the guide closed.
    fn return_from_guide(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.build(rom, windows)?;
        self.run_now(rom, SCRIPT_STATUS_WINDOW, windows)?;
        windows.set_cursor(STATUS_WINDOW, Some(self.status_line));
        windows.set_cursor(STATUS_WINDOW, None);
        self.run_now(rom, SCRIPT_BOOK_WINDOW, windows)?;
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        windows.set_cursor(BOOK_WINDOW, Some(self.book_line));
        windows.set_cursor(BOOK_WINDOW, None);
        self.busy = 0;
        self.returning = Some(0);
        Ok(())
    }

    /// Back from the formation screen (`0x0805203C`): the main menu built
    /// again in the dark with the cursor on 部隊編成 and the wallpaper from
    /// its start, and the party as the screen left it. The menu brightens
    /// and its list runs again 38 frames later.
    fn return_from_formation(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.roster = GameData::new(rom).roster(&self.game_state);
        self.build(rom, windows)?;
        windows.present(None);
        self.scroll = 0;
        self.busy = 0;
        self.formation_return = Some(0);
        Ok(())
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
            _ if self.battle => {
                windows.play_sound(LEAVE_SOUND);
                self.state = MenuState::Closed;
                return Ok(());
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
    /// Loads the status sprite of Zoid `zoid` when the menu has not yet.
    fn load_zoid_sprite(&mut self, rom: &[u8], zoid: u16) {
        if self.zoid_sprites.iter().any(|(known, _)| *known == zoid) {
            return;
        }
        if let Ok(sheet) = GameData::new(rom).zoid_status_sprite(usize::from(zoid)) {
            self.zoid_sprites.push((zoid, sheet));
        }
    }

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
        while !self.names.update(rom, self.held, windows)? {
            if self.names.is_waiting_for_key() {
                break;
            }
        }
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
            windows.play_sound(LEAVE_SOUND);
            self.state = MenuState::Closing(0);
            return Ok(());
        }
        self.main_line = usize::from(choice);
        match choice {
            ITEM_STATUS => {
                self.run_now(rom, SCRIPT_STATUS_WINDOW, windows)?;
                self.return_to(rom, Return::Status, windows)
            }
            ITEM_ITEMS => self.open_items(rom, windows),
            ITEM_WEAPONS => self.open_equipment(rom, windows),
            ITEM_FORMATION => {
                let formation = formation::Formation::new(&GameData::new(rom), &self.game_state);
                self.formation = Some(Box::new(formation));
                self.scroll += 1;
                Ok(())
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
            STATUS_WEAPONS => self.open_stock(rom, windows),
            STATUS_ZI_DATA => self.empty_list(SCRIPT_NO_ZI_DATA, windows),
            STATUS_ZI_ITEMS => self.empty_list(SCRIPT_NO_ZI_ITEMS, windows),
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
            Return::EmptyList => {
                windows.play_sound(EMPTY_BACK_SOUND);
                return self.return_to(rom, Return::Status, windows);
            }
            Return::Character => return self.reopen_character(rom, windows),
            Return::Weapons => return self.equipment_members_again(rom, windows),
            Return::Racks => return self.racks_again(rom, windows),
            Return::EmptyMain => {
                windows.play_sound(EMPTY_BACK_SOUND);
                return self.return_to(rom, Return::Main, windows);
            }
            Return::MainRebuilt => {
                self.build(rom, windows)?;
                return self.return_to(rom, Return::Main, windows);
            }
            Return::ItemList => return self.items_again(rom, windows),
            Return::ItemTarget => return self.targets_again(rom, windows),
            Return::ItemUsed => return self.item_used(rom, windows),
        }
        Ok(())
    }

    /// Loads the picture the rack's list shows, when it changed, and the
    /// sprites of the weapons on it.
    fn load_equip_image(&mut self, rom: &[u8]) {
        for weapon in self.mounted_weapons() {
            let key = (weapon.part, weapon.rack);
            if self.weapon_sprites.iter().all(|(known, _)| *known != key) {
                let sprite = saga_battle::weapon_sprite(rom, weapon.part, weapon.rack);
                self.weapon_sprites.push((key, sprite));
            }
        }
        let zoid = self.equipment_picture();
        if zoid == self.equip_image.as_ref().map(|(zoid, _)| *zoid) {
            return;
        }
        self.equip_image = zoid.and_then(|zoid| {
            let image = GameData::new(rom).zoid_image(u8::try_from(zoid).ok()?)?;
            Some((zoid, image))
        });
        if let Some(zoid) = zoid {
            for (rack, mount) in self.weapon_mounts.iter_mut().enumerate() {
                let (x, y) = saga_battle::weapon_mount(rom, zoid, rack).unwrap_or_default();
                *mount = (i32::from(x), i32::from(y));
            }
        }
    }

    /// The rack list's picture and the weapons on it, as the layers the
    /// original composes: sprites behind the Zoid (OBJ priorities 2 and
    /// 3), BG1 with the Zoid and the weapons drawn into it, and sprites in
    /// front (priority 1). Each weapon sits at its rack's mount in its
    /// first frame.
    fn equip_layers(&self, image: &BattleImage) -> EquipLayers {
        let mut layers = EquipLayers {
            back: vec![None; SCREEN_WIDTH * SCREEN_HEIGHT],
            picture: picture_layer(image),
            front: vec![None; SCREEN_WIDTH * SCREEN_HEIGHT],
        };
        for weapon in self.mounted_weapons().into_iter().filter(|w| w.visible) {
            let Some((_, Some(sprite))) = self
                .weapon_sprites
                .iter()
                .find(|(key, _)| *key == (weapon.part, weapon.rack))
            else {
                continue;
            };
            let mount = self.weapon_mounts[weapon.rack.min(RACKS - 1)];
            let frame = sprite.animation.first().map_or(0, |step| step.frame);
            let palette = Palette::new(sprite.palette.map(Palette::from_bgr555));
            let mut drawn = vec![None; SCREEN_WIDTH * SCREEN_HEIGHT];
            for piece in sprite.frames.get(frame).into_iter().flatten() {
                draw_piece(&mut drawn, sprite, &palette, piece, mount, false);
            }
            if weapon.baked {
                bake(&mut layers.picture, &drawn, weapon.rack == 0);
            } else {
                let layer = if weapon.rack == 0 {
                    &mut layers.front
                } else {
                    &mut layers.back
                };
                for (target, color) in layer.iter_mut().zip(drawn) {
                    if target.is_none() {
                        *target = color;
                    }
                }
            }
        }
        layers
    }

    /// Draws the wallpaper and the windows.
    pub fn draw(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
    ) {
        if let Some(formation) = &self.formation {
            if formation.covers() {
                formation.draw(frame, windows, skin, painter, &self.game_state);
            } else {
                self.draw_menu(frame, windows, skin, painter);
                darken(frame, shown_level(formation.fade()));
            }
            return;
        }
        if let Some(frames) = self.formation_return {
            self.draw_menu(frame, windows, skin, painter);
            darken(frame, formation_return_darkness(frames));
            return;
        }
        if let Some(guide) = &self.guide {
            if guide.covered() {
                self.draw_menu(frame, windows, skin, painter);
                darken(frame, guide.darkness());
            } else {
                guide.draw(frame, windows, skin, painter);
            }
            return;
        }
        self.draw_menu(frame, windows, skin, painter);
        if let Some(frames) = self.returning {
            darken(frame, return_darkness(frames));
        }
        if let Some(frames) = self.intro {
            darken(frame, intro_darkness(frames));
        }
        if let Some(frames) = self.shop_intro {
            darken(frame, shop_intro_darkness(frames));
        }
        if let MenuState::Closing(frames) = self.state {
            let delay = if self.shop.is_some() {
                SHOP_CLOSE_DELAY
            } else {
                CLOSE_DELAY
            };
            let level = frames.saturating_sub(delay).min(u32::from(FADE_STEPS));
            darken(frame, u8::try_from(level).unwrap_or(FADE_STEPS));
        }
    }

    /// The wallpaper: its texture, and the logo drifting over it.
    fn draw_wallpaper(&self, frame: &mut Frame) {
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
    }

    fn draw_menu(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
    ) {
        frame.fill(Rgb::default());
        if !self.battle {
            self.draw_wallpaper(frame);
        }
        if let Some((_, image)) = &self.equip_image
            && self.equipment_picture().is_some()
        {
            let layers = self.equip_layers(image);
            composite(frame, &layers.back);
            let blends = self.equipment_blends();
            for (index, color) in layers.picture.iter().enumerate() {
                let (x, y) = (index % SCREEN_WIDTH, index / SCREEN_WIDTH);
                let Some(color) = *color else {
                    continue;
                };
                let color = match frame.pixel(x, y) {
                    Some(below) if blends => half_and_half(color, below),
                    _ => color,
                };
                frame.set_pixel(x, y, color);
            }
            composite(frame, &layers.front);
        }
        windows.draw(frame, skin, painter);
        let sheet = self
            .shown_zoid
            .and_then(|zoid| self.zoid_sprites.iter().find(|(known, _)| *known == zoid));
        if let Some((_, sheet)) = sheet
            && let (Some(sprite), Some(image)) = (sheet.frames.first(), sheet.frame_image(0))
        {
            let masked = self.racks_mask_sprites().then(|| {
                let (left, top, right, bottom) = RACK_SPRITE_MASK;
                (top..bottom)
                    .flat_map(|y| (left..right).map(move |x| (x, y)))
                    .filter_map(|(x, y)| Some((x, y, frame.pixel(x, y)?)))
                    .collect::<Vec<_>>()
            });
            draw_sprite(
                frame,
                ZOID_ANCHOR.0 + i32::from(sprite.x),
                ZOID_ANCHOR.1 + i32::from(sprite.y),
                &image,
                &sheet.palette,
                sprite.mirrored,
            );
            for (x, y, color) in masked.into_iter().flatten() {
                frame.set_pixel(x, y, color);
            }
        }
    }
}

/// Puts a sprite layer's pixels over `frame`.
fn composite(frame: &mut Frame, layer: &[Option<Rgb>]) {
    for (index, color) in layer.iter().enumerate() {
        if let Some(color) = color {
            frame.set_pixel(index % SCREEN_WIDTH, index / SCREEN_WIDTH, *color);
        }
    }
}

/// The layers of the rack list's picture.
struct EquipLayers {
    back: Vec<Option<Rgb>>,
    picture: Vec<Option<Rgb>>,
    front: Vec<Option<Rgb>>,
}

/// The rack list's Zoid picture as BG1 holds it: its 16×16 tiles in order
/// from the top-left corner, index 0 clear. The third rack's list draws
/// it half over what lies below (`BLDALPHA` `0x0808`).
fn picture_layer(image: &BattleImage) -> Vec<Option<Rgb>> {
    let palette = FullPalette::from_bgr555(&image.palette);
    let mut layer = vec![None; SCREEN_WIDTH * SCREEN_HEIGHT];
    for y in 0..EQUIP_IMAGE_PIXELS {
        for x in 0..EQUIP_IMAGE_PIXELS {
            let tile = (y / TILE_PIXELS) * EQUIP_IMAGE_TILES + x / TILE_PIXELS;
            let index = image.tiles.get(tile).map_or(0, |pixels| {
                pixels[(y % TILE_PIXELS) * TILE_PIXELS + x % TILE_PIXELS]
            });
            if index != 0 {
                layer[y * SCREEN_WIDTH + x] = Some(palette.color(index));
            }
        }
    }
    layer
}

/// Draws a weapon into the picture's tiles as `0x08053204` does, within
/// the picture's 128×128 pixels: over the Zoid for the first rack
/// (priority 1), only where the Zoid leaves the picture clear otherwise.
fn bake(picture: &mut [Option<Rgb>], weapon: &[Option<Rgb>], in_front: bool) {
    for y in 0..EQUIP_IMAGE_PIXELS {
        for x in 0..EQUIP_IMAGE_PIXELS {
            let index = y * SCREEN_WIDTH + x;
            if let Some(color) = weapon[index]
                && (in_front || picture[index].is_none())
            {
                picture[index] = Some(color);
            }
        }
    }
}

/// Half of each color, per 5-bit channel.
fn half_and_half(top: Rgb, below: Rgb) -> Rgb {
    let channel = |top: u8, bottom: u8| {
        let mixed = ((u16::from(top >> 3) * 8 + u16::from(bottom >> 3) * 8) >> 4).min(31);
        let five = u8::try_from(mixed).unwrap_or(31);
        five << 3 | five >> 2
    };
    Rgb::new(
        channel(top.r, below.r),
        channel(top.g, below.g),
        channel(top.b, below.b),
    )
}

/// The wallpaper's scroll `frames` after the menu was built: still for
/// five frames, then a pixel a frame but for two frames the menu's
/// scripts take.
fn intro_scroll(frames: u32) -> i32 {
    let scroll = match frames {
        0..=INTRO_STILL => 0,
        _ if frames <= INTRO_STALL => frames - INTRO_STILL,
        _ if frames <= INTRO_STALL + INTRO_STALL_FRAMES => INTRO_STALL - INTRO_STILL,
        _ => frames - INTRO_STILL - INTRO_STALL_FRAMES,
    };
    i32::try_from(scroll).unwrap_or(0)
}

/// How dark the menu is `frames` after it was built: the game's fade
/// level holds at 31, falls a level a frame but for the same two frames,
/// and shows from 16 down.
fn intro_darkness(frames: u32) -> u8 {
    let fallen = frames.saturating_sub(INTRO_HOLD);
    let fallen = if frames > INTRO_STALL + INTRO_STALL_FRAMES {
        fallen - INTRO_STALL_FRAMES
    } else {
        fallen.min(INTRO_STALL - INTRO_HOLD)
    };
    let level = GUIDE_FADE_TOP.saturating_sub(fallen);
    u8::try_from(level.min(u32::from(FADE_STEPS))).unwrap_or(FADE_STEPS)
}

/// The wallpaper's scroll `frames` after a shop was built: still for six
/// frames, then a pixel a frame but for the frames the welcome overran.
fn shop_intro_scroll(frames: u32, lag: u32) -> i32 {
    let scroll = frames.saturating_sub(SHOP_INTRO_STILL);
    let scroll = scroll - (frames + 1).saturating_sub(SHOP_WELCOME_FRAME).min(lag);
    i32::try_from(scroll).unwrap_or(0)
}

/// How dark a shop is `frames` after it was built: the game's fade level
/// holds at 31 for nine frames, then falls a level a frame.
fn shop_intro_darkness(frames: u32) -> u8 {
    shown_level(GUIDE_FADE_TOP.saturating_sub(frames.saturating_sub(SHOP_INTRO_HOLD)))
}

/// How dark the menu is `frames` after the guide closed: the game's fade
/// level stays at 31 for 10 frames, then falls one a frame, and the
/// screen shows it from 16 down.
fn return_darkness(frames: u32) -> u8 {
    let level = GUIDE_FADE_TOP.saturating_sub(frames.saturating_sub(GUIDE_FADE_HOLD));
    u8::try_from(level.min(u32::from(FADE_STEPS))).unwrap_or(FADE_STEPS)
}

/// The wallpaper's scroll `frames` after the menu was rebuilt behind the
/// formation screen: it moves a pixel a frame from its start but for the
/// two frames the menu's scripts take.
fn formation_return_scroll(frames: u32) -> i32 {
    let (first, last) = FORMATION_RETURN_STALL;
    let scroll = if frames < first {
        frames
    } else if frames <= last {
        first - 1
    } else {
        frames - (last - first + 1)
    };
    i32::try_from(scroll).unwrap_or(0)
}

/// How dark the rebuilt menu is `frames` after it was built: the game's
/// fade level holds at 31, falls a level a frame but for the same two
/// frames, and shows from 16 down.
fn formation_return_darkness(frames: u32) -> u8 {
    let (first, last) = FORMATION_RETURN_STALL;
    let fallen = frames.saturating_sub(FORMATION_RETURN_HOLD);
    let fallen = if frames > last {
        fallen - (last - first + 1)
    } else {
        fallen.min(first - 1 - FORMATION_RETURN_HOLD)
    };
    shown_level(GUIDE_FADE_TOP.saturating_sub(fallen))
}

/// The darkness a fade level of 0 to 31 shows: the screen is black from 16.
fn shown_level(level: u32) -> u8 {
    u8::try_from(level.min(u32::from(FADE_STEPS))).unwrap_or(FADE_STEPS)
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
    fn a_first_rack_weapon_covers_the_zoid_and_the_others_fill_its_gaps() {
        let zoid = Rgb::new(1, 1, 1);
        let gun = Rgb::new(9, 9, 9);
        let mut picture = vec![None; SCREEN_WIDTH * SCREEN_HEIGHT];
        picture[0] = Some(zoid);
        let mut weapon = vec![None; SCREEN_WIDTH * SCREEN_HEIGHT];
        weapon[0] = Some(gun);
        weapon[1] = Some(gun);
        weapon[EQUIP_IMAGE_PIXELS] = Some(gun);
        let mut behind = picture.clone();
        bake(&mut behind, &weapon, false);
        assert_eq!(behind[..2], [Some(zoid), Some(gun)]);
        bake(&mut picture, &weapon, true);
        assert_eq!(picture[..2], [Some(gun), Some(gun)]);
        assert_eq!(picture[EQUIP_IMAGE_PIXELS], None);
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
