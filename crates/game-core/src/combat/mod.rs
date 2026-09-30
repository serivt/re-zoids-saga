//! The real battles (`0x0800C3AC`): the screen the two sides face each
//! other on, the opening messages, the battle menu and the retreat.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! battle controller (task `0x0802AC0C` in slot 4), the opening and menu
//! task (`0x0802E9CC`, slot 5), the results task (`0x08035778`, slot 5),
//! the message wait (`0x08003F70`, slot 9, armed by `0x08004018`), the fade
//! task (`0x08004034`, slot 10), the screen's builder (`0x08031074`) and the
//! result routine (`0x08032684`); checked frame by frame against a battle
//! met on the world map in a reference emulator. See `docs/combat.md`.
//!
//! The original runs these as tasks, each one step a frame in slot order;
//! a script call blocks its task until it returns, and the task goes on in
//! the frame it does. The port keeps the same tasks and steps, so the
//! screen changes on the same frames.

pub mod ai;
pub mod aim;
pub mod attack;
mod commands;
pub mod deck;
pub mod effects;
mod items;
mod results;
mod rows;
pub mod scene;
mod turn;
pub mod units;

use std::collections::VecDeque;

use extraction::saga::SpriteSheet;
use extraction::saga_combat::{self, Grounds, PanelGraphics, SLOTS};
use extraction::saga_encounter::{Formation, Lineup, StoryBattle};
use extraction::saga_formation::FieldLayer;
use extraction::saga_party;
use formats::tile::Tileset;
use gba_runtime::ppu::{FADE_STEPS, Palette, PaletteBank, darken, draw_background};
use platform::{Frame, Input, Rgb};

use crate::data::GameData;
use crate::rng::Rng;
use crate::script::{ScriptError, ScriptRunner};
use crate::translation::{
    BATTLE_LABEL_TABLE, BATTLE_MENU_TABLE, BATTLE_TEXT_TABLE, ITEM_TABLE, NAME_TABLE, PART_TABLE,
};
use crate::windows::ScriptWindows;
use crate::{FrameStyle, ScriptHost, TextPainter, WindowPainter, draw_sprite};
use results::Results;

const WIDTH: usize = 240;
const HEIGHT: usize = 160;
/// The song the battles play (`0x02005C70 + 3` after `0x080329E4`).
pub const BATTLE_SONG: u16 = 0x17;
/// The song the opening starts (`0x0802E9CC`, state 0).
const OPENING_SONG: u16 = 0x16;
/// The sound a retreat plays (`0x08035778`, state 0).
const RETREAT_SOUND: u16 = 0x53;
/// Frames from the controller's start to the opening task's.
const OPENING_START: u32 = 5;
/// Frames the screen's build takes before the fade task starts, besides
/// [`BUILD_PER_PANEL`] a panel: the scripts that name each panel's unit.
const BUILD_FRAMES: u32 = 9;
const BUILD_PER_PANEL: u32 = 8;
const BLACK: u8 = 31;
const MESSAGE_WINDOW: u8 = 1;
/// The opening's waits after each message, by message speed (the
/// game-state block's `+0x3618`, speed − 1): frames at ROM `0x664668`;
/// the slowest waits for A.
const MESSAGE_WAITS: [u16; 5] = [1, 15, 30, 45, 60];
const KEY_WAIT_SPEED: u8 = 5;
const SPEED_BYTE: usize = 0x3618;
/// The game-state block's chapter byte.
const CHAPTER_BYTE: usize = 2;
const PLAYER: usize = 0;
const ENEMY: usize = 1;
const SECOND_BAR: u16 = 0x13;
const PANEL_BANK: u8 = 14;
/// A map entry's tile and flips, and the panel frame's palette 14 the
/// panels take while dimmed.
const TILE_BITS: u16 = 0x0FFF;
const DIM_PANEL_BITS: u16 = 0xE000;
/// The tile the text system loads the window frame's tiles from on BG1
/// (`0x0803E1C8`).
const FRAME_FIRST_TILE: usize = 0x56;
const TEXT_BANK: u8 = 15;

// Scripts of the battle-menu table.
const MENU_MAIN: u16 = 0;
/// The story battles' menu, without 退却 (`0x0802E9CC` state 2000).
const MENU_MAIN_STORY: u16 = 0x18;
const MENU_RESET: u16 = 1;
/// The frames the battle's text-system reset takes when nothing was drawn
/// since the last one.
const QUICK_RESET_FRAMES: u32 = 2;
const MENU_MESSAGE_WINDOW: u16 = 2;
const MENU_PRESENT: u16 = 5;
const MENU_DRAW: u16 = 6;
const MENU_CLEAR: u16 = 7;
// Scripts of the battle-text table.
const TEXT_MET: u16 = 1;
const TEXT_RETREATED: u16 = 0x0D;
const TEXT_CONFIRMED: u16 = 0x0F;
const TEXT_READY: u16 = 0x10;
/// Where the names of the Zoids start in the name table (`0x08676020`).
const ZOID_NAMES: usize = 1;
/// Where the pilots' names start in it (`0x08676284`).
const CHARACTER_NAMES: usize = 154;
/// `battle-text` 0x33: the player's name.
const PLAYER_NAME_TEXT: u16 = 0x33;
/// The window the panels' names are printed into.
const NAME_WINDOW: u8 = 0;
const NO_FRAME: u8 = 0x40;
const NAME_RECT: (u8, u8, u8, u8) = (0, 0, 15, 2);

/// The menu's lines.
const CHOICE_FIGHT: u16 = 0;
const CHOICE_FORMATION: u16 = 1;
const CHOICE_DECK: u16 = 2;
const CHOICE_STATUS: u16 = 3;
/// Frames from the character screen's fade out to its build, and from the
/// build to its fade in and to its menu.
const STATUS_BUILD: u32 = 7;
const STATUS_FADE: u32 = 5;
const STATUS_MENU: u32 = 41;
const CHOICE_RETREAT: u16 = 4;
/// Frames of the rebuild before the panels' names are printed, and after
/// them before the message window opens again.
const REBUILD_NAMES: u32 = 3;
const REBUILD_MESSAGE: u32 = 3;

/// How a battle ended, as `0x0800C3AC` tells its caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The enemies were beaten or fled (result 1 or 2).
    Won,
    /// The party was beaten (result 3).
    Lost,
    /// The party retreated (result 4).
    Retreated,
}

/// A unit on the battle screen: its Zoid's status sprite.
#[derive(Debug, Clone)]
struct Unit {
    sheet: Option<SpriteSheet>,
}

/// How the panels are lit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelLight {
    /// In their own colors.
    Own,
    /// With the frame's darker palette, but for panel `lit`.
    Dimmed { lit: Option<usize> },
}

/// A unit as the battle screen places it.
struct Placed<'a> {
    anchor: (i32, i32),
    mirrored: bool,
    sheet: &'a SpriteSheet,
    dim: u8,
    /// A glow's change of its colors, see [`tint_color`].
    tint: Option<(turn::Glow, u8)>,
    shake: i32,
}

/// A panel of the party's side: its unit's bars.
#[derive(Debug, Clone)]
struct Panel {
    /// Its unit's formation slot.
    slot: usize,
    /// Its pilot.
    character: u8,
    /// Its pilot's name, as printed while the screen is built.
    name: String,
    hp: (u32, u32),
    ep: (u32, u32),
}

/// A script call the running task makes.
#[derive(Debug, Clone, Copy)]
enum Call {
    Menu(u16),
    Text(u16),
    /// A Zoid's name.
    Name(u16),
    /// A string of the labels' table (`0x675D10`).
    Label(u16),
    /// A pilot's name (`0x08032818`): `name` string 154 + the character.
    Character(u8),
    /// A string of the `name` table by its index.
    NameIndex(u16),
    /// A string of the `item` table.
    Item(u16),
    /// A part's name.
    Part(u16),
}

/// What the running slot-5 task does next once its calls return.
#[derive(Debug, Clone, Copy)]
enum Act {
    Call(Call),
    /// Arms the message wait (`0x08004018`).
    Wait,
    /// The results' roll of the spoils, once the money's message is read.
    Spoils,
    /// What the statistics' menu returned.
    Allocation,
    /// The window the texts go to from then on.
    PrintWindow(u8),
    /// Frames the task's calls run past, as the trace measured them: the
    /// queue goes on `n` frames later.
    Pause(u8),
    /// Starts the fade task in; fading out when `out`.
    Fade {
        out: bool,
    },
    Sound(u16),
    Music(u16),
    /// Opens the window a panel's name is printed into (`0x0866B984`): the
    /// frameless window 0 at the top left, 15 × 2.
    OpenNameWindow,
    /// Keeps what window 0 shows as panel `n`'s name.
    CaptureName(usize),
    /// Shows party slot `n`'s bars and lights its panel (`0x08031618`,
    /// `0x08031598`).
    Panel(usize),
    /// Prints `text` until the current line of the print window has
    /// `column` characters, as the item list pads its names
    /// (`0x08039518`).
    Pad(usize, u16),
    /// What the item list's menu returned, taken as its script ends.
    ItemChoice,
    /// The deck's menu returned.
    CommandChoice,
}

