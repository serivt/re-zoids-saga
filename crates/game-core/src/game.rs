//! The whole game as one state machine: logo, title, name entry, the
//! opening cutscene in the first room, then the field with its
//! conversations and exits.
//!
//! The opening follows the original's entity trace: the player sits at
//! the desk on metatile (6, 2) facing up, Regina starts on (6, 4), walks
//! three steps left, turns right and walks five, turns left and walks
//! two, faces the player, and the first dialogue string runs. Afterwards
//! the player stands up onto (5, 2) and control begins.
//!
//! Continuing darkens the title 10 frames after A, fades it out over 16,
//! keeps the screen black for 73 and fades the saved room in over 16, as
//! the original does. When the loader has a notice (no save, a broken one,
//! the backup used) it appears 23 frames into the black; once it is
//! dismissed the title starts over, or, with the backup, the room fades
//! in after 33 more black frames.

use extraction::saga::{
    BootError, FIRST_ROOM_MAP, OPENING_SEEN_FLAG, PLAYER_START, SpriteSheetError,
};
use extraction::saga_save::SaveDataError;
use formats::Progress;
use formats::m4a::M4aError;
use formats::progress::{FLAG_WORDS, encode_name};
use gba_runtime::apu::SoundEngine;
use gba_runtime::ppu::{FADE_STEPS, SCREEN_HEIGHT, SCREEN_WIDTH, darken};
use platform::{Button, Frame, Input, Rgb, SaveStorage};
use thiserror::Error;

use crate::boot::{LogoScreen, NameEntry, TitleChoice, TitleScreen};
use crate::data::GameData;
use crate::extension::{Event, GameSound, SharedExtensions};
use crate::field::{Direction, Field, FieldError, FieldEvent, NpcCommand};
use crate::menu::{MenuStep, Party, PauseMenu};
use crate::save::{Found, SaveFile, SavedGame};
use crate::script::{ScriptError, ScriptRunner};
use crate::text::TextMetrics;
use crate::translation::{DIALOGUE_TABLE, Translation, TranslationExtension};
use crate::windows::{DEFAULT_PLAYER_NAME, ScriptWindows};
use crate::{ScriptHost, TextPainter, WindowPainter};

const TALK_START_DELAY: u32 = 3;
const NAME_TO_ROOM_BLACK_FRAMES: u32 = 60;
const INTRO_DIALOGUE: usize = 40;
const INTRO_SECOND_DIALOGUE: usize = 41;
const INTRO_PLAYER_CELL: (usize, usize) = (6, 2);
const REGINA_SPRITE: usize = 0x99;
const REGINA_START: (usize, usize) = (6, 4);
const INTRO_FADE_FRAMES: u32 = 32;
const INTRO_DARK_FRAMES: u32 = 158;
const INTRO_DIALOGUE_DELAY: u32 = 60;
const INTRO_STAND_UP_DELAY: u32 = 2;
const CONTINUE_HOLD_FRAMES: u32 = 8;
const CONTINUE_FADE_FRAMES: u32 = 16;
const CONTINUE_BLACK_FRAMES: u32 = 72;
const CONTINUE_NOTICE_DELAY: u32 = 22;
const CONTINUE_AFTER_NOTICE_FRAMES: u32 = 33;
const MUSIC_PLAYER: usize = 0;
/// The area a save names when its object table does not describe the
/// current one; the original rebuilds the table on loading it. The port
/// does not build the table, so it keeps a loaded area only while the
/// player stays in it.
const NO_AREA: u8 = 0;
/// Regina's walk before the first line, as `(frames to wait, order)`.
const INTRO_ARRIVAL: [(u32, IntroOrder); 13] = [
    (15, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (25, IntroOrder::Regina(NpcCommand::Face(Direction::Right))),
    (31, IntroOrder::Regina(NpcCommand::Step(Direction::Right))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Right))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Right))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Right))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Right))),
    (0, IntroOrder::Regina(NpcCommand::Face(Direction::Left))),
    (61, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (11, IntroOrder::Regina(NpcCommand::Face(Direction::Up))),
];
/// Regina's pacing while the screen is dark, from the first line's end.
const INTRO_PACING: [(u32, IntroOrder); 10] = [
    (4, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (0, IntroOrder::Regina(NpcCommand::Face(Direction::Right))),
    (61, IntroOrder::Regina(NpcCommand::Step(Direction::Right))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Right))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Right))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Right))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Right))),
    (0, IntroOrder::Regina(NpcCommand::Face(Direction::Left))),
];
/// Regina's way out after the second line, the player watching her go.
const INTRO_LEAVING: [(u32, IntroOrder); 15] = [
    (6, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Down))),
    (23, IntroOrder::Player(Direction::Down)),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Down))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Down))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Left))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Down))),
    (1, IntroOrder::Regina(NpcCommand::Step(Direction::Down))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Down))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Down))),
    (0, IntroOrder::Regina(NpcCommand::Step(Direction::Down))),
];

