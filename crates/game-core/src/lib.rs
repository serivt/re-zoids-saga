//! Game systems shared across the Zoids titles: menus, battle framework, world, script interpreter and save system.

pub mod boot;
pub mod data;
pub mod extension;
pub mod field;
pub mod game;
pub mod menu;
pub mod rng;
pub mod script;
pub mod sprite;
pub mod text;
pub mod translation;
pub mod window;
pub mod windows;

pub use boot::{LogoScreen, NameEntry, TitleChoice, TitleScreen};
pub use data::GameData;
pub use extension::{Event, Extension, Extensions, GameSound, Rect, SharedExtensions};
pub use field::{
    Direction, Field, FieldError, FieldEvent, Npc, NpcCommand, Player, current_frame, draw_scene,
};
pub use game::{Game, GameError, Stage};
pub use menu::{Character, Party, PauseMenu, Unit};
pub use rng::Rng;
pub use script::{ScriptError, ScriptHost, ScriptRunner};
pub use sprite::draw_sprite;
pub use text::{TextMetrics, TextPainter};
pub use translation::{AlphabetPage, Scope, Translation, TranslationError, TranslationExtension};
pub use window::{DIALOGUE_TEXT_AREA, FrameStyle, WindowPainter};
pub use windows::{DEFAULT_PLAYER_NAME, ScriptWindows, TextLayout, Window};