/// The opening and menu task (`0x0802E9CC`), by the state it waits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opening {
    Build,
    FadeIn,
    AwaitFade,
    Met,
    AwaitMet,
    Check(usize),
    Next(usize),
    AwaitConfirmed(usize),
    Ready,
    AwaitReady,
    Menu,
    Retreat,
    /// 部隊編成: the formation screen opens in the next frame (state
    /// `0xBB8`), then runs (`0xC1C`).
    OpenFormation,
    Formation,
    /// コマンド作成: the deck screen opens in the next frame (state `0xFA0`),
    /// then runs (`0x1004`).
    OpenDeck,
    Deck,
    /// ステータス: the screen fades out from the frame the menu returned
    /// (states `0x1388`, `0x1392`).
    StatusFadeOut(u32),
    /// Black since the given frame, until the character screen is built
    /// (`0x13EC`, the task `0x08052284`).
    StatusBuild(u32),
    /// The character screen, built and fading in since the given frame.
    Status(u32),
    /// B left it: the fade out.
    StatusLeave,
    /// Back from the formation screen since the given frame (`0xC80`): the
    /// screen is built again in the dark.
    Rebuild(u32),
    /// Its fade in (`0xCE4`), then the menu again.
    AwaitRebuildFade,
    /// 戦闘に入る: the panels and grounds come down (state `0x2328`).
    Engage(u32),
    Done,
}

/// The controller (`0x0802AC0C`) after the opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Controller {
    Opening,
    /// The fight, by its stage.
    Fight(turn::Stage),
    StartResults,
    AwaitResults,
    FadeOut,
    AwaitFade,
    Finish,
    Done,
}

/// The message wait (`0x08003F70`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wait {
    Idle,
    Armed,
    Counting(u16),
    Key,
    Done,
}

/// The fade task (`0x08004034`): frames since it started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fade {
    out: bool,
    age: u32,
}

/// The frame, from the fade task's start, the tasks before it in the slot
/// order see that it ended.
const FADE_SEEN_AGE: u32 = 35;

/// A battle met on the field.
pub struct Combat {
    grid: Option<FieldLayer>,
    grounds: (Option<FieldLayer>, Option<FieldLayer>),
    panel_graphics: Option<PanelGraphics>,
    /// The window frame's tiles, which the text system loads from tile
    /// `0x56` on and the panels' frames use.
    frame_tiles: Option<Tileset>,
    banks: PaletteBank,
    panels: Vec<Panel>,
    units: [[Option<Unit>; SLOTS]; 2],
    anchors: [[(i32, i32); SLOTS]; 2],
    menu: ScriptRunner,
    text: ScriptRunner,
    names: ScriptRunner,
    labels: ScriptRunner,
    items: ScriptRunner,
    parts: ScriptRunner,
    /// The game-state block the battle writes its results into.
    state: Vec<u8>,
    /// The enemies: a roaming formation or a story battle's.
    lineup: Lineup,
    rom_experience: Vec<u32>,
    /// The levels the battle brought, and their points' sharing out.
    levels: u8,
    allocation: results::Allocation,
    /// The window the texts print into: the message window but while the
    /// results draw the statistics' window.
    print_window: u8,
    /// Frames the queued calls wait (`Act::Pause`).
    pause: u8,
    /// The units that fight.
    sides: ai::Sides,
    /// The party's units as a revival builds them again from the game
    /// state (`0x0802B5D0`), whole, by formation slot, with their figures.
    revived: [Option<units::BattleUnit>; SLOTS],
    revived_figures: [Option<Unit>; SLOTS],
    /// The grounds the party and the enemy stand on.
    terrains: (u8, u8),
    /// A story battle's (`0x02005C70` bit 0): no retreat, the story's
    /// song, and no spoils.
    story: bool,
    /// The game's chapter (the game-state block's byte 2).
    chapter: u8,
    rng: Rng,
    vblank: u16,
    fight: turn::Fight,
    task: turn::Task,
    /// The panels' and the grounds' vertical scroll in pixels, and what the
    /// frame shows of them.
    scrolls: (i32, i32),
    shown_scrolls: (i32, i32),
    /// How much each unit's colors are darkened, per 5-bit channel
    /// (`0x08031E90`).
    dims: [[u8; SLOTS]; 2],
    /// Whether the panels show their own colors or the frame's darker
    /// palette 14 during a turn (`0x080316EC`), but for the acting unit's
    /// (`0x08031598`).
    dim_panels: PanelLight,
    /// The formation screen 部隊編成 opened (task `0x08037B84`, slot 7).
    formation_screen: Option<Box<crate::menu::formation::Formation>>,
    /// The round's commands: the deck, what the commands left.
    command_state: commands::CommandState,
    /// Changes to the panels' bars and the target marker's appearances: a
    /// text-system reset after any takes a frame more, as if it waited for
    /// their copy (seen after a repair, a sacrifice and a command's Zoid
    /// choice; not after glows or the panels' lighting alone).
    palette_writes: u32,
    /// The writes at the last reset.
    reset_signature: u32,
    /// The deck screen コマンド作成 opened (task `0x0803B004`, slot 7).
    deck_screen: Option<Box<deck::DeckScreen>>,
    /// The character screen ステータス opened (task `0x08052284`, slot 6).
    status_screen: Option<Box<crate::menu::PauseMenu>>,
    /// The window until whose closing the screen keeps the panels it
    /// shows: the maps the panels were drawn to reach the screen with the
    /// item list's reset.
    panels_held: Option<u8>,
    /// The screen effects the displays show.
    effect_sprites: turn::EffectSprites,
    /// The figures L shows over the party's units.
    figure_glyphs: Option<extraction::saga_battle::UnitLabels>,
    /// Whether the figures' task runs (`0x0802F8F8`, slot 8), whether they
    /// show, and whether the frame shows them (its sprites of the frame
    /// before).
    figures_task: bool,
    figures: Option<Figures>,
    shown_figures: Option<Figures>,
    /// The controller's stage at the last run of the figures' task.
    figures_stage: Option<turn::Stage>,
    /// While an item's glow plays, the figures' task holds them hidden
    /// (`0x0802FA58`).
    figures_held: bool,
    /// The item task's list and choices, what it reported, the marker's
    /// slot and frames, and the slots the marker goes to.
    item_menu: items::ItemMenu,
    item_used: Option<bool>,
    targeting: Option<usize>,
    shown_marker: Option<usize>,
    marker: Option<extraction::saga_battle::EffectSprite>,
    neighbour_slots: Vec<Vec<Vec<u8>>>,
    /// The sparks as the frame shows them: the sprites' OAM of the frame
    /// before.
    shown_sparks: Vec<turn::Spark>,
    /// The units an explosion took off the screen, and the sprites moving
    /// up a row, as the frame shows them.
    shown_gone: [[bool; SLOTS]; 2],
    shown_row_moves: Vec<rows::RowMove>,
    /// Frames the main loop is held up for (a load), in which nothing
    /// runs.
    lag: u32,
    /// The panels as the frame shows them: the map the vertical blank
    /// copies.
    shown_panels: (Vec<Panel>, PanelLight),
    acts: VecDeque<Act>,
    running: Option<Call>,
    frame: u32,
    build_frames: u32,
    controller: Controller,
    opening: Opening,
    results: Results,
    wait: Wait,
    wait_frames: Option<u16>,
    fade: Option<Fade>,
    level: u8,
    shown_level: u8,
    outcome: Option<Outcome>,
    choice: u16,
    sounds: Vec<u16>,
    music: Option<u16>,
    input: Input,
    previous: Input,
}

impl Combat {
    /// A battle against `formation`, the party standing on terrain
    /// `player_terrain` and the enemy on `enemy_terrain`; `state` is the
    /// game-state block the party comes from, whose units' statistics the
    /// battle computes again. It starts in the frame the field has gone
    /// black.
    #[must_use]
    pub fn new(
        data: &GameData<'_>,
        state: &mut [u8],
        formation: &Formation,
        terrains: (u8, u8),
    ) -> Self {
        Self::with_lineup(data, state, Lineup::Roaming(*formation), terrains)
    }

    /// Story battle `battle` (`0x08008D28`), both sides on its terrain;
    /// otherwise as [`Combat::new`].
    #[must_use]
    pub fn story(data: &GameData<'_>, state: &mut [u8], battle: StoryBattle) -> Self {
        let terrain = battle.terrain();
        let mut combat = Self::with_lineup(data, state, Lineup::Story(battle), (terrain, terrain));
        combat.story = true;
        combat
    }

