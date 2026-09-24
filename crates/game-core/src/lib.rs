//! Game systems shared across the Zoids titles: menus, battle framework, world, script interpreter and save system.

pub mod battle;
pub mod boot;
pub mod data;
pub mod event;
pub mod extension;
pub mod field;
pub mod game;
pub mod guide;
pub mod menu;
pub mod rng;
pub mod save;
pub mod script;
pub mod sprite;
pub mod story;
pub mod text;
pub mod translation;
pub mod window;
pub mod windows;

pub use boot::{LogoScreen, NameEntry, TitleChoice, TitleScreen};
pub use data::GameData;
pub use event::{EventHost, Events, Op};
pub use extension::{Event, Extension, Extensions, GameSound, Rect, SharedExtensions};
pub use field::{
    Actor, Command, Direction, Field, FieldError, FieldEvent, Walk, current_frame, draw_scene,
    frame_at,
};
pub use game::{Game, GameError, Stage};
pub use menu::{Member, MenuStep, Party, PauseMenu, Roster};
pub use rng::Rng;
pub use save::{Found, SaveError, SaveFile, SavedGame};
pub use script::{ScriptError, ScriptHost, ScriptRunner};
pub use sprite::draw_sprite;
pub use text::{TextMetrics, TextPainter};
pub use translation::{AlphabetPage, Scope, Translation, TranslationError, TranslationExtension};
pub use window::{DIALOGUE_TEXT_AREA, FrameStyle, WindowPainter};
pub use windows::{DEFAULT_PLAYER_NAME, ScriptWindows, TextLayout, Window};