/// One order of the opening's choreography.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntroOrder {
    /// Regina moves or turns.
    Regina(NpcCommand),
    /// The player turns.
    Player(Direction),
}

/// Where the opening is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntroPhase {
    Arriving,
    FirstTalk,
    FadingOut(u32),
    Dark(u32),
    FadingIn(u32),
    SecondTalkWait,
    SecondTalk,
    Leaving,
    StandingUp,
}

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
    /// The opening cutscene.
    Intro,
    /// Free play.
    Field,
    /// The pause menu.
    Menu,
    /// Between the title and a continued game.
    Continuing,
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
}

enum Screen {
    Logo(LogoScreen),
    Title(TitleScreen),
    NameEntry(NameEntry),
    Loading(u32),
    Intro(IntroState),
    Field,
    Menu(PauseMenu),
    Continuing(Continuing),
}

/// Where continuing is: the title held then fading out, black (with the
/// loader's notice once it is due), then the room fading in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContinuePhase {
    Holding(u32),
    FadingOut(u32),
    Black(u32),
    Notice,
    AfterNotice(u32),
    FadingIn(u32),
}

struct Continuing {
    title: TitleScreen,
    phase: ContinuePhase,
}

impl Continuing {
    fn darkness(&self) -> u8 {
        let level = match self.phase {
            ContinuePhase::Holding(_) => 0,
            ContinuePhase::FadingOut(frames) => {
                frames * u32::from(FADE_STEPS) / CONTINUE_FADE_FRAMES
            }
            ContinuePhase::Black(_) | ContinuePhase::Notice | ContinuePhase::AfterNotice(_) => {
                u32::from(FADE_STEPS)
            }
            ContinuePhase::FadingIn(frames) => {
                u32::from(FADE_STEPS) - frames * u32::from(FADE_STEPS) / CONTINUE_FADE_FRAMES
            }
        };
        u8::try_from(level).unwrap_or(FADE_STEPS)
    }
}

struct IntroState {
    regina: usize,
    path: &'static [(u32, IntroOrder)],
    step: usize,
    wait: u32,
    phase: IntroPhase,
}

impl IntroState {
    fn follow(&mut self, path: &'static [(u32, IntroOrder)]) {
        self.path = path;
        self.step = 0;
        self.wait = path.first().map_or(0, |order| order.0);
    }

