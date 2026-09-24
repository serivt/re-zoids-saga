//! The objects the game-state block keeps for the maps whose record id
//! has bit 15, and the formations the area's roaming enemies stand for.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the map
//! loader (`0x08007188`), the rebuild of the object states (`0x08006E4C`),
//! the formation pick (`0x080328FC`) and the stepping command's write-back
//! (`0x0800B764`); checked against the object states and the formations
//! RAM held on the world map in a reference emulator, and against a
//! continue, which clears the area the rebuild compares with
//! (`0x0800C0CC`) so that the first map entered rebuilds the table.
//!
//! Entering a map whose area (its record id's low byte) is not the one
//! last entered rebuilds the table: for every record of the area whose id
//! has bit 15, one state per object after the player, with its cell,
//! sprite, command and parameter. Each map Zoid (behavior 1) gets the next
//! formation slot of the area and a formation drawn for it, and shows its
//! leader's sprite. The formations live in RAM beside the block
//! (`0x02004A70`), not in the save. Loading such a map then builds its
//! objects from the states, and each step writes the cell back.

use extraction::saga::MAP_COUNT;
use extraction::saga_encounter::{self, Formation};
use formats::progress::{ObjectState, object_states, set_object_cell, write_object_states};

use crate::data::GameData;
use crate::field::{Field, FieldError};
use crate::rng::Rng;

const AREA: usize = 0x02;
const AREA_INDEX: usize = 0x03;
const PERSISTENT: u16 = 0x8000;
const MAP_ZOID: u16 = 1;
/// Formation slots RAM has room for; a map Zoid past them stands for none.
const FORMATION_SLOTS: usize = 0x7F;
const ROLL_RANGE: u16 = 100;

/// The area whose object states the block holds, and its formations.
#[derive(Debug, Clone, Default)]
pub struct AreaObjects {
    /// The area last entered (RAM `0x0200000D`); `None` after the boot or
    /// a continue.
    area: Option<u8>,
    formations: Vec<Formation>,
    rng: Rng,
}

impl AreaObjects {
    /// Forgets the area, as a continue does: the next map entered
    /// rebuilds the table.
    pub fn forget(&mut self) {
        self.area = None;
    }

    /// The formation of slot `slot`, once drawn.
    #[must_use]
    pub fn formation(&self, slot: u16) -> Option<&Formation> {
        self.formations.get(usize::from(slot))
    }

    /// Enters map `map`: records its area in the block and rebuilds the
    /// object states when the area changed; `frame` is the frame counter
    /// the random draws mix in.
    pub fn enter(&mut self, data: &GameData<'_>, state: &mut [u8], map: usize, frame: u16) {
        let Ok(record) = data.map_record(map) else {
            return;
        };
        let area = record.id.to_le_bytes()[0];
        if let Some(byte) = state.get_mut(AREA) {
            *byte = area;
        }
        if self.area != Some(area) {
            self.rebuild(data, state, area, frame);
        }
        self.area = Some(area);
        if let Some(byte) = state.get_mut(AREA_INDEX) {
            *byte = area.wrapping_sub(1);
        }
    }

    /// Builds the objects of `field`, map `map`, from the block's states
    /// when its record keeps them.
    ///
    /// # Errors
    ///
    /// Returns [`FieldError`] when a sprite cannot be read.
    pub fn place(
        data: &GameData<'_>,
        state: &[u8],
        map: usize,
        field: &mut Field,
    ) -> Result<(), FieldError> {
        let Ok(record) = data.map_record(map) else {
            return Ok(());
        };
        if record.id & PERSISTENT == 0 {
            return Ok(());
        }
        let states = object_states(state);
        let first = states
            .iter()
            .position(|object| usize::from(object.map) == map);
        field.apply_object_states(data, &states, first)
    }

    /// Writes back the cells `field`'s objects stand on.
    pub fn record(state: &mut [u8], field: &Field) {
        for (slot, (column, row)) in field.object_cells() {
            let cell = (
                u8::try_from(column).unwrap_or(u8::MAX),
                u8::try_from(row).unwrap_or(u8::MAX),
            );
            set_object_cell(state, slot, cell);
        }
    }

    fn rebuild(&mut self, data: &GameData<'_>, state: &mut [u8], area: u8, frame: u16) {
        let rom = data.bytes();
        let same_area = |map: usize| {
            data.map_record(map)
                .is_ok_and(|record| record.id & !PERSISTENT == u16::from(area))
        };
        let Some(first) = (0..MAP_COUNT).find(|map| same_area(*map)) else {
            write_object_states(state, &[]);
            return;
        };
        self.formations.clear();
        let mut objects = Vec::new();
        for map in (first..MAP_COUNT).take_while(|map| same_area(*map)) {
            let persistent = data
                .map_record(map)
                .is_ok_and(|record| record.id & PERSISTENT != 0);
            if !persistent {
                continue;
            }
            let Ok(list) = data.map_objects(map) else {
                continue;
            };
            for object in list.iter().skip(1) {
                let mut kept = ObjectState {
                    map: u16::try_from(map).unwrap_or(u16::MAX),
                    present: true,
                    column: u8::try_from(object.column).unwrap_or(u8::MAX),
                    row: u8::try_from(object.row).unwrap_or(u8::MAX),
                    sprite: object.sprite,
                    group: 0,
                    parameter: object.parameter,
                    command: u8::try_from(object.kind & 0xFF).unwrap_or(0),
                    extra: [0; 3],
                };
                if object.behavior == MAP_ZOID {
                    let slot = self.formations.len();
                    kept.group = u16::try_from(slot).unwrap_or(u16::MAX);
                    let column = object.sprite.to_le_bytes()[0];
                    let formation = self.draw(rom, area, column, frame);
                    kept.sprite = if slot < FORMATION_SLOTS {
                        u16::from(saga_encounter::leader_sprite(rom, &formation))
                    } else {
                        u16::from(u8::MAX)
                    };
                    self.formations.push(formation);
                }
                objects.push(kept);
            }
        }
        write_object_states(state, &objects);
    }

    /// Draws a formation for a map Zoid of sprite column `column`: a roll
    /// of 100 picks the rarity class, a second draw one of the class.
    fn draw(&mut self, rom: &[u8], area: u8, column: u8, frame: u16) -> Formation {
        let Some(candidates) = saga_encounter::formations(rom, area, column) else {
            return [0; saga_encounter::FORMATION_LEN];
        };
        let class = saga_encounter::rarity_for(self.rng.next(frame) % ROLL_RANGE);
        let found = saga_encounter::of_class(&candidates, class);
        let count = u16::try_from(found.len()).unwrap_or(1).max(1);
        let pick = usize::from(self.rng.next(frame) % count);
        candidates[found[pick]]
    }
}
