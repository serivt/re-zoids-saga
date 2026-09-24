//! The Zoid and character guides the title's options and the pause
//! menu's 図鑑 open (see `docs/guide.md`).
//!
//! Both follow the original's code: a menu script of the system table
//! picks a row of up to 20 entries (an army's type, or a series' group),
//! then each entry is a short script of the guide's own table that prints
//! into windows a helper script opened. A picks next, previous or back in
//! a popup, L and R page directly, B goes back to the menu. Changing entry
//! fades out 10 frames after the key over 16 frames, keeps the screen
//! black while the next one loads and fades back in over 8; the menus fade
//! in over 16. The black lasts what the original's loading took, measured
//! frame by frame for each transition. In the Zoid
//! guide the picture of the Zoid then slides in from the right over its
//! backdrop, 8 pixels a frame; a Zoid the player has not seen is drawn as
//! a silhouette over the question-mark entry. The character guide shows
//! the portrait and map sprite of known characters only and skips the
//! rest.

use extraction::saga::{BootError, SpriteSheet, SpriteSheetError};
use extraction::saga_guide::{
    BACKDROP_COLORS, CHARACTER_COUNT, CHARACTER_GROUP_FLAGS, CHARACTER_GROUPS,
    CHARACTER_GUIDE_CLEAR, CHARACTER_GUIDE_ENTRIES, CHARACTER_GUIDE_FLAGS, CHARACTER_GUIDE_MENU,
    CHARACTER_GUIDE_OPEN, CHARACTER_GUIDE_POPUP, CHARACTER_GUIDE_SCRIPTS, CHARACTER_SERIES,
    CHARACTER_SERIES_FLAGS, CHARACTER_SPRITE_BASE, GUIDE_ROW, GuidePicture, PICTURE_TILES,
    SYSTEM_SCRIPTS, ZOID_ARMIES, ZOID_COLORS, ZOID_GUIDE_CLEAR, ZOID_GUIDE_ENTRIES,
    ZOID_GUIDE_MENU, ZOID_GUIDE_OPEN, ZOID_GUIDE_POPUP, ZOID_GUIDE_SCRIPTS, ZOID_GUIDE_UNKNOWN,
    ZOID_TYPES, ZoidPart,
};
use extraction::string_table::StringTableError;
use formats::progress::{character_known, zoid_seen};
use gba_runtime::ppu::{FADE_STEPS, FullPalette, darken, draw_background_256};
use platform::{Button, Frame, Input, Rgb};
use thiserror::Error;

use crate::boot::TitleScreen;
use crate::data::GameData;
use crate::field::current_frame;
use crate::script::{ScriptError, ScriptRunner};
use crate::sprite::draw_sprite;
use crate::windows::ScriptWindows;
use crate::{ScriptHost, TextPainter, WindowPainter};

