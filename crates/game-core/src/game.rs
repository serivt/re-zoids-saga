//! The whole game as one state machine: logo, title, name entry, then the
//! field with its conversations, exits and events.
//!
//! Entering a map runs the map's own handler (see [`crate::story`]); a new
//! game enters the first room from black and brightens it a level a frame
//! while the game holds, and the room's handler starts the opening. Objects
//! whose script is code run the event transcribed for it; events run as
//! tasks after the actors every frame (see [`crate::event`]).
//!
//! Continuing, as the original does it once the title's script has ended:
//! three frames on, the title darkens a level a frame to 31 (black from the
//! 16th), and 75 frames after the script's end the saved room loads and
//! brightens like any map entered. When the loader has a notice (no save, a
//! broken one, the backup used) its script starts 40 frames after the
//! script's end; once it is dismissed the title starts over, or, with the
//! backup, the room loads 16 frames later. With several save slots, a
//! port feature (see [`crate::slots`]), つづきから and セーブ first ask
//! which one.

use extraction::saga::{
    BootError, CHEST_FLAG_BASE, FIRST_ROOM_MAP, OPENING_SEEN_FLAG, ObjectScript, PLAYER_START,
    Reward, SpriteSheetError,
};
use extraction::saga_save::SaveDataError;
use extraction::saga_shop::{self, ITEM_LIMIT, Item, ItemKind, MONEY_LIMIT, PART_LIMIT};
use extraction::{saga_arena, saga_party};
use formats::Progress;
use formats::m4a::M4aError;
use formats::progress::{FLAG_WORDS, encode_name};
use gba_runtime::apu::SoundEngine;
use gba_runtime::ppu::{FADE_STEPS, SCREEN_HEIGHT, SCREEN_WIDTH, darken};
use platform::{Button, Frame, Input, Rgb, SaveStorage};
use thiserror::Error;

use crate::battle::Staged;
use crate::boot::{LogoScreen, NameEntry, TitleChoice, TitleScreen};
use crate::combat::{Combat, Outcome};
use crate::data::GameData;
use crate::demo::{DemoEnd, DemoStep};
use crate::event::{
    BLACK, ChestKind, EventHost, Events, FIELD_HOOK, FIELD_WATCH, HoldStep, MAP_TASK, Op,
};
use crate::extension::{Event, GameSound, SharedExtensions};
use crate::field::{Command, Direction, Field, FieldError, FieldEvent};
use crate::guide::{Cover, Guide, GuideError, GuideKind};
use crate::menu::{MenuStep, Party, PauseMenu, Shop};
use crate::objects::AreaObjects;
use crate::play_mode::{Enhancements, PlayMode};
use crate::save::{Found, SaveFile, SavedGame};
use crate::script::{ScriptContext, ScriptError, ScriptRunner};
use crate::slots::{self, Pick, Purpose, Slot, SlotPicker};
use crate::story;
use crate::text::TextMetrics;
use crate::translation::{
    DIALOGUE_TABLE, ITEM_TABLE, NAME_TABLE, PART_TABLE, PAUSE_MENU_TABLE, Translation,
    TranslationExtension,
};
use crate::windows::{DEFAULT_PLAYER_NAME, ScriptWindows, TRANSLATED_PLAYER_NAME};
use crate::{ScriptHost, TextPainter, WindowPainter};

const TALK_START_DELAY: u32 = 3;
/// Frames between A and a chest opening. An object whose script is other
/// code runs it in the frame it is spoken to, from within the player's
/// update.
const CHEST_START_DELAY: u32 = 1;
const CHEST_SOUND: u16 = 0x48;
const SMALL_CHEST_SOUND: u16 = 0x46;
const SMALL_CHEST_SPRITE: &str = "tb00";
const CHEST_OPEN_ANIMATION: usize = 1;
const MONEY_WINDOW: u8 = 1;
/// Where the Zoids' names start in the `name` table (`0x08032800`).
const ZOID_NAMES: u16 = 1;
/// Where the consumables' names start in the `name` table (`0x08032840`).
const CONSUMABLE_NAMES: u16 = 241;
/// Frames between the step onto an exit and the first darker level.
const EXIT_FADE_DELAY: u8 = 1;
/// Frames between pushing against a door and the screen darkening.
const DOOR_FADE_DELAY: u8 = 2;
/// Black frames after a door before the new map brightens, measured on a
/// door from a Zoid map into a town (the original's load takes longer
/// than a room's).
const DOOR_BLACK_FRAMES: u8 = 15;
const ON_FOOT_SPRITE: u16 = 0x98;
const ON_FOOT_DOOR_SOUND: u16 = 0x45;
const NO_DOOR_SOUND: u16 = 0x44;
/// Frames from START on the field until the pause menu is built; the
/// field darkens a level a frame from the fourth.
const MENU_OPEN_FRAMES: u32 = 33;
const MENU_OPEN_DELAY: u32 = 2;
/// Black frames before the field brightens again after the menu.
const MENU_RETURN_BLACK_FRAMES: u8 = 16;
/// Where the actors' animations stand when the field returns.
const MENU_RETURN_ANIMATION: u32 = 1;
/// Frames the destination stays black once loaded, and frames the game
/// stays held once it is bright again.
const WARP_BLACK_FRAMES: u8 = 10;
/// The sprites from which a frame has no time left to redraw the windows a
/// close leaves open (see `start_dialogue`).
const BUSY_SPRITES: usize = 4;
/// The world map, whose load takes the original longer than a room's.
const WORLD_MAP: usize = 1;
/// The frames the original's load of the world map takes beyond a room's,
/// measured on the labyrinth's exit (the level falls from 31 six frames
/// later); its scene is the largest and it has seven objects.
const WORLD_MAP_LOAD_FRAMES: u8 = 6;
const WARP_SETTLE_FRAMES: u8 = 1;
/// Once the name entry's script ends the entry stays this many frames,
/// then darkens a level a frame (visibly for 16, on to 31), and the first
/// room loads this many frames after the script's end.
const NAME_HOLD_FRAMES: u32 = 4;
const NAME_TO_ROOM_FRAMES: u32 = 67;
/// The name entry's music stops this many frames after the script's end.
const NAME_MUSIC_STOP_FRAMES: u32 = 43;
const CONTINUE_HOLD_FRAMES: u32 = 3;
const CONTINUE_LOAD_FRAMES: u32 = 75;
const CONTINUE_NOTICE_FRAMES: u32 = 40;
const CONTINUE_AFTER_NOTICE_FRAMES: u32 = 16;
const MUSIC_PLAYER: usize = 0;
/// Frames a roaming enemy stands still after the party retreated from it
/// (entity state 8, `0x0800BD74`).
const RETREAT_PAUSE: u16 = 180;
/// What an enemy met stands still for until the battle's outcome takes
/// hold: for good, after a lost battle.
const MEETING_PAUSE: u16 = u16::MAX;
/// The shift of the player's animation once the battle is over
/// (`0x0800B9CC` sets `+0x38`).
const AFTER_COMBAT_SHIFT: i8 = 1;
/// Frames between the player's explosion ending and the screen darkening
/// after a lost battle: the first frame of `0x08001524` sets level 0.
const DEFEAT_FADE_DELAY: u8 = 1;
/// Black frames once the return point is loaded, before it brightens.
const DEFEAT_BLACK_FRAMES: u8 = 13;
/// Where the player's animation stands when the outcome takes hold: its
/// update runs after the battle's handler returns, in the same frame, while
/// the other objects have already had theirs.
const PLAYER_UPDATED: u32 = 1;
/// The bits of the player's footing attribute the battle takes as its
/// terrain; with any other set it takes the enemy's (`0x0800B9CC`).
const TERRAIN_MASK: u16 = 0xFF;
/// The area a map with no record belongs to.
const NO_AREA: u8 = 0;
/// Where the game is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The publisher logo.
    Logo,
    /// The title screen and its menu.
    Title,
    /// Entering the player's name.
    NameEntry,
    /// Black between the name entry and the room.
    Loading,
    /// Free play.
    Field,
    /// The pause menu.
    Menu,
    /// Between the title and a continued game.
    Continuing,
    /// The Zoid or character guide.
    Guide,
}

