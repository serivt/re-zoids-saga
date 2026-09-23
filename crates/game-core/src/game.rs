//! The whole game as one state machine: logo, title, name entry, the
//! opening cutscene in the first room, then the field with its
//! conversations and exits.
//!
//! The opening follows the original's entity trace: the player sits at
//! the desk on metatile (6, 2) facing up, Regina starts on (6, 4), walks
//! three steps left, turns right and walks five, turns left and walks
//! two, faces the player, and the first dialogue string runs. Afterwards
//! the player stands up onto (5, 2) and control begins.

use extraction::saga::{self, BootError, FIRST_ROOM_MAP, PLAYER_START, SpriteSheetError};
use formats::m4a::M4aError;
use gba_runtime::apu::SoundEngine;
use gba_runtime::ppu::{FADE_STEPS, SCREEN_HEIGHT, SCREEN_WIDTH, darken};
use platform::{Button, Frame, Input, Rgb};
use thiserror::Error;

use crate::boot::{LogoScreen, NameEntry, TitleChoice, TitleScreen};
use crate::field::{Direction, Field, FieldError, FieldEvent, NpcCommand};
use crate::menu::{Party, PauseMenu};
use crate::script::{ScriptError, ScriptRunner};
use crate::text::TextMetrics;
use crate::translation::{DIALOGUE_TABLE, Translation};
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
}

enum Screen {
    Logo(LogoScreen),
    Title(TitleScreen),
    NameEntry(NameEntry),
    Loading(u32),
    Intro(IntroState),
    Field,
    Menu(PauseMenu),
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
    rom: &'rom [u8],
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
        game.screen = Screen::Logo(LogoScreen::new(rom)?);
        Ok(game)
    }

    /// Starts the game in the first room, skipping the opening.
    ///
    /// # Errors
    ///
    /// Returns [`GameError`] when the room or text assets cannot be read.
    pub fn in_first_room(rom: &'rom [u8]) -> Result<Self, GameError> {
        let mut game = Self::bare(rom)?;
        game.field = Some(Field::load(rom, FIRST_ROOM_MAP, PLAYER_START)?);
        game.screen = Screen::Field;
        game.sound.play(saga::MUSIC_FIRST_ROOM)?;
        Ok(game)
    }

    fn bare(rom: &'rom [u8]) -> Result<Self, GameError> {
        let (glyphs, fallback) =
            saga::font(rom).map_err(|error| GameError::Text(error.to_string()))?;
        let skin = saga::window_skin(rom).map_err(|error| GameError::Text(error.to_string()))?;
        let dialogue = saga::string_table("dialogue")
            .ok_or_else(|| GameError::Text("no dialogue table".to_owned()))?
            .read(rom)
            .map_err(|error| GameError::Text(error.to_string()))?;
        Ok(Self {
            rom,
            painter: TextPainter::new(rom, glyphs, Some(fallback)),
            skin: WindowPainter::new(skin.tiles, &skin.palette),
            windows: {
                let mut windows = ScriptWindows::new(rom, DEFAULT_PLAYER_NAME);
                windows.set_metrics(TextMetrics::standard());
                windows
            },
            sound: SoundEngine::new(rom, saga::SONG_TABLE, saga::SONG_COUNT, saga::MASTER_VOLUME),
            dialogue: ScriptRunner::named(
                DIALOGUE_TABLE,
                dialogue.iter().map(|string| string.offset).collect(),
            ),
            field: None,
            screen: Screen::Loading(0),
            pending_talk: None,
            player_name: DEFAULT_PLAYER_NAME.to_owned(),
            party: Party::default(),
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
        match &mut self.screen {
            Screen::Logo(logo) => {
                if logo.update() {
                    self.screen = Screen::Title(TitleScreen::new(self.rom)?);
                    self.sound.play(saga::MUSIC_TITLE)?;
                }
            }
            Screen::Title(title) => {
                if start && !self.windows.any_open() {
                    self.sound.play(saga::SOUND_TITLE_START)?;
                }
                if title.update(self.rom, input, &mut self.windows)? == Some(TitleChoice::NewGame) {
                    let mut entry = NameEntry::new(self.rom, &self.player_name, input)?;
                    entry.open(&mut self.windows);
                    self.screen = Screen::NameEntry(entry);
                    self.sound.play(saga::MUSIC_NAME_ENTRY)?;
                }
            }
            Screen::NameEntry(entry) => {
                if entry.update(self.rom, input, &mut self.windows)? {
                    self.player_name = entry.name();
                    self.windows.set_player_name(&self.player_name);
                    self.windows.close_window(None);
                    self.screen = Screen::Loading(0);
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
                    let mut menu = PauseMenu::new(self.rom, self.party.clone())?;
                    menu.open(self.rom, &mut self.windows)?;
                    self.screen = Screen::Menu(menu);
                } else {
                    self.update_field(input)?;
                }
            }
            Screen::Menu(menu) => {
                if menu.update(self.rom, input, &mut self.windows)? {
                    self.party = menu.party();
                    self.screen = Screen::Field;
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
    /// hold.
    ///
    /// # Errors
    ///
    /// Returns [`GameError`] when the script tables cannot be read.
    pub fn set_translation(
        &mut self,
        mut translation: Translation,
    ) -> Result<Vec<String>, GameError> {
        let problems = translation.fit(self.rom, self.painter.metrics())?;
        self.windows.set_translation(translation);
        Ok(problems)
    }

    /// The samples of the frame the last update produced, stereo
    /// interleaved at [`gba_runtime::apu::SAMPLE_RATE`].
    #[must_use]
    pub fn audio(&self) -> &[i16] {
        self.sound.output()
    }

    fn start_intro(&mut self) -> Result<(), GameError> {
        let mut field = Field::load(self.rom, FIRST_ROOM_MAP, INTRO_PLAYER_CELL)?;
        field.player.facing = Direction::Up;
        let regina = field.spawn_npc(self.rom, REGINA_SPRITE, REGINA_START, Direction::Up)?;
        self.field = Some(field);
        let mut intro = IntroState {
            regina,
            path: &INTRO_ARRIVAL,
            step: 0,
            wait: 0,
            phase: IntroPhase::Arriving,
        };
        intro.follow(&INTRO_ARRIVAL);
        self.screen = Screen::Intro(intro);
        self.sound.play(saga::MUSIC_OPENING)?;
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
                if self.dialogue.update(self.rom, input, &mut self.windows)? {
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
                if self.dialogue.update(self.rom, input, &mut self.windows)? {
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
                self.sound.play(saga::MUSIC_FIRST_ROOM)?;
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
                    field.warp(self.rom, exit)?;
                    self.sound.play(saga::SOUND_DOOR)?;
                    if let Some(music) = saga::map_music(self.rom, field.map()) {
                        self.sound.play_if_changed(music)?;
                    }
                }
                Some(FieldEvent::Talk { dialogue: id, .. }) => {
                    self.pending_talk = Some((id, TALK_START_DELAY));
                }
                None => {}
            }
        } else {
            self.dialogue.update(self.rom, input, &mut self.windows)?;
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
        }
    }
}

/// The screen size the game draws at.
#[must_use]
pub const fn screen_size() -> (usize, usize) {
    (SCREEN_WIDTH, SCREEN_HEIGHT)
}