    fn with_lineup(
        data: &GameData<'_>,
        state: &mut [u8],
        lineup: Lineup,
        (player_terrain, enemy_terrain): (u8, u8),
    ) -> Self {
        let rom = data.bytes();
        let offsets = |table| {
            data.script_offsets(table)
                .ok()
                .flatten()
                .unwrap_or_default()
        };
        let (panels, [player, enemy]) = screen_units(data, state, &lineup);
        let sides = battle_units(rom, state, &lineup);
        let anchors = slot_anchors(rom);
        let grounds = (
            saga_combat::grounds(rom, player_terrain).map(|Grounds { player, .. }| player),
            saga_combat::grounds(rom, enemy_terrain).map(|Grounds { enemy, .. }| enemy),
        );
        let (panel_graphics, frame_tiles, banks) = screen_graphics(data);
        let build_frames =
            BUILD_FRAMES + BUILD_PER_PANEL * u32::try_from(panels.len()).unwrap_or(0);
        Self {
            grid: data.battle_field().map(|field| field.grid),
            grounds,
            panel_graphics,
            frame_tiles,
            banks,
            panels,
            units: [player.clone(), enemy],
            anchors,
            menu: ScriptRunner::named(BATTLE_MENU_TABLE, offsets(BATTLE_MENU_TABLE))
                .with_quick_redraws(),
            text: ScriptRunner::named(BATTLE_TEXT_TABLE, offsets(BATTLE_TEXT_TABLE))
                .with_quick_redraws(),
            names: ScriptRunner::named(NAME_TABLE, offsets(NAME_TABLE)).with_quick_redraws(),
            labels: ScriptRunner::named(BATTLE_LABEL_TABLE, offsets(BATTLE_LABEL_TABLE))
                .with_quick_redraws(),
            items: ScriptRunner::named(ITEM_TABLE, offsets(ITEM_TABLE)).with_quick_redraws(),
            parts: ScriptRunner::named(PART_TABLE, offsets(PART_TABLE)).with_quick_redraws(),
            state: state.to_vec(),
            lineup,
            rom_experience: results::experience_table(rom),
            levels: 0,
            allocation: results::Allocation::default(),
            print_window: MESSAGE_WINDOW,
            pause: 0,
            revived: revived_units(rom, state),
            revived_figures: player.clone(),
            sides,
            terrains: (player_terrain, enemy_terrain),
            story: false,
            chapter: state.get(CHAPTER_BYTE).copied().unwrap_or(0),
            rng: Rng::default(),
            vblank: 0,
            fight: turn::Fight::default(),
            task: turn::Task::Idle,
            scrolls: (turn::PANELS_HIDDEN, 0),
            shown_scrolls: (turn::PANELS_HIDDEN, 0),
            dims: [[0; SLOTS]; 2],
            dim_panels: PanelLight::Own,
            panels_held: None,
            formation_screen: None,
            status_screen: None,
            deck_screen: None,
            command_state: commands::CommandState::new(state, rom),
            palette_writes: 0,
            reset_signature: 0,
            effect_sprites: turn::EffectSprites::load(rom),
            figure_glyphs: extraction::saga_battle::unit_labels(rom),
            figures_task: false,
            figures: None,
            shown_figures: None,
            figures_stage: None,
            figures_held: false,
            item_menu: items::ItemMenu::default(),
            item_used: None,
            targeting: None,
            shown_marker: None,
            marker: extraction::saga_battle::screen_sprite(rom, MARKER_SPRITE),
            neighbour_slots: extraction::saga_battle::neighbours(rom),
            shown_sparks: Vec::new(),
            shown_gone: [[false; SLOTS]; 2],
            shown_row_moves: Vec::new(),
            lag: 0,
            shown_panels: (Vec::new(), PanelLight::Own),
            acts: VecDeque::new(),
            running: None,
            frame: 0,
            build_frames,
            controller: Controller::Opening,
            opening: Opening::Build,
            results: Results::Idle,
            wait: Wait::Idle,
            wait_frames: message_wait(state),
            fade: None,
            level: BLACK,
            shown_level: BLACK,
            outcome: None,
            choice: 0,
            sounds: Vec::new(),
            music: None,
            input: Input::default(),
            previous: Input::default(),
        }
    }

    /// Makes the protagonist's attacks land and beat what they hurt, or
    /// ordinary again. A debugging aid; the original has no such thing.
    pub fn set_overpowered(&mut self, overpowered: bool) {
        self.fight.overpowered = overpowered;
    }

    /// Plays the attacks with their scenes, as the original, or without
    /// them, as the enhanced mode can (a port feature): the enemies' attacks
    /// then show only their outcome, and the party's scene stops at the
    /// aim.
    pub fn set_attack_scenes(&mut self, shown: bool) {
        self.fight.quick = !shown;
    }

    /// Draws from `rng` from now on, the vertical blank counter at
    /// `vblank` (IWRAM `0x03002338`): to replay a battle of the original's.
    pub fn resume_random(&mut self, rng: Rng, vblank: u16) {
        self.rng = rng;
        self.vblank = vblank;
    }

    /// The game-state block with the battle's results written in, which
    /// the game takes back once the battle has handed back.
    #[must_use]
    pub fn state(&self) -> &[u8] {
        &self.state
    }

