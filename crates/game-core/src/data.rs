//! The data provider: every asset and table the game needs, resolved by a
//! stable identifier. Today everything comes from the ROM image; mod packs
//! will answer first once they exist. Game logic never reads the ROM
//! through anything else (see `docs/extensibility.md`).

use extraction::saga::{
    self, BootError, FontReadError, Logo, MapError, MapObject, MapRecord, NameEntryGraphics,
    PauseWallpaper, Portrait, PortraitError, Scene, SceneError, SpriteSheet, SpriteSheetError,
    TitleGraphics, Warp, WindowSkin, WindowSkinError,
};
use extraction::string_table::StringTableError;
use formats::font::{Glyph, GlyphIndex};

use crate::extension::GameSound;
use crate::translation::{DIALOGUE_TABLE, NAME_ENTRY_TABLE, PAUSE_MENU_TABLE, TITLE_TABLE};

/// The game's data, keyed by identifier.
#[derive(Debug, Clone, Copy)]
pub struct GameData<'rom> {
    rom: &'rom [u8],
}

impl<'rom> GameData<'rom> {
    /// Data backed by a ROM image.
    #[must_use]
    pub fn new(rom: &'rom [u8]) -> Self {
        Self { rom }
    }

    /// The raw image, for the two engines that address it directly: the
    /// script interpreter (bytecode and message bytes at offsets) and the
    /// sound driver (samples and songs). Nothing else reads it.
    #[must_use]
    pub fn bytes(&self) -> &'rom [u8] {
        self.rom
    }

    /// Map record `map`.
    ///
    /// # Errors
    ///
    /// Returns [`MapError`] when the map does not exist.
    pub fn map_record(&self, map: usize) -> Result<MapRecord, MapError> {
        saga::map_record(self.rom, map)
    }

    /// Exit `exit` of map `map`.
    ///
    /// # Errors
    ///
    /// Returns [`MapError`] when the map or exit does not exist.
    pub fn warp(&self, map: usize, exit: usize) -> Result<Warp, MapError> {
        saga::warp(self.rom, map, exit)
    }

    /// The objects placed on map `map`.
    ///
    /// # Errors
    ///
    /// Returns [`MapError`] when the map does not exist.
    pub fn map_objects(&self, map: usize) -> Result<Vec<MapObject>, MapError> {
        saga::map_objects(self.rom, map)
    }

    /// Scene `scene` (tiles, maps, attributes).
    ///
    /// # Errors
    ///
    /// Returns [`SceneError`] when the scene cannot be read.
    pub fn scene(&self, scene: usize) -> Result<Scene, SceneError> {
        saga::scene(self.rom, scene)
    }

    /// Sprite sheet `id`.
    ///
    /// # Errors
    ///
    /// Returns [`SpriteSheetError`] when the sheet cannot be read.
    pub fn sprite_sheet(&self, id: usize) -> Result<SpriteSheet, SpriteSheetError> {
        saga::sprite_sheet(self.rom, id)
    }

    /// Portrait `expression` of character `character`.
    ///
    /// # Errors
    ///
    /// Returns [`PortraitError`] when it cannot be read.
    pub fn portrait(&self, character: usize, expression: usize) -> Result<Portrait, PortraitError> {
        saga::portrait(self.rom, character, expression)
    }

    /// The text font and its fallback glyph.
    ///
    /// # Errors
    ///
    /// Returns [`FontReadError`] when the font cannot be read.
    pub fn font(&self) -> Result<(GlyphIndex, Glyph), FontReadError> {
        saga::font(self.rom)
    }

    /// The window frame tiles and palette.
    ///
    /// # Errors
    ///
    /// Returns [`WindowSkinError`] when they cannot be read.
    pub fn window_skin(&self) -> Result<WindowSkin, WindowSkinError> {
        saga::window_skin(self.rom)
    }

    /// The publisher logo.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when it cannot be read.
    pub fn logo(&self) -> Result<Logo, BootError> {
        saga::logo(self.rom)
    }

    /// The title screen's graphics.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when they cannot be read.
    pub fn title(&self) -> Result<TitleGraphics, BootError> {
        saga::title(self.rom)
    }

    /// The name entry's graphics.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when they cannot be read.
    pub fn name_entry_graphics(&self) -> Result<NameEntryGraphics, BootError> {
        saga::name_entry_graphics(self.rom)
    }

    /// The name entry's character table, by row.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when it cannot be read.
    pub fn kana_table(&self) -> Result<Vec<Vec<char>>, BootError> {
        saga::kana_table(self.rom)
    }

    /// The pause menu's wallpaper.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when it cannot be read.
    pub fn pause_wallpaper(&self) -> Result<PauseWallpaper, BootError> {
        saga::pause_wallpaper(self.rom)
    }

    /// Experience needed to reach the level after `level`.
    #[must_use]
    pub fn experience_to_next(&self, level: usize) -> Option<u32> {
        saga::experience_to_next(self.rom, level)
    }

    /// The song a map's own code starts, if one is found.
    #[must_use]
    pub fn map_music(&self, map: usize) -> Option<usize> {
        saga::map_music(self.rom, map)
    }

    /// The song number of a game sound.
    #[must_use]
    pub fn sound(&self, sound: GameSound) -> usize {
        match sound {
            GameSound::TitleMusic => saga::MUSIC_TITLE,
            GameSound::NameEntryMusic => saga::MUSIC_NAME_ENTRY,
            GameSound::OpeningMusic => saga::MUSIC_OPENING,
            GameSound::FirstRoomMusic => saga::MUSIC_FIRST_ROOM,
            GameSound::TitleStart => saga::SOUND_TITLE_START,
            GameSound::Door => saga::SOUND_DOOR,
        }
    }

    /// Where the sound driver's song table is: offset, entries and the
    /// master volume.
    #[must_use]
    pub fn song_table(&self) -> (usize, usize, u8) {
        (saga::SONG_TABLE, saga::SONG_COUNT, saga::MASTER_VOLUME)
    }

    /// Offsets of the strings of script table `table` (`0` marks an absent
    /// string), or `None` for a name this game has no table for.
    ///
    /// # Errors
    ///
    /// Returns [`StringTableError`] when the table cannot be read.
    pub fn script_offsets(&self, table: &str) -> Result<Option<Vec<usize>>, StringTableError> {
        Ok(Some(match table {
            TITLE_TABLE => vec![saga::TITLE_MENU_SCRIPT_OFFSET],
            NAME_ENTRY_TABLE => saga::NAME_ENTRY_SCRIPTS.offsets(self.rom)?,
            PAUSE_MENU_TABLE => saga::PAUSE_MENU_SCRIPTS.offsets(self.rom)?,
            DIALOGUE_TABLE => match saga::string_table(DIALOGUE_TABLE) {
                Some(table) => table.offsets(self.rom)?,
                None => return Ok(None),
            },
            _ => return Ok(None),
        }))
    }
}