/// Why the game could not go on.
#[derive(Debug, Error)]
pub enum GameError {
    /// Boot screen data could not be read.
    #[error(transparent)]
    Boot(#[from] BootError),
    /// The field could not be loaded.
    #[error(transparent)]
    Field(#[from] FieldError),
    /// A script failed.
    #[error(transparent)]
    Script(#[from] ScriptError),
    /// A sprite could not be read.
    #[error(transparent)]
    Sprite(#[from] SpriteSheetError),
    /// A song could not be read.
    #[error(transparent)]
    Sound(#[from] M4aError),
    /// A translation does not match the script tables.
    #[error(transparent)]
    Translation(#[from] crate::translation::TranslationError),
    /// The font or window skin could not be read.
    #[error("cannot read the text assets: {0}")]
    Text(String),
    /// The save layout or the new-game state could not be read.
    #[error(transparent)]
    SaveData(#[from] SaveDataError),
    /// A guide failed.
    #[error(transparent)]
    Guide(#[from] GuideError),
}

enum Screen {
    Logo(LogoScreen),
    Title(TitleScreen),
    NameEntry(NameEntry),
    Loading,
    LeavingNameEntry(Box<NameEntry>, u32),
    Field,
    /// START on the field: frames while the field darkens before the
    /// pause menu is built.
    OpeningMenu(u32),
    Menu(Box<PauseMenu>),
    Continuing(Continuing),
    Guide(Box<Guide>),
    /// The port's end of the demo, over the field.
    DemoEnd(Box<DemoEnd>),
}

/// Where continuing is: frames since the title's script ended (the title
/// held, darkening, then black), the loader's notice, or frames since it
/// was dismissed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContinuePhase {
    Leaving(u32),
    Notice,
    AfterNotice(u32),
}

struct Continuing {
    title: TitleScreen,
    phase: ContinuePhase,
}

impl Continuing {
    fn darkness(&self) -> u8 {
        match self.phase {
            ContinuePhase::Leaving(frames) => {
                u8::try_from(frames.saturating_sub(CONTINUE_HOLD_FRAMES))
                    .map_or(BLACK, |level| level.min(BLACK))
            }
            ContinuePhase::Notice | ContinuePhase::AfterNotice(_) => BLACK,
        }
    }
}

/// What speaking to an object starts once the talk delay has passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Talk {
    /// A string of the dialogue table.
    Dialogue(usize),
    /// A transcribed event.
    Event(&'static [Op]),
}

/// What the story's events set on the field beyond its objects.
#[derive(Debug, Default)]
struct StoryEffects {
    /// No enemy meets the player (`0x02000008` bit 1), until the next
    /// map's handler runs.
    calm: bool,
    /// How far the field is brightened toward white (`BLDY`), 0 to 16.
    whiten: u8,
    /// The guard that last saw the player.
    seen_guard: Option<usize>,
}

/// The running game.
pub struct Game<'rom> {
    data: GameData<'rom>,
    extensions: SharedExtensions,
    frame: u64,
    painter: TextPainter<'rom>,
    skin: WindowPainter,
    windows: ScriptWindows<'rom>,
    sound: SoundEngine<'rom>,
    dialogue: ScriptRunner,
    scripts: Vec<ScriptRunner>,
    active_script: Option<usize>,
    last_runner: Option<usize>,
    battle: Option<Box<Staged>>,
    /// The shop a keeper opened, while it is open.
    shop: Option<Box<PauseMenu>>,
    /// The battle against a roaming enemy, while it runs.
    combat: Option<Box<Combat>>,
    /// The roaming enemy the player met, until its battle ends.
    encounter: Option<usize>,
    /// The enemy met and the battle's outcome, until the field is bright
    /// again and they take hold.
    aftermath: Option<(usize, Outcome)>,
    /// Whether the party lost and is being taken to its return point.
    defeated: bool,
    /// Whether the last battle an event fought was lost.
    battle_lost: bool,
    field: Option<Field>,
    /// Whether the port's debugging mode is on (see
    /// [`Game::toggle_debug_mode`]).
    debug: bool,
    /// The map and flag where the demo ends (see [`Game::set_demo_end`]).
    demo_end: Option<(usize, u16)>,
    /// How the game plays (see [`Game::set_play_mode`]).
    play_mode: PlayMode,
    events: Events,
    screen: Screen,
    pending_talk: Option<(Talk, u32)>,
    exit_taken: Option<usize>,
    /// The door the exit being taken is, when it is one.
    door_taken: Option<usize>,
    warped: Option<usize>,
    /// The map and exit whose portal the player last pushed toward.
    portal_exit: Option<(usize, usize)>,
    chest: Option<(usize, u16)>,
    /// A reward an event gives through the chest's ops (see
    /// [`Op::Gift`](crate::event::Op::Gift)).
    gift: Option<Reward>,
    effects: StoryEffects,
    /// How far the frame shown is brightened toward white.
    shown_whiten: u8,
    player_name: String,
    party: Party,
    state: Vec<u8>,
    /// The area whose objects the block keeps, and its formations.
    objects: AreaObjects,
    save: SaveFile,
    /// Where each save slot is kept; the original has one.
    slots: Vec<Box<dyn SaveStorage>>,
    /// The slot the game was continued from or last saved to.
    slot: Option<usize>,
    /// The title's list of slots, while the player chooses one to
    /// continue.
    slot_picker: Option<SlotPicker>,
    found: Option<Found>,
    previous: Input,
    latched: Input,
    /// The brightness the screen shows: the game writes the register from
    /// its level at the vertical blank, so a frame shows the level of the
    /// frame before, as it shows the field's sprites and scroll.
    shown_brightness: u8,
}

impl<'rom> Game<'rom> {
    /// Starts the game from the logo.
    ///
    /// # Errors
    ///
    /// Returns [`GameError`] when the boot data or text assets cannot be read.
    pub fn new(rom: &'rom [u8]) -> Result<Self, GameError> {
        let mut game = Self::bare(rom)?;
        game.screen = Screen::Logo(LogoScreen::new(&game.data)?);
        Ok(game)
    }

    /// Starts the game in the first room, skipping the opening.
    ///
    /// # Errors
    ///
    /// Returns [`GameError`] when the room or text assets cannot be read.
    pub fn in_first_room(rom: &'rom [u8]) -> Result<Self, GameError> {
        let mut game = Self::bare(rom)?;
        game.windows.set_flag(OPENING_SEEN_FLAG, true);
        game.enter_map(FIRST_ROOM_MAP, PLAYER_START)?;
        game.screen = Screen::Field;
        Self::play(
            &mut game.sound,
            &game.data,
            &game.extensions,
            GameSound::FirstRoomMusic,
        )?;
        Ok(game)
    }

    /// Keeps the save in `storage`, as the original's one save:
    /// continuing reads it and saving replaces it.
    pub fn set_save_storage(&mut self, storage: Box<dyn SaveStorage>) {
        self.slots = vec![storage];
    }

    /// Keeps the saves in `slots`, a port feature: with more than one,
    /// saving and continuing ask which slot to use (see
    /// [`crate::slots`]).
    pub fn set_save_slots(&mut self, slots: Vec<Box<dyn SaveStorage>>) {
        self.slots = slots;
    }

    /// The slot the game was continued from or last saved to.
    #[must_use]
    pub fn save_slot(&self) -> Option<usize> {
        self.slot
    }

    /// Ends the demo, a port feature, once the player walks freely on
    /// `map` with `flag` set: the game thanks the player, offers to save
    /// and goes back to the title (see [`crate::demo`]). `None` lets the
    /// game go on. By default the demo ends where the port's story does.
    pub fn set_demo_end(&mut self, end: Option<(usize, u16)>) {
        self.demo_end = end;
    }

    /// Plays as the original, or with the port's enhancements (see
    /// [`crate::play_mode`]). The launcher sets it before the game starts;
    /// by default the game plays as the original.
    pub fn set_play_mode(&mut self, mode: PlayMode) {
        self.play_mode = mode;
    }

    /// How the game plays.
    #[must_use]
    pub fn play_mode(&self) -> PlayMode {
        self.play_mode
    }

    /// Keeps the enhancements the pause menu hands back, in the enhanced
    /// mode, and tells the extensions when they changed.
    fn take_enhancements(&mut self, chosen: Option<Enhancements>) {
        let (PlayMode::Enhanced(current), Some(chosen)) = (self.play_mode, chosen) else {
            return;
        };
        if chosen != current {
            self.play_mode = PlayMode::Enhanced(chosen);
            Self::emit(&self.extensions, &Event::EnhancementsChanged(chosen));
        }
    }

    /// The extensions the game raises events to and asks questions of.
    #[must_use]
    pub fn extensions(&self) -> &SharedExtensions {
        &self.extensions
    }

    fn emit(extensions: &SharedExtensions, event: &Event) {
        extensions.borrow_mut().emit(event);
    }

    /// Plays the song an extension, or else the data, assigns to `sound`.
    fn play(
        engine: &mut SoundEngine<'rom>,
        data: &GameData<'rom>,
        extensions: &SharedExtensions,
        sound: GameSound,
    ) -> Result<(), GameError> {
        let song = extensions
            .borrow()
            .sound_for(sound)
            .unwrap_or_else(|| data.sound(sound));
        Self::emit(extensions, &Event::SoundRequested(song));
        engine.play(song)?;
        Ok(())
    }

    fn bare(rom: &'rom [u8]) -> Result<Self, GameError> {
        let data = GameData::new(rom);
        let text_error = |error: &dyn std::fmt::Display| GameError::Text(error.to_string());
        let (glyphs, fallback) = data.font().map_err(|error| text_error(&error))?;
        let skin = data.window_skin().map_err(|error| text_error(&error))?;
        let dialogue = data
            .script_offsets(DIALOGUE_TABLE)
            .map_err(|error| text_error(&error))?
            .ok_or_else(|| GameError::Text("no dialogue table".to_owned()))?;
        let extensions = SharedExtensions::default();
        let save = SaveFile::new(data.save_layout()?);
        let state = data.new_game_state()?;
        Ok(Self {
            data,
            extensions: extensions.clone(),
            frame: 0,
            painter: TextPainter::new(rom, glyphs, Some(fallback)),
            skin: WindowPainter::new(skin.tiles, &skin.palette),
            windows: {
                let mut windows = ScriptWindows::new(rom, DEFAULT_PLAYER_NAME);
                windows.set_metrics(TextMetrics::standard());
                windows.set_extensions(extensions);
                windows
            },
            sound: SoundEngine::new(data.bytes(), data.sound_layout()),
            dialogue: ScriptRunner::named(DIALOGUE_TABLE, dialogue),
            scripts: Vec::new(),
            active_script: None,
            last_runner: None,
            battle: None,
            shop: None,
            combat: None,
            encounter: None,
            aftermath: None,
            battle_lost: false,
            defeated: false,
            field: None,
            debug: false,
            demo_end: Some(story::DEMO_END),
            play_mode: PlayMode::default(),
            events: Events::new(),
            screen: Screen::Loading,
            pending_talk: None,
            exit_taken: None,
            door_taken: None,
            warped: None,
            portal_exit: None,
            chest: None,
            gift: None,
            effects: StoryEffects::default(),
            shown_whiten: 0,
            player_name: DEFAULT_PLAYER_NAME.to_owned(),
            party: Party::default(),
            state,
            objects: AreaObjects::default(),
            save,
            slots: Vec::new(),
            slot: None,
            slot_picker: None,
            found: None,
            previous: Input::default(),
            latched: Input::default(),
            shown_brightness: 0,
        })
    }

    /// Where the game is.
    #[must_use]
    pub fn stage(&self) -> Stage {
        match self.screen {
            Screen::Logo(_) => Stage::Logo,
            Screen::Title(_) => Stage::Title,
            Screen::NameEntry(_) => Stage::NameEntry,
            Screen::Loading | Screen::LeavingNameEntry(..) => Stage::Loading,
            Screen::Field | Screen::OpeningMenu(_) | Screen::DemoEnd(_) => Stage::Field,
            Screen::Menu(_) => Stage::Menu,
            Screen::Continuing(_) => Stage::Continuing,
            Screen::Guide(_) => Stage::Guide,
        }
    }

    /// The field, once the room is loaded.
    #[must_use]
    pub fn field(&self) -> Option<&Field> {
        self.field.as_ref()
    }

    /// The script windows on screen.
    #[must_use]
    pub fn windows(&self) -> &ScriptWindows<'rom> {
        &self.windows
    }

    /// The brightness events set: 0 normal, 16 and above black.
    #[must_use]
    pub fn brightness(&self) -> u8 {
        self.events.brightness()
    }

    /// The game-state block as the game holds it, the one a save keeps
    /// (see `docs/formats/save.md`).
    #[must_use]
    pub fn state(&self) -> &[u8] {
        &self.state
    }

    /// The player's name.
    #[must_use]
    pub fn player_name(&self) -> &str {
        &self.player_name
    }

    /// Keeps what the field screen shows this frame: the original copies its
    /// sprite table, scroll, text layers and brightness at the vertical
    /// blank, so a frame shows them as the frame before left them. The
    /// field or battle just set up takes the debugging mode and the play
    /// mode's enhancements too.
    fn latch_screen(&mut self) {
        self.latch_debug_mode();
        self.latch_enhancements();
        if let Some(field) = self.field.as_mut() {
            field.latch();
        }
        if let Some(combat) = self.combat.as_mut() {
            combat.latch();
        }
        if let Some(stage) = self.battle.as_mut() {
            stage.latch();
        }
        self.windows.latch();
        self.shown_brightness = self.events.brightness();
        self.shown_whiten = self.effects.whiten;
    }

    /// Turns the port's debugging mode on or off and says whether it is on
    /// now. While on, the roaming enemies are intangible (the player walks
    /// through them and no battle starts) and the protagonist's attacks
    /// always land and beat what they hurt, so a battle ends in a blow or
    /// two. The original has no such thing.
    pub fn toggle_debug_mode(&mut self) -> bool {
        self.debug = !self.debug;
        self.latch_debug_mode();
        self.debug
    }

    /// Hands the enhancements that apply to the battle on screen.
    fn latch_enhancements(&mut self) {
        let enhancements = self.play_mode.enhancements();
        if let Some(combat) = self.combat.as_mut() {
            combat.set_attack_scenes(enhancements.battle_animations);
            combat.set_damage_numbers(enhancements.damage_numbers);
        }
    }

    /// Hands the debugging mode to the field and the battle on screen.
    fn latch_debug_mode(&mut self) {
        if let Some(field) = self.field.as_mut() {
            field.set_intangible(self.debug);
            field.set_calm(self.effects.calm);
        }
        if let Some(combat) = self.combat.as_mut() {
            combat.set_overpowered(self.debug);
        }
    }

    /// Advances one frame. The game acts on the buttons of the previous
    /// call, as the original reads the keys at the vertical blank before
    /// the frame's logic.
    ///
    /// # Errors
    ///
    /// Returns [`GameError`] when a screen or script fails.
    pub fn update(&mut self, input: Input) -> Result<(), GameError> {
        let input = std::mem::replace(&mut self.latched, input);
        let start = input.is_held(Button::Start) && !self.previous.is_held(Button::Start);
        self.previous = input;
        self.frame += 1;
        Self::emit(&self.extensions, &Event::Frame(self.frame));
        self.latch_screen();
        let rom = self.data.bytes();
        match &mut self.screen {
            Screen::Logo(logo) => {
                if logo.update() {
                    self.screen = Screen::Title(TitleScreen::new(&self.data)?);
                    Self::emit(&self.extensions, &Event::TitleShown);
                    Self::play(
                        &mut self.sound,
                        &self.data,
                        &self.extensions,
                        GameSound::TitleMusic,
                    )?;
                }
            }
            Screen::Title(_) => self.update_title(input, start)?,
            Screen::NameEntry(_) | Screen::LeavingNameEntry(..) => self.update_name_entry(input)?,
            Screen::Loading => {}
            Screen::Field => {
                if self.demo_ends_here() {
                    self.begin_demo_end(input);
                } else if start && self.player_in_control() {
                    self.screen = Screen::OpeningMenu(0);
                } else {
                    self.update_field(input)?;
                }
            }
            Screen::DemoEnd(_) => self.update_demo_end(input)?,
            Screen::OpeningMenu(frames) => {
                if let Some(field) = &mut self.field {
                    field.update(Input::default());
                }
                let frames = *frames + 1;
                self.screen = Screen::OpeningMenu(frames);
                if frames >= MENU_OPEN_FRAMES {
                    let mut menu =
                        PauseMenu::new(&self.data, self.party.clone(), self.state.clone())?;
                    self.offer_save_slots(&mut menu);
                    menu.set_enhancements(match self.play_mode {
                        PlayMode::Classic => None,
                        PlayMode::Enhanced(enhancements) => Some(enhancements),
                    });
                    menu.open(rom, &mut self.windows)?;
                    self.screen = Screen::Menu(Box::new(menu));
                    Self::emit(&self.extensions, &Event::MenuOpened);
                }
            }
            Screen::Menu(menu) => match menu.update(rom, input, &mut self.windows)? {
                MenuStep::Open => {}
                MenuStep::Save => {
                    let party = menu.party();
                    let slot = menu.save_slot();
                    self.state.clone_from_slice(menu.state());
                    let written = self.write_save(&party, slot);
                    let mut screen = std::mem::replace(&mut self.screen, Screen::Loading);
                    if let Screen::Menu(menu) = &mut screen {
                        self.offer_save_slots(menu);
                        menu.finish_save(written)?;
                    }
                    self.screen = screen;
                }
                MenuStep::Closed => {
                    self.party = menu.party();
                    self.state.clone_from_slice(menu.state());
                    let enhancements = menu.enhancements();
                    self.screen = Screen::Field;
                    self.take_enhancements(enhancements);
                    self.events.set_brightness(BLACK);
                    self.events.fade_in_after(MENU_RETURN_BLACK_FRAMES, 0);
                    if let Some(field) = &mut self.field {
                        field.restart_animations(MENU_RETURN_ANIMATION);
                    }
                    Self::emit(&self.extensions, &Event::MenuClosed);
                }
            },
            Screen::Continuing(_) => self.update_continue(input)?,
            Screen::Guide(guide) => {
                guide.update(&self.data, input, &mut self.windows)?;
                if guide.is_closed() {
                    self.windows.close_window(None);
                    self.screen = Screen::Title(TitleScreen::new(&self.data)?);
                    Self::emit(&self.extensions, &Event::TitleShown);
                }
            }
        }
        for sound in self.windows.take_sounds() {
            self.sound.play(usize::from(sound))?;
        }
        self.sound.frame()?;
        Ok(())
    }

    /// Shows the messages `translation` covers in place of the ROM's text.
    /// Fits the windows to it first; returns the messages no window can
    /// hold. A player who still carries the default name takes its Latin
    /// form, [`TRANSLATED_PLAYER_NAME`], which the name entry then offers.
    ///
    /// # Errors
    ///
    /// Returns [`GameError`] when the script tables cannot be read.
    pub fn set_translation(
        &mut self,
        mut translation: Translation,
    ) -> Result<Vec<String>, GameError> {
        let problems = translation.fit(&self.data, self.painter.metrics())?;
        self.extensions
            .borrow_mut()
            .insert(Box::new(TranslationExtension::new(translation)));
        self.player_name = translated_name(&self.player_name).to_owned();
        self.windows.set_player_name(&self.player_name);
        Ok(problems)
    }

    /// The samples of the frame the last update produced, stereo
    /// interleaved at [`gba_runtime::apu::SAMPLE_RATE`].
    #[must_use]
    pub fn audio(&self) -> &[i16] {
        self.sound.output()
    }

    /// Runs the name entry, then holds and darkens it until the first room
    /// loads.
    fn update_name_entry(&mut self, input: Input) -> Result<(), GameError> {
        let rom = self.data.bytes();
        match &mut self.screen {
            Screen::NameEntry(entry) => {
                if !entry.update(rom, input, &mut self.windows)? {
                    return Ok(());
                }
                self.player_name = entry.name();
                self.windows.set_player_name(&self.player_name);
                self.windows.close_window(None);
                Self::emit(
                    &self.extensions,
                    &Event::NameConfirmed(self.player_name.clone()),
                );
                let screen = std::mem::replace(&mut self.screen, Screen::Loading);
                if let Screen::NameEntry(entry) = screen {
                    self.screen = Screen::LeavingNameEntry(Box::new(entry), 0);
                }
            }
            Screen::LeavingNameEntry(_, frames) => {
                *frames += 1;
                if *frames == NAME_MUSIC_STOP_FRAMES {
                    self.sound.stop_music();
                }
                if *frames >= NAME_TO_ROOM_FRAMES {
                    self.start_new_game_room()?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// A frame of the title: its menu, or the port's list of save slots
    /// while the player chooses one to continue.
    fn update_title(&mut self, input: Input, start: bool) -> Result<(), GameError> {
        let rom = self.data.bytes();
        let Screen::Title(title) = &mut self.screen else {
            return Ok(());
        };
        if let Some(picker) = self.slot_picker.as_mut() {
            let pick = picker.update(rom, input, &mut self.windows)?;
            if let Some(pick) = pick {
                picker.close(&mut self.windows);
                self.slot_picker = None;
                match pick {
                    Pick::Slot(slot) => self.begin_continue(slot),
                    Pick::Canceled => title.reopen_menu(input)?,
                }
            }
            return Ok(());
        }
        if start && !self.windows.any_open() {
            Self::play(
                &mut self.sound,
                &self.data,
                &self.extensions,
                GameSound::TitleStart,
            )?;
        }
        match title.update(rom, input, &mut self.windows)? {
            Some(TitleChoice::NewGame) => self.new_game(input)?,
            Some(TitleChoice::Continue) => self.choose_continue(input),
            Some(TitleChoice::ZoidGuide) => self.open_guide(GuideKind::Zoids)?,
            Some(TitleChoice::CharacterGuide) => self.open_guide(GuideKind::Characters)?,
            _ => {}
        }
        Ok(())
    }

    fn new_game(&mut self, input: Input) -> Result<(), GameError> {
        self.state = self.data.new_game_state()?;
        self.slot = None;
        self.party = Party::default();
        self.windows.set_flags([]);
        let mut entry = NameEntry::new(&self.data, &self.player_name, input)?;
        entry.open(&mut self.windows);
        self.screen = Screen::NameEntry(entry);
        Self::play(
            &mut self.sound,
            &self.data,
            &self.extensions,
            GameSound::NameEntryMusic,
        )?;
        Ok(())
    }

    /// Leaves the title for a guide, which reads what the player has seen
    /// from the save, or from a new game's state when there is none.
    fn open_guide(&mut self, kind: GuideKind) -> Result<(), GameError> {
        let contents = self.slot_contents();
        let found = match Self::latest_slot(&self.slots, &contents) {
            Some(slot) => self.read_slot(slot),
            None => Found::Missing,
        };
        let state = match found.game() {
            Some(saved) => saved.state.clone(),
            None => self.data.new_game_state()?,
        };
        let screen = std::mem::replace(&mut self.screen, Screen::Loading);
        let Screen::Title(title) = screen else {
            self.screen = screen;
            return Ok(());
        };
        self.windows.close_window(None);
        self.screen = Screen::Guide(Box::new(Guide::new(
            &self.data,
            kind,
            state,
            Cover::Title(Box::new(title)),
        )?));
        Ok(())
    }

    /// つづきから: with several save slots of which one holds anything,
    /// the list to choose from, the cursor on the latest game; otherwise
    /// the one slot, as the original.
    fn choose_continue(&mut self, held: Input) {
        let contents = self.slot_contents();
        if self.slots.len() < 2 || contents.iter().all(|slot| *slot == Slot::Empty) {
            self.begin_continue(0);
            return;
        }
        let line = Self::latest_slot(&self.slots, &contents).unwrap_or(0);
        self.windows.close_window(None);
        let mut picker = SlotPicker::new(contents, Purpose::Load, slots::TITLE_LAYOUT, line);
        picker.open(held, &mut self.windows);
        self.slot_picker = Some(picker);
    }

    /// Leaves the title for the game saved in `slot`: reads the save now
    /// and starts the fade.
    fn begin_continue(&mut self, slot: usize) {
        Self::emit(&self.extensions, &Event::LoadRequested);
        self.found = Some(self.read_slot(slot));
        self.slot = Some(slot);
        let screen = std::mem::replace(&mut self.screen, Screen::Loading);
        let Screen::Title(title) = screen else {
            self.screen = screen;
            return;
        };
        self.windows.close_window(None);
        self.sound.stop_music();
        self.screen = Screen::Continuing(Continuing {
            title,
            phase: ContinuePhase::Leaving(0),
        });
    }

    fn update_continue(&mut self, input: Input) -> Result<(), GameError> {
        let Screen::Continuing(continuing) = &mut self.screen else {
            return Ok(());
        };
        let notice = self.found.as_ref().and_then(Found::notice);
        match continuing.phase {
            ContinuePhase::Leaving(frames) => {
                let frames = frames + 1;
                continuing.phase = ContinuePhase::Leaving(frames);
                match notice {
                    Some(notice) if frames >= CONTINUE_NOTICE_FRAMES => {
                        start_dialogue(
                            &mut self.dialogue,
                            &self.scripts,
                            &mut self.last_runner,
                            (notice, false),
                        )?;
                        continuing.phase = ContinuePhase::Notice;
                    }
                    None if frames >= CONTINUE_LOAD_FRAMES => self.resume()?,
                    _ => {}
                }
            }
            ContinuePhase::Notice => {
                if !self
                    .dialogue
                    .update(self.data.bytes(), input, &mut self.windows)?
                {
                    return Ok(());
                }
                if self.found.as_ref().and_then(Found::game).is_some() {
                    continuing.phase = ContinuePhase::AfterNotice(0);
                } else {
                    self.resume()?;
                }
            }
            ContinuePhase::AfterNotice(frames) => {
                if frames + 1 >= CONTINUE_AFTER_NOTICE_FRAMES {
                    self.resume()?;
                } else {
                    continuing.phase = ContinuePhase::AfterNotice(frames + 1);
                }
            }
        }
        Ok(())
    }

    /// Puts the loaded game in place and brightens its room like any map
    /// entered, or goes back to the title when there is none.
    fn resume(&mut self) -> Result<(), GameError> {
        let saved = self.found.take().as_ref().and_then(Found::game).cloned();
        let Some(progress) = saved.and_then(|saved| self.restore(&saved)) else {
            self.screen = Screen::Title(TitleScreen::new(&self.data)?);
            Self::emit(&self.extensions, &Event::TitleShown);
            return Self::play(
                &mut self.sound,
                &self.data,
                &self.extensions,
                GameSound::TitleMusic,
            );
        };
        let map = usize::from(progress.map);
        let cell = (usize::from(progress.column), usize::from(progress.row));
        self.objects.forget();
        if !self.windows.flag(OPENING_SEEN_FLAG) && map == FIRST_ROOM_MAP {
            return self.start_new_game_room();
        }
        // The fade in is set before the map's handler runs, so a load the
        // handler makes holds it back. The frame shows black already: its
        // level was latched while the title was still dark.
        self.events.set_brightness(BLACK);
        self.shown_brightness = BLACK;
        self.events.fade_in_holding();
        self.enter_map(map, cell)?;
        let song = Some(usize::from(progress.song))
            .filter(|song| *song != 0)
            .or_else(|| self.extensions.borrow().music_for_map(map))
            .or_else(|| self.data.map_music(map));
        if let Some(song) = song {
            Self::emit(&self.extensions, &Event::SoundRequested(song));
            self.sound.play(song)?;
        }
        self.screen = Screen::Field;
        Ok(())
    }

    /// Takes the saved block, name, party and flags; returns the block's
    /// fields, or `None` when they cannot be read.
    fn restore(&mut self, saved: &SavedGame) -> Option<Progress> {
        let progress = saved.progress()?;
        self.state.clone_from(&saved.state);
        self.player_name.clone_from(&saved.player_name);
        self.windows.set_player_name(&self.player_name);
        self.windows.set_flags(progress.set_flags());
        self.party = Party {
            level: u32::from(progress.level),
            experience: progress.experience,
            money: progress.money,
            message_speed: u16::from(progress.message_speed),
        };
        Some(progress)
    }

    /// The save memory of `slot`, as continuing finds it; a slot that
    /// cannot be read is reported and taken as empty.
    fn read_slot(&self, slot: usize) -> Found {
        let image = match self.slots.get(slot).map(|storage| storage.load()) {
            Some(Ok(image)) => image,
            Some(Err(error)) => {
                Self::emit(&self.extensions, &Event::StorageFailed(error.to_string()));
                None
            }
            None => None,
        };
        self.save.read(image)
    }

    /// What every save slot holds; a game's area is its map record's,
    /// which is right even in a block made by hand whose area byte was
    /// never written.
    fn slot_contents(&self) -> Vec<Slot> {
        (0..self.slots.len())
            .map(|slot| {
                let found = self.read_slot(slot);
                let mut content = Slot::from_found(&found);
                let record = found
                    .game()
                    .and_then(SavedGame::progress)
                    .and_then(|progress| self.data.map_record(usize::from(progress.map)).ok());
                if let (Slot::Game(summary), Some(record)) = (&mut content, record) {
                    summary.area = record.id.to_le_bytes()[0];
                }
                content
            })
            .collect()
    }

    /// The slot with the game saved last (see [`slots::latest`]).
    fn latest_slot(storages: &[Box<dyn SaveStorage>], contents: &[Slot]) -> Option<usize> {
        let times: Vec<_> = storages.iter().map(|storage| storage.modified()).collect();
        slots::latest(&times, contents)
    }

    /// The slot the pause menu's list starts on: the game's own, else the
    /// first empty one, else the latest game.
    fn default_save_slot(&self, contents: &[Slot]) -> usize {
        self.slot
            .filter(|slot| *slot < contents.len())
            .or_else(|| contents.iter().position(|slot| *slot == Slot::Empty))
            .or_else(|| Self::latest_slot(&self.slots, contents))
            .unwrap_or(0)
    }

    /// Hands the pause menu the slots to choose from when there are
    /// several.
    fn offer_save_slots(&self, menu: &mut PauseMenu) {
        if self.slots.len() > 1 {
            let contents = self.slot_contents();
            let line = self.default_save_slot(&contents);
            menu.set_save_slots(contents, line);
        }
    }

    /// Writes the game as it stands, with `party` from the menu, into save
    /// slot `slot`; returns whether it was stored.
    fn write_save(&mut self, party: &Party, slot: usize) -> bool {
        Self::emit(&self.extensions, &Event::SaveRequested);
        if !self.update_state(party) {
            return false;
        }
        let Some(storage) = self.slots.get_mut(slot) else {
            return false;
        };
        let failed = |extensions: &SharedExtensions, error: &dyn std::fmt::Display| {
            Self::emit(extensions, &Event::StorageFailed(error.to_string()));
            false
        };
        let previous = match storage.load() {
            Ok(previous) => previous,
            Err(error) => {
                failed(&self.extensions, &error);
                None
            }
        };
        let image = match self.save.write(previous, &self.state, &self.player_name) {
            Ok(image) => image,
            Err(error) => return failed(&self.extensions, &error),
        };
        match storage.store(&image) {
            Ok(()) => {
                self.slot = Some(slot);
                true
            }
            Err(error) => failed(&self.extensions, &error),
        }
    }

    /// Writes what the port models into the game-state block.
    fn update_state(&mut self, party: &Party) -> bool {
        let (Some(field), Ok(mut progress)) = (&self.field, Progress::read(&self.state)) else {
            return false;
        };
        let half = |value: usize| u16::try_from(value).unwrap_or(u16::MAX);
        progress.map = half(field.map());
        progress.column = half(field.player().column);
        progress.row = half(field.player().row);
        progress.flags = [0; FLAG_WORDS];
        for flag in self.windows.flags() {
            progress.set_flag(flag, true);
        }
        progress.level = u8::try_from(party.level).unwrap_or(u8::MAX);
        progress.experience = party.experience;
        progress.money = party.money;
        progress.message_speed = u8::try_from(party.message_speed).unwrap_or(u8::MAX);
        progress.name = encode_name(&self.player_name).0;
        if let Some(song) = self.sound.playing(MUSIC_PLAYER) {
            progress.song = half(song);
        }
        progress.write(&mut self.state).is_ok()
    }

    /// Enters the first room of a new game from black: the room's handler
    /// starts the opening, and the room brightens while the game holds.
    fn start_new_game_room(&mut self) -> Result<(), GameError> {
        self.enter_map(FIRST_ROOM_MAP, PLAYER_START)?;
        self.play_map_music(FIRST_ROOM_MAP)?;
        self.events.set_brightness(BLACK);
        self.events.fade_in_holding();
        self.screen = Screen::Field;
        Ok(())
    }

    /// Loads map `map` with the player on `cell` and runs the map's handler.
    fn enter_map(&mut self, map: usize, cell: (usize, usize)) -> Result<(), GameError> {
        self.objects
            .enter(&self.data, &mut self.state, map, frame_counter(self.frame));
        let mut field = Field::load(&self.data, map, cell)?;
        AreaObjects::place(&self.data, &self.state, map, &mut field)?;
        field.show_party_zoids(&self.data, &self.state)?;
        show_opened_chests(&mut field, &self.windows);
        self.field = Some(field);
        Self::emit(&self.extensions, &Event::RoomEntered { map, cell });
        self.run_handler(story::map_handler(map))
    }

    /// Runs a map's handler once the map is loaded. Every handler of the
    /// original starts with `0x0800BEE4` or `0x0800802C`, which clear the
    /// field's state halfword (`0x02000008`), so the enemies meet the party
    /// again on the next map.
    fn run_handler(&mut self, handler: Option<&'static [Op]>) -> Result<(), GameError> {
        self.effects.calm = false;
        let Some(handler) = handler else {
            return Ok(());
        };
        let mut events = std::mem::take(&mut self.events);
        let mut host = self.host();
        events.run_now(handler, &mut host);
        let result = host.finish();
        self.events = events;
        result
    }

    fn host(&mut self) -> Host<'_, 'rom> {
        Host {
            data: self.data,
            field: &mut self.field,
            windows: &mut self.windows,
            dialogue: &mut self.dialogue,
            scripts: &mut self.scripts,
            active_script: &mut self.active_script,
            last_runner: &mut self.last_runner,
            battle: &mut self.battle,
            shop: &mut self.shop,
            combat: &mut self.combat,
            encounter: self.encounter,
            aftermath: &mut self.aftermath,
            battle_lost: self.battle_lost,
            warped: &mut self.warped,
            portal_exit: self.portal_exit,
            chest: self.chest,
            gift: &mut self.gift,
            effects: &mut self.effects,
            party: &mut self.party,
            sound: &mut self.sound,
            extensions: &self.extensions,
            state: &mut self.state,
            objects: &mut self.objects,
            frame: frame_counter(self.frame),
            error: None,
        }
    }

    /// Whether the demo ends now: the player walks freely, in full light,
    /// on its last map with its flag set.
    fn demo_ends_here(&self) -> bool {
        self.demo_end.is_some_and(|(map, flag)| {
            self.windows.flag(flag)
                && self.field.as_ref().is_some_and(|field| field.map() == map)
                && self.events.brightness() == 0
                && self.player_in_control()
        })
    }

    /// Starts the end of the demo over the field.
    fn begin_demo_end(&mut self, input: Input) {
        let offsets = self
            .data
            .script_offsets(PAUSE_MENU_TABLE)
            .ok()
            .flatten()
            .unwrap_or_default();
        let (contents, line) = if self.slots.len() > 1 {
            let contents = self.slot_contents();
            let line = self.default_save_slot(&contents);
            (contents, line)
        } else {
            (Vec::new(), 0)
        };
        let mut demo = DemoEnd::new(offsets, contents, line);
        demo.open(input);
        self.screen = Screen::DemoEnd(Box::new(demo));
    }

    /// A frame of the end of the demo: the field goes on moving behind it,
    /// the game is saved when asked, and the title follows the fade.
    fn update_demo_end(&mut self, input: Input) -> Result<(), GameError> {
        if let Some(field) = &mut self.field {
            field.update(Input::default());
        }
        let rom = self.data.bytes();
        let Screen::DemoEnd(demo) = &mut self.screen else {
            return Ok(());
        };
        match demo.update(rom, input, &mut self.windows)? {
            DemoStep::Continue => {}
            DemoStep::Save(slot) => {
                let party = self.party.clone();
                let written = self.write_save(&party, slot);
                if let Screen::DemoEnd(demo) = &mut self.screen {
                    demo.saved(written, &mut self.windows);
                }
            }
            DemoStep::Leaving => self.sound.stop_music(),
            DemoStep::Title => {
                self.windows.close_window(None);
                self.field = None;
                self.screen = Screen::Title(TitleScreen::new(&self.data)?);
                Self::emit(&self.extensions, &Event::TitleShown);
                Self::play(
                    &mut self.sound,
                    &self.data,
                    &self.extensions,
                    GameSound::TitleMusic,
                )?;
            }
        }
        Ok(())
    }

    /// Whether the player walks freely: no conversation, event hold or
    /// talk about to start, and the player's own command in force.
    fn player_in_control(&self) -> bool {
        self.dialogue.is_done()
            && self.pending_talk.is_none()
            && !self.events.holding()
            && self
                .field
                .as_ref()
                .is_some_and(|field| field.player().command == Command::Player)
    }

    /// A frame of the battle scene an event holds the game for; its end
    /// lets the event go on.
    fn update_battle(&mut self, input: Input) -> Result<(), GameError> {
        let done = match self.battle.as_mut() {
            Some(stage) => {
                stage.update(&self.data, input, &mut self.windows)?;
                for sound in stage.take_sounds() {
                    Self::emit(&self.extensions, &Event::SoundRequested(usize::from(sound)));
                    self.sound.play(usize::from(sound))?;
                }
                stage.is_done()
            }
            None => true,
        };
        if done {
            self.battle = None;
        }
        self.update_events(|events, host| {
            events.update_hold(done, host);
        })
    }

    /// A frame of the shop an event holds the game for; once it closes,
    /// the party and the game state it changed are kept and the event goes
    /// on.
    fn update_shop(&mut self, input: Input) -> Result<(), GameError> {
        let closed = match self.shop.as_mut() {
            Some(shop) => {
                shop.update(self.data.bytes(), input, &mut self.windows)? == MenuStep::Closed
            }
            None => true,
        };
        if closed {
            if let Some(shop) = self.shop.take() {
                self.party = shop.party();
                self.state.clone_from_slice(shop.state());
                if matches!(shop.shop_kind(), Some(Shop::Lab(_))) {
                    extraction::saga_party::heal_all(&mut self.state);
                }
                Self::emit(&self.extensions, &Event::ShopClosed);
            }
        }
        self.update_events(|events, host| {
            events.update_hold(closed, host);
        })
    }

    /// A frame of the scripts an event holds the game for. A handler the
    /// game called directly runs its scripts one after another within the
    /// call, so the next one starts in the frame the last one ends.
    fn update_dialogue_hold(&mut self, input: Input) -> Result<(), GameError> {
        loop {
            let runner = match self.active_script {
                Some(index) => &mut self.scripts[index],
                None => &mut self.dialogue,
            };
            let done = runner.update(self.data.bytes(), input, &mut self.windows)?;
            if done {
                self.active_script = None;
            }
            self.update_events(|events, host| {
                events.update_hold(done, host);
            })?;
            if !(done && self.events.in_dialogue()) {
                return Ok(());
            }
        }
    }

    /// A frame on the field. While a chest is searched no actor moves,
    /// animates or reads the keys: the search waits within the player's
    /// update (`0x0800B938`) until the reward.
    fn update_field(&mut self, input: Input) -> Result<(), GameError> {
        if self.field.is_none() {
            return Ok(());
        }
        if self.events.in_battle() {
            return self.update_battle(input);
        }
        if self.events.in_shop() {
            return self.update_shop(input);
        }
        if self.events.in_combat() {
            return self.update_combat(input);
        }
        if self.events.in_dialogue() {
            if self.events.dialogue_in_task()
                && let Some(field) = self.field.as_mut()
            {
                field.update(input);
            }
            return self.update_dialogue_hold(input);
        }
        if self.events.holding() {
            let mut step = HoldStep::Free;
            self.update_events(|events, host| step = events.update_hold(false, host))?;
            match step {
                HoldStep::Held => return Ok(()),
                HoldStep::Darkened => {
                    if std::mem::take(&mut self.defeated) {
                        return self.return_home();
                    }
                    if let Some(exit) = self.exit_taken.take() {
                        let black = if self.door_taken.take().is_some() {
                            DOOR_BLACK_FRAMES
                        } else {
                            WARP_BLACK_FRAMES
                        };
                        self.warp(exit, black)?;
                    }
                    return Ok(());
                }
                HoldStep::Free | HoldStep::Released => {}
            }
        }
        if let Some((talk, delay)) = self.pending_talk.take() {
            if delay > 1 {
                self.pending_talk = Some((talk, delay - 1));
                return Ok(());
            }
            return match talk {
                Talk::Dialogue(id) => Ok(start_dialogue(
                    &mut self.dialogue,
                    &self.scripts,
                    &mut self.last_runner,
                    (id, false),
                )?),
                Talk::Event(program) => self.run_handler(Some(program)),
            };
        }
        if !self.dialogue.is_done() {
            self.dialogue
                .update(self.data.bytes(), input, &mut self.windows)?;
            return Ok(());
        }
        self.update_events(|events, host| events.update_watch(host))?;
        let event = if self.events.runs_handler(story::CHEST) {
            None
        } else {
            self.field.as_mut().and_then(|field| field.update(input))
        };
        if let Some(field) = self.field.as_mut() {
            AreaObjects::record(&mut self.state, field);
            for sound in field.take_sounds() {
                Self::emit(&self.extensions, &Event::SoundRequested(usize::from(sound)));
                self.sound.play(usize::from(sound))?;
            }
        }
        if self.follow(event)? {
            return Ok(());
        }
        let talking = self.events.in_dialogue();
        self.update_events(|events, host| events.update(host))?;
        // A task's dialogue call runs the script's first step at once.
        if !talking && self.events.in_dialogue() && self.events.dialogue_in_task() {
            return self.update_dialogue_hold(input);
        }
        Ok(())
    }

    /// Follows what the field reported; whether the frame ends there.
    fn follow(&mut self, event: Option<FieldEvent>) -> Result<bool, GameError> {
        match event {
            Some(FieldEvent::Exit(exit)) => {
                self.take_exit(exit, false)?;
                return Ok(true);
            }
            Some(FieldEvent::Door(exit)) => {
                self.take_exit(exit, true)?;
                return Ok(true);
            }
            Some(FieldEvent::Portal(exit)) => {
                if let Some(field) = self.field.as_mut() {
                    self.portal_exit = Some((field.map(), exit));
                    field.player_mut().command = Command::Idle;
                }
                self.events.spawn(story::PORTAL_TASK, story::PORTAL_TRIP);
            }
            Some(FieldEvent::Talk {
                actor,
                script: ObjectScript::Dialogue(id),
            }) => {
                let id = usize::from(id);
                Self::emit(
                    &self.extensions,
                    &Event::Talk {
                        npc: actor,
                        dialogue: id,
                    },
                );
                self.pending_talk = Some((Talk::Dialogue(id), TALK_START_DELAY));
            }
            Some(FieldEvent::Chest { actor, chest }) => {
                let flag = CHEST_FLAG_BASE + chest;
                if !self.windows.flag(flag) {
                    self.chest = Some((actor, chest));
                    self.pending_talk = Some((Talk::Event(story::CHEST), CHEST_START_DELAY));
                }
            }
            Some(FieldEvent::Talk {
                script: ObjectScript::Code(address),
                ..
            }) => {
                if let Some(program) = story::talk_handler(address) {
                    self.run_handler(Some(program))?;
                    return Ok(true);
                }
            }
            Some(FieldEvent::Encounter { enemy }) => {
                self.meet_enemy(enemy)?;
                return Ok(true);
            }
            Some(FieldEvent::Wrecked { actor: 0 }) => {
                self.defeated = true;
                self.events.fade_out_holding(DEFEAT_FADE_DELAY);
                return Ok(true);
            }
            Some(FieldEvent::Wrecked { .. }) | None => {}
        }
        Ok(false)
    }

    fn update_events(
        &mut self,
        run: impl FnOnce(&mut Events, &mut Host<'_, 'rom>),
    ) -> Result<(), GameError> {
        let mut events = std::mem::take(&mut self.events);
        let mut host = self.host();
        run(&mut events, &mut host);
        let result = host.finish();
        self.events = events;
        result?;
        if let Some(map) = self.warped.take() {
            let cell = self
                .field
                .as_ref()
                .map_or((0, 0), |field| (field.player().column, field.player().row));
            Self::emit(&self.extensions, &Event::RoomEntered { map, cell });
            self.play_map_music(map)?;
            self.events.end(FIELD_WATCH);
            self.run_handler(story::map_handler(map))?;
        }
        Ok(())
    }

    /// Follows `exit` once the screen is black: loads the destination, runs
    /// its handler and starts its song, then holds black and brightens.
    fn warp(&mut self, exit: usize, black: u8) -> Result<(), GameError> {
        let Some(field) = self.field.as_mut() else {
            return Ok(());
        };
        let from = field.map();
        let destination = self.data.warp(from, exit).map_err(FieldError::from)?.map;
        self.objects.enter(
            &self.data,
            &mut self.state,
            destination,
            frame_counter(self.frame),
        );
        let warp = field.warp(&self.data, exit)?;
        AreaObjects::place(&self.data, &self.state, destination, field)?;
        field.show_party_zoids(&self.data, &self.state)?;
        show_opened_chests(field, &self.windows);
        let arrived = field.map();
        Self::emit(
            &self.extensions,
            &Event::ExitTaken {
                map: from,
                exit,
                destination: warp.map,
            },
        );
        Self::emit(
            &self.extensions,
            &Event::RoomEntered {
                map: arrived,
                cell: (warp.column, warp.row),
            },
        );
        self.play_map_music(arrived)?;
        let black = if arrived == WORLD_MAP {
            black + WORLD_MAP_LOAD_FRAMES
        } else {
            black
        };
        self.events.fade_in_after(black, WARP_SETTLE_FRAMES);
        self.events.end(MAP_TASK);
        self.events.end(FIELD_HOOK);
        self.events.end(FIELD_WATCH);
        self.run_handler(story::map_handler(arrived))
    }

    /// The player and the roaming enemy `enemy` ran into each other: the
    /// field darkens and the battle starts (`0x0800B9CC`). Both stand still
    /// until its outcome takes hold (entity states 4 and 1).
    fn meet_enemy(&mut self, enemy: usize) -> Result<(), GameError> {
        self.encounter = Some(enemy);
        if let Some(field) = self.field.as_mut() {
            field.player_mut().command = Command::Idle;
            if let Some(actor) = field.actor_mut(enemy) {
                actor.pause = MEETING_PAUSE;
            }
        }
        self.run_handler(Some(story::ENCOUNTER))
    }

    /// A frame of the battle an event holds the game for; once it hands
    /// back, the enemy and the player take its outcome and the event goes
    /// on.
    fn update_combat(&mut self, input: Input) -> Result<(), GameError> {
        let outcome = match self.combat.as_mut() {
            Some(combat) => {
                combat.update(self.data.bytes(), input, &mut self.windows)?;
                if let Some(song) = combat.take_music() {
                    Self::emit(&self.extensions, &Event::SoundRequested(usize::from(song)));
                    self.sound.play(usize::from(song))?;
                }
                for sound in combat.take_sounds() {
                    Self::emit(&self.extensions, &Event::SoundRequested(usize::from(sound)));
                    self.sound.play(usize::from(sound))?;
                }
                combat.outcome()
            }
            None => Some(Outcome::Retreated),
        };
        let done = outcome.is_some();
        if let Some(outcome) = outcome {
            if let Some(combat) = self.combat.take()
                && combat.state().len() == self.state.len()
            {
                self.state.clone_from_slice(combat.state());
                take_party(&self.state, &mut self.party);
            }
            // The battle hands back as it queues the text system's reset,
            // which clears whatever its results left on the screen.
            self.windows.close_window(None);
            self.battle_lost = outcome == Outcome::Lost;
            self.end_encounter(outcome)?;
        }
        self.update_events(|events, host| {
            events.update_hold(done, host);
        })
    }

    /// A battle's end: the field reloads with the player and the enemy on
    /// their cells, their animations restarted (`0x0800B9CC`), the outcome
    /// waits for the field to be bright again ([`Op::AfterCombat`]), and
    /// the map's song plays again.
    fn end_encounter(&mut self, outcome: Outcome) -> Result<(), GameError> {
        Self::emit(&self.extensions, &Event::CombatEnded(outcome));
        if self.encounter.is_none() {
            // A story battle: its event goes on in the dark.
            return Ok(());
        }
        self.aftermath = self.encounter.take().map(|enemy| (enemy, outcome));
        if let (Some((enemy, _)), Some(field)) = (self.aftermath, self.field.as_mut()) {
            for index in [0, enemy] {
                if let Some(actor) = field.actor_mut(index) {
                    actor.place((actor.column, actor.row));
                    actor.face(actor.facing);
                }
            }
        }
        let map = self.field.as_ref().map(Field::map);
        if let Some(map) = map {
            self.play_map_music(map)?;
        }
        Ok(())
    }

    /// Takes the party, beaten, to its area's return point once the screen
    /// is black (`0x08006E08`, `0x08007188`): the player stands there facing
    /// up, and the map brightens after its load.
    fn return_home(&mut self) -> Result<(), GameError> {
        let index = AreaObjects::area_index(&self.state);
        let Some(point) = extraction::saga_encounter::return_point(self.data.bytes(), index) else {
            return Err(GameError::Text(format!("no return point {index}")));
        };
        self.objects.enter(
            &self.data,
            &mut self.state,
            point.map,
            frame_counter(self.frame),
        );
        let mut field = Field::load(&self.data, point.map, point.cell)?;
        AreaObjects::place(&self.data, &self.state, point.map, &mut field)?;
        field.show_party_zoids(&self.data, &self.state)?;
        field.player_mut().face(Direction::Up);
        show_opened_chests(&mut field, &self.windows);
        self.field = Some(field);
        Self::emit(
            &self.extensions,
            &Event::RoomEntered {
                map: point.map,
                cell: point.cell,
            },
        );
        self.play_map_music(point.map)?;
        self.events
            .fade_in_after(DEFEAT_BLACK_FRAMES, WARP_SETTLE_FRAMES);
        self.events.end(MAP_TASK);
        self.events.end(FIELD_HOOK);
        self.events.end(FIELD_WATCH);
        self.run_handler(story::map_handler(point.map))
    }

    /// Starts leaving by exit `exit`, a door when `door`: the screen
    /// darkens with the door's sound, then the warp loads the next map.
    fn take_exit(&mut self, exit: usize, door: bool) -> Result<(), GameError> {
        self.exit_taken = Some(exit);
        if door {
            self.door_taken = Some(exit);
            self.events.fade_out_holding(DOOR_FADE_DELAY);
            return self.play_door_sound(exit);
        }
        self.events.fade_out_holding(EXIT_FADE_DELAY);
        Self::play(
            &mut self.sound,
            &self.data,
            &self.extensions,
            GameSound::Door,
        )
    }

    /// The sound a door plays (`0x080083B8`): its warp's own, none for
    /// `0x44`, or by default `0x45` when the player is on foot (sprite
    /// `0x98`) and the door sound otherwise.
    fn play_door_sound(&mut self, exit: usize) -> Result<(), GameError> {
        let Some(map) = self.field.as_ref().map(Field::map) else {
            return Ok(());
        };
        Self::door_sound(&mut self.sound, &self.data, &self.extensions, map, exit)
    }

    /// The sound exit `exit` of map `map` plays (`0x080083B8`).
    fn door_sound(
        engine: &mut SoundEngine<'rom>,
        data: &GameData<'rom>,
        extensions: &SharedExtensions,
        map: usize,
        exit: usize,
    ) -> Result<(), GameError> {
        let Ok(warp) = data.warp(map, exit) else {
            return Ok(());
        };
        let on_foot = data
            .map_objects(map)
            .ok()
            .and_then(|objects| objects.first().map(|player| player.sprite))
            == Some(ON_FOOT_SPRITE);
        let sound = match warp.sound {
            NO_DOOR_SOUND => return Ok(()),
            0 if on_foot => usize::from(ON_FOOT_DOOR_SOUND),
            0 => return Self::play(engine, data, extensions, GameSound::Door),
            sound => usize::from(sound),
        };
        Self::emit(extensions, &Event::SoundRequested(sound));
        engine.play(sound)?;
        Ok(())
    }

    /// Starts the song map `map` names unless it is already playing.
    fn play_map_music(&mut self, map: usize) -> Result<(), GameError> {
        let music = self
            .extensions
            .borrow()
            .music_for_map(map)
            .or_else(|| self.data.map_music(map));
        if let Some(music) = music {
            Self::emit(&self.extensions, &Event::SoundRequested(music));
            self.sound.play_if_changed(music)?;
        }
        Ok(())
    }

    /// Draws the current frame.
    pub fn draw(&self, frame: &mut Frame) {
        match &self.screen {
            Screen::Logo(logo) => logo.draw(frame),
            Screen::Title(title) => {
                title.draw(frame);
                self.windows.draw(frame, &self.skin, &self.painter);
            }
            Screen::NameEntry(entry) => entry.draw(frame, &self.windows, &self.skin, &self.painter),
            Screen::Loading => {
                frame.fill(Rgb::default());
                darken(frame, FADE_STEPS);
            }
            Screen::LeavingNameEntry(entry, frames) => {
                entry.draw(frame, &self.windows, &self.skin, &self.painter);
                let level = frames.saturating_sub(NAME_HOLD_FRAMES);
                darken(frame, u8::try_from(level).unwrap_or(BLACK));
            }
            Screen::Field if self.shop.is_some() => {
                if let Some(shop) = &self.shop {
                    shop.draw(frame, &self.windows, &self.skin, &self.painter);
                }
            }
            Screen::Field if self.combat.is_some() => {
                if let Some(combat) = &self.combat {
                    combat.draw(frame, &self.windows, &self.skin, &self.painter);
                }
            }
            Screen::Field if self.battle.is_some() => {
                if let Some(stage) = &self.battle {
                    stage.draw(frame, &self.windows, &self.skin, &self.painter);
                }
            }
            Screen::Field => {
                if let Some(field) = &self.field {
                    field.draw(frame);
                }
                whiten(frame, self.shown_whiten);
                self.windows.draw_shown(frame, &self.skin, &self.painter);
                darken(frame, self.shown_brightness);
            }
            Screen::OpeningMenu(frames) => {
                if let Some(field) = &self.field {
                    field.draw(frame);
                }
                self.windows.draw(frame, &self.skin, &self.painter);
                let level = frames.saturating_sub(MENU_OPEN_DELAY).min(u32::from(BLACK));
                darken(frame, u8::try_from(level).unwrap_or(BLACK));
            }
            Screen::Menu(menu) => menu.draw(frame, &self.windows, &self.skin, &self.painter),
            Screen::Guide(guide) => guide.draw(frame, &self.windows, &self.skin, &self.painter),
            Screen::DemoEnd(demo) => {
                if let Some(field) = &self.field {
                    field.draw(frame);
                }
                self.windows.draw(frame, &self.skin, &self.painter);
                darken(frame, demo.darkness());
            }
            Screen::Continuing(continuing) => {
                let darkness = continuing.darkness();
                if darkness < FADE_STEPS {
                    continuing.title.draw(frame);
                } else {
                    frame.fill(Rgb::default());
                }
                darken(frame, darkness);
                self.windows.draw(frame, &self.skin, &self.painter);
            }
        }
    }
}

/// The screen size the game draws at.
#[must_use]
pub const fn screen_size() -> (usize, usize) {
    (SCREEN_WIDTH, SCREEN_HEIGHT)
}

/// The frame counter the random draws mix in, as the game's 16-bit one.
fn frame_counter(frame: u64) -> u16 {
    u16::try_from(frame & u64::from(u16::MAX)).unwrap_or(0)
}

/// Shows the chests already opened open, as the game does from their flags.
fn show_opened_chests(field: &mut Field, windows: &ScriptWindows<'_>) {
    for actor in &mut field.actors {
        if let Some(chest) = actor.chest
            && windows.flag(CHEST_FLAG_BASE + chest)
        {
            actor.play(CHEST_OPEN_ANIMATION);
        }
    }
}

/// Writes the party's level, experience and money into the game-state
/// block, where a battle reads them and adds its winnings.
/// The name the player carries once a translation is set: the default
/// becomes [`TRANSLATED_PLAYER_NAME`], a name entered or loaded stays.
fn translated_name(name: &str) -> &str {
    if name == DEFAULT_PLAYER_NAME {
        TRANSLATED_PLAYER_NAME
    } else {
        name
    }
}

fn store_party(state: &mut [u8], party: &Party) {
    if let Ok(mut progress) = Progress::read(state) {
        progress.level = u8::try_from(party.level).unwrap_or(u8::MAX);
        progress.experience = party.experience;
        progress.money = party.money;
        let _ = progress.write(state);
    }
}

/// Takes the level, experience and money a battle left in the game-state
/// block back into the party, which the menu shows and a save writes.
fn take_party(state: &[u8], party: &mut Party) {
    if let Ok(progress) = Progress::read(state) {
        party.level = u32::from(progress.level);
        party.experience = progress.experience;
        party.money = progress.money;
    }
}

/// The game as events see it, borrowed for one frame.
struct Host<'a, 'rom> {
    data: GameData<'rom>,
    field: &'a mut Option<Field>,
    windows: &'a mut ScriptWindows<'rom>,
    dialogue: &'a mut ScriptRunner,
    scripts: &'a mut Vec<ScriptRunner>,
    active_script: &'a mut Option<usize>,
    last_runner: &'a mut Option<usize>,
    battle: &'a mut Option<Box<Staged>>,
    shop: &'a mut Option<Box<PauseMenu>>,
    combat: &'a mut Option<Box<Combat>>,
    encounter: Option<usize>,
    aftermath: &'a mut Option<(usize, Outcome)>,
    battle_lost: bool,
    warped: &'a mut Option<usize>,
    portal_exit: Option<(usize, usize)>,
    chest: Option<(usize, u16)>,
    gift: &'a mut Option<Reward>,
    effects: &'a mut StoryEffects,
    party: &'a mut Party,
    sound: &'a mut SoundEngine<'rom>,
    extensions: &'a SharedExtensions,
    state: &'a mut Vec<u8>,
    objects: &'a mut AreaObjects,
    frame: u16,
    error: Option<GameError>,
}

impl Host<'_, '_> {
    fn fail(&mut self, error: impl Into<GameError>) {
        if self.error.is_none() {
            self.error = Some(error.into());
        }
    }

    fn finish(self) -> Result<(), GameError> {
        self.error.map_or(Ok(()), Err)
    }
}

impl Host<'_, '_> {
    /// What the chest being searched gives.
    fn chest_reward(&self) -> Reward {
        if let Some(gift) = *self.gift {
            return gift;
        }
        self.chest
            .and_then(|(_, chest)| self.data.treasure(usize::from(chest)))
            .map_or(Reward::Nothing, |treasure| treasure.reward())
    }
}

impl EventHost for Host<'_, '_> {
    fn field(&mut self) -> Option<&mut Field> {
        self.field.as_mut()
    }

    fn flag(&self, flag: u16) -> bool {
        self.windows.flag(flag)
    }

    fn set_flag(&mut self, flag: u16, set: bool) {
        self.windows.set_flag(flag, set);
    }

    fn start_dialogue(&mut self, index: u16, called: bool) {
        // A task's call redraws the windows a close leaves open within the
        // close's frame when the frame has time left: on a Zoid map with few
        // sprites to draw (measured with one to three: the world map, the
        // factory's door and hall). Rooms and towns (the castle, Arcana)
        // and a Zoid map with four sprites or more (the hall once Blood's
        // squad is in) take the frame.
        let quick = called
            && self
                .field
                .as_ref()
                .is_some_and(|field| field.zoid_map() && field.drawn_actors() < BUSY_SPRITES);
        if let Err(error) = start_dialogue(
            self.dialogue,
            self.scripts,
            self.last_runner,
            (usize::from(index), quick),
        ) {
            self.fail(error);
        }
    }

    fn start_battle(&mut self, scene: u8) {
        match Staged::new(&self.data, scene) {
            Ok(stage) => *self.battle = Some(Box::new(stage)),
            Err(missing) => self.fail(GameError::Text(format!("no battle scene {}", missing.0))),
        }
    }

    fn start_shop(&mut self, shop: Shop) {
        if matches!(shop, Shop::Lab(_)) {
            self.objects
                .rebuild_current(&self.data, self.state, self.frame);
        }
        let rom = self.data.bytes();
        let menu = PauseMenu::new(&self.data, self.party.clone(), self.state.clone());
        match menu {
            Ok(mut menu) => match menu.open_shop(rom, shop, self.windows) {
                Ok(()) => {
                    *self.shop = Some(Box::new(menu));
                    Game::emit(self.extensions, &Event::ShopOpened(shop));
                }
                Err(error) => self.fail(error),
            },
            Err(error) => self.fail(error),
        }
    }

    fn start_combat(&mut self) {
        let Some(enemy) = self.encounter else {
            return;
        };
        let Some(field) = self.field.as_ref() else {
            return;
        };
        let attribute = |actor: &crate::Actor| {
            let (column, row) = actor.footing();
            field.scene().attribute(column, row).unwrap_or(0)
        };
        let Some(zoid) = field.actor(enemy) else {
            return;
        };
        let enemy_terrain = attribute(zoid).to_le_bytes()[0];
        let footing = attribute(field.player());
        let player_terrain = if footing & !TERRAIN_MASK == 0 {
            footing.to_le_bytes()[0]
        } else {
            enemy_terrain
        };
        let formation = self
            .objects
            .formation(zoid.group)
            .copied()
            .unwrap_or([0; extraction::saga_encounter::FORMATION_LEN]);
        store_party(self.state, self.party);
        let mut combat = Combat::new(
            &self.data,
            self.state.as_mut_slice(),
            &formation,
            (player_terrain, enemy_terrain),
        );
        if let Err(error) = combat.update(self.data.bytes(), Input::default(), self.windows) {
            self.fail(error);
        }
        *self.combat = Some(Box::new(combat));
        Game::emit(self.extensions, &Event::CombatStarted { enemy });
    }

    fn after_combat(&mut self) {
        let Some((enemy, outcome)) = self.aftermath.take() else {
            return;
        };
        let explosion = match outcome {
            Outcome::Retreated => None,
            Outcome::Won | Outcome::Lost => {
                match self.data.sprite_sheet(crate::field::EXPLOSION_SPRITE) {
                    Ok(sheet) => Some(sheet),
                    Err(error) => return self.fail(error),
                }
            }
        };
        let Some(field) = self.field.as_mut() else {
            return;
        };
        match (outcome, explosion) {
            (Outcome::Lost, Some(explosion)) => {
                field.wreck(0, explosion);
                let player = field.player_mut();
                player.animation_shift = AFTER_COMBAT_SHIFT;
                player.animation = PLAYER_UPDATED;
            }
            (Outcome::Won, Some(explosion)) => {
                field.player_mut().command = Command::Player;
                field.wreck(enemy, explosion);
                if let Some(slot) = field.actor_mut(enemy).and_then(|actor| actor.slot.take()) {
                    formats::progress::remove_object(self.state, slot);
                }
                formats::progress::count_battle(self.state);
            }
            _ => {
                let player = field.player_mut();
                player.command = Command::Player;
                player.animation_shift = AFTER_COMBAT_SHIFT;
                if let Some(actor) = field.actor_mut(enemy) {
                    actor.pause = RETREAT_PAUSE;
                }
            }
        }
    }

    fn play_music(&mut self, song: u16) {
        let song = usize::from(song);
        Game::emit(self.extensions, &Event::SoundRequested(song));
        if let Err(error) = self.sound.play_if_changed(song) {
            self.fail(error);
        }
    }

    fn restart_music(&mut self, song: u16) {
        let song = usize::from(song);
        Game::emit(self.extensions, &Event::SoundRequested(song));
        if let Err(error) = self.sound.play(song) {
            self.fail(error);
        }
    }

    fn play_sound(&mut self, sound: u16) {
        let sound = usize::from(sound);
        Game::emit(self.extensions, &Event::SoundRequested(sound));
        if let Err(error) = self.sound.play(sound) {
            self.fail(error);
        }
    }

    fn load_map(&mut self, map: usize, player: (usize, usize), objects: u32, count: usize) {
        let player = if player == crate::event::HERE {
            self.field
                .as_ref()
                .map_or((0, 0), |field| (field.player().column, field.player().row))
        } else {
            player
        };
        self.objects.enter(&self.data, self.state, map, self.frame);
        let loaded = self
            .data
            .objects_at(objects, count)
            .map_err(FieldError::from)
            .and_then(|objects| Field::load_with(&self.data, map, player, &objects))
            .and_then(|mut field| {
                field.show_party_zoids(&self.data, self.state)?;
                Ok(field)
            });
        match loaded {
            Ok(field) => *self.field = Some(field),
            Err(error) => self.fail(error),
        }
    }

    fn load_arena(&mut self, game: u8) -> usize {
        let map = saga_arena::ARENA_MAP;
        self.objects.enter(&self.data, self.state, map, self.frame);
        let loaded = saga_arena::arena_objects(self.data.bytes(), self.state, usize::from(game))
            .map_err(FieldError::from)
            .and_then(|objects| {
                let count = objects.len();
                let mut field =
                    Field::load_with(&self.data, map, saga_arena::ARENA_CELL, &objects)?;
                field.show_party_zoids(&self.data, self.state)?;
                Ok((field, count))
            });
        match loaded {
            Ok((field, count)) => {
                *self.field = Some(field);
                count
            }
            Err(error) => {
                self.fail(error);
                0
            }
        }
    }

    fn meets_regulation(&self, game: u8) -> bool {
        saga_arena::meets_regulation(self.data.bytes(), self.state, usize::from(game))
    }

    fn start_map_music(&mut self, map: usize) {
        let song = self
            .extensions
            .borrow()
            .music_for_map(map)
            .or_else(|| self.data.map_music(map))
            .and_then(|song| u16::try_from(song).ok());
        if let Some(song) = song {
            self.play_music(song);
        }
    }

    fn meet(&mut self, group: u8) {
        self.data.meet_characters(self.state, usize::from(group));
    }

    fn join(&mut self, list: u8) {
        if self
            .data
            .join_group(self.state, usize::from(list))
            .is_none()
        {
            self.fail(GameError::Text(format!("no party list {list}")));
        }
    }

    fn leave(&mut self, list: u8) {
        if self
            .data
            .leave_group(self.state, usize::from(list))
            .is_none()
        {
            self.fail(GameError::Text(format!("no party list {list}")));
        }
    }

    fn start_script(&mut self, table: &'static str, index: u16) {
        let found = self
            .scripts
            .iter()
            .position(|runner| runner.table() == table);
        let slot = match found {
            Some(slot) => slot,
            None => match self.data.script_offsets(table) {
                Ok(Some(offsets)) => {
                    self.scripts.push(ScriptRunner::named(table, offsets));
                    self.scripts.len() - 1
                }
                Ok(None) => return self.fail(GameError::Text(format!("no script table {table}"))),
                Err(error) => return self.fail(GameError::Text(error.to_string())),
            },
        };
        let context = last_context(self.dialogue, self.scripts, *self.last_runner);
        self.scripts[slot].resume(context);
        *self.last_runner = Some(slot);
        if let Err(error) = self.scripts[slot].start(usize::from(index)) {
            return self.fail(error);
        }
        *self.active_script = Some(slot);
    }

    fn learn_command(&mut self, command: u8) {
        formats::progress::learn_command(self.state, usize::from(command));
    }

    fn command_learned(&self, command: u8) -> bool {
        formats::progress::command_learned(self.state, usize::from(command))
    }

    fn area(&self) -> u8 {
        self.field
            .as_ref()
            .and_then(|field| self.data.map_record(field.map()).ok())
            .map_or(NO_AREA, |record| record.id.to_le_bytes()[0])
    }

    fn saved_vars(&self) -> [u16; 8] {
        match *self.active_script {
            Some(index) => *self.scripts[index].saved_vars(),
            None => *self.dialogue.saved_vars(),
        }
    }

    fn form_party(&mut self, choice: u8) {
        self.data.form_party(self.state, usize::from(choice));
    }

    fn see_zoid(&mut self, id: u8) {
        formats::progress::see_zoid(self.state, usize::from(id));
    }

    fn open_chest(&mut self) {
        let Some((actor, _)) = self.chest else {
            return;
        };
        let Some(chest) = self.field.as_mut().and_then(|field| field.actor_mut(actor)) else {
            return;
        };
        chest.play(CHEST_OPEN_ANIMATION);
        let small = chest
            .sheet
            .as_ref()
            .is_some_and(|sheet| sheet.tag == SMALL_CHEST_SPRITE);
        self.play_sound(if small {
            SMALL_CHEST_SOUND
        } else {
            CHEST_SOUND
        });
    }

    fn mark_chest(&mut self) {
        if let Some((_, chest)) = self.chest {
            self.windows.set_flag(CHEST_FLAG_BASE + chest, true);
        }
    }

    fn chest_kind(&self) -> Option<ChestKind> {
        match self.chest_reward() {
            Reward::Core(_) => Some(ChestKind::Core),
            Reward::ZiData(_) => Some(ChestKind::ZiData),
            Reward::Part(_) => Some(ChestKind::Part),
            Reward::Consumable(_) => Some(ChestKind::Consumable),
            Reward::Money(_) => Some(ChestKind::Money),
            Reward::Nothing => None,
        }
    }

    fn zi_data_held(&self) -> bool {
        matches!(self.chest_reward(), Reward::ZiData(zoid)
            if formats::progress::zoid_seen(self.state, usize::from(zoid)))
    }

    fn take_chest(&mut self) {
        let one_more = |state: &mut [u8], kind, id| {
            let item = Item { kind, id };
            let count = saga_shop::item_count(state, item);
            saga_shop::set_item_count(state, item, count.saturating_add(1).min(ITEM_LIMIT));
        };
        match self.chest_reward() {
            Reward::Core(id) => one_more(self.state, ItemKind::Core, id),
            Reward::Consumable(id) => one_more(self.state, ItemKind::Consumable, id),
            Reward::Part(id) => {
                let count = saga_party::stock(self.state, id);
                saga_shop::set_stock(self.state, id, count.saturating_add(1).min(PART_LIMIT));
            }
            Reward::ZiData(zoid) => formats::progress::see_zoid(self.state, usize::from(zoid)),
            Reward::Money(money) => {
                self.party.money = self.party.money.saturating_add(money).min(MONEY_LIMIT);
                for digit in money.to_string().chars() {
                    ScriptHost::put_char(self.windows, MONEY_WINDOW, digit);
                }
            }
            Reward::Nothing => {}
        }
    }

    fn set_gift(&mut self, reward: Option<Reward>) {
        *self.gift = reward;
    }

    fn set_calm(&mut self, calm: bool) {
        self.effects.calm = calm;
        if let Some(field) = self.field.as_mut() {
            field.set_calm(calm);
        }
    }

    fn set_whiten(&mut self, level: u8) {
        self.effects.whiten = level.min(WHITE_LEVELS);
    }

    fn seen_guard(&self) -> Option<usize> {
        self.effects.seen_guard
    }

    fn set_seen_guard(&mut self, guard: usize) {
        self.effects.seen_guard = Some(guard);
    }

    fn zoid_owned(&self, zoid: u16) -> bool {
        saga_party::owns_zoid(self.state, zoid)
    }

    fn state_byte(&self, at: usize) -> u8 {
        self.state.get(at).copied().unwrap_or(0)
    }

    fn start_chest_name(&mut self) {
        let (table, index) = match self.chest_reward() {
            Reward::Core(id) => (ITEM_TABLE, u16::from(id)),
            Reward::ZiData(zoid) => (NAME_TABLE, u16::from(zoid) + ZOID_NAMES),
            Reward::Part(id) => (PART_TABLE, id),
            Reward::Consumable(id) => (NAME_TABLE, u16::from(id) + CONSUMABLE_NAMES),
            Reward::Money(_) | Reward::Nothing => return,
        };
        self.start_script(table, index);
    }

    fn start_story_battle(&mut self, battle: u8) {
        let Some(battle) = extraction::saga_encounter::story_battle(self.data.bytes(), battle)
        else {
            return self.fail(GameError::Text(format!("no story battle {battle}")));
        };
        store_party(self.state, self.party);
        let mut combat = Combat::story(&self.data, self.state.as_mut_slice(), battle);
        if let Err(error) = combat.update(self.data.bytes(), Input::default(), self.windows) {
            self.fail(error);
        }
        *self.combat = Some(Box::new(combat));
    }

    fn battle_lost(&self) -> bool {
        self.battle_lost
    }

    fn return_point(&self) -> Option<(usize, (usize, usize))> {
        let index = AreaObjects::area_index(self.state);
        extraction::saga_encounter::return_point(self.data.bytes(), index)
            .map(|point| (point.map, point.cell))
    }

    fn sound_ended(&self, n: u16) -> bool {
        self.sound.song_ended(usize::from(n))
    }

    fn music_playing(&self) -> Option<u16> {
        self.sound
            .playing(MUSIC_PLAYER)
            .and_then(|song| u16::try_from(song).ok())
    }

    fn restart_map_music(&mut self) {
        let Some(map) = self.field.as_ref().map(Field::map) else {
            return;
        };
        let song = self
            .extensions
            .borrow()
            .music_for_map(map)
            .or_else(|| self.data.map_music(map))
            .and_then(|song| u16::try_from(song).ok());
        if let Some(song) = song {
            self.restart_music(song);
        }
    }

    fn battles_won(&self) -> u16 {
        formats::progress::battles_won(self.state)
    }

    fn forget_battles_won(&mut self) {
        formats::progress::forget_battles(self.state);
    }

    fn set_return_point(&mut self, index: u8) {
        AreaObjects::set_area_index(self.state, index);
    }

    fn exit_sound(&mut self) {
        let Some((map, exit)) = self.portal_exit else {
            return;
        };
        if let Err(error) = Game::door_sound(self.sound, &self.data, self.extensions, map, exit) {
            self.fail(error);
        }
    }

    fn take_exit(&mut self) -> usize {
        let Some((map, exit)) = self.portal_exit else {
            return 0;
        };
        match self.data.warp(map, exit) {
            Ok(warp) => self.warp(warp.map, (warp.column, warp.row), None),
            Err(error) => {
                self.fail(FieldError::from(error));
                0
            }
        }
    }

    fn exit_arrival(&self) -> Option<(usize, usize)> {
        let (map, exit) = self.portal_exit?;
        let warp = self.data.warp(map, exit).ok()?;
        Some((warp.column, warp.row))
    }

    fn set_sprite(&mut self, actor: usize, sprite: usize) {
        let sheet = match self.data.sprite_sheet(sprite) {
            Ok(sheet) => sheet,
            Err(error) => return self.fail(error),
        };
        if let Some(field) = self.field.as_mut() {
            field.set_sheet(actor, sheet);
        }
    }

    fn warp(&mut self, map: usize, cell: (usize, usize), facing: Option<Direction>) -> usize {
        let standing = self.field.as_ref().map(|field| {
            let player = field.player();
            ((player.column, player.row), player.facing)
        });
        let (cell, facing) = match standing {
            Some((here, faced)) if cell == crate::event::HERE => (here, facing.or(Some(faced))),
            _ => (cell, facing),
        };
        self.objects.enter(&self.data, self.state, map, self.frame);
        let loaded = Field::load(&self.data, map, cell).and_then(|mut field| {
            AreaObjects::place(&self.data, self.state, map, &mut field)?;
            field.show_party_zoids(&self.data, self.state)?;
            Ok(field)
        });
        match loaded {
            Ok(mut field) => {
                if let Some(facing) = facing {
                    field.player_mut().face(facing);
                }
                let count = field.actors.len();
                show_opened_chests(&mut field, self.windows);
                *self.field = Some(field);
                *self.warped = Some(map);
                count
            }
            Err(error) => {
                self.fail(error);
                0
            }
        }
    }
}

/// The levels of the brightening toward white (`BLDY`).
const WHITE_LEVELS: u8 = 16;

/// Brightens every pixel of `frame` toward white by `level` of 16, as the
/// GBA's brightness increase does on each 5-bit channel.
fn whiten(frame: &mut Frame, level: u8) {
    if level == 0 {
        return;
    }
    let level = u16::from(level.min(WHITE_LEVELS));
    let up = |channel: u8| {
        let five = u16::from(channel >> 3);
        let raised = five + (31 - five) * level / 16;
        u8::try_from(raised << 3 | raised >> 2).unwrap_or(u8::MAX)
    };
    for y in 0..frame.height() {
        for x in 0..frame.width() {
            if let Some(color) = frame.pixel(x, y) {
                frame.set_pixel(x, y, Rgb::new(up(color.r), up(color.g), up(color.b)));
            }
        }
    }
}

/// The interpreter state the runner that ran last left: `last` is a slot
/// of `scripts`, or `None` for the dialogue runner.
fn last_context(
    dialogue: &ScriptRunner,
    scripts: &[ScriptRunner],
    last: Option<usize>,
) -> ScriptContext {
    last.and_then(|slot| scripts.get(slot))
        .unwrap_or(dialogue)
        .context()
}

/// Starts string `index` of the dialogue table where the last string left
/// the interpreter, as a task's call when `called`.
fn start_dialogue(
    dialogue: &mut ScriptRunner,
    scripts: &[ScriptRunner],
    last: &mut Option<usize>,
    (index, called): (usize, bool),
) -> Result<(), ScriptError> {
    dialogue.set_called(called);
    let context = last_context(dialogue, scripts, *last);
    dialogue.resume(context);
    *last = None;
    dialogue.start(index)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_PLAYER_NAME, Party, store_party, take_party, translated_name, whiten};
    use formats::Progress;
    use formats::progress::{ProgressError, STATE_LEN};
    use platform::{Frame, Rgb};

    #[test]
    fn whitening_raises_each_channel_by_its_share_of_the_way() {
        let mut frame = Frame::new(1, 1, Rgb::new(0, 128, 248));
        whiten(&mut frame, 0);
        assert_eq!(frame.pixel(0, 0), Some(Rgb::new(0, 128, 248)));
        whiten(&mut frame, 8);
        assert_eq!(frame.pixel(0, 0), Some(Rgb::new(123, 189, 255)));
        let mut white = Frame::new(1, 1, Rgb::new(0, 0, 0));
        whiten(&mut white, 16);
        assert_eq!(white.pixel(0, 0), Some(Rgb::new(255, 255, 255)));
    }

    #[test]
    fn a_translation_gives_the_default_name_its_latin_form() {
        assert_eq!(translated_name(DEFAULT_PLAYER_NAME), "Atory");
        assert_eq!(translated_name("Iñigo"), "Iñigo");
        assert_eq!(translated_name("Atory"), "Atory");
    }

    #[test]
    fn a_battles_winnings_reach_the_party() -> Result<(), ProgressError> {
        let mut state = vec![0; STATE_LEN];
        let mut party = Party {
            level: 3,
            experience: 120,
            money: 500,
            message_speed: 3,
        };
        store_party(&mut state, &party);
        let mut progress = Progress::read(&state)?;
        assert_eq!(
            (progress.level, progress.experience, progress.money),
            (3, 120, 500)
        );
        progress.level = 4;
        progress.experience = 260;
        progress.money = 740;
        progress.write(&mut state)?;
        take_party(&state, &mut party);
        assert_eq!((party.level, party.experience, party.money), (4, 260, 740));
        assert_eq!(party.message_speed, 3);
        Ok(())
    }
}