const OPENING_HOLD_FRAMES: u32 = 3;
const OPENING_BLACK_FRAMES: u32 = 23;
/// Black frames before the menu when the pause menu opened the guide: it
/// starts 36 frames after the choice instead of 42.
const PAUSE_OPENING_BLACK_FRAMES: u32 = 16;
const SCRIPT_HOLD_FRAMES: u32 = 7;
const POPUP_HOLD_FRAMES: u32 = 6;
const KEY_HOLD_FRAMES: u32 = 6;
const FADE_OUT_FRAMES: u32 = 16;
const FADE_IN_FRAMES: u32 = 8;
/// Frames from the menu script's start until the menu is fully bright: it
/// stays black two frames, then brightens a level a frame from 31, visibly
/// over the last 16.
const MENU_FADE_IN_FRAMES: u32 = 33;
/// The first menu after the pause menu opened the guide stays black
/// five frames rather than two.
const PAUSE_MENU_FADE_IN_FRAMES: u32 = 37;
const MENU_BLACK_FRAMES: u32 = 22;
const EXIT_BLACK_FRAMES: u32 = 44;
/// Black frames after leaving when the pause menu opened the guide.
const PAUSE_EXIT_BLACK_FRAMES: u32 = 18;
const ZOID_FIRST_BLACK_FRAMES: u32 = 48;
const ZOID_NEXT_BLACK_FRAMES: u32 = 33;
const CHARACTER_FIRST_BLACK_FRAMES: u32 = 50;
const CHARACTER_NEXT_BLACK_FRAMES: u32 = 37;
const SETTLE_FRAMES: u32 = 13;
/// The slide lasts 17 frames; `u` frames from its end the Zoid is
/// `u × (150 − u) / 18` pixels right of its place, a fit of the offsets
/// measured on the original (it speeds up from about 6.5 to 8.3 pixels a
/// frame).
const SLIDE_FRAMES: u32 = 17;
const SLIDE_CURVE: u32 = 150;
const SLIDE_SCALE: u32 = 18;
/// Where the Zoid stands once it has slid in: right of the 14-tile
/// windows.
const PICTURE_X: usize = 112;
/// The Zoid's layer is 64 tiles wide, the picture in its first 16.
const LAYER_COLUMNS: usize = 64;
const LAYER_WIDTH: usize = LAYER_COLUMNS * 8;
/// Where the character's map sprite stands: the 32×32 frame's corner at
/// (80, 16), inside the second window.
const SPRITE_ANCHOR: (i32, i32) = (96, 32);
const SPRITE_FACING_DOWN: usize = 1;
const EXIT: u16 = 0xFF;
/// What the popup leaves in var0: next, previous, back to the menu; B
/// leaves 0 and closes it.
const CHOICE_NEXT: u16 = 1;
const CHOICE_PREVIOUS: u16 = 2;
const CHOICE_MENU: u16 = 0xFF;
const RUN_LIMIT: usize = 10_000;
const NO_TILE: u16 = 0x3FF;
const CHANNEL_MASK: u16 = 0x1F;
const SILHOUETTE_SHIFT: u16 = 3;

/// Which guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideKind {
    /// ゾイド図鑑.
    Zoids,
    /// キャラクター図鑑.
    Characters,
}