    fn darkness(&self) -> u8 {
        let level = match self.phase {
            IntroPhase::FadingOut(frames) => frames * u32::from(FADE_STEPS) / INTRO_FADE_FRAMES,
            IntroPhase::Dark(_) => u32::from(FADE_STEPS),
            IntroPhase::FadingIn(frames) => {
                u32::from(FADE_STEPS) - frames * u32::from(FADE_STEPS) / INTRO_FADE_FRAMES
            }
            _ => 0,
        };
        u8::try_from(level).unwrap_or(FADE_STEPS)
    }
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
    field: Option<Field>,
    screen: Screen,
    pending_talk: Option<(usize, u32)>,
    player_name: String,
    party: Party,
    state: Vec<u8>,
    save: SaveFile,
    storage: Option<Box<dyn SaveStorage>>,
    found: Option<Found>,
    previous: Input,
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
        game.field = Some(Field::load(&game.data, FIRST_ROOM_MAP, PLAYER_START)?);
        Self::emit(
            &game.extensions,
            &Event::RoomEntered {
                map: FIRST_ROOM_MAP,
                cell: PLAYER_START,
            },
        );
        game.screen = Screen::Field;
        Self::play(
            &mut game.sound,
            &game.data,
            &game.extensions,
            GameSound::FirstRoomMusic,
        )?;
        Ok(game)
    }

    /// Keeps the save in `storage`: continuing reads it and saving
    /// replaces it.
    pub fn set_save_storage(&mut self, storage: Box<dyn SaveStorage>) {
        self.storage = Some(storage);
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
        let (song_table, song_count, master_volume) = data.song_table();
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
            sound: SoundEngine::new(data.bytes(), song_table, song_count, master_volume),
            dialogue: ScriptRunner::named(DIALOGUE_TABLE, dialogue),
            field: None,
            screen: Screen::Loading(0),
            pending_talk: None,
            player_name: DEFAULT_PLAYER_NAME.to_owned(),
            party: Party::default(),
            state,
            save,
            storage: None,
            found: None,
            previous: Input::default(),
        })
    }

    /// Where the game is.
    #[must_use]
    pub fn stage(&self) -> Stage {
        match self.screen {
            Screen::Logo(_) => Stage::Logo,
            Screen::Title(_) => Stage::Title,
            Screen::NameEntry(_) => Stage::NameEntry,
            Screen::Loading(_) => Stage::Loading,
            Screen::Intro(_) => Stage::Intro,
            Screen::Field => Stage::Field,
            Screen::Menu(_) => Stage::Menu,
            Screen::Continuing(_) => Stage::Continuing,
        }
    }

    /// The field, once the room is loaded.
    #[must_use]
    pub fn field(&self) -> Option<&Field> {
        self.field.as_ref()
    }

    /// The player's name.
    #[must_use]
    pub fn player_name(&self) -> &str {
        &self.player_name
    }

    /// Advances one frame.
    ///
    /// # Errors
    ///
    /// Returns [`GameError`] when a screen or script fails.
    pub fn update(&mut self, input: Input) -> Result<(), GameError> {
        let start = input.is_held(Button::Start) && !self.previous.is_held(Button::Start);
        self.previous = input;
        self.frame += 1;
        Self::emit(&self.extensions, &Event::Frame(self.frame));
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
            Screen::Title(title) => {
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
                    Some(TitleChoice::Continue) => self.begin_continue(),
                    _ => {}
                }
            }
            Screen::NameEntry(entry) => {
                if entry.update(rom, input, &mut self.windows)? {
                    self.player_name = entry.name();
                    self.windows.set_player_name(&self.player_name);
                    self.windows.close_window(None);
                    self.screen = Screen::Loading(0);
                    Self::emit(
                        &self.extensions,
                        &Event::NameConfirmed(self.player_name.clone()),
                    );
                    self.sound.stop_music();
                }
            }
            Screen::Loading(frames) => {
                *frames += 1;
                if *frames >= NAME_TO_ROOM_BLACK_FRAMES {
                    self.start_intro()?;
                }
            }
            Screen::Intro(_) => self.update_intro(input)?,
            Screen::Field => {
                if start && self.dialogue.is_done() && self.pending_talk.is_none() {
                    let mut menu = PauseMenu::new(&self.data, self.party.clone())?;
                    menu.open(rom, &mut self.windows)?;
                    self.screen = Screen::Menu(menu);
                    Self::emit(&self.extensions, &Event::MenuOpened);
                } else {
                    self.update_field(input)?;
                }
            }
            Screen::Menu(menu) => match menu.update(rom, input, &mut self.windows)? {
                MenuStep::Open => {}
                MenuStep::Save => {
                    let party = menu.party();
                    let written = self.write_save(&party);
                    if let Screen::Menu(menu) = &mut self.screen {
                        menu.finish_save(written)?;
                    }
                }
                MenuStep::Closed => {
                    self.party = menu.party();
                    self.screen = Screen::Field;
                    Self::emit(&self.extensions, &Event::MenuClosed);
                }
            },
            Screen::Continuing(_) => self.update_continue(input)?,
        }
        for sound in self.windows.take_sounds() {
            self.sound.play(usize::from(sound))?;
        }
        self.sound.frame()?;
        Ok(())
    }

    /// Shows the messages `translation` covers in place of the ROM's text.
    /// Fits the windows to it first; returns the messages no window can
    /// hold.
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
        Ok(problems)
    }

    /// The samples of the frame the last update produced, stereo
    /// interleaved at [`gba_runtime::apu::SAMPLE_RATE`].
    #[must_use]
    pub fn audio(&self) -> &[i16] {
        self.sound.output()
    }

    fn new_game(&mut self, input: Input) -> Result<(), GameError> {
        self.state = self.data.new_game_state()?;
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

    /// Leaves the title for a saved game: reads the save now and starts
    /// the fade.
    fn begin_continue(&mut self) {
        Self::emit(&self.extensions, &Event::LoadRequested);
        let image = match self.storage.as_ref().map(|storage| storage.load()) {
            Some(Ok(image)) => image,
            Some(Err(error)) => {
                Self::emit(&self.extensions, &Event::StorageFailed(error.to_string()));
                None
            }
            None => None,
        };
        self.found = Some(self.save.read(image));
        let screen = std::mem::replace(&mut self.screen, Screen::Loading(0));
        let Screen::Title(title) = screen else {
            self.screen = screen;
            return;
        };
        self.windows.close_window(None);
        self.sound.stop_music();
        self.screen = Screen::Continuing(Continuing {
            title,
            phase: ContinuePhase::Holding(0),
        });
    }

    fn update_continue(&mut self, input: Input) -> Result<(), GameError> {
        let Screen::Continuing(continuing) = &mut self.screen else {
            return Ok(());
        };
        let notice = self.found.as_ref().and_then(Found::notice);
        match continuing.phase {
            ContinuePhase::Holding(frames) => {
                continuing.phase = if frames + 1 >= CONTINUE_HOLD_FRAMES {
                    ContinuePhase::FadingOut(0)
                } else {
                    ContinuePhase::Holding(frames + 1)
                };
            }
            ContinuePhase::FadingOut(frames) => {
                continuing.phase = if frames + 1 >= CONTINUE_FADE_FRAMES {
                    ContinuePhase::Black(0)
                } else {
                    ContinuePhase::FadingOut(frames + 1)
                };
            }
            ContinuePhase::Black(frames) => match notice {
                Some(notice) if frames + 1 >= CONTINUE_NOTICE_DELAY => {
                    self.dialogue.start(notice)?;
                    continuing.phase = ContinuePhase::Notice;
                }
                None if frames + 1 >= CONTINUE_BLACK_FRAMES => self.resume()?,
                _ => continuing.phase = ContinuePhase::Black(frames + 1),
            },
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
            ContinuePhase::FadingIn(frames) => {
                if frames + 1 >= CONTINUE_FADE_FRAMES {
                    self.screen = Screen::Field;
                } else {
                    continuing.phase = ContinuePhase::FadingIn(frames + 1);
                }
            }
        }
        Ok(())
    }

    /// Puts the loaded game in place and fades its room in, or goes back
    /// to the title when there is none.
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
        if !self.windows.flag(OPENING_SEEN_FLAG) && usize::from(progress.map) == FIRST_ROOM_MAP {
            return self.start_intro();
        }
        let map = usize::from(progress.map);
        let cell = (usize::from(progress.column), usize::from(progress.row));
        self.field = Some(Field::load(&self.data, map, cell)?);
        Self::emit(&self.extensions, &Event::RoomEntered { map, cell });
        let song = Some(usize::from(progress.song))
            .filter(|song| *song != 0)
            .or_else(|| self.extensions.borrow().music_for_map(map))
            .or_else(|| self.data.map_music(map));
        if let Some(song) = song {
            Self::emit(&self.extensions, &Event::SoundRequested(song));
            self.sound.play(song)?;
        }
        if let Screen::Continuing(continuing) = &mut self.screen {
            continuing.phase = ContinuePhase::FadingIn(0);
        }
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
        let mut party = Party::default();
        if let Some(leader) = party.members.first_mut() {
            leader.level = u32::from(progress.level);
            leader.experience = progress.experience;
        }
        party.money = progress.money;
        party.message_speed = u16::from(progress.message_speed);
        self.party = party;
        Some(progress)
    }

    /// Writes the game as it stands, with `party` from the menu, into the
    /// save; returns whether it was stored.
    fn write_save(&mut self, party: &Party) -> bool {
        Self::emit(&self.extensions, &Event::SaveRequested);
        if !self.update_state(party) {
            return false;
        }
        let Some(storage) = self.storage.as_mut() else {
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
            Ok(()) => true,
            Err(error) => failed(&self.extensions, &error),
        }
    }

    /// Writes what the port models into the game-state block.
    fn update_state(&mut self, party: &Party) -> bool {
        let (Some(field), Ok(mut progress)) = (&self.field, Progress::read(&self.state)) else {
            return false;
        };
        let half = |value: usize| u16::try_from(value).unwrap_or(u16::MAX);
        let area = self
            .data
            .map_record(field.map())
            .map_or(NO_AREA, |record| record.id.to_le_bytes()[0]);
        if progress.area != area {
            progress.area = NO_AREA;
        }
        progress.map = half(field.map());
        progress.column = half(field.player.column);
        progress.row = half(field.player.row);
        progress.flags = [0; FLAG_WORDS];
        for flag in self.windows.flags() {
            progress.set_flag(flag, true);
        }
        if let Some(leader) = party.members.first() {
            progress.level = u8::try_from(leader.level).unwrap_or(u8::MAX);
            progress.experience = leader.experience;
        }
        progress.money = party.money;
        progress.message_speed = u8::try_from(party.message_speed).unwrap_or(u8::MAX);
        progress.name = encode_name(&self.player_name).0;
        if let Some(song) = self.sound.playing(MUSIC_PLAYER) {
            progress.song = half(song);
        }
        progress.write(&mut self.state).is_ok()
    }

    fn start_intro(&mut self) -> Result<(), GameError> {
        self.windows.set_flag(OPENING_SEEN_FLAG, true);
        let mut field = Field::load(&self.data, FIRST_ROOM_MAP, INTRO_PLAYER_CELL)?;
        field.player.facing = Direction::Up;
        let regina = field.spawn_npc(&self.data, REGINA_SPRITE, REGINA_START, Direction::Up)?;
        self.field = Some(field);
        Self::emit(
            &self.extensions,
            &Event::RoomEntered {
                map: FIRST_ROOM_MAP,
                cell: INTRO_PLAYER_CELL,
            },
        );
        let mut intro = IntroState {
            regina,
            path: &INTRO_ARRIVAL,
            step: 0,
            wait: 0,
            phase: IntroPhase::Arriving,
        };
        intro.follow(&INTRO_ARRIVAL);
        self.screen = Screen::Intro(intro);
        Self::play(
            &mut self.sound,
            &self.data,
            &self.extensions,
            GameSound::OpeningMusic,
        )?;
        Ok(())
    }

    fn update_intro(&mut self, input: Input) -> Result<(), GameError> {
        let Some(field) = self.field.as_mut() else {
            return Ok(());
        };
        let Screen::Intro(intro) = &mut self.screen else {
            return Ok(());
        };
        field.update(Input::default());
        let path_done = Self::follow_path(field, intro);
        match intro.phase {
            IntroPhase::Arriving if path_done => {
                self.dialogue.start(INTRO_DIALOGUE)?;
                intro.phase = IntroPhase::FirstTalk;
            }
            IntroPhase::FirstTalk => {
                if self
                    .dialogue
                    .update(self.data.bytes(), input, &mut self.windows)?
                {
                    intro.phase = IntroPhase::FadingOut(0);
                    intro.follow(&INTRO_PACING);
                }
            }
            IntroPhase::FadingOut(frames) => {
                intro.phase = if frames + 1 >= INTRO_FADE_FRAMES {
                    IntroPhase::Dark(0)
                } else {
                    IntroPhase::FadingOut(frames + 1)
                };
            }
            IntroPhase::Dark(frames) => {
                intro.phase = if frames + 1 >= INTRO_DARK_FRAMES {
                    IntroPhase::FadingIn(0)
                } else {
                    IntroPhase::Dark(frames + 1)
                };
            }
            IntroPhase::FadingIn(frames) => {
                intro.phase = if frames + 1 >= INTRO_FADE_FRAMES {
                    IntroPhase::SecondTalkWait
                } else {
                    IntroPhase::FadingIn(frames + 1)
                };
            }
            IntroPhase::SecondTalkWait if path_done => {
                self.dialogue.start(INTRO_SECOND_DIALOGUE)?;
                intro.phase = IntroPhase::SecondTalk;
            }
            IntroPhase::SecondTalk => {
                if self
                    .dialogue
                    .update(self.data.bytes(), input, &mut self.windows)?
                {
                    intro.phase = IntroPhase::Leaving;
                    intro.follow(&INTRO_LEAVING);
                }
            }
            IntroPhase::Leaving if path_done => {
                field.npcs.remove(intro.regina);
                field.player.facing = Direction::Left;
                field.update(Input::default().with(platform::Button::Left));
                intro.phase = IntroPhase::StandingUp;
            }
            IntroPhase::StandingUp if !field.player.walking => {
                self.screen = Screen::Field;
                Self::play(
                    &mut self.sound,
                    &self.data,
                    &self.extensions,
                    GameSound::FirstRoomMusic,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Issues the next order of the intro's path when its wait has passed
    /// (Regina's orders also wait for her to stand still); returns whether
    /// the path has ended, its closing wait included.
    fn follow_path(field: &mut Field, intro: &mut IntroState) -> bool {
        let Some((_, order)) = intro.path.get(intro.step) else {
            if intro.wait > 0 {
                intro.wait -= 1;
            }
            return intro.wait == 0 && field.npc_idle(intro.regina);
        };
        if matches!(order, IntroOrder::Regina(_)) && !field.npc_idle(intro.regina) {
            return false;
        }
        if intro.wait > 0 {
            intro.wait -= 1;
            return false;
        }
        match *order {
            IntroOrder::Regina(command) => {
                field.command_npc(intro.regina, command);
            }
            IntroOrder::Player(direction) => field.player.facing = direction,
        }
        intro.step += 1;
        intro.wait = match intro.path.get(intro.step) {
            Some(next) => next.0,
            None => match intro.phase {
                IntroPhase::Leaving => INTRO_STAND_UP_DELAY,
                _ => INTRO_DIALOGUE_DELAY,
            },
        };
        false
    }

    fn update_field(&mut self, input: Input) -> Result<(), GameError> {
        let Some(field) = self.field.as_mut() else {
            return Ok(());
        };
        if let Some((id, delay)) = self.pending_talk {
            self.pending_talk = if delay > 1 {
                Some((id, delay - 1))
            } else {
                self.dialogue.start(id)?;
                None
            };
        } else if self.dialogue.is_done() {
            match field.update(input) {
                Some(FieldEvent::Exit(exit)) => {
                    let from = field.map();
                    let warp = field.warp(&self.data, exit)?;
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
                    Self::play(
                        &mut self.sound,
                        &self.data,
                        &self.extensions,
                        GameSound::Door,
                    )?;
                    let music = self
                        .extensions
                        .borrow()
                        .music_for_map(arrived)
                        .or_else(|| self.data.map_music(arrived));
                    if let Some(music) = music {
                        Self::emit(&self.extensions, &Event::SoundRequested(music));
                        self.sound.play_if_changed(music)?;
                    }
                }
                Some(FieldEvent::Talk { npc, dialogue: id }) => {
                    Self::emit(&self.extensions, &Event::Talk { npc, dialogue: id });
                    self.pending_talk = Some((id, TALK_START_DELAY));
                }
                None => {}
            }
        } else {
            self.dialogue
                .update(self.data.bytes(), input, &mut self.windows)?;
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
            Screen::Loading(_) => {
                frame.fill(Rgb::default());
                darken(frame, FADE_STEPS);
            }
            Screen::Intro(intro) => {
                if let Some(field) = &self.field {
                    field.draw(frame);
                }
                self.windows.draw(frame, &self.skin, &self.painter);
                darken(frame, intro.darkness());
            }
            Screen::Field => {
                if let Some(field) = &self.field {
                    field.draw(frame);
                }
                self.windows.draw(frame, &self.skin, &self.painter);
            }
            Screen::Menu(menu) => menu.draw(frame, &self.windows, &self.skin, &self.painter),
            Screen::Continuing(continuing) => {
                match continuing.phase {
                    ContinuePhase::Holding(_) | ContinuePhase::FadingOut(_) => {
                        continuing.title.draw(frame);
                    }
                    ContinuePhase::FadingIn(_) => {
                        if let Some(field) = &self.field {
                            field.draw(frame);
                        }
                    }
                    ContinuePhase::Black(_)
                    | ContinuePhase::Notice
                    | ContinuePhase::AfterNotice(_) => frame.fill(Rgb::default()),
                }
                darken(frame, continuing.darkness());
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
