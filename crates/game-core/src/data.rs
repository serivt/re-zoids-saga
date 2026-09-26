//! The data provider: every asset and table the game needs, resolved by a
//! stable identifier. Today everything comes from the ROM image; mod packs
//! will answer first once they exist. Game logic never reads the ROM
//! through anything else (see `docs/extensibility.md`).

use extraction::saga::{
    self, BootError, FontReadError, Logo, MapError, MapObject, MapRecord, NameEntryGraphics,
    PauseWallpaper, Portrait, PortraitError, Scene, SceneError, SpriteSheet, SpriteSheetError,
    TitleGraphics, Warp, WindowSkin, WindowSkinError,
};
use extraction::saga_battle::{self, BattleImage, BattleScene, EffectSprite};
use extraction::saga_formation::{self, BattleField};
use extraction::saga_guide::{
    self, CHARACTER_ENTRIES, CHARACTER_GUIDE_SCRIPTS, GuidePicture, SYSTEM_SCRIPTS,
    ZOID_GUIDE_SCRIPTS, ZoidPart,
};
use extraction::saga_party;
use extraction::saga_save::{self, SaveDataError};
use extraction::string_table::StringTableError;
use formats::SaveLayout;
use formats::font::{Glyph, GlyphIndex};

use crate::extension::GameSound;
use crate::menu::{Member, Roster, UNIT_SLOTS};
use crate::translation::{
    BATTLE_TABLE, DIALOGUE_TABLE, ITEM_TABLE, NAME_ENTRY_TABLE, NAME_TABLE, PART_TABLE,
    PAUSE_MENU_TABLE, TITLE_TABLE,
};

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

    /// What chest `chest` holds.
    #[must_use]
    pub fn treasure(&self, chest: usize) -> Option<saga::Treasure> {
        saga::treasure(self.rom, chest)
    }

    /// The `count` objects a cutscene places from its own list at ROM
    /// address `address`.
    ///
    /// # Errors
    ///
    /// Returns [`MapError`] when the list is outside the ROM.
    pub fn objects_at(&self, address: u32, count: usize) -> Result<Vec<MapObject>, MapError> {
        saga::objects_at(self.rom, address, count)
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

    /// Where the save routine puts its blocks.
    ///
    /// # Errors
    ///
    /// Returns [`SaveDataError`] when the descriptor cannot be read.
    pub fn save_layout(&self) -> Result<SaveLayout, SaveDataError> {
        saga_save::save_layout(self.rom)
    }

    /// The game-state block as a new game starts it.
    ///
    /// # Errors
    ///
    /// Returns [`SaveDataError`] when a table cannot be read.
    pub fn new_game_state(&self) -> Result<Vec<u8>, SaveDataError> {
        saga_save::new_game_state(self.rom)
    }

    /// Puts the characters of list `list` in `state`'s character guide;
    /// `None` when the list cannot be read.
    pub fn meet_characters(&self, state: &mut [u8], list: usize) -> Option<()> {
        saga_save::meet_characters(self.rom, state, list)
    }

    /// Adds the characters of list `list` to the party in `state`, each
    /// with a unit of its Zoid; `None` when the list cannot be read.
    pub fn join_group(&self, state: &mut [u8], list: usize) -> Option<()> {
        saga_party::join_group(self.rom, state, list)
    }

    /// Takes the characters of list `list` out of the party in `state`;
    /// `None` when the list cannot be read.
    pub fn leave_group(&self, state: &mut [u8], list: usize) -> Option<()> {
        saga_party::leave_group(self.rom, state, list)
    }

    /// Battle scene `index` of the table cutscenes stage.
    #[must_use]
    pub fn battle_scene(&self, index: usize) -> Option<BattleScene> {
        saga_battle::battle_scene(self.rom, index)
    }

    /// The battle scenery `scenery`.
    #[must_use]
    pub fn scenery_image(&self, scenery: u8) -> Option<BattleImage> {
        saga_battle::scenery_image(self.rom, scenery)
    }

    /// Effect sprite `id`, of the shots battle scenes show.
    #[must_use]
    pub fn effect_sprite(&self, id: u16) -> Option<EffectSprite> {
        saga_battle::effect_sprite(self.rom, usize::from(id))
    }

    /// Zoid `zoid`'s battle image.
    #[must_use]
    pub fn zoid_image(&self, zoid: u8) -> Option<BattleImage> {
        saga_battle::zoid_image(self.rom, zoid)
    }

    /// The picture the status screens show of Zoid `zoid`.
    ///
    /// # Errors
    ///
    /// Returns [`SpriteSheetError`] when it cannot be read.
    pub fn zoid_status_sprite(&self, zoid: usize) -> Result<SpriteSheet, SpriteSheetError> {
        saga::zoid_status_sprite(self.rom, zoid)
    }

    /// The field the formation screen shows the party on.
    #[must_use]
    pub fn battle_field(&self) -> Option<BattleField> {
        saga_formation::battle_field(self.rom)
    }

    /// Where the unit in formation slot `slot` stands on the formation
    /// screen.
    #[must_use]
    pub fn slot_anchor(&self, slot: usize) -> Option<(i32, i32)> {
        saga_formation::slot_anchor(self.rom, slot)
    }

    /// The cursor the formation screen marks a slot with.
    #[must_use]
    pub fn slot_cursor(&self) -> Option<EffectSprite> {
        saga_formation::slot_cursor(self.rom)
    }

    /// The party's members and formation as `state` holds them.
    #[must_use]
    pub fn roster(&self, state: &[u8]) -> Roster {
        let unit = |character| {
            saga_party::character_unit(state, character)
                .and_then(|unit| saga_party::unit_status(state, unit))
        };
        let members: Vec<Member> = saga_party::members(state)
            .into_iter()
            .map(|character| Member {
                character,
                bonuses: saga_party::pilot_bonuses(self.rom, state, character).unwrap_or_default(),
                unit: unit(character),
                parts: saga_party::unit_parts(self.rom, state, character),
                keeps_equipment: saga_party::keeps_equipment(state, character),
            })
            .collect();
        let mut formation = [None; UNIT_SLOTS];
        for (slot, entry) in formation.iter_mut().zip(saga_party::formation(state)) {
            *slot = entry.and_then(|(unit, character)| {
                saga_party::unit_status(state, unit).map(|status| (character, status))
            });
        }
        if members.is_empty() {
            return Roster::default();
        }
        Roster { members, formation }
    }

    /// Forms the party in `state` around the Zoid picked in the hangar;
    /// `None` when a table cannot be read or no unit slot is free.
    pub fn form_party(&self, state: &mut [u8], choice: usize) -> Option<()> {
        saga_party::form_party(self.rom, state, choice)
    }

    /// The picture of Zoid `id` in the guide, if it has one.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when it cannot be read.
    pub fn zoid_picture(&self, id: usize) -> Result<Option<GuidePicture>, BootError> {
        saga_guide::zoid_picture(self.rom, id)
    }

    /// The backdrop behind Zoid `id` in the guide, if it has one.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when it cannot be read.
    pub fn zoid_backdrop(&self, id: usize) -> Result<Option<GuidePicture>, BootError> {
        saga_guide::zoid_backdrop(self.rom, id)
    }

    /// The parts the guide draws over Zoid `id`'s picture.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when one cannot be read.
    pub fn zoid_parts(&self, id: usize) -> Result<Vec<ZoidPart>, BootError> {
        saga_guide::zoid_parts(self.rom, id)
    }

    /// Where every character's guide entry starts, in the order of the
    /// save's character table.
    ///
    /// # Errors
    ///
    /// Returns [`StringTableError`] when the table cannot be read.
    pub fn character_entries(&self) -> Result<Vec<usize>, StringTableError> {
        CHARACTER_ENTRIES.offsets(self.rom)
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
            PART_TABLE => saga::PART_NAME_SCRIPTS.offsets(self.rom)?,
            name if name == SYSTEM_SCRIPTS.name => SYSTEM_SCRIPTS.offsets(self.rom)?,
            name if name == ZOID_GUIDE_SCRIPTS.name => ZOID_GUIDE_SCRIPTS.offsets(self.rom)?,
            name if name == CHARACTER_GUIDE_SCRIPTS.name => {
                CHARACTER_GUIDE_SCRIPTS.offsets(self.rom)?
            }
            name if name == saga::BATTLE_MENU_SCRIPTS.name => {
                saga::BATTLE_MENU_SCRIPTS.offsets(self.rom)?
            }
            name if name == saga::BATTLE_TEXT_SCRIPTS.name => {
                saga::BATTLE_TEXT_SCRIPTS.offsets(self.rom)?
            }
            name if name == saga::BATTLE_LABEL_SCRIPTS.name => {
                saga::BATTLE_LABEL_SCRIPTS.offsets(self.rom)?
            }
            BATTLE_TABLE | DIALOGUE_TABLE | ITEM_TABLE | NAME_TABLE => {
                match saga::string_table(table) {
                    Some(table) => table.offsets(self.rom)?,
                    None => return Ok(None),
                }
            }
            _ => return Ok(None),
        }))
    }
}