    /// How the battle ended, once it has handed back to the field.
    #[must_use]
    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome.filter(|_| self.controller == Controller::Done)
    }

    /// The controller's state by the original's numbers (`0x0802AC0C`),
    /// for comparing runs; 0 outside the fight.
    #[must_use]
    pub fn controller_state(&self) -> u16 {
        match self.controller {
            Controller::Fight(stage) => stage.number(),
            _ => 0,
        }
    }

    /// The attack scene's state and its aim's, by the original's numbers
    /// (`0x08042348`, `0x08045BF8`), while a scene plays.
    #[must_use]
    pub fn scene_state(&self) -> Option<(u16, Option<u16>)> {
        self.fight
            .scene
            .as_ref()
            .map(|scene| (scene.state(), scene.aim_state()))
    }

    /// The sound effects requested since the last call.
    pub fn take_sounds(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.sounds)
    }

    /// The song requested since the last call.
    pub fn take_music(&mut self) -> Option<u16> {
        self.music.take()
    }

    /// Keeps what the screen shows this frame: the brightness register is
    /// written at the vertical blank, so a frame shows the level of the one
    /// before.
    pub fn latch(&mut self) {
        self.shown_level = self.level;
        self.shown_scrolls = self.scrolls;
        if self.panels_held.is_none() {
            self.shown_panels = (self.panels.clone(), self.dim_panels);
        }
        self.shown_figures = self.figures;
        self.shown_marker = self.targeting;
        self.shown_sparks = self
            .fight
            .display
            .as_ref()
            .map(|display| display.sparks.clone())
            .unwrap_or_default();
        self.shown_gone = self.fight.gone;
        self.shown_row_moves.clone_from(&self.fight.row_moves);
        if let Some(scene) = self.fight.scene.as_mut() {
            scene.latch();
        }
    }

    /// Advances one frame, the tasks in slot order.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a script cannot run.
    pub fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.previous = self.input;
        self.input = input;
        if self.lag > 0 {
            self.lag -= 1;
            self.frame += 1;
            self.vblank = self.vblank.wrapping_add(1);
            return Ok(());
        }
        self.update_controller(rom);
        self.update_formation_screen(rom, input, windows)?;
        self.update_status_screen(rom, input, windows)?;
        self.update_deck_screen(rom, input, windows)?;
        if self.controller == Controller::Fight(turn::Stage::Scene) {
            self.update_scene(rom, input, windows)?;
        } else {
            self.update_slot5(rom, windows)?;
        }
        self.update_figures();
        if let Some(id) = self.panels_held
            && windows
                .windows()
                .get(usize::from(id))
                .is_none_or(Option::is_none)
        {
            self.panels_held = None;
        }
        self.update_wait();
        self.update_fade();
        self.frame += 1;
        self.vblank = self.vblank.wrapping_add(1);
        Ok(())
    }

    /// The figures' task (`0x0802F8F8`, run after the controller and slot
    /// 5): the controller starts it for the battle's menus and turns and
    /// stops it as an attack starts, from the screen's return on; L toggles
    /// the figures, which a flag of the game state keeps from one battle
    /// to the next.
    fn update_figures(&mut self) {
        let stage = match self.controller {
            Controller::Fight(stage) => Some(stage),
            _ => None,
        };
        let left = std::mem::replace(&mut self.figures_stage, stage);
        if !self.figures_running() || self.figures_held {
            self.figures_task = false;
            self.figures = None;
            return;
        }
        let pressed =
            self.input.is_held(platform::Button::L) && !self.previous.is_held(platform::Button::L);
        let shown = if self.figures_task {
            self.figures.is_some() != pressed
        } else {
            self.figures_task = true;
            pressed || formats::progress::battle_labels(&self.state)
        };
        // The controller asks for them again as it leaves these stages
        // (`0x0802FA94`), with the units' places and figures of then.
        let refresh = left != stage
            && matches!(
                left,
                Some(
                    turn::Stage::RoundStart
                        | turn::Stage::Actor
                        | turn::Stage::Next
                        | turn::Stage::Over
                )
            );
        if !shown {
            self.figures = None;
        } else if self.figures.is_none() || refresh {
            self.figures = Some(self.figures_now());
        }
        formats::progress::set_battle_labels(&mut self.state, shown);
    }

    /// The party's units' places, hit points and energy as the figures take
    /// them (`0x08031A10`). The figures are sprites off the grounds' layer:
    /// they show the places as they are, and stay where they are while the
    /// grounds scroll.
    fn figures_now(&self) -> Figures {
        std::array::from_fn(|slot| {
            self.sides[PLAYER][slot]
                .as_ref()
                .filter(|unit| unit.fighting())
                .map(|unit| (self.anchors[PLAYER][slot], unit.hp, unit.ep))
        })
    }

    /// Whether the controller has the figures' task running: from the
    /// battle's start to its end (`0x2530`), but from an attack's start
    /// (`0x1004`) to the end of the screen's return (`0x13EC`).
    fn figures_running(&self) -> bool {
        match self.controller {
            Controller::Fight(stage) => !matches!(
                stage,
                turn::Stage::AwaitAttackFade
                    | turn::Stage::StartScene
                    | turn::Stage::Scene
                    | turn::Stage::Return
                    | turn::Stage::AwaitReturn
            ),
            Controller::Finish | Controller::Done => false,
            _ => true,
        }
    }

    fn pressed_a(&self) -> bool {
        self.input.is_held(platform::Button::A) && !self.previous.is_held(platform::Button::A)
    }

    /// The controller, slot 4: it waits for the opening, starts the results
    /// once it ends, darkens the screen and hands back.
    fn update_controller(&mut self, rom: &[u8]) {
        match self.controller {
            Controller::Opening => {
                if self.opening == Opening::Done {
                    self.controller = if self.outcome.is_some() {
                        Controller::StartResults
                    } else {
                        Controller::Fight(turn::Stage::Engaged)
                    };
                }
            }
            Controller::Fight(_) => self.step_fight(rom),
            Controller::StartResults => {
                self.results = Results::Start;
                self.controller = Controller::AwaitResults;
            }
            Controller::AwaitResults => {
                if self.results == Results::Done {
                    self.controller = Controller::FadeOut;
                }
            }
            Controller::FadeOut => {
                self.fade = Some(Fade { out: true, age: 0 });
                self.controller = Controller::AwaitFade;
            }
            Controller::AwaitFade => {
                if self.fade.is_some_and(|fade| fade.age >= FADE_SEEN_AGE) {
                    self.fade = None;
                    self.controller = Controller::Finish;
                }
            }
            Controller::Finish => {
                self.acts.push_back(Act::Call(Call::Menu(MENU_RESET)));
                self.controller = Controller::Done;
            }
            Controller::Done => {}
        }
    }

    /// Slot 5: the opening, then the results. A call it makes blocks it
    /// until the call returns; it goes on in that frame and steps its state
    /// again the next.
    fn update_slot5(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let busy = self.running.is_some() || !self.acts.is_empty();
        if !busy {
            if self.controller == Controller::Opening {
                self.step_opening();
            } else if self.controller == Controller::AwaitResults {
                self.step_results(rom);
            }
        }
        if self.fight_task_active() && (!busy || matches!(self.task, turn::Task::Return(_))) {
            self.step_fight_task();
        }
        self.run_acts(rom, windows)?;
        // The item and command tasks go on in the frame their last call
        // returns.
        if matches!(
            self.task,
            turn::Task::Item(items::ItemStep::Closed(_))
                | turn::Task::Command(
                    commands::CommandStep::Show(_) | commands::CommandStep::Closed(_)
                )
        ) && self.running.is_none()
            && self.acts.is_empty()
        {
            self.step_fight_task();
        }
        Ok(())
    }

    /// What a menu's return does, in the frame it returns.
    fn returned(&mut self, call: Call) {
        match call {
            Call::Menu(MENU_MAIN | MENU_MAIN_STORY) => self.choose(self.menu.vars()[1]),
            Call::Menu(
                turn::MENU_ROUND
                | turn::MENU_ROUND_STORY
                | turn::MENU_ACTION
                | turn::MENU_ACTION_NO_ITEMS,
            ) => self.choose_in_fight(self.menu.vars()[1]),
            _ => {}
        }
    }

    fn run_acts(&mut self, rom: &[u8], windows: &mut ScriptWindows<'_>) -> Result<(), ScriptError> {
        if self.pause > 0 {
            self.pause -= 1;
            return Ok(());
        }
        loop {
            if let Some(call) = self.running {
                let input = self.input;
                let runner = self.runner(call);
                if !runner.update(rom, input, windows)? {
                    return Ok(());
                }
                self.running = None;
                self.returned(call);
            }
            let Some(act) = self.acts.pop_front() else {
                return Ok(());
            };
            match act {
                Act::Call(call) => {
                    if matches!(call, Call::Menu(MENU_RESET)) {
                        let signature = self.palette_writes;
                        let frames = if signature == self.reset_signature {
                            QUICK_RESET_FRAMES
                        } else {
                            QUICK_RESET_FRAMES + 1
                        };
                        self.menu.set_reset_frames(frames);
                        self.reset_signature = signature;
                    }
                    let printing_names = self.printing_names();
                    let print_window = self.print_window;
                    let runner = self.runner(call);
                    let index = match call {
                        Call::Menu(index)
                        | Call::Text(index)
                        | Call::Label(index)
                        | Call::NameIndex(index)
                        | Call::Item(index)
                        | Call::Part(index) => usize::from(index),
                        Call::Name(zoid) => ZOID_NAMES + usize::from(zoid),
                        Call::Character(character) => CHARACTER_NAMES + usize::from(character),
                    };
                    if !matches!(call, Call::Menu(_)) && !printing_names {
                        runner.select_window(print_window);
                    }
                    runner.start(index)?;
                    self.running = Some(call);
                }
                Act::Wait => self.wait = Wait::Armed,
                Act::Spoils => self.roll_spoils(rom),
                Act::Allocation => self.allocate(),
                Act::PrintWindow(id) => self.print_window = id,
                Act::ItemChoice => self.take_item_choice(),
                Act::CommandChoice => self.take_command_choice(),
                Act::Pad(column, text) => {
                    let length = windows
                        .windows()
                        .get(usize::from(self.print_window))
                        .and_then(Option::as_ref)
                        .and_then(|window| window.lines.last())
                        .map_or(0, |line| line.chars().count());
                    for _ in length..column {
                        self.acts.push_front(Act::Call(Call::Text(text)));
                    }
                }
                Act::Pause(frames) => {
                    self.pause = frames.saturating_sub(1);
                    return Ok(());
                }
                Act::Fade { out } => self.fade = Some(Fade { out, age: 0 }),
                Act::Sound(sound) => self.sounds.push(sound),
                Act::Music(song) => self.music = Some(song),
                Act::OpenNameWindow => {
                    windows.open_window(NAME_WINDOW, NO_FRAME, NAME_RECT, 0);
                    self.names.select_window(NAME_WINDOW);
                    self.text.select_window(NAME_WINDOW);
                }
                Act::Panel(slot) => {
                    self.refresh_panel(slot);
                    self.light_panel(slot);
                }
                Act::CaptureName(index) => {
                    let name = windows
                        .windows()
                        .get(usize::from(NAME_WINDOW))
                        .and_then(Option::as_ref)
                        .and_then(|window| window.shown_lines().first().cloned())
                        .unwrap_or_default();
                    if let Some(panel) = self.panels.get_mut(index) {
                        panel.name = name;
                    }
                }
            }
        }
    }

    fn runner(&mut self, call: Call) -> &mut ScriptRunner {
        match call {
            Call::Menu(_) => &mut self.menu,
            Call::Text(_) => &mut self.text,
            Call::Name(_) | Call::Character(_) | Call::NameIndex(_) => &mut self.names,
            Call::Label(_) => &mut self.labels,
            Call::Item(_) => &mut self.items,
            Call::Part(_) => &mut self.parts,
        }
    }

    /// Prints each panel's pilot's name into window 0 and keeps it, as
    /// the screen's build copies the window's tiles into the panel
    /// (`0x08031074`); the player's is `battle-text` 0x33.
    fn print_panel_names(&mut self) {
        for index in 0..self.panels.len() {
            let character = self.panels[index].character;
            self.acts.push_back(Act::Call(Call::Menu(MENU_RESET)));
            self.acts.push_back(Act::OpenNameWindow);
            let call = if character == 0 {
                Call::Text(PLAYER_NAME_TEXT)
            } else {
                Call::Character(character)
            };
            self.acts.push_back(Act::Call(call));
            self.acts.push_back(Act::CaptureName(index));
        }
        self.acts.push_back(Act::Call(Call::Menu(MENU_RESET)));
    }

    fn message(&mut self, calls: &[Call]) {
        self.message_then(calls, None);
    }

    /// A message, then `after` once it is presented, then the wait.
    fn message_then(&mut self, calls: &[Call], after: Option<Act>) {
        self.acts.push_back(Act::Call(Call::Menu(MENU_DRAW)));
        self.acts.push_back(Act::Call(Call::Menu(MENU_CLEAR)));
        for call in calls {
            self.acts.push_back(Act::Call(*call));
        }
        self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
        if let Some(after) = after {
            self.acts.push_back(after);
        }
        self.acts.push_back(Act::Wait);
    }

    /// Whether the panels' names are being printed into their window.
    fn printing_names(&self) -> bool {
        self.acts
            .iter()
            .any(|act| matches!(act, Act::CaptureName(_)))
    }

    fn wait_done(&mut self) -> bool {
        if self.wait == Wait::Done {
            self.wait = Wait::Idle;
            return true;
        }
        false
    }

    /// One step of the opening (`0x0802E9CC`).
    /// The formation screen's task: opened the frame after the choice, it
    /// runs until it hands back; the party is then taken again from the
    /// game state it changed.
    fn update_formation_screen(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if self.opening == Opening::OpenFormation {
            self.formation_screen = Some(Box::new(crate::menu::formation::Formation::with_timing(
                &GameData::new(rom),
                &self.state,
                crate::menu::formation::BATTLE,
            )));
            self.opening = Opening::Formation;
        }
        let Some(screen) = self.formation_screen.as_mut() else {
            return Ok(());
        };
        screen.update(rom, input, windows, &mut self.state)?;
        if screen.is_closed() {
            // The screen hands back black: the battle's stays so.
            self.level = BLACK;
            self.shown_level = BLACK;
            self.formation_screen = None;
            self.take_party(rom);
            self.opening = Opening::Rebuild(self.frame);
        }
        Ok(())
    }

    /// The deck screen's task: opened the frame after the choice, it runs
    /// until it hands back, black; the screen is then built again.
    fn update_deck_screen(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if self.opening == Opening::OpenDeck {
            // The port has no link battles, which forbid some commands.
            let screen = deck::DeckScreen::new(&GameData::new(rom), &self.state, false);
            self.deck_screen = Some(Box::new(screen));
            self.opening = Opening::Deck;
        }
        let Some(screen) = self.deck_screen.as_mut() else {
            return Ok(());
        };
        screen.update(rom, input, windows, &mut self.state)?;
        if screen.is_closed() {
            self.level = BLACK;
            self.shown_level = BLACK;
            self.deck_screen = None;
            self.command_state.deck = commands::deck_of(&self.state);
            self.opening = Opening::Rebuild(self.frame);
        }
        Ok(())
    }

    /// The character screen's task: built in the dark, it fades in and its
    /// menu runs once it is bright; B fades it out.
    fn update_status_screen(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        match self.opening {
            Opening::StatusBuild(start) if self.frame >= start + STATUS_BUILD => {
                if let Ok(mut screen) =
                    crate::menu::PauseMenu::battle_status(&GameData::new(rom), self.state.clone())
                {
                    screen.open_battle_status(rom, windows)?;
                    self.status_screen = Some(Box::new(screen));
                }
                self.opening = Opening::Status(self.frame);
            }
            Opening::Status(start) if self.frame == start + STATUS_FADE => {
                self.acts.push_back(Act::Fade { out: false });
            }
            Opening::Status(start) if self.frame >= start + STATUS_MENU => {
                if self.fade.is_some_and(|fade| fade.age >= FADE_SEEN_AGE) {
                    self.fade = None;
                    self.level = 0;
                }
                let Some(screen) = self.status_screen.as_mut() else {
                    self.opening = Opening::StatusLeave;
                    return Ok(());
                };
                // The character screen opens no guide, whose loading is
                // all that fails otherwise.
                let step = match screen.update(rom, input, windows) {
                    Ok(step) => step,
                    Err(crate::guide::GuideError::Script(error)) => return Err(error),
                    Err(_) => crate::menu::MenuStep::Closed,
                };
                if step == crate::menu::MenuStep::Closed {
                    // The original's menu (opcode `0x36`) returns in the
                    // key's frame, the port's a frame later: the fade out
                    // starts a frame on.
                    self.fade = Some(Fade { out: true, age: 1 });
                    self.opening = Opening::StatusLeave;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The fades around the character screen: out before it is built, and
    /// out after B, until the battle's screen is built again.
    fn step_status_fade(&mut self) {
        if let Opening::StatusFadeOut(start) = self.opening
            && self.frame == start + 1
        {
            self.acts.push_back(Act::Fade { out: true });
            return;
        }
        if self.fade.is_none_or(|fade| fade.age < FADE_SEEN_AGE) {
            return;
        }
        self.fade = None;
        self.level = BLACK;
        self.opening = if self.opening == Opening::StatusLeave {
            self.status_screen = None;
            Opening::Rebuild(self.frame + 1)
        } else {
            Opening::StatusBuild(self.frame)
        };
    }

    /// The party's units, panels and pictures from the game state again
    /// (`0x0802B5D0`, `0x08031074`).
    fn take_party(&mut self, rom: &[u8]) {
        let data = GameData::new(rom);
        let (panels, [player, _]) = screen_units(&data, &mut self.state, &self.lineup);
        let [party, _] = battle_units(rom, &self.state, &self.lineup);
        self.panels = panels;
        self.revived_figures.clone_from(&player);
        self.units[PLAYER] = player;
        self.sides[ai::PARTY] = party;
        self.revived = revived_units(rom, &self.state);
        self.build_frames =
            BUILD_FRAMES + BUILD_PER_PANEL * u32::try_from(self.panels.len()).unwrap_or(0);
    }

    /// A frame of the screen's rebuild after the formation screen (state
    /// `0xC80`): in the dark, the panels' names, the text system's reset
    /// (`0x0802F07C`), the message window with 戦闘態勢に入ります, then the
    /// fade in.
    fn step_rebuild(&mut self, frame: u32) {
        if frame == REBUILD_NAMES {
            self.print_panel_names();
        }
        if frame >= REBUILD_NAMES + self.build_frames + REBUILD_MESSAGE {
            self.acts
                .push_back(Act::Call(Call::Menu(MENU_MESSAGE_WINDOW)));
            self.acts.push_back(Act::Call(Call::Menu(MENU_DRAW)));
            self.acts.push_back(Act::Call(Call::Menu(MENU_CLEAR)));
            self.acts.push_back(Act::Call(Call::Text(TEXT_READY)));
            self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
            self.acts.push_back(Act::Fade { out: false });
            self.opening = Opening::AwaitRebuildFade;
        }
    }

    fn step_opening(&mut self) {
        match self.opening {
            Opening::Build => {
                if self.frame == OPENING_START {
                    self.acts.push_back(Act::Music(OPENING_SONG));
                    self.print_panel_names();
                }
                if self.frame >= OPENING_START + self.build_frames {
                    self.opening = Opening::FadeIn;
                }
            }
            Opening::FadeIn => {
                self.acts.push_back(Act::Fade { out: false });
                self.opening = Opening::AwaitFade;
            }
            Opening::AwaitFade => {
                if self.fade.is_some_and(|fade| fade.age >= FADE_SEEN_AGE) {
                    self.fade = None;
                    self.opening = Opening::Met;
                }
            }
            Opening::Met => {
                self.acts
                    .push_back(Act::Call(Call::Menu(MENU_MESSAGE_WINDOW)));
                self.acts.push_back(Act::Call(Call::Text(TEXT_MET)));
                self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
                self.acts.push_back(Act::Wait);
                self.opening = Opening::AwaitMet;
            }
            Opening::AwaitMet => {
                if self.wait_done() {
                    self.opening = Opening::Check(0);
                }
            }
            Opening::Check(slot) => self.check_enemy(slot),
            Opening::AwaitConfirmed(slot) => {
                if self.wait_done() {
                    self.opening = Opening::Next(slot);
                }
            }
            Opening::Next(slot) => {
                self.opening = if slot + 1 < SLOTS {
                    Opening::Check(slot + 1)
                } else {
                    Opening::Ready
                };
            }
            Opening::Ready => {
                self.message(&[Call::Text(TEXT_READY)]);
                self.opening = Opening::AwaitReady;
            }
            Opening::AwaitReady => {
                if self.wait_done() {
                    self.opening = Opening::Menu;
                }
            }
            Opening::Menu => {
                let menu = if self.story {
                    MENU_MAIN_STORY
                } else {
                    MENU_MAIN
                };
                self.acts.push_back(Act::Call(Call::Menu(menu)));
            }
            Opening::Retreat => {
                self.outcome = Some(Outcome::Retreated);
                self.opening = Opening::Done;
            }
            Opening::Rebuild(start) => self.step_rebuild(self.frame.saturating_sub(start)),
            Opening::StatusFadeOut(_) | Opening::StatusLeave => self.step_status_fade(),
            Opening::AwaitRebuildFade => {
                if self.fade.is_some_and(|fade| fade.age >= FADE_SEEN_AGE) {
                    self.fade = None;
                    self.figures_held = false;
                    self.opening = Opening::Menu;
                }
            }
            Opening::Engage(frame) => {
                self.scrolls = turn::engage_scrolls(frame);
                self.opening = if frame + 1 >= turn::engage_frames() {
                    Opening::Done
                } else {
                    Opening::Engage(frame + 1)
                };
            }
            Opening::OpenFormation
            | Opening::Formation
            | Opening::OpenDeck
            | Opening::Deck
            | Opening::StatusBuild(_)
            | Opening::Status(_)
            | Opening::Done => {}
        }
    }

    /// Goes on from the menu's line `line` in the frame the menu returns
    /// (the jump table at `0x0802ED08`): 退却 retreats; the other lines are
    /// not modeled yet and show the menu again.
    /// The message of the enemy in slot `slot`, if there is one: its name,
    /// with its letter, and を確認しました.
    fn check_enemy(&mut self, slot: usize) {
        if self.units[ENEMY][slot].is_none() {
            self.opening = Opening::Next(slot);
            return;
        }
        let mut calls = self.unit_name(Some((ENEMY, slot)));
        calls.push(Call::Text(TEXT_CONFIRMED));
        self.message(&calls);
        self.opening = Opening::AwaitConfirmed(slot);
    }

    fn choose(&mut self, line: u16) {
        self.choice = line;
        self.opening = match line {
            CHOICE_FIGHT => Opening::Engage(0),
            CHOICE_FORMATION => {
                self.figures_held = true;
                Opening::OpenFormation
            }
            CHOICE_DECK => {
                self.figures_held = true;
                Opening::OpenDeck
            }
            CHOICE_STATUS => {
                self.figures_held = true;
                Opening::StatusFadeOut(self.frame)
            }
            CHOICE_RETREAT => Opening::Retreat,
            _ => Opening::Menu,
        };
    }

    /// The message wait, slot 9: armed, it counts the speed's frames, or
    /// waits for A at the slowest speed.
    fn update_wait(&mut self) {
        self.wait = match self.wait {
            Wait::Armed => self.wait_frames.map_or(Wait::Key, Wait::Counting),
            Wait::Counting(left) if left <= 1 => Wait::Done,
            Wait::Counting(left) => Wait::Counting(left - 1),
            Wait::Key if self.pressed_a() => Wait::Done,
            other => other,
        };
    }

    /// The fade task, slot 10 (`0x08004034`): a level a frame from its
    /// second frame, to normal or to black.
    fn update_fade(&mut self) {
        let Some(fade) = self.fade.as_mut() else {
            return;
        };
        let level = match (fade.out, fade.age) {
            (_, 0) => None,
            (false, 1) => Some(u32::from(BLACK)),
            (false, age) => u32::from(BLACK).checked_sub(age.saturating_sub(2)),
            (true, 1) => Some(0),
            (true, age) => Some((age - 2).min(u32::from(BLACK))),
        };
        if let Some(level) = level {
            self.level = u8::try_from(level.min(u32::from(BLACK))).unwrap_or(BLACK);
        }
        fade.age += 1;
    }

    /// Draws the battle screen: the grid, the grounds, the units, the
    /// panels and the windows, darkened by the level the frame shows.
    pub fn draw(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
    ) {
        if let Some(scene) = self.fight.scene.as_ref()
            && self.controller == Controller::Fight(turn::Stage::Scene)
        {
            scene.draw(frame, windows, skin, painter);
            return;
        }
        if let Some(screen) = &self.formation_screen
            && screen.covers()
        {
            screen.draw(frame, windows, skin, painter, &self.state);
            return;
        }
        if let Some(screen) = &self.deck_screen
            && screen.covers()
        {
            screen.draw(frame, windows, skin, painter);
            return;
        }
        if let Some(screen) = &self.status_screen {
            screen.draw(frame, windows, skin, painter);
            darken(frame, self.shown_level.min(FADE_STEPS));
            return;
        }
        frame.fill(Rgb::default());
        if self.shown_level < FADE_STEPS {
            if let Some(grid) = &self.grid {
                draw_layer(frame, grid, 0);
            }
            for layer in [&self.grounds.0, &self.grounds.1].into_iter().flatten() {
                draw_layer(frame, layer, self.shown_scrolls.1);
            }
            self.draw_units(frame);
            self.draw_sparks(frame);
            self.draw_figures(frame);
            self.draw_marker(frame);
            self.draw_panels(frame, skin, painter);
            windows.draw_shown(frame, skin, painter);
        }
        darken(frame, self.shown_level.min(FADE_STEPS));
        let screen_fade = self
            .formation_screen
            .as_ref()
            .map(|screen| screen.fade())
            .or_else(|| self.deck_screen.as_ref().map(|screen| screen.fade()));
        if let Some(level) = screen_fade {
            let level = level.min(u32::from(FADE_STEPS));
            darken(frame, u8::try_from(level).unwrap_or(FADE_STEPS));
        }
    }

    /// The units in their slots, the nearer (lower) ones over the others;
    /// the enemy's drawn mirrored, facing the party.
    fn draw_units(&self, frame: &mut Frame) {
        let mut placed: Vec<Placed<'_>> = Vec::new();
        for side in [PLAYER, ENEMY] {
            for (slot, unit) in self.units[side].iter().enumerate() {
                let fighting = self.sides[side][slot]
                    .as_ref()
                    .is_none_or(units::BattleUnit::fighting);
                let vanished = self.shown_gone[side][slot]
                    || self
                        .shown_sparks
                        .iter()
                        .any(|spark| spark.side == side && spark.slot == slot && spark.vanished());
                if !fighting || vanished {
                    continue;
                }
                if let Some(sheet) = unit.as_ref().and_then(|unit| unit.sheet.as_ref()) {
                    placed.push(Placed {
                        anchor: self
                            .row_move_at(side, slot)
                            .unwrap_or(self.anchors[side][slot]),
                        mirrored: side == ENEMY,
                        sheet,
                        dim: self.dims[side][slot],
                        tint: self.tint_of(side, slot),
                        shake: self.shake_of(side, slot),
                    });
                }
            }
        }
        placed.sort_by_key(|placed| placed.anchor.1);
        for Placed {
            anchor: (x, y),
            mirrored,
            sheet,
            dim,
            tint,
            shake,
        } in placed
        {
            let x = x + shake;
            let (Some(sprite), Some(image)) = (sheet.frames.first(), sheet.frame_image(0)) else {
                continue;
            };
            let width = i32::try_from(sprite.width).unwrap_or(0);
            let left = if mirrored {
                x - i32::from(sprite.x) - width
            } else {
                x + i32::from(sprite.x)
            };
            // A glow's colors are the Zoid's own, whatever darkened it.
            let dim = if tint.is_some() { 0 } else { dim };
            let palette = sheet
                .palette
                .map(|color| tint_color(darken_color(color, dim), tint));
            draw_sprite(
                frame,
                left,
                y + i32::from(sprite.y) - self.shown_scrolls.1,
                &image,
                &palette,
                sprite.mirrored != mirrored,
            );
        }
    }

    /// How far a hit unit's sprite is shaken this frame.
    fn shake_of(&self, side: usize, slot: usize) -> i32 {
        self.shown_sparks
            .iter()
            .filter(|spark| spark.side == side && spark.slot == slot)
            .map(turn::Spark::shake)
            .sum()
    }

    /// The party's units' hit points and energy over them (`0x08031A10`,
    /// `0x08031FB4`): four orange digits, a slash and three blue ones, each
    /// an 8×8 tile, from 32 pixels left of the unit's place and 16 above,
    /// at most 9999 and 999.
    fn draw_figures(&self, frame: &mut Frame) {
        let (Some(glyphs), Some(figures)) = (self.figure_glyphs.as_ref(), self.shown_figures)
        else {
            return;
        };
        let palette = Palette::new(glyphs.palette.map(Palette::from_bgr555));
        // The first slot's sprites come first in OAM, in front of the
        // others'.
        for ((x, y), hp, ep) in figures.into_iter().rev().flatten() {
            let digits = |value: i32, count: u32, first: usize| {
                let value = u32::try_from(value.max(0))
                    .unwrap_or(0)
                    .min(10u32.pow(count) - 1);
                (0..count).map(move |place| {
                    let digit = value / 10u32.pow(count - 1 - place) % 10;
                    first + usize::try_from(digit).unwrap_or(0)
                })
            };
            let tiles: Vec<usize> = digits(hp, 4, LABEL_HP_DIGITS)
                .chain(std::iter::once(LABEL_SLASH))
                .chain(digits(ep, 3, LABEL_EP_DIGITS))
                .collect();
            let top = y + LABEL_OFFSET.1;
            for (place, tile) in tiles.into_iter().enumerate() {
                let Some(pixels) = glyphs.tiles.tile(tile) else {
                    continue;
                };
                let left = x + LABEL_OFFSET.0 + 8 * i32::try_from(place).unwrap_or(0);
                for (index, &color) in pixels.iter().enumerate() {
                    if color == 0 {
                        continue;
                    }
                    let (px, py) = (
                        left + i32::try_from(index % 8).unwrap_or(0),
                        top + i32::try_from(index / 8).unwrap_or(0),
                    );
                    if let (Ok(px), Ok(py)) = (usize::try_from(px), usize::try_from(py))
                        && px < WIDTH
                        && py < HEIGHT
                    {
                        frame.set_pixel(px, py, palette.color(color));
                    }
                }
            }
        }
    }

    /// The item's target marker (`0x08031B44`): its entity does not
    /// animate, so it shows its first frame.
    fn draw_marker(&self, frame: &mut Frame) {
        let (Some(sprite), Some(slot)) = (self.marker.as_ref(), self.shown_marker) else {
            return;
        };
        let index = sprite.animation.first().map_or(0, |step| step.frame);
        let palette = Palette::new(sprite.palette.map(Palette::from_bgr555));
        let (x, y) = self.anchors[PLAYER][slot];
        let mut layer: Vec<Option<Rgb>> = vec![None; WIDTH * HEIGHT];
        for piece in sprite.frames.get(index).into_iter().flatten() {
            crate::battle::draw_piece(
                &mut layer,
                sprite,
                &palette,
                piece,
                (x + MARKER_OFFSET.0, y + MARKER_OFFSET.1),
                false,
            );
        }
        for (index, color) in layer.into_iter().enumerate() {
            if let Some(color) = color {
                frame.set_pixel(index % WIDTH, index / WIDTH, color);
            }
        }
    }

    /// How much a glow takes the red of a unit's colors to its top and
    /// back this frame.
    fn tint_of(&self, side: usize, slot: usize) -> Option<(turn::Glow, u8)> {
        self.shown_sparks
            .iter()
            .filter(|spark| spark.side == side && spark.slot == slot)
            .find_map(turn::Spark::tint)
    }

    /// The hit units' sparks and the raised ones' glows, on the grounds'
    /// layer.
    fn draw_sparks(&self, frame: &mut Frame) {
        let mut layer: Vec<Option<Rgb>> = vec![None; WIDTH * HEIGHT];
        for spark in &self.shown_sparks {
            let Some(sprite) = self.spark_sprite(spark) else {
                continue;
            };
            let palette = Palette::new(sprite.palette.map(Palette::from_bgr555));
            let Some(index) = spark.sprite_frame(sprite) else {
                continue;
            };
            let (x, y) = spark.anchor(self.anchors[spark.side][spark.slot]);
            for piece in sprite.frames.get(index).into_iter().flatten() {
                crate::battle::draw_piece(
                    &mut layer,
                    sprite,
                    &palette,
                    piece,
                    (x, y - self.shown_scrolls.1),
                    false,
                );
            }
        }
        for (index, color) in layer.into_iter().enumerate() {
            if let Some(color) = color {
                frame.set_pixel(index % WIDTH, index / WIDTH, color);
            }
        }
    }

    /// The party's panels: BG1 scrolled two rows up, so their bars show and
    /// the names above them stay hidden until the party engages.
    fn draw_panels(&self, frame: &mut Frame, skin: &WindowPainter, painter: &TextPainter) {
        let Some(graphics) = &self.panel_graphics else {
            return;
        };
        let panels = if self.shown_panels.0.is_empty() {
            &self.panels
        } else {
            &self.shown_panels.0
        };
        let count = panels.len();
        let columns = saga_combat::PANEL_COLUMNS;
        let entry = |column: usize, row: usize| -> u16 {
            let panel = (0..count).find(|index| {
                let first = saga_combat::panel_column(*index, count);
                (first..first + columns).contains(&column)
            });
            let Some(index) = panel else {
                return 0;
            };
            let cell = column - saga_combat::panel_column(index, count);
            if row >= saga_combat::PANEL_ROWS {
                return 0;
            }
            let entry = {
                let frame_entry = graphics.map[row * columns + cell];
                if cell == 0 || cell == columns - 1 {
                    frame_entry
                } else {
                    let panel = &panels[index];
                    let (value, base) = if row == 2 {
                        (panel.hp, 0)
                    } else {
                        (panel.ep, SECOND_BAR)
                    };
                    let level = saga_combat::bar_level(value.0, value.1);
                    graphics.bars[usize::from(level)][cell - 1] + base
                }
            };
            if self.panel_dimmed(index) {
                entry & TILE_BITS | DIM_PANEL_BITS
            } else {
                entry
            }
        };
        let tile = |index: usize| {
            if let Some(frame_tile) = index.checked_sub(FRAME_FIRST_TILE) {
                return self
                    .frame_tiles
                    .as_ref()
                    .and_then(|tiles| tiles.tile(frame_tile));
            }
            index
                .checked_sub(saga_combat::PANEL_FIRST_TILE)
                .and_then(|index| graphics.tiles.tile(index))
        };
        let scroll = usize::try_from(self.shown_scrolls.0).unwrap_or(0);
        draw_background(frame, entry, tile, &self.banks, (0, scroll), true);
        for (index, panel) in panels.iter().enumerate() {
            let column = saga_combat::panel_column(index, count) + 1;
            let x = i32::try_from(column * 8).unwrap_or(0);
            let dim = self.panel_dimmed(index).then_some(&graphics.palette);
            draw_name(
                frame,
                &panel.name,
                (x, -self.shown_scrolls.0),
                (skin, dim),
                painter,
            );
        }
    }

    /// Whether panel `index` shows with the frame's darker palette.
    fn panel_dimmed(&self, index: usize) -> bool {
        match self.shown_panels.1 {
            PanelLight::Own => false,
            PanelLight::Dimmed { lit } => lit != Some(index),
        }
    }
}

/// The frames a message waits before it goes on at the save's message
/// speed, or none when it waits for a key.
fn message_wait(state: &[u8]) -> Option<u16> {
    let speed = state.get(SPEED_BYTE).copied().unwrap_or(2);
    MESSAGE_WAITS
        .get(usize::from(speed))
        .copied()
        .filter(|_| speed != KEY_WAIT_SPEED)
}

/// The panels' graphics, the window frames' tiles and the palettes both
/// use.
fn screen_graphics(data: &GameData<'_>) -> (Option<PanelGraphics>, Option<Tileset>, PaletteBank) {
    let panel_graphics = saga_combat::panel_graphics(data.bytes());
    let mut bank = [[0u16; 16]; 16];
    if let Some(graphics) = &panel_graphics {
        bank[usize::from(PANEL_BANK)] = graphics.palette;
    }
    let skin = data.window_skin().ok();
    if let Some(skin) = &skin {
        bank[usize::from(TEXT_BANK)] = skin.palette;
    }
    (
        panel_graphics,
        skin.map(|skin| skin.tiles),
        PaletteBank::from_bgr555(&bank),
    )
}

/// Where each side's slots stand on the screen (ROM `0x66B484`).
fn slot_anchors(rom: &[u8]) -> [[(i32, i32); SLOTS]; 2] {
    std::array::from_fn(|side| {
        std::array::from_fn(|slot| {
            saga_combat::slot_anchor(rom, side == ENEMY, slot).unwrap_or_default()
        })
    })
}

/// The battle screen's units and the party's panels: each unit's Zoid and
/// status sprite, and each party unit's bars, its statistics computed
/// again.
fn screen_units(
    data: &GameData<'_>,
    state: &mut [u8],
    lineup: &Lineup,
) -> (Vec<Panel>, [[Option<Unit>; SLOTS]; 2]) {
    let rom = data.bytes();
    let mut sheets: Vec<(u16, SpriteSheet)> = Vec::new();
    let mut unit = |zoid: u16| {
        if let Some((_, sheet)) = sheets.iter().find(|(known, _)| *known == zoid) {
            return Unit {
                sheet: Some(sheet.clone()),
            };
        }
        let sheet = data.zoid_status_sprite(usize::from(zoid)).ok();
        if let Some(sheet) = &sheet {
            sheets.push((zoid, sheet.clone()));
        }
        Unit { sheet }
    };
    let mut panels = Vec::new();
    let mut player: [Option<Unit>; SLOTS] = Default::default();
    for (slot, entry) in saga_party::formation(state).iter().enumerate() {
        let Some((unit_index, character)) = *entry else {
            continue;
        };
        saga_party::refresh_stats(rom, state, character, unit_index);
        let status = saga_party::unit_status(state, unit_index);
        panels.push(Panel {
            slot,
            character,
            name: String::new(),
            hp: status.map_or((0, 1), |status| status.hp),
            ep: status.map_or((0, 1), |status| status.ep),
        });
        if let Some(status) = status {
            player[slot] = Some(unit(status.zoid));
        }
    }
    let enemy: [Option<Unit>; SLOTS] = std::array::from_fn(|slot| {
        lineup
            .enemy_record(rom, slot)
            .map(|record| unit(u16::from(record[0])))
    });
    (panels, [player, enemy])
}

/// The item's target marker (record 0 of the battle screen's own sprites),
/// 8 pixels right of and 16 above the unit (`0x080385B0`).
const MARKER_SPRITE: usize = 0;
const MARKER_OFFSET: (i32, i32) = (8, -16);

/// The figures of each party slot: the unit's place, hit points and
/// energy when they were taken.
type Figures = [Option<((i32, i32), i32, i32)>; SLOTS];

/// The figures' tiles: the hit points' digits, the energy's, the slash.
const LABEL_HP_DIGITS: usize = 0;
const LABEL_EP_DIGITS: usize = 10;
const LABEL_SLASH: usize = 20;
/// Where the figures' first tile is from the unit's place.
const LABEL_OFFSET: (i32, i32) = (-32, -16);

/// A BGR555 color as a glow changes it (`0x08031E90`, a pass a flag):
/// bits 4, 8 and `0x10` pick red, green and blue, bit 0 lowers them by the
/// amount, at least 0, and they are raised otherwise, at most 31; so a
/// raise's brightest reds dim (6, then 5).
fn tint_color(color: u16, tint: Option<(turn::Glow, u8)>) -> u16 {
    let Some((glow, amount)) = tint.filter(|(_, amount)| *amount > 0) else {
        return color;
    };
    let amount = u16::from(amount);
    let mut channels = [color & 0x1F, (color >> 5) & 0x1F, (color >> 10) & 0x1F];
    for &flags in glow.flags() {
        for (index, channel) in channels.iter_mut().enumerate() {
            if flags & (4 << index) == 0 {
                continue;
            }
            *channel = if flags & 1 != 0 {
                channel.saturating_sub(amount)
            } else {
                (*channel + amount).min(0x1F)
            };
        }
    }
    let [red, green, blue] = channels;
    red | green << 5 | blue << 10 | (color & 0x8000)
}

/// A BGR555 color with `amount` taken off each channel (`0x08031E90` with
/// flags `0x1D`).
fn darken_color(color: u16, amount: u8) -> u16 {
    if amount == 0 {
        return color;
    }
    let amount = u16::from(amount);
    let channel = |shift: u16| ((color >> shift) & 0x1F).saturating_sub(amount) << shift;
    channel(0) | channel(5) | channel(10)
}

/// A panel's name: the first four cells of its window (32 × 16 pixels,
/// the window's fill behind the text), as the build copies eight of the
/// window's tiles over the panel's.
fn draw_name(
    frame: &mut Frame,
    name: &str,
    (x, y): (i32, i32),
    (skin, dim): (&WindowPainter, Option<&[u16; 16]>),
    painter: &TextPainter,
) {
    const WIDTH: usize = 32;
    const HEIGHT: usize = 16;
    let mut cells = Frame::new(WIDTH, HEIGHT, Rgb::default());
    skin.draw_framed(&mut cells, 0, 0, WIDTH / 8, HEIGHT / 8, FrameStyle::None);
    painter.draw(&mut cells, 0, 0, name, skin.palette());
    let remap = |color: Rgb| -> Rgb {
        let Some(dim) = dim else {
            return color;
        };
        (0..16u8)
            .find(|index| skin.palette().color(*index) == color)
            .map_or(color, |index| Palette::from_bgr555(dim[usize::from(index)]))
    };
    for row in 0..HEIGHT {
        for column in 0..WIDTH {
            let Some(color) = cells.pixel(column, row).map(remap) else {
                continue;
            };
            let (Ok(px), Ok(py)) = (
                usize::try_from(x + i32::try_from(column).unwrap_or(0)),
                usize::try_from(y + i32::try_from(row).unwrap_or(0)),
            ) else {
                continue;
            };
            if px < frame.width() && py < frame.height() {
                frame.set_pixel(px, py, color);
            }
        }
    }
}

/// The units that fight: the party's formation and the enemy's.
fn battle_units(rom: &[u8], state: &[u8], lineup: &Lineup) -> ai::Sides {
    let formation_slots = saga_party::formation(state);
    let unarmed = lineup.story().is_some_and(StoryBattle::unarmed);
    [
        std::array::from_fn(|slot| {
            formation_slots[slot].and_then(|(unit, character)| {
                units::BattleUnit::party(rom, state, unit, character, unarmed)
            })
        }),
        std::array::from_fn(|slot| units::BattleUnit::enemy(rom, state, lineup, slot)),
    ]
}

/// The party's units as a revival builds them from the game state
/// (`0x0802B5D0`, `0x0803376C`): the whole party's parts, with all their hit
/// points and energy.
fn revived_units(rom: &[u8], state: &[u8]) -> [Option<units::BattleUnit>; SLOTS] {
    let formation_slots = saga_party::formation(state);
    std::array::from_fn(|slot| {
        let (unit, character) = formation_slots[slot]?;
        let mut unit = units::BattleUnit::party(rom, state, unit, character, false)?;
        unit.hp = unit.max_hp;
        unit.ep = unit.max_ep;
        Some(unit)
    })
}

/// Draws a field layer over the frame, scrolled `scroll` pixels up (the
/// map wraps every 32 rows): its tile 0 is clear.
fn draw_layer(frame: &mut Frame, layer: &FieldLayer, scroll: i32) {
    const MAP_ROWS: usize = 32;
    let mut palettes = [[0u16; 16]; 16];
    palettes[usize::from(layer.bank & 0x0F)] = layer.palette;
    let bank = PaletteBank::from_bgr555(&palettes);
    let height = i32::try_from(MAP_ROWS * 8).unwrap_or(256);
    let scroll = usize::try_from(scroll.rem_euclid(height)).unwrap_or(0);
    draw_background(
        frame,
        |column, row| layer.entry(column, row % MAP_ROWS),
        |tile| layer.tiles.tile(tile),
        &bank,
        (0, scroll),
        true,
    );
}

/// Frames a battle's panels would take to name their units, for a party
/// of `panels`: what the port counts before the fade in.
#[must_use]
pub const fn build_frames(panels: u32) -> u32 {
    BUILD_FRAMES + BUILD_PER_PANEL * panels
}

#[cfg(test)]
mod tests {
    use super::*;

    const FADE_LAST_AGE: u32 = 33;

    #[test]
    fn the_message_wait_ends_the_speeds_frames_after_it_is_armed() {
        let rom = Vec::new();
        let data = GameData::new(&rom);
        let mut state = vec![0; 0x3F10];
        state[SPEED_BYTE] = 2;
        let mut combat = Combat::new(&data, &mut state, &[0xFF; 36], (0, 0));
        combat.wait = Wait::Armed;
        combat.update_wait();
        let frames = (1..100).find(|_| {
            combat.update_wait();
            combat.wait == Wait::Done
        });
        assert_eq!(frames, Some(30));
        state[SPEED_BYTE] = KEY_WAIT_SPEED;
        let mut combat = Combat::new(&data, &mut state, &[0xFF; 36], (0, 0));
        combat.wait = Wait::Armed;
        combat.update_wait();
        assert_eq!(combat.wait, Wait::Key);
    }

    #[test]
    fn the_menus_lines_open_their_screens_and_retreat_ends_it() {
        let rom = Vec::new();
        let data = GameData::new(&rom);
        let mut state = vec![0; 0x3F10];
        let mut combat = Combat::new(&data, &mut state, &[0xFF; 36], (0, 0));
        combat.choose(CHOICE_DECK);
        assert_eq!(combat.opening, Opening::OpenDeck);
        combat.choose(CHOICE_STATUS);
        assert!(matches!(combat.opening, Opening::StatusFadeOut(_)));
        combat.choose(CHOICE_FORMATION);
        assert_eq!(combat.opening, Opening::OpenFormation);
        combat.choose(CHOICE_FIGHT);
        assert_eq!(combat.opening, Opening::Engage(0));
        combat.choose(CHOICE_RETREAT);
        combat.step_opening();
        assert_eq!(combat.opening, Opening::Done);
        assert_eq!(combat.outcome, Some(Outcome::Retreated));
    }

    #[test]
    fn a_full_party_takes_longer_to_build() {
        assert_eq!(build_frames(4), 41);
        assert!(build_frames(6) > build_frames(4));
    }

    #[test]
    fn the_fade_in_brightens_a_level_a_frame_from_its_third() {
        let mut fade = Fade { out: false, age: 0 };
        let mut levels = Vec::new();
        for _ in 0..=FADE_LAST_AGE {
            let level = match fade.age {
                0 => None,
                1 => Some(31),
                age => 31u32.checked_sub(age - 2),
            };
            levels.push(level);
            fade.age += 1;
        }
        assert_eq!(levels[1], Some(31));
        assert_eq!(levels[2], Some(31));
        assert_eq!(levels[33], Some(0));
    }
}
