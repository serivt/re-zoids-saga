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
use gba_runtime::ppu::{FADE_STEPS, SCREEN_HEIGHT, SCREEN_WIDTH, darken};
use platform::{Frame, Input, Rgb};
use thiserror::Error;

use crate::boot::{LogoScreen, NameEntry, TitleChoice, TitleScreen};
use crate::field::{Direction, Field, FieldError, FieldEvent, NpcCommand};
use crate::script::{ScriptError, ScriptRunner};
use crate::windows::{DEFAULT_PLAYER_NAME, ScriptWindows};
use crate::{ScriptHost, TextPainter, WindowPainter};

const TALK_START_DELAY: u32 = 3;
const NAME_TO_ROOM_BLACK_FRAMES: u32 = 60;
const INTRO_DIALOGUE: usize = 40;
const INTRO_PLAYER_CELL: (usize, usize) = (6, 2);
const REGINA_SPRITE: usize = 0x99;
const REGINA_START: (usize, usize) = (6, 4);
/// Regina's walk before the first line, as `(frames to wait, order)`.
const INTRO_PATH: [(u32, NpcCommand); 12] = [
    (15, NpcCommand::Step(Direction::Left)),
    (0, NpcCommand::Step(Direction::Left)),
    (0, NpcCommand::Step(Direction::Left)),
    (25, NpcCommand::Face(Direction::Right)),
    (31, NpcCommand::Step(Direction::Right)),
    (0, NpcCommand::Step(Direction::Right)),
    (0, NpcCommand::Step(Direction::Right)),
    (0, NpcCommand::Step(Direction::Right)),
    (0, NpcCommand::Step(Direction::Right)),
    (0, NpcCommand::Face(Direction::Left)),
    (61, NpcCommand::Step(Direction::Left)),
    (0, NpcCommand::Step(Direction::Left)),
];
const INTRO_FACE_UP_DELAY: u32 = 11;
const INTRO_DIALOGUE_DELAY: u32 = 60;

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
}

struct IntroState {
    regina: usize,
    step: usize,
    wait: u32,
    dialogue_started: bool,
    stood_up: bool,
}

/// The running game.
pub struct Game<'rom> {
    rom: &'rom [u8],
    painter: TextPainter<'rom>,
    skin: WindowPainter,
    windows: ScriptWindows<'rom>,
    dialogue: ScriptRunner,
    field: Option<Field>,
    screen: Screen,
    pending_talk: Option<(usize, u32)>,
    player_name: String,
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
            windows: ScriptWindows::new(rom, DEFAULT_PLAYER_NAME),
            dialogue: ScriptRunner::new(dialogue.iter().map(|string| string.offset).collect()),
            field: None,
            screen: Screen::Loading(0),
            pending_talk: None,
            player_name: DEFAULT_PLAYER_NAME.to_owned(),
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
        match &mut self.screen {
            Screen::Logo(logo) => {
                if logo.update() {
                    self.screen = Screen::Title(TitleScreen::new(self.rom)?);
                }
            }
            Screen::Title(title) => {
                if title.update(self.rom, input, &mut self.windows)? == Some(TitleChoice::NewGame) {
                    let entry = NameEntry::new(self.rom, &self.player_name, input)?;
                    entry.open(&mut self.windows);
                    self.screen = Screen::NameEntry(entry);
                }
            }
            Screen::NameEntry(entry) => {
                if entry.update(self.rom, input, &mut self.windows)? {
                    self.player_name = entry.name();
                    self.windows.set_player_name(&self.player_name);
                    self.windows.close_window(None);
                    self.screen = Screen::Loading(0);
                }
            }
            Screen::Loading(frames) => {
                *frames += 1;
                if *frames >= NAME_TO_ROOM_BLACK_FRAMES {
                    self.start_intro()?;
                }
            }
            Screen::Intro(_) => self.update_intro(input)?,
            Screen::Field => self.update_field(input)?,
        }
        Ok(())
    }

    fn start_intro(&mut self) -> Result<(), GameError> {
        let mut field = Field::load(self.rom, FIRST_ROOM_MAP, INTRO_PLAYER_CELL)?;
        field.player.facing = Direction::Up;
        let regina = field.spawn_npc(self.rom, REGINA_SPRITE, REGINA_START, Direction::Up)?;
        self.field = Some(field);
        self.screen = Screen::Intro(IntroState {
            regina,
            step: 0,
            wait: INTRO_PATH[0].0,
            dialogue_started: false,
            stood_up: false,
        });
        Ok(())
    }

    fn update_intro(&mut self, input: Input) -> Result<(), GameError> {
        let Some(field) = self.field.as_mut() else {
            return Ok(());
        };
        let Screen::Intro(intro) = &mut self.screen else {
            return Ok(());
        };
        if intro.dialogue_started {
            if self.dialogue.update(self.rom, input, &mut self.windows)? {
                if !intro.stood_up {
                    intro.stood_up = true;
                    field.player.facing = Direction::Left;
                    field.update(Input::default().with(platform::Button::Left));
                    return Ok(());
                }
                if !field.player.walking {
                    field.npcs.retain(|npc| npc.sheet.tag != "ch01");
                    self.screen = Screen::Field;
                }
            }
            field.update(Input::default());
            return Ok(());
        }
        field.update(Input::default());
        if !field.npc_idle(intro.regina) {
            return Ok(());
        }
        if intro.wait > 0 {
            intro.wait -= 1;
            return Ok(());
        }
        if let Some((_, command)) = INTRO_PATH.get(intro.step) {
            field.command_npc(intro.regina, *command);
            intro.step += 1;
            intro.wait = INTRO_PATH
                .get(intro.step)
                .map_or(INTRO_FACE_UP_DELAY, |next| next.0);
        } else if intro.step == INTRO_PATH.len() {
            field.command_npc(intro.regina, NpcCommand::Face(Direction::Up));
            intro.step += 1;
            intro.wait = INTRO_DIALOGUE_DELAY;
        } else {
            self.dialogue.start(INTRO_DIALOGUE)?;
            intro.dialogue_started = true;
        }
        Ok(())
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
            Screen::Intro(_) | Screen::Field => {
                if let Some(field) = &self.field {
                    field.draw(frame);
                }
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