/// Why a guide could not go on.
#[derive(Debug, Error)]
pub enum GuideError {
    /// A script failed.
    #[error(transparent)]
    Script(#[from] ScriptError),
    /// A script table could not be read.
    #[error(transparent)]
    Table(#[from] StringTableError),
    /// A picture could not be read.
    #[error(transparent)]
    Picture(#[from] BootError),
    /// A map sprite could not be read.
    #[error(transparent)]
    Sprite(#[from] SpriteSheetError),
}

/// The key the entry screen reacts to this frame, in the order it checks
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Popup,
    Next,
    Previous,
    Back,
}

/// What comes after the screen goes black.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
    Entry { first: bool },
    Menu,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Opening { frames: u32, black: bool },
    Menu { fading_in: u32 },
    Holding { frames: u32, next: Next },
    FadingOut { frames: u32, next: Next },
    Black { frames: u32, next: Next },
    FadingIn(u32),
    Settling(u32),
    Sliding(u32),
    Viewing,
    Popup,
    Done,
}

/// What a Zoid's entry shows behind the windows.
struct ZoidView {
    palette: FullPalette,
    picture: GuidePicture,
    backdrop: Option<GuidePicture>,
    backdrop_color: Rgb,
    parts: Vec<ZoidPart>,
}

/// What the guide opened over, which fades out before it and which it
/// returns to.
pub enum Cover {
    /// The title's options; the guide draws the title while it fades.
    Title(Box<TitleScreen>),
    /// The pause menu's 図鑑, which draws itself while the guide fades.
    PauseMenu,
}

/// A guide screen.
pub struct Guide {
    kind: GuideKind,
    menu: ScriptRunner,
    pages: ScriptRunner,
    state: Vec<u8>,
    characters: Vec<usize>,
    title: Option<TitleScreen>,
    from_pause: bool,
    menu_fade: u32,
    phase: Phase,
    row: usize,
    entry: usize,
    zoid: Option<ZoidView>,
    sprite: Option<SpriteSheet>,
    elapsed: u32,
    previous: Input,
}

impl Guide {
    /// Opens `kind` over `cover`, which fades out first; `state` is the
    /// game-state block that says what the player has seen.
    ///
    /// # Errors
    ///
    /// Returns [`GuideError`] when a script table cannot be read.
    pub fn new(
        data: &GameData<'_>,
        kind: GuideKind,
        state: Vec<u8>,
        cover: Cover,
    ) -> Result<Self, GuideError> {
        let pages = match kind {
            GuideKind::Zoids => ZOID_GUIDE_SCRIPTS,
            GuideKind::Characters => CHARACTER_GUIDE_SCRIPTS,
        };
        Ok(Self {
            kind,
            menu: ScriptRunner::named(SYSTEM_SCRIPTS.name, SYSTEM_SCRIPTS.offsets(data.bytes())?),
            pages: ScriptRunner::named(pages.name, pages.offsets(data.bytes())?),
            characters: data.character_entries()?,
            state,
            from_pause: matches!(cover, Cover::PauseMenu),
            menu_fade: if matches!(cover, Cover::PauseMenu) {
                PAUSE_MENU_FADE_IN_FRAMES
            } else {
                MENU_FADE_IN_FRAMES
            },
            title: match cover {
                Cover::Title(title) => Some(*title),
                Cover::PauseMenu => None,
            },
            phase: Phase::Opening {
                frames: 0,
                black: false,
            },
            row: 0,
            entry: 0,
            zoid: None,
            sprite: None,
            elapsed: 0,
            previous: Input::default(),
        })
    }

    /// Whether the player has left the guide.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.phase == Phase::Done
    }

    /// Advances one frame.
    ///
    /// # Errors
    ///
    /// Returns [`GuideError`] when a script, picture or sprite fails.
    pub fn update(
        &mut self,
        data: &GameData<'_>,
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), GuideError> {
        let pressed = |button: Button| input.is_held(button) && !self.previous.is_held(button);
        let key = [
            (Button::A, Key::Popup),
            (Button::R, Key::Next),
            (Button::L, Key::Previous),
            (Button::B, Key::Back),
        ]
        .into_iter()
        .find_map(|(button, key)| pressed(button).then_some(key));
        self.previous = input;
        self.elapsed = self.elapsed.wrapping_add(1);
        let rom = data.bytes();
        self.phase = match self.phase {
            Phase::Opening { frames, black } => self.open(frames, black, windows)?,
            Phase::Menu { fading_in } => {
                if self.menu.update(rom, input, windows)? {
                    self.choose(windows)
                } else {
                    Phase::Menu {
                        fading_in: (fading_in + 1).min(self.menu_fade),
                    }
                }
            }
            Phase::Holding { frames: 0, next } => Phase::FadingOut { frames: 0, next },
            Phase::Holding { frames, next } => Phase::Holding {
                frames: frames - 1,
                next,
            },
            Phase::FadingOut { frames, next } if frames + 1 >= FADE_OUT_FRAMES => {
                windows.close_window(None);
                self.after_fade(data, next, windows)?
            }
            Phase::FadingOut { frames, next } => Phase::FadingOut {
                frames: frames + 1,
                next,
            },
            Phase::Black { frames, next } if frames + 1 >= self.black_frames(next) => {
                self.after_black(next, windows)?
            }
            Phase::Black { frames, next } => Phase::Black {
                frames: frames + 1,
                next,
            },
            Phase::FadingIn(frames) if frames + 1 >= FADE_IN_FRAMES => match self.kind {
                GuideKind::Zoids => Phase::Settling(0),
                GuideKind::Characters => Phase::Viewing,
            },
            Phase::FadingIn(frames) => Phase::FadingIn(frames + 1),
            Phase::Settling(frames) if frames + 1 >= SETTLE_FRAMES => Phase::Sliding(0),
            Phase::Settling(frames) => Phase::Settling(frames + 1),
            Phase::Sliding(frames) if slide_offset(frames + 1) == 0 => Phase::Viewing,
            Phase::Sliding(frames) => Phase::Sliding(frames + 1),
            Phase::Viewing => self.view(key)?,
            Phase::Popup => {
                if self.pages.update(rom, input, windows)? {
                    self.popup_choice()
                } else {
                    Phase::Popup
                }
            }
            Phase::Done => Phase::Done,
        };
        Ok(())
    }

    /// The title held, faded out and the screen black, then the menu.
    fn open(
        &mut self,
        frames: u32,
        black: bool,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<Phase, GuideError> {
        let frames = frames + 1;
        if !black && frames >= OPENING_HOLD_FRAMES + FADE_OUT_FRAMES {
            self.title = None;
            return Ok(Phase::Opening {
                frames: 0,
                black: true,
            });
        }
        let black_frames = if self.from_pause {
            PAUSE_OPENING_BLACK_FRAMES
        } else {
            OPENING_BLACK_FRAMES
        };
        if black && frames >= black_frames {
            self.open_menu(windows)?;
            return Ok(Phase::Menu { fading_in: 0 });
        }
        Ok(Phase::Opening { frames, black })
    }

    fn black_frames(&self, next: Next) -> u32 {
        match (self.kind, next) {
            (_, Next::Menu) => MENU_BLACK_FRAMES,
            (_, Next::Exit) if self.from_pause => PAUSE_EXIT_BLACK_FRAMES,
            (_, Next::Exit) => EXIT_BLACK_FRAMES,
            (GuideKind::Zoids, Next::Entry { first: true }) => ZOID_FIRST_BLACK_FRAMES,
            (GuideKind::Zoids, Next::Entry { first: false }) => ZOID_NEXT_BLACK_FRAMES,
            (GuideKind::Characters, Next::Entry { first: true }) => CHARACTER_FIRST_BLACK_FRAMES,
            (GuideKind::Characters, Next::Entry { first: false }) => CHARACTER_NEXT_BLACK_FRAMES,
        }
    }

    fn open_menu(&mut self, windows: &mut ScriptWindows<'_>) -> Result<(), GuideError> {
        windows.close_window(None);
        let script = match self.kind {
            GuideKind::Zoids => ZOID_GUIDE_MENU,
            GuideKind::Characters => {
                self.set_character_flags(windows);
                CHARACTER_GUIDE_MENU
            }
        };
        self.menu.start(script)?;
        Ok(())
    }

    /// The flags the character menu reads, set as the original's code does
    /// before running it: every series with a known character sets its
    /// flag, and each of its groups with one sets the flags after it.
    fn set_character_flags(&self, windows: &mut ScriptWindows<'_>) {
        for flag in 0..CHARACTER_GUIDE_FLAGS {
            windows.set_flag(flag, false);
        }
        for (series, base) in CHARACTER_SERIES_FLAGS.iter().enumerate() {
            let mut groups = 0;
            for (group, flag) in (1..=CHARACTER_GROUPS).zip(base + 1..) {
                if self.row_has_known(character_row(series, group - 1)) {
                    windows.set_flag(flag, true);
                    groups += 1;
                }
            }
            if groups > 0 {
                windows.set_flag(*base, true);
            }
        }
    }

    fn row_has_known(&self, row: usize) -> bool {
        (0..GUIDE_ROW).any(|entry| self.entry_known(row, entry))
    }

    /// Reads the menu's answer: the army and type, or series and group, or
    /// leaving the guide.
    fn choose(&mut self, windows: &ScriptWindows<'_>) -> Phase {
        let vars = *self.menu.saved_vars();
        let (first, second) = (vars[7], vars[6]);
        if first == EXIT {
            let frames = if self.from_pause {
                SCRIPT_HOLD_FRAMES - 1
            } else {
                SCRIPT_HOLD_FRAMES
            };
            return Phase::Holding {
                frames,
                next: Next::Exit,
            };
        }
        let row = match self.kind {
            GuideKind::Zoids => zoid_row(usize::from(first), usize::from(second)),
            GuideKind::Characters => {
                let series = nth_set(windows, &CHARACTER_SERIES_FLAGS, usize::from(first));
                let (low, high) = CHARACTER_GROUP_FLAGS[series.min(CHARACTER_SERIES - 1)];
                let candidates: Vec<u16> = (low..=high).collect();
                let group = nth_set(windows, &candidates, usize::from(second));
                character_row(series, group)
            }
        };
        self.row = row;
        self.entry = 0;
        Phase::Holding {
            frames: SCRIPT_HOLD_FRAMES,
            next: Next::Entry { first: true },
        }
    }

    fn after_fade(
        &mut self,
        data: &GameData<'_>,
        next: Next,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<Phase, GuideError> {
        if let Next::Entry { .. } = next {
            self.show_entry(data, windows)?;
        }
        Ok(Phase::Black { frames: 0, next })
    }

    fn after_black(
        &mut self,
        next: Next,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<Phase, GuideError> {
        Ok(match next {
            Next::Entry { .. } => Phase::FadingIn(0),
            Next::Menu => {
                self.zoid = None;
                self.sprite = None;
                self.menu_fade = MENU_FADE_IN_FRAMES;
                self.open_menu(windows)?;
                Phase::Menu { fading_in: 0 }
            }
            Next::Exit => Phase::Done,
        })
    }

    /// Runs the entry's scripts into fresh windows and loads what it shows
    /// behind them.
    fn show_entry(
        &mut self,
        data: &GameData<'_>,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), GuideError> {
        let rom = data.bytes();
        if self.kind == GuideKind::Characters {
            self.skip_unknown(true);
        }
        let (open, clear) = match self.kind {
            GuideKind::Zoids => (ZOID_GUIDE_OPEN, ZOID_GUIDE_CLEAR),
            GuideKind::Characters => (CHARACTER_GUIDE_OPEN, CHARACTER_GUIDE_CLEAR),
        };
        self.run(rom, open, windows)?;
        self.run(rom, clear, windows)?;
        self.run(rom, self.row + self.entry, windows)?;
        match self.kind {
            GuideKind::Zoids => {
                let id = usize::from(self.pages.saved_vars()[5]);
                let seen = zoid_seen(&self.state, id);
                if !seen {
                    self.run(rom, ZOID_GUIDE_CLEAR, windows)?;
                    self.run(rom, ZOID_GUIDE_UNKNOWN, windows)?;
                }
                self.zoid = zoid_view(data, id, seen)?;
            }
            GuideKind::Characters => {
                self.sprite = match self.character(self.row + self.entry) {
                    Some(character) => Some(data.sprite_sheet(CHARACTER_SPRITE_BASE + character)?),
                    None => None,
                };
            }
        }
        Ok(())
    }

    fn run(
        &mut self,
        rom: &[u8],
        script: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), GuideError> {
        self.pages.start(script)?;
        for _ in 0..RUN_LIMIT {
            if self.pages.update(rom, Input::default(), windows)? {
                break;
            }
        }
        Ok(())
    }

    fn view(&mut self, key: Option<Key>) -> Result<Phase, GuideError> {
        let next = match key {
            None => return Ok(Phase::Viewing),
            Some(Key::Popup) => {
                let popup = match self.kind {
                    GuideKind::Zoids => ZOID_GUIDE_POPUP,
                    GuideKind::Characters => CHARACTER_GUIDE_POPUP,
                };
                self.pages.start(popup)?;
                return Ok(Phase::Popup);
            }
            Some(Key::Next) => self.step(true),
            Some(Key::Previous) => self.step(false),
            Some(Key::Back) => Next::Menu,
        };
        Ok(Phase::Holding {
            frames: KEY_HOLD_FRAMES,
            next,
        })
    }

    fn popup_choice(&mut self) -> Phase {
        let next = match self.pages.saved_vars()[0] {
            CHOICE_NEXT => self.step(true),
            CHOICE_PREVIOUS => self.step(false),
            CHOICE_MENU => Next::Menu,
            _ => return Phase::Viewing,
        };
        Phase::Holding {
            frames: POPUP_HOLD_FRAMES,
            next,
        }
    }

    /// Moves to the next or previous entry of the row, wrapping around; the
    /// character guide passes over characters the player does not know.
    fn step(&mut self, forward: bool) -> Next {
        self.entry = self.neighbour(self.entry, forward);
        if self.kind == GuideKind::Characters {
            self.skip_unknown(forward);
        }
        Next::Entry { first: false }
    }

    fn neighbour(&self, entry: usize, forward: bool) -> usize {
        let present = |entry: usize| self.pages.string_offset(self.row + entry).is_some();
        if forward {
            if entry + 1 < GUIDE_ROW && present(entry + 1) {
                entry + 1
            } else {
                0
            }
        } else if entry > 0 {
            entry - 1
        } else {
            (0..GUIDE_ROW)
                .rev()
                .find(|entry| present(*entry))
                .unwrap_or(0)
        }
    }

    fn skip_unknown(&mut self, forward: bool) {
        for _ in 0..GUIDE_ROW {
            if self.entry_known(self.row, self.entry) {
                return;
            }
            self.entry = self.neighbour(self.entry, forward);
        }
    }

    /// Whether an entry exists and, in the character guide, whether its
    /// character is known.
    fn entry_known(&self, row: usize, entry: usize) -> bool {
        match self.kind {
            GuideKind::Zoids => self.pages.string_offset(row + entry).is_some(),
            GuideKind::Characters => self
                .character(row + entry)
                .is_some_and(|character| character_known(&self.state, character)),
        }
    }

    /// The character an entry string describes: its place in the table
    /// of every character's entry.
    fn character(&self, string: usize) -> Option<usize> {
        let offset = self.pages.string_offset(string)?;
        self.characters
            .iter()
            .take(CHARACTER_COUNT)
            .position(|entry| *entry == offset)
    }

    /// Whether what the guide opened over still shows, fading out.
    #[must_use]
    pub fn covered(&self) -> bool {
        matches!(self.phase, Phase::Opening { black: false, .. })
    }

    /// Darkness of the current frame, 0 (full) to `FADE_STEPS` (black).
    #[must_use]
    pub fn darkness(&self) -> u8 {
        let level = match self.phase {
            Phase::Black { .. } | Phase::Opening { black: true, .. } => u32::from(FADE_STEPS),
            Phase::Opening { frames, .. } => {
                frames.saturating_sub(OPENING_HOLD_FRAMES) * u32::from(FADE_STEPS) / FADE_OUT_FRAMES
            }
            Phase::FadingOut { frames, .. } => frames * u32::from(FADE_STEPS) / FADE_OUT_FRAMES,
            Phase::Menu { fading_in } => self
                .menu_fade
                .saturating_sub(fading_in)
                .min(u32::from(FADE_STEPS)),
            Phase::FadingIn(frames) => fade_in_level(frames, FADE_IN_FRAMES),
            _ => 0,
        };
        u8::try_from(level).unwrap_or(FADE_STEPS)
    }

    /// Draws the guide for the current frame.
    pub fn draw(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
    ) {
        if let (Phase::Opening { black: false, .. }, Some(title)) = (self.phase, &self.title) {
            title.draw(frame);
            darken(frame, self.darkness());
            return;
        }
        frame.fill(Rgb::default());
        let showing_entry = !matches!(self.phase, Phase::Menu { .. } | Phase::Done)
            && !matches!(
                self.phase,
                Phase::Black {
                    next: Next::Menu | Next::Exit,
                    ..
                }
            );
        if showing_entry {
            if let Some(zoid) = &self.zoid {
                self.draw_zoid(frame, zoid);
            }
        }
        windows.draw(frame, skin, painter);
        if showing_entry {
            if let Some(sheet) = &self.sprite {
                draw_map_sprite(frame, sheet, self.elapsed);
            }
        }
        darken(frame, self.darkness());
    }

    fn draw_zoid(&self, frame: &mut Frame, zoid: &ZoidView) {
        frame.fill(zoid.backdrop_color);
        if let Some(backdrop) = &zoid.backdrop {
            draw_background_256(
                frame,
                |x, y| picture_entry(backdrop, x % PICTURE_TILES, y),
                |index| backdrop.tiles.tile(index),
                &zoid.palette,
                (0, 0),
                true,
            );
        }
        let offset = match self.phase {
            Phase::Sliding(frames) => Some(slide_offset(frames)),
            Phase::Viewing | Phase::Popup | Phase::Holding { .. } | Phase::FadingOut { .. } => {
                Some(0)
            }
            _ => None,
        };
        let Some(offset) = offset else {
            return;
        };
        let scroll = (LAYER_WIDTH - (PICTURE_X + offset) % LAYER_WIDTH) % LAYER_WIDTH;
        draw_background_256(
            frame,
            |x, y| picture_entry(&zoid.picture, x % LAYER_COLUMNS, y),
            |index| zoid.picture.tiles.tile(index),
            &zoid.palette,
            (scroll, 0),
            true,
        );
        let left = i32::try_from(PICTURE_X + offset).unwrap_or(i32::MAX);
        for part in &zoid.parts {
            draw_part(frame, part, left, self.elapsed);
        }
    }
}

fn picture_entry(picture: &GuidePicture, column: usize, row: usize) -> u16 {
    if column >= PICTURE_TILES || row >= PICTURE_TILES {
        return NO_TILE;
    }
    picture
        .map
        .get(row * PICTURE_TILES + column)
        .copied()
        .unwrap_or(NO_TILE)
}

fn zoid_view(data: &GameData<'_>, id: usize, seen: bool) -> Result<Option<ZoidView>, GuideError> {
    let Some(picture) = data.zoid_picture(id)? else {
        return Ok(None);
    };
    let backdrop = data.zoid_backdrop(id)?;
    let mut parts = data.zoid_parts(id)?;
    if !seen {
        for part in &mut parts {
            part.palette = part.palette.map(silhouette);
        }
    }
    let colors: Vec<u16> = picture
        .palette
        .iter()
        .map(|color| if seen { *color } else { silhouette(*color) })
        .collect();
    let mut palette = FullPalette::from_bgr555(&[]);
    palette.write(ZOID_COLORS, &colors);
    if let Some(backdrop) = &backdrop {
        palette.write(BACKDROP_COLORS, &backdrop.palette);
    }
    Ok(Some(ZoidView {
        backdrop_color: palette.color(0),
        palette,
        picture,
        backdrop,
        parts,
    }))
}

/// Draws a part's first animation over the picture whose top-left corner
/// is at `left`, 0.
fn draw_part(frame: &mut Frame, part: &ZoidPart, left: i32, elapsed: u32) {
    let Some(steps) = part.animations.first() else {
        return;
    };
    let cycle: u32 = steps.iter().map(|step| step.duration.max(1)).sum();
    let mut remaining = elapsed % cycle.max(1);
    let Some(step) = steps.iter().find(|step| {
        let frames = step.duration.max(1);
        if remaining < frames {
            true
        } else {
            remaining -= frames;
            false
        }
    }) else {
        return;
    };
    let Some(pieces) = part.frames.get(step.frame) else {
        return;
    };
    for piece in pieces {
        draw_sprite(
            frame,
            left + i32::from(part.x) + i32::from(piece.x),
            i32::from(part.y) + i32::from(piece.y),
            &part.piece_image(piece),
            &part.palette,
            piece.mirrored,
        );
    }
}

/// A color as an unseen Zoid shows it: each channel divided by eight.
fn silhouette(color: u16) -> u16 {
    let channel = |shift: u16| ((color >> shift) & CHANNEL_MASK) >> SILHOUETTE_SHIFT << shift;
    channel(0) | channel(5) | channel(10)
}

/// How far right of its place the Zoid is `frames` into its slide.
fn slide_offset(frames: u32) -> usize {
    let left = SLIDE_FRAMES.saturating_sub(frames);
    usize::try_from(left * SLIDE_CURVE.saturating_sub(left) / SLIDE_SCALE).unwrap_or(0)
}

fn fade_in_level(frames: u32, total: u32) -> u32 {
    u32::from(FADE_STEPS).saturating_sub(frames * u32::from(FADE_STEPS) / total)
}

fn zoid_row(army: usize, kind: usize) -> usize {
    ZOID_GUIDE_ENTRIES
        + (army.min(ZOID_ARMIES - 1) * ZOID_TYPES + kind.min(ZOID_TYPES - 1)) * GUIDE_ROW
}

fn character_row(series: usize, group: usize) -> usize {
    CHARACTER_GUIDE_ENTRIES
        + (series.min(CHARACTER_SERIES - 1) * CHARACTER_GROUPS + group.min(CHARACTER_GROUPS - 1))
            * GUIDE_ROW
}

/// The place, among `candidates`, of the `line`-th flag that is set: how
/// the original turns a menu line back into a series or group when locked
/// ones are left out of the menu.
fn nth_set(windows: &ScriptWindows<'_>, candidates: &[u16], line: usize) -> usize {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, flag)| windows.flag(**flag))
        .nth(line)
        .map_or(0, |(place, _)| place)
}

fn draw_map_sprite(frame: &mut Frame, sheet: &SpriteSheet, elapsed: u32) {
    let Some(index) = current_frame(sheet, SPRITE_FACING_DOWN, elapsed) else {
        return;
    };
    let (Some(record), Some(image)) = (sheet.frames.get(index), sheet.frame_image(index)) else {
        return;
    };
    draw_sprite(
        frame,
        SPRITE_ANCHOR.0 + i32::from(record.x),
        SPRITE_ANCHOR.1 + i32::from(record.y),
        &image,
        &sheet.palette,
        record.mirrored,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silhouette_divides_each_channel_by_eight() {
        assert_eq!(silhouette(0x7FFF), 0x0C63);
        assert_eq!(silhouette(0x001F), 0x0003);
        assert_eq!(silhouette(0x0007), 0);
    }

    #[test]
    fn the_zoid_speeds_up_as_it_slides_in() {
        assert_eq!(slide_offset(0), 125);
        assert_eq!(slide_offset(6), 84);
        assert_eq!(slide_offset(11), 48);
        assert_eq!(slide_offset(16), 8);
        assert_eq!(slide_offset(17), 0);
        assert_eq!(slide_offset(40), 0);
    }

    #[test]
    fn rows_follow_the_tables_layout() {
        assert_eq!(zoid_row(0, 0), ZOID_GUIDE_ENTRIES);
        assert_eq!(zoid_row(1, 2), ZOID_GUIDE_ENTRIES + (20 + 2) * 20);
        assert_eq!(character_row(1, 1), CHARACTER_GUIDE_ENTRIES + 11 * 20);
    }

    #[test]
    fn a_menu_line_is_the_nth_unlocked_candidate() {
        let mut windows = ScriptWindows::new(&[], "");
        windows.set_flags([5, 25]);
        assert_eq!(nth_set(&windows, &CHARACTER_SERIES_FLAGS, 0), 1);
        assert_eq!(nth_set(&windows, &CHARACTER_SERIES_FLAGS, 1), 4);
    }
}
