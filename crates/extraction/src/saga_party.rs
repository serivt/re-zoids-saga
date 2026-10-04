//! Forming the party in the game-state block, as the hangar's choice does.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! choice routine at `0x08037644`, the unit allocator at `0x08036A30` and
//! its initializer at `0x08036B2C`, the pilot assignment at `0x08036BE0`
//! with the unit statistics at `0x08036CB0` and the percentage routine at
//! `0x080346C0`, the starting units at `0x080374B8` / `0x080372D0`, the
//! formation slots at `0x08037AB4`, the warriors' growth at `0x080368BC`,
//! the member list the status screens build at `0x0804E34C`, and the part
//! slots the Zoid status screen describes (`0x0804E04C`, `0x0804DFCC`,
//! with the part values at `0x08036E74`).
//!
//! Units are 0x38-byte records at `+0xD2C` of the game state, 0x99
//! ordinary slots and 0x14 special ones after them, counted at `+0x3304`:
//!
//! | Offset | Content |
//! |---|---|
//! | 0 | The Zoid record's first half-word |
//! | 2 | Flags: 1 in use, 2 in the formation, 4 piloted, 8 special |
//! | 4 | The unit's slot |
//! | 6 | Zoid (index of the 76-byte records at ROM `0x670210`) |
//! | 8, 12 | Current values of the first two statistics |
//! | 16 | Six four-byte part entries, the part id at +2 (`0xFFFF` none) |
//! | 40 | Statistics: two words and two half-words |
//! | 52 | Training level: half of it is added in percent to every statistic |
//! | 53 | The Zoid record's byte 4 |

use formats::progress::STATE_LEN;

use crate::revision::locate;

const UNITS: usize = 0xD2C;
/// Bytes of a unit in the game-state block.
pub const UNIT_LEN: usize = 0x38;
const ORDINARY_UNITS: usize = 0x99;
const ALL_UNITS: usize = 0xAD;
const UNIT_COUNT: usize = 0x3304;
const IN_USE: u16 = 1;
const IN_FORMATION: u16 = 2;
const PILOTED: u16 = 4;
const SPECIAL: u16 = 8;
const SPECIAL_TRAINING: u8 = 100;
const SPECIAL_UNIT: u16 = 4;
pub(crate) const ZOID_RECORDS: usize = 0x0067_0210;
pub(crate) const ZOID_RECORD_LEN: usize = 0x4C;
const ZOID_PARTS: usize = 8;
const DEVELOPMENT_MONEY: usize = 0x24;
const DEVELOPMENT_ZOID: usize = 0x2C;
const DEVELOPMENT_ITEMS: usize = 0x2D;
/// The Zi-data items' counts in the game-state block.
const ZI_ITEMS: usize = 0x330C;
/// The kinds of Zi-data item (Zoid cores) the counts cover, the ones the
/// lists read (`0x0804E308`: ids 0–63).
pub const ZI_ITEM_KINDS: usize = 64;
/// The lists of Zoids a development's special kinds take, from `0xFA`.
const SPECIAL_KINDS: usize = 0x0075_C018;
const SPECIAL_KINDS_FROM: u8 = 0xFA;
const SPECIAL_KIND_LEN: usize = 8;
/// The part slots that are racks.
const RACKS: usize = 3;
/// The Zoid pictures whose Zi data the party can hold: the bytes at
/// `+0x33E2` the Zi data list reads (`0x0804E3A0`).
pub const ZI_DATA_ZOIDS: u8 = 0x99;
/// A development's Zoid or item that is not needed.
pub const NOT_NEEDED: u8 = 0xFF;
const ZOID_STATS: usize = 0x40;
const PARTS_LEN: usize = 24;
const STATS_LEN: usize = 12;
const UNIT_PARTS: usize = 0x10;
const UNIT_STATS: usize = 0x28;
const UNIT_TRAINING: usize = 0x34;
const UNIT_VARIANT: usize = 0x35;
const PART_RECORDS: usize = 0x0066_C8F8;
const PART_RECORD_LEN: usize = 24;
/// Part slots of a unit.
pub const PART_SLOTS: usize = 6;
const NO_PART: u16 = 0xFFFF;
const PART_WEAPON: u32 = 1;
const PART_ACCURACY: usize = 8;
const PART_COST: usize = 0x10;
const PART_RANGE: usize = 0x12;
const PART_TURNS: usize = 0x14;
const RACK_KIND: u16 = 3;
const STOCK: usize = 0x334C;
/// Parts that can be kept in stock: ids below this.
pub const STOCKED_PARTS: usize = 150;
/// The most of one part the stock holds.
pub const STOCK_LIMIT: u8 = 9;
const PART_ACTIVE: u32 = 0x2000_0000;
const PART_BONUSES: [(u32, usize); 6] = [
    (0x4000, 10),
    (0x8000, 10),
    (0x1_0000, 8),
    (0x2_0000, 8),
    (0x80_0000, 0),
    (0x100_0000, 4),
];
const PART_BONUS: usize = 0xC;
const CHARACTERS: usize = 0x34A4;
const CHARACTER_LEN: usize = 4;
const CHARACTER_UNIT: usize = 2;
const CHARACTER_IN_FORMATION: u16 = 0x10;
const KEEPS_EQUIPMENT: u16 = 0x08;
const NO_UNIT: u8 = 0xFF;
const FORMATION: usize = 0x3600;
const MEMBER_RECORDS: usize = 0xCD8;
const PILOT_FACE: usize = 2;
const MEMBER_RECORD_LEN: usize = 16;
const MEMBERS: usize = 4;
const WARRIORS: usize = 3;
/// The pilot bonuses a record holds.
pub const PILOT_VALUES: usize = 5;
const PARTY_LEVEL: usize = 0xCD2;
const GROWTH: usize = 0x0066_BB38;
const GROWTH_LEN: usize = 10;
const CHARACTER_COUNT: usize = 87;
const PARTY_MEMBER: u16 = 2;
/// Slots of the formation.
pub const FORMATION_SLOTS: usize = 6;
/// Slots of a formation column: the front, then the back.
const COLUMN: usize = 3;
/// The size class of an L unit, which fills a column.
const LARGE: u8 = 2;
const PILOT_TABLE: usize = 0x0067_B35C;
const PILOT_CHAPTERS: usize = 10;
const AREA: usize = 2;
const STARTING_LISTS: usize = 0x0067_E380;
const LIST_END: u8 = 0xFF;
const SHIELD_LIGER: u16 = 0x39;
const SABER_TIGER: u16 = 0x0F;
const RAYNOS: u16 = 0x5C;
const WARRIOR_SLOTS: [(u8, u8); 3] = [(1, 4), (2, 0), (3, 2)];
const MAX_WORD: [i32; 2] = [9999, 999];
const MAX_HALF: [i32; 2] = [9999, 999];
const ROM_BASE: u32 = 0x0800_0000;

/// Forms the party around the Zoid picked in the hangar (0 the Shield
/// Liger and any other value, 1 the Saber Tiger, 2 the Raynos): the
/// prince pilots it in formation slot 1, the warriors get their starting
/// Zoids in slots 4, 0 and 2. Returns `None` when `state` is not a
/// game-state block, no unit slot is free or a table is outside `rom`.
pub fn form_party(rom: &[u8], state: &mut [u8], choice: usize) -> Option<()> {
    if state.len() != STATE_LEN {
        return None;
    }
    let zoid = match choice {
        1 => SABER_TIGER,
        2 => RAYNOS,
        _ => SHIELD_LIGER,
    };
    let unit = add_unit(rom, state, zoid, false)?;
    assign(rom, state, 0, unit)?;
    place(state, 0, 1);
    for (character, bits, zoid) in starting_list(rom, 0)? {
        add_character(rom, state, character, bits, zoid)?;
    }
    for (character, slot) in WARRIOR_SLOTS {
        place(state, character, slot);
    }
    grow_members(rom, state)
}

/// Sets the bonuses of the three warriors' member records to the party
/// level times each one's growth from ROM `0x66BB38` (`0x080368BC`); the
/// hangar does it after the units' statistics are computed.
fn grow_members(rom: &[u8], state: &mut [u8]) -> Option<()> {
    let level = u16::from(state[PARTY_LEVEL]);
    for member in 0..WARRIORS {
        let growth_at = locate(rom, GROWTH + member * GROWTH_LEN);
        let growth = rom.get(growth_at..growth_at + GROWTH_LEN)?;
        let record = MEMBER_RECORDS + (member + 1) * MEMBER_RECORD_LEN;
        for value in 0..PILOT_VALUES {
            let bonus = half(growth, value * 2).wrapping_mul(level);
            set_half(state, record + 4 + value * 2, bonus);
        }
    }
    Some(())
}

/// The characters in the party, in the order the game lists them: every
/// character whose flags have bit 2 (`0x0804E34C`).
#[must_use]
pub fn members(state: &[u8]) -> Vec<u8> {
    (0..CHARACTER_COUNT)
        .filter(|&character| {
            state
                .get(CHARACTERS + character * CHARACTER_LEN..)
                .is_some_and(|entry| entry.len() >= 2 && half(entry, 0) & PARTY_MEMBER != 0)
        })
        .filter_map(|character| u8::try_from(character).ok())
        .collect()
}

/// Whether `character` keeps its equipment as it is: the equipment
/// screen refuses to change it when the character's flags have bit
/// `0x08`.
#[must_use]
pub fn keeps_equipment(state: &[u8], character: u8) -> bool {
    let at = CHARACTERS + usize::from(character) * CHARACTER_LEN;
    state
        .get(at..at + 2)
        .is_some_and(|entry| half(entry, 0) & KEEPS_EQUIPMENT != 0)
}

/// The unit `character` pilots, if any.
#[must_use]
pub fn character_unit(state: &[u8], character: u8) -> Option<u8> {
    let unit = *state.get(CHARACTERS + usize::from(character) * CHARACTER_LEN + CHARACTER_UNIT)?;
    (unit != NO_UNIT).then_some(unit)
}

/// An object sprite of bit 15 names a party member's Zoid.
const PARTY_ZOID_SPRITE: u16 = 0x8000;
/// Such a sprite's low byte plus this, wrapped to a byte, is the character.
const PARTY_ZOID_CHARACTER: u16 = 0x68;
/// The sprite of the Zoid the character pilots: its unit's Zoid index.
const UNIT_ZOID_INDEX: usize = 6;
/// What such an object shows when its character has no unit: the carrier
/// for the player's, a soldier's Zoid for the others'.
const PLAYER_ZOID_FALLBACK: (u16, u16) = (0x98, 0x39);
const MEMBER_ZOID_FALLBACK: (u16, u16) = (0x99, 0x46);
const MEMBERS_AFTER_PLAYER: u16 = 3;

/// The sprite an object of sprite `sprite` shows (`0x080086E0`): with bit
/// 15 set, the Zoid of character `(sprite + 0x68) & 0xFF` (`0x08037484`:
/// its unit's Zoid index), or when it has none `0x39` for `0x8098` and
/// `0x46` for `0x8099`–`0x809B`; any other sprite as it is.
#[must_use]
pub fn object_sprite(state: &[u8], sprite: u16) -> u16 {
    if sprite & PARTY_ZOID_SPRITE == 0 {
        return sprite;
    }
    let character = u8::try_from(sprite.wrapping_add(PARTY_ZOID_CHARACTER) & 0xFF).unwrap_or(0);
    let zoid = character_unit(state, character)
        .and_then(|unit| state.get(unit_at(unit) + UNIT_ZOID_INDEX).copied())
        .filter(|&zoid| zoid != NO_UNIT);
    if let Some(zoid) = zoid {
        return u16::from(zoid);
    }
    let plain = sprite & !PARTY_ZOID_SPRITE;
    if plain == PLAYER_ZOID_FALLBACK.0 {
        PLAYER_ZOID_FALLBACK.1
    } else if plain.wrapping_sub(MEMBER_ZOID_FALLBACK.0) < MEMBERS_AFTER_PLAYER {
        MEMBER_ZOID_FALLBACK.1
    } else {
        plain
    }
}

/// The formation slots: the unit and its pilot, or `None` for an empty
/// slot.
#[must_use]
pub fn formation(state: &[u8]) -> [Option<(u8, u8)>; FORMATION_SLOTS] {
    std::array::from_fn(|slot| {
        let at = FORMATION + slot * 4;
        let (unit, character) = (*state.get(at)?, *state.get(at + 1)?);
        (unit != NO_UNIT).then_some((unit, character))
    })
}

/// The 56 bytes of unit `unit` in the game-state block, when it is in use.
#[must_use]
pub fn unit_record(state: &[u8], unit: u8) -> Option<[u8; UNIT_LEN]> {
    let at = unit_at(unit);
    let record: [u8; UNIT_LEN] = state.get(at..at + UNIT_LEN)?.try_into().ok()?;
    (half(&record, 2) & IN_USE != 0).then_some(record)
}

/// What the status screens show of a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitStatus {
    /// The Zoid record's first half-word; bit `0x800` hides the hit points.
    pub flags: u16,
    /// The Zoid.
    pub zoid: u16,
    /// Hit points, current and full.
    pub hp: (u32, u32),
    /// Energy points, current and full.
    pub ep: (u32, u32),
    /// The two half-word statistics (SP and DF).
    pub sp: i16,
    /// See [`UnitStatus::sp`].
    pub df: i16,
    /// Training level.
    pub training: u8,
    /// Size class the Zoid record gives: 0 S, 1 M, 2 L.
    pub size: u8,
}

/// Unit `unit`'s record, when it is in use.
#[must_use]
pub fn unit_status(state: &[u8], unit: u8) -> Option<UnitStatus> {
    let at = unit_at(unit);
    let record = state.get(at..at + UNIT_LEN)?;
    if half(record, 2) & IN_USE == 0 {
        return None;
    }
    let signed = |at: usize| i16::from_ne_bytes(half(record, at).to_ne_bytes());
    Some(UnitStatus {
        flags: half(record, 0),
        zoid: half(record, 6),
        hp: (word(record, 8), word(record, UNIT_STATS)),
        ep: (word(record, 12), word(record, UNIT_STATS + 4)),
        sp: signed(UNIT_STATS + 8),
        df: signed(UNIT_STATS + 10),
        training: record[UNIT_TRAINING],
        size: record[UNIT_VARIANT],
    })
}

/// A pilot's bonuses in percent in the character screen's order: 耐久,
/// 攻撃, 防御, 反応, 命中 (record fields 4, 10, 8, 6, 12).
#[must_use]
pub fn pilot_bonuses(rom: &[u8], state: &[u8], character: u8) -> Option<[i32; PILOT_VALUES]> {
    let [durability, reaction, defense, attack, accuracy] = pilot(rom, state, character)?;
    Some([durability, attack, defense, reaction, accuracy])
}

/// A part as the screens describe it: its 24-byte record at ROM
/// `0x66C8F8`, with a weapon's power and accuracy raised by its pilot's
/// bonuses (`0x08036E74`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part {
    /// The part: its name is entry `id` of the part-name table.
    pub id: u16,
    /// What the part is and does: bit 0 a weapon; otherwise the kind of
    /// support (see `docs/menu.md`).
    pub flags: u32,
    /// Accuracy in percent (a support part's second value).
    pub accuracy: i16,
    /// A weapon's power in 16.16, or a support part's value.
    pub power: i32,
    /// Energy points it costs.
    pub cost: i16,
    /// A weapon's range, and how many enemies it reaches.
    pub range: (u8, u8),
    /// Turns a support part's effect lasts; 0 until the battle ends.
    pub turns: u8,
}

/// One of a unit's six part slots: three weapon racks, then three fixed
/// weapons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartSlot {
    /// The Zoid record's slot flags: the equipment screen offers the
    /// stocked parts a rack of flags 1, 2 or 3 takes.
    pub flags: u16,
    /// The rack's kind, the low two bits of `flags`: 1 attack, 2 defense,
    /// 3 both, 0 fixed.
    pub rack: u8,
    /// Whether the Zoid record fits the slot with a part of its own; a
    /// fixed slot without one is no rack at all.
    pub fitted: bool,
    /// The part the unit carries.
    pub part: Option<Part>,
}

/// Part `id` as its record gives it, which the stock list shows.
#[must_use]
pub fn part_record(rom: &[u8], id: u16) -> Option<Part> {
    let at = locate(rom, PART_RECORDS + usize::from(id) * PART_RECORD_LEN);
    let record = rom.get(at..at + PART_RECORD_LEN)?;
    let signed = |at: usize| i16::from_ne_bytes(half(record, at).to_ne_bytes());
    Some(Part {
        id,
        flags: word(record, 0),
        accuracy: signed(PART_ACCURACY),
        power: i32::from_ne_bytes(word(record, PART_BONUS).to_ne_bytes()),
        cost: signed(PART_COST),
        range: (record[PART_RANGE], record[PART_RANGE + 1]),
        turns: record[PART_TURNS],
    })
}

/// Part `id` as `character` would use it (`0x08036E74`): a weapon's power
/// and accuracy gain the pilot's attack and accuracy bonuses in percent.
#[must_use]
pub fn part(rom: &[u8], state: &[u8], character: u8, id: u16) -> Option<Part> {
    let mut part = part_record(rom, id)?;
    if part.flags & PART_WEAPON != 0 {
        let [_, _, _, attack, accuracy] = pilot(rom, state, character)?;
        part.power = part.power.wrapping_add(percent(part.power, attack));
        let raised = i32::from(part.accuracy) + percent(i32::from(part.accuracy), accuracy);
        let [low, high, ..] = raised.to_le_bytes();
        part.accuracy = i16::from_le_bytes([low, high]);
    }
    Some(part)
}

/// The part slots of the unit `character` pilots, as the Zoid status
/// screen's pages show them (`0x0804E04C`, `0x0804DFCC`); `None` when the
/// character has no unit.
#[must_use]
pub fn unit_parts(rom: &[u8], state: &[u8], character: u8) -> Option<[PartSlot; PART_SLOTS]> {
    slots_of(
        rom,
        state,
        character_unit(state, character)?,
        Some(character),
    )
}

/// The part slots of unit `unit`, its parts gaining the bonuses of its
/// pilot when it has one, as the lab's unit pages show them
/// (`0x08055024`).
#[must_use]
pub fn parts_of(rom: &[u8], state: &[u8], unit: u8) -> Option<[PartSlot; PART_SLOTS]> {
    slots_of(rom, state, unit, pilot_of(state, unit))
}

/// The part slots of unit `unit` as `character` would use them, as the
/// lab's pilot change shows the unit a character is to board.
#[must_use]
pub fn parts_for(
    rom: &[u8],
    state: &[u8],
    unit: u8,
    character: u8,
) -> Option<[PartSlot; PART_SLOTS]> {
    slots_of(rom, state, unit, Some(character))
}

fn slots_of(
    rom: &[u8],
    state: &[u8],
    unit: u8,
    pilot: Option<u8>,
) -> Option<[PartSlot; PART_SLOTS]> {
    let at = unit_at(unit);
    let unit = state.get(at..at + UNIT_LEN)?;
    let record = zoid_record(rom, half(unit, 6))?;
    let mut slots = [PartSlot {
        flags: 0,
        rack: 0,
        fitted: false,
        part: None,
    }; PART_SLOTS];
    for (index, slot) in slots.iter_mut().enumerate() {
        let entry = ZOID_PARTS + index * 4;
        slot.flags = half(record, entry);
        slot.rack = u8::try_from(slot.flags & RACK_KIND).unwrap_or(0);
        slot.fitted = half(record, entry + 2) != NO_PART;
        let id = half(unit, UNIT_PARTS + index * 4 + 2);
        if id != NO_PART {
            slot.part = Some(match pilot {
                Some(character) => part(rom, state, character, id)?,
                None => part_record(rom, id)?,
            });
        }
    }
    Some(slots)
}

/// The part slots Zoid `zoid`'s record gives a new unit, with the parts
/// as their records give them, as the lab's development shows them.
#[must_use]
pub fn record_parts(rom: &[u8], zoid: u16) -> Option<[PartSlot; PART_SLOTS]> {
    let record = zoid_record(rom, zoid)?;
    let mut slots = [PartSlot {
        flags: 0,
        rack: 0,
        fitted: false,
        part: None,
    }; PART_SLOTS];
    for (index, slot) in slots.iter_mut().enumerate() {
        let entry = ZOID_PARTS + index * 4;
        slot.flags = half(record, entry);
        slot.rack = u8::try_from(slot.flags & RACK_KIND).unwrap_or(0);
        let id = half(record, entry + 2);
        slot.fitted = id != NO_PART;
        if id != NO_PART {
            slot.part = Some(part_record(rom, id)?);
        }
    }
    Some(slots)
}

/// A Zoid's values as its record gives them to a new unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoidValues {
    /// Hit points.
    pub hp: u32,
    /// Energy points.
    pub ep: u32,
    /// SP.
    pub sp: i16,
    /// DF, in percent.
    pub df: i16,
    /// Size class: 0 S, 1 M, 2 L.
    pub size: u8,
}

/// Zoid `zoid`'s record values (`+0x40` on, and its size at `+4`).
#[must_use]
pub fn zoid_values(rom: &[u8], zoid: u16) -> Option<ZoidValues> {
    let record = zoid_record(rom, zoid)?;
    let signed = |at: usize| i16::from_ne_bytes(half(record, at).to_ne_bytes());
    Some(ZoidValues {
        hp: word(record, ZOID_STATS),
        ep: word(record, ZOID_STATS + 4),
        sp: signed(ZOID_STATS + 8),
        df: signed(ZOID_STATS + 10),
        size: record[4],
    })
}

/// How many of part `id` the party keeps in stock, 0 for a part that
/// cannot be stocked.
#[must_use]
pub fn stock(state: &[u8], id: u16) -> u8 {
    if usize::from(id) >= STOCKED_PARTS {
        return 0;
    }
    state.get(STOCK + usize::from(id)).copied().unwrap_or(0)
}

/// The stocked parts whose flags share a bit with `mask`, by id
/// (`0x0804E24C`): the status screen's weapon list takes weapons and
/// support parts (mask 15), the rack's list those that fit the rack.
#[must_use]
pub fn stocked_parts(rom: &[u8], state: &[u8], mask: u32) -> Vec<u16> {
    (0..STOCKED_PARTS)
        .filter_map(|id| u16::try_from(id).ok())
        .filter(|&id| stock(state, id) > 0)
        .filter(|&id| {
            let at = locate(rom, PART_RECORDS + usize::from(id) * PART_RECORD_LEN);
            rom.get(at..at + 4)
                .is_some_and(|flags| word(flags, 0) & mask != 0)
        })
        .collect()
}

/// Puts `part` on slot `slot` of the unit `character` pilots, or takes
/// the slot's part off when `part` is `None` (`0x08051B26`). The part it
/// held goes back to stock unless `discard` (the game asks first when
/// that stock is full), the new one leaves it, and the unit's statistics
/// are computed again, its current hit and energy points kept within
/// them. `None` when the character has no unit or a table is outside
/// `rom`.
pub fn equip(
    rom: &[u8],
    state: &mut [u8],
    character: u8,
    slot: usize,
    part: Option<u16>,
    discard: bool,
) -> Option<()> {
    let unit = character_unit(state, character)?;
    let at = unit_at(unit);
    let entry = at + UNIT_PARTS + slot.min(PART_SLOTS - 1) * 4 + 2;
    let old = half(state, entry);
    if old != NO_PART && !discard {
        let count = STOCK + usize::from(old);
        if usize::from(old) < STOCKED_PARTS {
            state[count] = state[count].wrapping_add(1);
        }
    }
    match part {
        Some(id) => {
            set_half(state, entry, id);
            if usize::from(id) < STOCKED_PARTS {
                let count = STOCK + usize::from(id);
                state[count] = state[count].wrapping_sub(1);
            }
        }
        None => set_half(state, entry, NO_PART),
    }
    compute_stats(rom, state, character, unit)?;
    for (current, full) in [(8, UNIT_STATS), (12, UNIT_STATS + 4)] {
        let full = word(state, at + full);
        if word(state, at + current) > full {
            set_word(state, at + current, full);
        }
    }
    Some(())
}

/// The items the pause menu can use on a Zoid, by id: what each gives
/// back (`0x08039580` with the routines at ROM `0x683AA8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemEffect {
    /// Hit points back, at most the full.
    Repair(i32),
    /// Paralysis gone (the unit's bit `0x4000`).
    Cure,
    /// Paralysis gone and the hit points full.
    Restore,
    /// Half the full hit points back.
    RepairHalf,
}

/// Item `item`'s effect on a Zoid, for the items that have one (0–5).
#[must_use]
pub const fn item_effect(item: u8) -> Option<ItemEffect> {
    match item {
        0 => Some(ItemEffect::Repair(300)),
        1 => Some(ItemEffect::Repair(150)),
        2 => Some(ItemEffect::Repair(50)),
        3 => Some(ItemEffect::Cure),
        4 => Some(ItemEffect::Restore),
        5 => Some(ItemEffect::RepairHalf),
        _ => None,
    }
}

const PARALYSED: u16 = 0x4000;
const UNIT_HP: usize = 8;

/// Uses item `item` on unit `unit` of the game-state block: its hit points
/// (`+8`, at most the full at `+0x28`) and its paralysis (bit `0x4000` of
/// the first half-word), as `0x0803967C` and its neighbours do.
pub fn use_item(state: &mut [u8], unit: u8, item: u8) -> Option<()> {
    let effect = item_effect(item)?;
    let at = unit_at(unit);
    let record = state.get_mut(at..at + UNIT_LEN)?;
    let signed = |value: u32| i32::from_ne_bytes(value.to_ne_bytes());
    let full = signed(word(record, UNIT_STATS));
    let repair = |record: &mut [u8], amount: i32| {
        let hp = signed(word(record, UNIT_HP)).wrapping_add(amount);
        let hp = if hp > full { full } else { hp };
        set_word(record, UNIT_HP, u32::from_ne_bytes(hp.to_ne_bytes()));
    };
    match effect {
        ItemEffect::Repair(amount) => repair(record, amount),
        ItemEffect::RepairHalf => repair(record, full >> 1),
        ItemEffect::Cure | ItemEffect::Restore => {
            let flags = half(record, 0) & !PARALYSED;
            set_half(record, 0, flags);
            if effect == ItemEffect::Restore {
                set_word(record, UNIT_HP, word(record, UNIT_STATS));
            }
        }
    }
    Some(())
}

const BROKEN: u16 = 0x800;
const UNIT_EP: usize = 0xC;
const ZOID_PRICE: usize = 0x28;
const REVIVAL_DIVISOR: i32 = 10;
const ZI_DATA: usize = 0x33E2;
const ZI_DATA_COUNT: usize = 0x99;

/// The units whose Zoid is broken (`0x080552A8`): every unit slot with a
/// Zoid (`+6` not 0) whose first half-word has bit `0x800`.
#[must_use]
pub fn broken_units(state: &[u8]) -> Vec<u8> {
    (0..ALL_UNITS)
        .filter_map(|unit| u8::try_from(unit).ok())
        .filter(|&unit| {
            let at = unit_at(unit);
            state.get(at..at + UNIT_LEN).is_some_and(|record| {
                half(record, UNIT_ZOID_INDEX) != 0 && half(record, 0) & BROKEN != 0
            })
        })
        .collect()
}

/// What the lab asks to revive unit `unit` (`0x08055E60`): its Zoid
/// record's price (`+0x28`) raised by the unit's training in percent, a
/// tenth of it.
#[must_use]
pub fn revival_price(rom: &[u8], state: &[u8], unit: u8) -> Option<u32> {
    let record = state.get(unit_at(unit)..unit_at(unit) + UNIT_LEN)?;
    let zoid = zoid_record(rom, half(record, UNIT_ZOID_INDEX))?;
    let value = i32::from_ne_bytes(word(zoid, ZOID_PRICE).to_ne_bytes());
    let raised = value.wrapping_add(percent(value, i32::from(record[UNIT_TRAINING])));
    u32::try_from(raised / REVIVAL_DIVISOR).ok()
}

/// Revives unit `unit` (`0x08055F0C`): the broken bit cleared, the hit
/// and energy points full.
pub fn revive(state: &mut [u8], unit: u8) {
    let at = unit_at(unit);
    let Some(record) = state.get_mut(at..at + UNIT_LEN) else {
        return;
    };
    set_half(record, 0, half(record, 0) & !BROKEN);
    fill(record);
}

/// Fills the hit and energy points of every unit in use (`0x08037148`,
/// after the lab): broken ones too, which stay broken.
pub fn heal_all(state: &mut [u8]) {
    for unit in (0..ALL_UNITS).filter_map(|unit| u8::try_from(unit).ok()) {
        let at = unit_at(unit);
        if let Some(record) = state.get_mut(at..at + UNIT_LEN)
            && half(record, 2) & IN_USE != 0
        {
            fill(record);
        }
    }
}

fn fill(record: &mut [u8]) {
    set_word(record, UNIT_HP, word(record, UNIT_STATS));
    set_word(record, UNIT_EP, word(record, UNIT_STATS + 4));
}

/// Whether a unit in use (`+6` not 0) is short of its full hit or energy
/// points and not broken: the lab then says it repaired them
/// (`0x080558E0`).
#[must_use]
pub fn any_damaged(state: &[u8]) -> bool {
    (0..ALL_UNITS)
        .filter_map(|unit| u8::try_from(unit).ok())
        .any(|unit| {
            let at = unit_at(unit);
            state.get(at..at + UNIT_LEN).is_some_and(|record| {
                half(record, UNIT_ZOID_INDEX) != 0
                    && half(record, 0) & BROKEN == 0
                    && (word(record, UNIT_STATS) > word(record, UNIT_HP)
                        || word(record, UNIT_STATS + 4) > word(record, UNIT_EP))
            })
        })
}

/// The party member who pilots unit `unit`, if any.
#[must_use]
pub fn pilot_of(state: &[u8], unit: u8) -> Option<u8> {
    members(state)
        .into_iter()
        .find(|&character| character_unit(state, character) == Some(unit))
}

/// How many Zoids' Zi data the party holds (`0x0804E3A0`: bytes `+0x33E2`
/// on, for the Zoids 0–0x98).
#[must_use]
pub fn zi_data_count(state: &[u8]) -> usize {
    state
        .get(ZI_DATA..ZI_DATA + ZI_DATA_COUNT)
        .map_or(0, |bytes| bytes.iter().filter(|&&byte| byte != 0).count())
}

/// How many kinds of Zi-data item (Zoid cores) the party holds at least
/// one of (the counts at `+0x330C`).
#[must_use]
pub fn zi_item_kinds(state: &[u8]) -> usize {
    state
        .get(ZI_ITEMS..ZI_ITEMS + ZI_ITEM_KINDS)
        .map_or(0, |counts| {
            counts.iter().filter(|&&count| count != 0).count()
        })
}

/// The units the party has (`+0x3304`).
#[must_use]
pub fn unit_count(state: &[u8]) -> u8 {
    state.get(UNIT_COUNT).copied().unwrap_or(0)
}

/// Puts the formation right after pilots changed Zoids (the lab's way
/// out, `0x0805581A`): a slot whose pilot has no unit any more, or a
/// broken one, is emptied; one whose pilot flies another unit now takes
/// that unit.
pub fn fix_formation(state: &mut [u8]) {
    for slot in 0..FORMATION_SLOTS {
        let at = FORMATION + slot * 4;
        let (Some(&unit), Some(&character)) = (state.get(at), state.get(at + 1)) else {
            continue;
        };
        if unit == NO_UNIT {
            continue;
        }
        let flown = character_unit(state, character).unwrap_or(NO_UNIT);
        if flown == unit {
            continue;
        }
        let broken = flown != NO_UNIT
            && state
                .get(unit_at(flown)..unit_at(flown) + 2)
                .is_some_and(|flags| half(flags, 0) & BROKEN != 0);
        if flown == NO_UNIT || broken {
            leave_formation(state, slot);
            continue;
        }
        let old = unit_at(unit) + 2;
        let new = unit_at(flown) + 2;
        if state.len() >= old.max(new) + 2 {
            let flags = half(state, old) & !IN_FORMATION;
            set_half(state, old, flags);
            let flags = half(state, new) | IN_FORMATION;
            set_half(state, new, flags);
            state[at] = flown;
        }
    }
}

fn half(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn set_half(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn set_word(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn unit_at(unit: u8) -> usize {
    UNITS + usize::from(unit) * UNIT_LEN
}

/// Zoid `zoid`'s class: its record's flags (`+0`, 2 for a flying one) and
/// size class (`+4`: 0 S, 1 M, 2 L).
#[must_use]
pub fn zoid_class(rom: &[u8], zoid: u16) -> Option<(u16, u8)> {
    let record = zoid_record(rom, zoid)?;
    Some((half(record, 0), record[4]))
}

fn zoid_record(rom: &[u8], zoid: u16) -> Option<&[u8]> {
    let at = locate(rom, ZOID_RECORDS + usize::from(zoid) * ZOID_RECORD_LEN);
    rom.get(at..at + ZOID_RECORD_LEN)
}

/// Allocates a unit of `zoid` in the first free slot, among the special
/// ones when `special` (`0x08036A30`), recounting the ordinary units in
/// use; `None` when none is free or the Zoid is unknown.
fn add_unit(rom: &[u8], state: &mut [u8], zoid: u16, special: bool) -> Option<u8> {
    let in_use = |state: &[u8], slot: usize| half(state, UNITS + slot * UNIT_LEN + 2) & IN_USE != 0;
    let slots = if special {
        ORDINARY_UNITS..ALL_UNITS
    } else {
        state[UNIT_COUNT] = u8::try_from(
            (0..ORDINARY_UNITS)
                .filter(|&slot| in_use(state, slot))
                .count(),
        )
        .unwrap_or(u8::MAX);
        0..ORDINARY_UNITS
    };
    let slot = slots.clone().find(|&slot| !in_use(state, slot))?;
    let unit = u8::try_from(slot).ok()?;
    let record = zoid_record(rom, zoid)?;
    let at = unit_at(unit);
    state[at..at + UNIT_LEN].fill(0);
    set_half(state, at + 4, u16::from(unit));
    set_half(state, at + 6, zoid);
    set_half(state, at + 2, IN_USE);
    state[UNIT_COUNT] = state[UNIT_COUNT].wrapping_add(1);
    set_half(state, at, half(record, 0));
    state[at + UNIT_TRAINING] = 0;
    state[at + UNIT_STATS..at + UNIT_STATS + STATS_LEN]
        .copy_from_slice(&record[ZOID_STATS..ZOID_STATS + STATS_LEN]);
    state[at + UNIT_VARIANT] = record[4];
    state[at + UNIT_PARTS..at + UNIT_PARTS + PARTS_LEN]
        .copy_from_slice(&record[ZOID_PARTS..ZOID_PARTS + PARTS_LEN]);
    let (current, energy) = (
        word(state, at + UNIT_STATS),
        word(state, at + UNIT_STATS + 4),
    );
    set_word(state, at + 8, current);
    set_word(state, at + 12, energy);
    Some(unit)
}

/// Computes again the statistics of `unit`, piloted by `character`, from
/// its Zoid, training, parts and pilot (`0x08036CB0`), as a battle does
/// for each unit of the formation when it starts (`0x0802B5D0`).
pub fn refresh_stats(rom: &[u8], state: &mut [u8], character: u8, unit: u8) -> Option<()> {
    compute_stats(rom, state, character, unit)
}

/// Makes `character` the pilot of `unit` and computes the unit's
/// statistics (`0x08036BE0`).
fn assign(rom: &[u8], state: &mut [u8], character: u8, unit: u8) -> Option<()> {
    state[CHARACTERS + usize::from(character) * CHARACTER_LEN + CHARACTER_UNIT] = unit;
    let at = unit_at(unit);
    let flags = half(state, at + 2) | PILOTED;
    set_half(state, at + 2, flags);
    compute_stats(rom, state, character, unit)
}

/// The unit's statistics from its Zoid, training, parts and pilot, capped
/// (`0x08036CB0`). The half-words are stored after every step, so they
/// wrap to 16 bits and read back signed.
fn compute_stats(rom: &[u8], state: &mut [u8], character: u8, unit: u8) -> Option<()> {
    let at = unit_at(unit);
    let record = zoid_record(rom, half(state, at + 6))?;
    let signed = |value: u16| i32::from(i16::from_ne_bytes(value.to_ne_bytes()));
    let as_half = |value: i32| {
        let bytes = value.to_le_bytes();
        u16::from_le_bytes([bytes[0], bytes[1]])
    };
    let mut words = [word(record, ZOID_STATS), word(record, ZOID_STATS + 4)]
        .map(|value| i32::from_ne_bytes(value.to_ne_bytes()));
    let mut halves = [half(record, ZOID_STATS + 8), half(record, ZOID_STATS + 10)];
    let training = i32::from(state[at + UNIT_TRAINING] >> 1);
    for value in &mut words {
        *value = value.wrapping_add(percent(*value, training));
    }
    for value in &mut halves {
        *value = as_half(signed(*value) + percent(signed(*value), training));
    }
    for slot in 0..PART_SLOTS {
        let part = half(state, at + UNIT_PARTS + slot * 4 + 2);
        if part == NO_PART {
            continue;
        }
        let part_at = locate(rom, PART_RECORDS + usize::from(part) * PART_RECORD_LEN);
        let record = rom.get(part_at..part_at + PART_RECORD_LEN)?;
        let flags = word(record, 0);
        if flags & PART_ACTIVE == 0 {
            continue;
        }
        let bonus = word(record, PART_BONUS);
        for (bit, field) in PART_BONUSES {
            if flags & bit == 0 {
                continue;
            }
            match field {
                0 | 4 => {
                    let value = &mut words[field / 4];
                    *value = value.wrapping_add(i32::from_ne_bytes(bonus.to_ne_bytes()));
                }
                _ => {
                    let value = &mut halves[(field - 8) / 2];
                    *value = value.wrapping_add(u16::try_from(bonus & 0xFFFF).unwrap_or(0));
                }
            }
        }
    }
    if character != NO_UNIT {
        let pilot = pilot(rom, state, character)?;
        words[0] = words[0].wrapping_add(percent(words[0], pilot[0]));
        for (value, bonus) in halves.iter_mut().zip([pilot[1], pilot[2]]) {
            *value = as_half(signed(*value) + percent(signed(*value), bonus));
        }
    }
    for (value, max) in words.iter_mut().zip(MAX_WORD) {
        *value = (*value).min(max);
    }
    for (value, max) in halves.iter_mut().zip(MAX_HALF) {
        if signed(*value) > max {
            *value = as_half(max);
        }
    }
    let base = at + UNIT_STATS;
    for (index, value) in words.iter().enumerate() {
        set_word(
            state,
            base + index * 4,
            u32::from_ne_bytes(value.to_ne_bytes()),
        );
    }
    for (index, value) in halves.iter().enumerate() {
        set_half(state, base + 8 + index * 2, *value);
    }
    Some(())
}

/// `percent`% of `value`, rounded to nearest for values below 0x10000, as
/// the routine at `0x080346C0` computes it.
#[must_use]
pub fn percent(value: i32, percent: i32) -> i32 {
    if value >> 16 == 0 {
        let scaled = (value << 16) / 100;
        let scaled = scaled.wrapping_mul(percent);
        (scaled >> 16) + i32::from(scaled & 0xFFFF > 0x7FFF)
    } else {
        (value / 100).wrapping_mul(percent)
    }
}

/// The pilot's bonuses in percent, in the record's order: to the first
/// statistic, the two half-words, then two the statistics routine does not
/// use: the member records for the first four characters, the
/// table at ROM `0x67B35C` by chapter for the others (`0x080334F8`).
#[must_use]
pub fn pilot(rom: &[u8], state: &[u8], character: u8) -> Option<[i32; PILOT_VALUES]> {
    let record = pilot_record(rom, state, character)?;
    let signed = |at: usize| i32::from(i16::from_ne_bytes(half(&record, at).to_ne_bytes()));
    Some([signed(4), signed(6), signed(8), signed(10), signed(12)])
}

/// Who a pilot is to the attack scenes: its record's byte 2, the portrait
/// they show and the `battle` strings they speak (`0x08041AA8`).
#[must_use]
pub fn pilot_face(rom: &[u8], state: &[u8], character: u8) -> Option<u8> {
    pilot_record(rom, state, character).map(|record| record[PILOT_FACE])
}

fn pilot_record(rom: &[u8], state: &[u8], character: u8) -> Option<Vec<u8>> {
    let character = usize::from(character);
    if character < MEMBERS {
        let at = MEMBER_RECORDS + character * MEMBER_RECORD_LEN;
        return Some(state.get(at..at + MEMBER_RECORD_LEN)?.to_vec());
    }
    let chapter = usize::from(state[AREA].wrapping_sub(1));
    let chapter = if chapter > PILOT_CHAPTERS { 0 } else { chapter };
    let at = locate(
        rom,
        PILOT_TABLE + (character * PILOT_CHAPTERS + chapter) * 4,
    );
    let pointer = word(rom.get(at..at + 4)?, 0);
    let start = usize::try_from(pointer.checked_sub(ROM_BASE)?).ok()?;
    Some(rom.get(start..start + MEMBER_RECORD_LEN)?.to_vec())
}

/// Puts `character`'s unit in formation slot `slot` (`0x08037AB4`).
fn place(state: &mut [u8], character: u8, slot: u8) {
    let entry = CHARACTERS + usize::from(character) * CHARACTER_LEN;
    let unit = state[entry + CHARACTER_UNIT];
    if unit == NO_UNIT {
        return;
    }
    let formation = FORMATION + usize::from(slot) * 4;
    state[formation + 1] = character;
    state[formation] = unit;
    let at = unit_at(unit);
    let flags = half(state, at + 2) | IN_FORMATION;
    set_half(state, at + 2, flags);
    let bits = half(state, entry) | CHARACTER_IN_FORMATION;
    set_half(state, entry, bits);
}

/// The formation slot `character` stands in (`0x080378A4`).
#[must_use]
pub fn formation_slot(state: &[u8], character: u8) -> Option<usize> {
    (0..FORMATION_SLOTS).find(|slot| state.get(FORMATION + slot * 4 + 1) == Some(&character))
}

/// Puts `character`'s unit in formation slot `slot` as the formation
/// screen does (`0x080371AC`). An L unit fills a column: it stands in its
/// middle slot (1 or 4) and empties the other two. Another unit empties
/// the slot first, or the middle of its column when an L unit fills it.
pub fn join_formation(state: &mut [u8], slot: usize, character: u8) {
    let (Some(unit), Ok(target)) = (character_unit(state, character), u8::try_from(slot)) else {
        return;
    };
    if slot >= FORMATION_SLOTS {
        return;
    }
    let emptied = if state.get(unit_at(unit) + UNIT_VARIANT) == Some(&LARGE) {
        if slot == 1 { 0..3 } else { 3..6 }
    } else {
        let emptied = large_occupant(state, slot).unwrap_or(slot);
        emptied..emptied + 1
    };
    for emptied in emptied {
        if state[FORMATION + emptied * 4 + 1] != NO_UNIT {
            unplace(state, emptied);
        }
    }
    place(state, character, target);
}

/// Empties formation slot `slot` as the formation screen does
/// (`0x08037258`), or the middle of its column when an L unit fills it.
pub fn leave_formation(state: &mut [u8], slot: usize) {
    if slot < FORMATION_SLOTS {
        unplace(state, large_occupant(state, slot).unwrap_or(slot));
    }
}

/// The middle slot of `slot`'s column when an L unit stands there
/// (`0x0803727C`).
fn large_occupant(state: &[u8], slot: usize) -> Option<usize> {
    let middle = if slot < COLUMN { 1 } else { COLUMN + 1 };
    let unit = *state.get(FORMATION + middle * 4)?;
    (unit != NO_UNIT && state.get(unit_at(unit) + UNIT_VARIANT) == Some(&LARGE)).then_some(middle)
}

/// Empties formation slot `slot` (`0x08037B1C`): the unit and its pilot
/// leave the formation. The game does not check that the slot holds
/// anyone: for an empty one it clears the bits at the entries of unit and
/// character `0xFF`, and the character's lies inside the block.
fn unplace(state: &mut [u8], slot: usize) {
    let at = FORMATION + slot * 4;
    let (unit, character) = (state[at], state[at + 1]);
    state[at] = NO_UNIT;
    state[at + 1] = NO_UNIT;
    clear_bits(state, unit_at(unit) + 2, IN_FORMATION);
    clear_bits(
        state,
        CHARACTERS + usize::from(character) * CHARACTER_LEN,
        CHARACTER_IN_FORMATION,
    );
}

fn clear_bits(state: &mut [u8], at: usize, bits: u16) {
    if at + 2 <= state.len() {
        let value = half(state, at) & !bits;
        set_half(state, at, value);
    }
}

/// Adds the characters of list `list` to the party (`0x080374B8`): each
/// entry marks its character and gives it a unit of its Zoid, as the
/// hangar's first list does (see [`form_party`]).
pub fn join_group(rom: &[u8], state: &mut [u8], list: usize) -> Option<()> {
    if state.len() != STATE_LEN {
        return None;
    }
    for (character, bits, zoid) in starting_list(rom, list)? {
        add_character(rom, state, character, bits, zoid)?;
    }
    Some(())
}

/// Takes the characters of list `list` out of the party (`0x080374E4`):
/// out of the formation, their own units gone with them, their seats in
/// other units left empty. When no member is left with a working unit,
/// the prince's is repaired (`0x08037510`).
pub fn leave_group(rom: &[u8], state: &mut [u8], list: usize) -> Option<()> {
    if state.len() != STATE_LEN {
        return None;
    }
    for (character, _, _) in starting_list(rom, list)? {
        remove_character(rom, state, character)?;
    }
    repair_when_all_broken(state);
    Some(())
}

/// When no member is left with a working unit, the prince's is repaired
/// (`0x08037510`).
fn repair_when_all_broken(state: &mut [u8]) {
    if !members(state).iter().any(|&member| {
        character_unit(state, member).is_some_and(|unit| half(state, unit_at(unit)) & BROKEN == 0)
    }) && let Some(unit) = character_unit(state, 0)
    {
        let at = unit_at(unit);
        let (full, energy) = (
            word(state, at + UNIT_STATS),
            word(state, at + UNIT_STATS + 4),
        );
        set_word(state, at + 8, full);
        set_word(state, at + 12, energy);
        let flags = half(state, at) & !BROKEN;
        set_half(state, at, flags);
    }
}

/// The characters chapter 9's base offers to come along (ROM `0x32AF28`,
/// 8-byte records: the character, its Zoid and what offers it).
const COMPANIONS: usize = 0x0032_AF28;
const COMPANION_LEN: usize = 8;
/// Records the table holds.
pub const COMPANION_COUNT: usize = 0x1D;
/// The level bits a companion joins with (`0x08026614`), and those of one
/// who joins without a Zoid (`0x0802A67C`).
const COMPANION_BITS: u16 = 0xE;
const LONE_COMPANION_BITS: u16 = 6;
/// The characters whose first flag bit offers the records of kinds 2 and
/// 3 (`+0x34EC` and `+0x34F0` of the game state).
const COMPANION_KEYS: [u8; 2] = [0x12, 0x13];

/// A record of the companions' table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Companion {
    /// The character.
    pub character: u8,
    /// The Zoid they come with.
    pub zoid: u16,
    /// What offers them: 0 always, 1 their own character's first flag bit,
    /// 2 and 3 that of characters `0x12` and `0x13`.
    pub offered_by: u16,
}

/// Record `index` of the companions' table, `None` past it.
#[must_use]
pub fn companion(rom: &[u8], index: usize) -> Option<Companion> {
    if index >= COMPANION_COUNT {
        return None;
    }
    let at = locate(rom, COMPANIONS + index * COMPANION_LEN);
    let record = rom.get(at..at + COMPANION_LEN)?;
    Some(Companion {
        character: u8::try_from(half(record, 0)).ok()?,
        zoid: half(record, 2),
        offered_by: half(record, 4),
    })
}

/// Whether record `index` is offered (`0x08026694`): always, or when its
/// character, or character `0x12` or `0x13`, has the first flag bit.
#[must_use]
pub fn companion_offered(rom: &[u8], state: &[u8], index: usize) -> bool {
    let Some(companion) = companion(rom, index) else {
        return false;
    };
    let flagged = |character: u8| {
        let entry = CHARACTERS + usize::from(character) * CHARACTER_LEN;
        state
            .get(entry..entry + 2)
            .is_some_and(|flags| flags[0] & 1 != 0)
    };
    match companion.offered_by {
        0 => true,
        1 => flagged(companion.character),
        kind @ (2 | 3) => flagged(COMPANION_KEYS[usize::from(kind - 2)]),
        _ => false,
    }
}

/// Adds the character of record `index` to the party: with a unit of its
/// Zoid when `zoid` (`0x08026614`, `0x080372D0` with bits `0xE`), as
/// chapter 9 does, else alone (`0x0802A67C`, `0x080372D0` with bits 6 and
/// no Zoid), as chapter 10 does.
pub fn join_companion(rom: &[u8], state: &mut [u8], index: usize, zoid: bool) -> Option<()> {
    if state.len() != STATE_LEN {
        return None;
    }
    let companion = companion(rom, index)?;
    if zoid {
        add_character(
            rom,
            state,
            companion.character,
            COMPANION_BITS,
            companion.zoid,
        )
    } else {
        add_character(rom, state, companion.character, LONE_COMPANION_BITS, 0)
    }
}

/// Takes the character of record `index` out of the party (`0x08026634`:
/// `0x0803738C`, then `0x08037510`).
pub fn leave_companion(rom: &[u8], state: &mut [u8], index: usize) -> Option<()> {
    if state.len() != STATE_LEN {
        return None;
    }
    let companion = companion(rom, index)?;
    remove_character(rom, state, companion.character)?;
    repair_when_all_broken(state);
    Some(())
}

/// Takes `character` out of the party (`0x0803738C`).
fn remove_character(rom: &[u8], state: &mut [u8], character: u8) -> Option<()> {
    let entry = CHARACTERS + usize::from(character) * CHARACTER_LEN;
    if half(state, entry) & PARTY_MEMBER == 0 {
        return Some(());
    }
    if half(state, entry) & CHARACTER_IN_FORMATION != 0 {
        let slot = (0..FORMATION_SLOTS).find(|slot| state[FORMATION + slot * 4 + 1] == character);
        if let Some(slot) = slot {
            unplace(state, slot);
        }
        let flags = half(state, entry) & !CHARACTER_IN_FORMATION;
        set_half(state, entry, flags);
    }
    if half(state, entry) & KEEPS_EQUIPMENT != 0 {
        let at = unit_at(state[entry + CHARACTER_UNIT]);
        state.get_mut(at..at + UNIT_LEN)?.fill(0);
        state[entry + CHARACTER_UNIT] = NO_UNIT;
        let flags = half(state, entry) & !KEEPS_EQUIPMENT;
        set_half(state, entry, flags);
    }
    if state[entry + CHARACTER_UNIT] != NO_UNIT {
        unassign(rom, state, character)?;
    }
    let flags = half(state, entry) & !(SPECIAL_UNIT | PARTY_MEMBER);
    set_half(state, entry, flags);
    Some(())
}

/// Leaves `character`'s unit without a pilot (`0x08036C2C`): its values
/// are worked out again without one, and what it has left is capped by
/// them.
fn unassign(rom: &[u8], state: &mut [u8], character: u8) -> Option<()> {
    let seat = CHARACTERS + usize::from(character) * CHARACTER_LEN + CHARACTER_UNIT;
    let unit = state[seat];
    if unit == NO_UNIT {
        return Some(());
    }
    state[seat] = NO_UNIT;
    let at = unit_at(unit);
    let flags = half(state, at + 2) & !PILOTED;
    set_half(state, at + 2, flags);
    compute_stats(rom, state, NO_UNIT, unit)?;
    for (current, full) in [(at + 8, at + UNIT_STATS), (at + 12, at + UNIT_STATS + 4)] {
        let signed = |at| i32::from_ne_bytes(word(state, at).to_ne_bytes());
        if signed(full) < signed(current) {
            let value = word(state, full);
            set_word(state, current, value);
        }
    }
    Some(())
}

/// The entries of starting list `list` (ROM `0x67E380`): the character's
/// flag bits, the character and its Zoid.
fn starting_list(rom: &[u8], list: usize) -> Option<Vec<(u8, u16, u16)>> {
    let at = locate(rom, STARTING_LISTS + list * 4);
    let pointer = word(rom.get(at..at + 4)?, 0);
    let mut at = usize::try_from(pointer.checked_sub(ROM_BASE)?).ok()?;
    let mut entries = Vec::new();
    loop {
        let entry = rom.get(at..at + 4)?;
        if entry[2] == LIST_END {
            return Some(entries);
        }
        entries.push((entry[2], half(entry, 0), u16::from(entry[3])));
        at += 4;
    }
}

/// Marks `character` with `bits` and, when `zoid` is not 0, gives them a
/// unit of it with full values (`0x080372D0`).
fn add_character(rom: &[u8], state: &mut [u8], character: u8, bits: u16, zoid: u16) -> Option<()> {
    let entry = CHARACTERS + usize::from(character) * CHARACTER_LEN;
    let flags = half(state, entry) | bits | 1;
    set_half(state, entry, flags);
    if zoid == 0 {
        return Some(());
    }
    let special = bits & SPECIAL_UNIT != 0;
    let Some(unit) = add_unit(rom, state, zoid, special) else {
        return Some(());
    };
    let at = unit_at(unit);
    if special {
        let flags = half(state, at + 2) | SPECIAL;
        set_half(state, at + 2, flags);
        state[at + UNIT_TRAINING] = SPECIAL_TRAINING;
    }
    assign(rom, state, character, unit)?;
    let (full, energy) = (
        word(state, at + UNIT_STATS),
        word(state, at + UNIT_STATS + 4),
    );
    set_word(state, at + 8, full);
    set_word(state, at + 12, energy);
    Some(())
}

/// What developing a Zoid from its Zi data asks for, as the Zi data list
/// shows it: its record's money, Zoid and two Zi-data items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Development {
    /// Money needed.
    pub money: u32,
    /// The Zoid needed: 0 none, below `0xFA` a Zoid, from `0xFA` one of
    /// the special kinds the pause menu names from its script 219.
    pub zoid: u8,
    /// The Zi-data items needed, [`NOT_NEEDED`] for none.
    pub items: [u8; 2],
}

/// What developing Zoid `zoid` asks for (its record at ROM `0x670210`,
/// from `+0x24`), `None` outside `rom`.
#[must_use]
pub fn development(rom: &[u8], zoid: u8) -> Option<Development> {
    let at = locate(rom, ZOID_RECORDS + usize::from(zoid) * ZOID_RECORD_LEN);
    let record = rom.get(at..at + ZOID_RECORD_LEN)?;
    let money = record.get(DEVELOPMENT_MONEY..DEVELOPMENT_MONEY + 4)?;
    Some(Development {
        money: u32::from_le_bytes(money.try_into().ok()?),
        zoid: *record.get(DEVELOPMENT_ZOID)?,
        items: [
            *record.get(DEVELOPMENT_ITEMS)?,
            *record.get(DEVELOPMENT_ITEMS + 1)?,
        ],
    })
}

/// A development's shortfall: what `0x0805534C` finds missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shortfall {
    /// The money the record asks for is more than `money`.
    pub money: bool,
    /// No unit can serve as the Zoid the record asks for.
    pub zoid: bool,
    /// One of the Zi-data items the record asks for is not carried.
    pub items: bool,
}

impl Shortfall {
    /// Whether nothing is missing.
    #[must_use]
    pub const fn is_met(self) -> bool {
        !self.money && !self.zoid && !self.items
    }
}

/// What developing Zoid `zoid` lacks with `money` in hand (`0x0805534C`):
/// the money, a unit to build it from (see [`development_bases`]) and
/// each Zi-data item it asks for, whose count at `+0x330C` must not be 0.
#[must_use]
pub fn development_shortfall(rom: &[u8], state: &[u8], money: u32, zoid: u8) -> Shortfall {
    let Some(needed) = development(rom, zoid) else {
        return Shortfall::default();
    };
    let carried = |item: u8| {
        item == NOT_NEEDED
            || state
                .get(ZI_ITEMS + usize::from(item))
                .is_some_and(|&n| n != 0)
    };
    Shortfall {
        money: money < needed.money,
        zoid: needed.zoid != 0 && development_bases(rom, state, zoid).is_empty(),
        items: !needed.items.iter().all(|&item| carried(item)),
    }
}

/// The units developing Zoid `zoid` can be built from (`0x08055198`),
/// every unit slot 0–`0xAC` in order: whose Zoid is the one the record
/// asks for, or, from `0xFA`, one of the Zoids of that kind's list (ROM
/// `0x75C018`, eight bytes a kind, `0xFF` after the last) other than
/// `zoid` itself. The slots are read as stored, in use or not.
#[must_use]
pub fn development_bases(rom: &[u8], state: &[u8], zoid: u8) -> Vec<u8> {
    let Some(needed) = development(rom, zoid) else {
        return Vec::new();
    };
    if needed.zoid == 0 {
        return Vec::new();
    }
    let kinds: Vec<u16> = if needed.zoid < SPECIAL_KINDS_FROM {
        vec![u16::from(needed.zoid)]
    } else {
        let at = locate(
            rom,
            SPECIAL_KINDS + usize::from(needed.zoid - SPECIAL_KINDS_FROM) * SPECIAL_KIND_LEN,
        );
        rom.get(at..)
            .unwrap_or_default()
            .iter()
            .take_while(|&&kind| kind != LIST_END)
            .filter(|&&kind| kind != zoid)
            .map(|&kind| u16::from(kind))
            .collect()
    };
    (0..ALL_UNITS)
        .filter_map(|slot| u8::try_from(slot).ok())
        .filter(|&unit| {
            let at = unit_at(unit) + UNIT_ZOID_INDEX;
            state
                .get(at..at + 2)
                .is_some_and(|zoid| kinds.contains(&half(zoid, 0)))
        })
        .collect()
}

/// The weapons on `unit`'s racks: each of its first three part slots that
/// holds a part and whose kind has bit 1 or 2, as the lab looks for them
/// before a unit is taken apart (`0x08056C94`, `0x08057F7C`).
#[must_use]
pub fn rack_weapons(state: &[u8], unit: u8) -> Vec<u16> {
    let at = unit_at(unit) + UNIT_PARTS;
    (0..RACKS)
        .filter_map(|rack| {
            let entry = state.get(at + rack * 4..at + rack * 4 + 4)?;
            let part = half(entry, 2);
            (part != NO_PART && half(entry, 0) & RACK_KIND != 0).then_some(part)
        })
        .collect()
}

/// Takes the weapons off `unit`'s racks (see [`rack_weapons`]) into the
/// stock, each while its count is below 9; a weapon the stock has no
/// room for is thrown away.
pub fn strip_racks(state: &mut [u8], unit: u8) {
    let at = unit_at(unit) + UNIT_PARTS;
    for rack in 0..RACKS {
        let entry = at + rack * 4;
        if state.len() < entry + 4 {
            return;
        }
        let part = half(state, entry + 2);
        if part == NO_PART || half(state, entry) & RACK_KIND == 0 {
            continue;
        }
        if let Some(count) = state.get_mut(STOCK + usize::from(part))
            && *count < STOCK_LIMIT
        {
            *count += 1;
        }
        set_half(state, entry + 2, NO_PART);
    }
}

/// Takes unit `unit` apart, as the lab does to one it builds from or
/// buys: its pilot leaves it (`0x08036C2C`) and its 56 bytes are cleared,
/// with the unit count less one (`0x08055314`).
pub fn take_apart(rom: &[u8], state: &mut [u8], unit: u8) -> Option<()> {
    if state.len() != STATE_LEN {
        return None;
    }
    if let Some(pilot) = pilot_of(state, unit) {
        unassign(rom, state, pilot)?;
    }
    let at = unit_at(unit);
    state[at..at + UNIT_LEN].fill(0);
    state[UNIT_COUNT] = state[UNIT_COUNT].wrapping_sub(1);
    Some(())
}

/// The units the lab's sale lists (`0x08055120` with `0xFF`): every unit
/// slot 0–`0xAC` whose Zoid is not 0, as stored.
#[must_use]
pub fn zoid_units(state: &[u8]) -> Vec<u8> {
    (0..ALL_UNITS)
        .filter_map(|slot| u8::try_from(slot).ok())
        .filter(|&unit| {
            let at = unit_at(unit) + UNIT_ZOID_INDEX;
            state.get(at..at + 2).is_some_and(|zoid| half(zoid, 0) != 0)
        })
        .collect()
}

/// Whether a unit slot 0–`0xAC` holds Zoid `zoid`, as stored
/// (`0x0802AACC`, which Dr. T asks).
#[must_use]
pub fn owns_zoid(state: &[u8], zoid: u16) -> bool {
    (0..ALL_UNITS).any(|slot| {
        let at = UNITS + slot * UNIT_LEN + UNIT_ZOID_INDEX;
        state
            .get(at..at + 2)
            .is_some_and(|stored| half(stored, 0) == zoid)
    })
}

/// How many of those units the party could part with (`0x08058458`):
/// the ones with no pilot or whose pilot does not keep them (flag
/// `0x08`). The lab buys nothing when fewer than five are.
#[must_use]
pub fn sellable_count(state: &[u8]) -> usize {
    zoid_units(state)
        .into_iter()
        .filter(|&unit| pilot_of(state, unit).is_none_or(|pilot| !keeps_equipment(state, pilot)))
        .count()
}

/// What the lab pays for unit `unit` (`0x08058458`): its Zoid record's
/// price (`+0x28`) raised by the unit's training in percent; 0 when the
/// record has no price.
#[must_use]
pub fn sale_price(rom: &[u8], state: &[u8], unit: u8) -> Option<u32> {
    let record = state.get(unit_at(unit)..unit_at(unit) + UNIT_LEN)?;
    let zoid = zoid_record(rom, half(record, UNIT_ZOID_INDEX))?;
    let value = i32::from_ne_bytes(word(zoid, ZOID_PRICE).to_ne_bytes());
    let raised = value.wrapping_add(percent(value, i32::from(record[UNIT_TRAINING])));
    u32::try_from(raised).ok()
}

/// Makes `character` the pilot of `unit` and works out the unit's values
/// (`0x08036BE0`), as the lab's pilot change does.
pub fn board(rom: &[u8], state: &mut [u8], character: u8, unit: u8) -> Option<()> {
    if state.len() != STATE_LEN {
        return None;
    }
    assign(rom, state, character, unit)
}

/// Leaves `character`'s unit without a pilot (`0x08036C2C`), as the lab's
/// pilot change does to the unit a character leaves and to the one they
/// take from another.
pub fn leave_unit(rom: &[u8], state: &mut [u8], character: u8) -> Option<()> {
    if state.len() != STATE_LEN {
        return None;
    }
    unassign(rom, state, character)
}

/// Unit `unit`'s values with no pilot (`0x08036CB0` with `0xFF`): its
/// full hit and energy points, SP and DF, as the pilot change's
/// comparison starts from.
#[must_use]
pub fn pilotless_values(rom: &[u8], state: &[u8], unit: u8) -> Option<ZoidValues> {
    let mut copy = state.to_vec();
    if copy.len() != STATE_LEN {
        return None;
    }
    compute_stats(rom, &mut copy, NO_UNIT, unit)?;
    let status = unit_status(&copy, unit)?;
    Some(ZoidValues {
        hp: status.hp.1,
        ep: status.ep.1,
        sp: status.sp,
        df: status.df,
        size: status.size,
    })
}

/// The units the lab's pilot change offers `character` (`0x08055120`):
/// every unit slot 0–`0xAC` whose Zoid is not 0, but the one they pilot.
#[must_use]
pub fn units_for(state: &[u8], character: u8) -> Vec<u8> {
    let own = character_unit(state, character);
    zoid_units(state)
        .into_iter()
        .filter(|&unit| Some(unit) != own)
        .collect()
}

/// What boarding an L unit does to `character`'s place in the formation
/// (`0x08057726`), when they stand in it: nothing in the middle of a
/// column whose other slots are empty; otherwise the column is put right
/// ([`FormationChange::Fix`]) when the other two are empty, or the
/// character leaves the formation ([`FormationChange::Leave`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationChange {
    /// The character moves to the middle of their column's slot `n`.
    Fix(usize),
    /// The character leaves slot `n`.
    Leave(usize),
}

/// See [`FormationChange`]; `None` when the unit is not L, the character
/// is not in the formation, or they already stand alone in a middle slot.
#[must_use]
pub fn formation_change(state: &[u8], character: u8, unit: u8) -> Option<FormationChange> {
    let entry = CHARACTERS + usize::from(character) * CHARACTER_LEN;
    let in_formation = state
        .get(entry..entry + 2)
        .is_some_and(|flags| half(flags, 0) & CHARACTER_IN_FORMATION != 0);
    if !in_formation || state.get(unit_at(unit) + UNIT_VARIANT) != Some(&LARGE) {
        return None;
    }
    let slot = formation_slot(state, character).unwrap_or(FORMATION_SLOTS);
    let column = if slot < COLUMN {
        0..COLUMN
    } else {
        COLUMN..FORMATION_SLOTS
    };
    let middle = column.start + 1;
    let crowded = column
        .filter(|&other| other != slot)
        .any(|other| state.get(FORMATION + other * 4) != Some(&NO_UNIT));
    if crowded {
        Some(FormationChange::Leave(slot))
    } else if slot == middle {
        None
    } else {
        Some(FormationChange::Fix(slot))
    }
}

/// Develops Zoid `zoid` (`0x08056C94`, past the questions): when it is
/// built from unit `base`, the unit's pilot leaves it (`0x08036C2C`) and
/// the unit is cleared (`0x08055314`); each Zi-data item the record asks
/// for is used up; a unit of the Zoid is allocated (`0x08036A30`) and its
/// values worked out with no pilot. The money is the caller's. Returns
/// the new unit, `None` when no slot is free or `rom` lacks the record.
pub fn develop(rom: &[u8], state: &mut [u8], zoid: u8, base: Option<u8>) -> Option<u8> {
    let needed = development(rom, zoid)?;
    if state.len() != STATE_LEN {
        return None;
    }
    if needed.zoid != 0
        && let Some(base) = base
    {
        take_apart(rom, state, base)?;
    }
    for item in needed.items {
        if item != NOT_NEEDED {
            let count = &mut state[ZI_ITEMS + usize::from(item)];
            *count = count.wrapping_sub(1);
        }
    }
    let unit = add_unit(rom, state, u16::from(zoid), false)?;
    compute_stats(rom, state, NO_UNIT, unit)?;
    Some(unit)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    #[test]
    fn the_lab_revives_broken_units_for_a_tenth_of_their_trained_price() {
        let mut rom = vec![0u8; ZOID_RECORDS + 4 * ZOID_RECORD_LEN];
        let price_at = ZOID_RECORDS + 3 * ZOID_RECORD_LEN + ZOID_PRICE;
        rom[price_at..price_at + 4].copy_from_slice(&6000u32.to_le_bytes());
        let mut state = vec![0u8; STATE_LEN];
        for unit in [1u8, 2] {
            let at = unit_at(unit);
            set_half(&mut state, at + 2, IN_USE);
            set_half(&mut state, at + UNIT_ZOID_INDEX, 3);
            set_word(&mut state, at + UNIT_HP, 10);
            set_word(&mut state, at + UNIT_STATS, 100);
            set_word(&mut state, at + UNIT_STATS + 4, 20);
        }
        set_half(&mut state, unit_at(2), BROKEN);
        state[unit_at(2) + UNIT_TRAINING] = 10;
        assert_eq!(broken_units(&state), [2]);
        assert_eq!(revival_price(&rom, &state, 2), Some(660));
        assert!(any_damaged(&state));
        revive(&mut state, 2);
        assert!(broken_units(&state).is_empty());
        assert_eq!(word(&state, unit_at(2) + UNIT_HP), 100);
        assert_eq!(word(&state, unit_at(2) + UNIT_EP), 20);
        heal_all(&mut state);
        assert_eq!(word(&state, unit_at(1) + UNIT_HP), 100);
        assert!(!any_damaged(&state));
    }

    #[test]
    fn items_repair_up_to_the_full_and_cure_paralysis() {
        let mut state = vec![0u8; UNITS + 3 * UNIT_LEN];
        let at = unit_at(2);
        set_word(&mut state, at + UNIT_HP, 40);
        set_word(&mut state, at + UNIT_STATS, 100);
        set_half(&mut state, at, 0x4001);
        use_item(&mut state, 2, 2).expect("in use");
        assert_eq!(word(&state, at + UNIT_HP), 90);
        use_item(&mut state, 2, 0).expect("in use");
        assert_eq!(word(&state, at + UNIT_HP), 100);
        set_word(&mut state, at + UNIT_HP, 10);
        use_item(&mut state, 2, 5).expect("in use");
        assert_eq!(word(&state, at + UNIT_HP), 60);
        use_item(&mut state, 2, 3).expect("in use");
        assert_eq!(half(&state, at), 1);
        use_item(&mut state, 2, 4).expect("in use");
        assert_eq!(word(&state, at + UNIT_HP), 100);
        assert!(use_item(&mut state, 2, 6).is_none());
    }

    #[test]
    fn percentages_round_small_values_and_truncate_large_ones() {
        assert_eq!(percent(250, 10), 25);
        assert_eq!(percent(25, 50), 13);
        assert_eq!(percent(15, 50), 7);
        assert_eq!(percent(0x2_0000, 50), 0x2_0000 / 100 * 50);
        assert_eq!(percent(100, 0), 0);
    }

    fn rom() -> Vec<u8> {
        let mut rom = vec![0; 0x0068_0000];
        for zoid in [0x39usize, 0x46] {
            let at = ZOID_RECORDS + zoid * ZOID_RECORD_LEN;
            rom[at] = 0x20;
            rom[at + 4] = 7;
            for slot in 0..PART_SLOTS {
                rom[at + ZOID_PARTS + slot * 4 + 2..at + ZOID_PARTS + slot * 4 + 4]
                    .copy_from_slice(&NO_PART.to_le_bytes());
            }
            rom[at + ZOID_STATS] = 100;
            rom[at + ZOID_STATS + 4] = 20;
            rom[at + ZOID_STATS + 8] = 250;
            rom[at + ZOID_STATS + 10] = 10;
        }
        let list = 0x0067_E000u32;
        rom[STARTING_LISTS..STARTING_LISTS + 4].copy_from_slice(&(ROM_BASE + list).to_le_bytes());
        let list = list as usize;
        rom[list..list + 8].copy_from_slice(&[2, 0, 1, 0x46, 0, 0, 0xFF, 0xFF]);
        rom[GROWTH..GROWTH + GROWTH_LEN].copy_from_slice(&[2, 0, 5, 0, 1, 0, 3, 0, 4, 0]);
        rom
    }

    fn state() -> Vec<u8> {
        let mut state = vec![0; STATE_LEN];
        for slot in 0..FORMATION_SLOTS {
            state[FORMATION + slot * 4..FORMATION + slot * 4 + 2].fill(NO_UNIT);
        }
        for character in 0..87 {
            state[CHARACTERS + character * CHARACTER_LEN + CHARACTER_UNIT] = NO_UNIT;
        }
        state
    }

    #[test]
    fn the_prince_pilots_the_picked_zoid_and_the_warriors_theirs() {
        let rom = rom();
        let mut state = state();
        form_party(&rom, &mut state, 0).expect("party");
        assert_eq!(state[UNIT_COUNT], 2);
        assert_eq!(half(&state, UNITS + 6), 0x39);
        assert_eq!(half(&state, UNITS + 2), IN_USE | PILOTED | IN_FORMATION);
        assert_eq!(state[UNITS + UNIT_VARIANT], 7);
        assert_eq!(word(&state, UNITS + UNIT_STATS), 100);
        assert_eq!(state[CHARACTERS + CHARACTER_UNIT], 0);
        assert_eq!(state[FORMATION + 4..FORMATION + 6], [0, 0]);
        let second = UNITS + UNIT_LEN;
        assert_eq!(half(&state, second + 6), 0x46);
        assert_eq!(word(&state, second + 8), 100);
        assert_eq!(state[CHARACTERS + CHARACTER_LEN + CHARACTER_UNIT], 1);
        assert_eq!(state[FORMATION + 16..FORMATION + 18], [1, 1]);
        assert_eq!(half(&state, CHARACTERS + CHARACTER_LEN) & 0x13, 0x13);
    }

    #[test]
    fn a_list_joins_the_party_and_leaves_it_again() {
        let mut rom = rom();
        let list = 0x0067_E100u32;
        rom[STARTING_LISTS + 4..STARTING_LISTS + 8]
            .copy_from_slice(&(ROM_BASE + list).to_le_bytes());
        let list = list as usize;
        rom[list..list + 12]
            .copy_from_slice(&[0x0A, 0, 4, 0x46, 0x02, 0, 5, 0x39, 0, 0, 0xFF, 0xFF]);
        let record = ROM_BASE + 0x0067_E200;
        for character in [4, 5] {
            let at = PILOT_TABLE + character * PILOT_CHAPTERS * 4;
            rom[at..at + 4].copy_from_slice(&record.to_le_bytes());
        }
        let mut state = state();
        mark_member(&mut state, 0);
        form_party(&rom, &mut state, 0).expect("party");
        join_group(&rom, &mut state, 1).expect("join");
        assert_eq!(members(&state), [0, 1, 4, 5]);
        let (own, borrowed) = (
            character_unit(&state, 4).expect("unit"),
            character_unit(&state, 5).expect("unit"),
        );
        assert_eq!(word(&state, unit_at(own) + 8), 100);
        join_formation(&mut state, 3, 4);
        assert_eq!(formation(&state)[3], Some((own, 4)));
        leave_group(&rom, &mut state, 1).expect("leave");
        assert_eq!(members(&state), [0, 1]);
        assert_eq!(formation(&state)[3], None);
        assert!(
            state[unit_at(own)..unit_at(own) + UNIT_LEN]
                .iter()
                .all(|&byte| byte == 0)
        );
        assert_eq!(half(&state, unit_at(borrowed) + 6), 0x39);
        assert_eq!(half(&state, unit_at(borrowed) + 2) & PILOTED, 0);
        assert_eq!(character_unit(&state, 5), None);
        assert_eq!(half(&state, CHARACTERS + 4 * CHARACTER_LEN) & 0x1E, 0);
        assert_eq!(join_group(&rom, &mut state[..8], 1), None);
    }

    #[test]
    fn the_status_screens_read_the_party_the_hangar_formed() {
        let rom = rom();
        let mut state = state();
        state[PARTY_LEVEL] = 2;
        mark_member(&mut state, 0);
        form_party(&rom, &mut state, 0).expect("party");
        assert_eq!(members(&state), [0, 1]);
        assert_eq!(character_unit(&state, 1), Some(1));
        assert_eq!(character_unit(&state, 5), None);
        let slots = formation(&state);
        assert_eq!(slots[1], Some((0, 0)));
        assert_eq!(slots[4], Some((1, 1)));
        assert_eq!(slots[0], None);
        let unit = unit_status(&state, 0).expect("unit");
        assert_eq!((unit.zoid, unit.hp, unit.ep), (0x39, (100, 100), (20, 20)));
        assert_eq!(
            (unit.sp, unit.df, unit.training, unit.size),
            (250, 10, 0, 7)
        );
        assert_eq!(unit_status(&state, 9), None);
        assert_eq!(pilot_bonuses(&rom, &state, 1), Some([4, 6, 2, 10, 8]));
        assert_eq!(pilot_bonuses(&rom, &state, 0), Some([0; PILOT_VALUES]));
    }

    #[test]
    fn a_weapon_gains_its_pilots_bonuses_and_a_support_part_does_not() {
        let mut rom = rom();
        let weapon = PART_RECORDS + 9 * PART_RECORD_LEN;
        rom[weapon] = 0x21;
        rom[weapon + PART_ACCURACY] = 110;
        rom[weapon + PART_BONUS + 2] = 25;
        rom[weapon + PART_RANGE..weapon + PART_TURNS].copy_from_slice(&[2, 1]);
        let shield = PART_RECORDS + 0x13C * PART_RECORD_LEN;
        rom[shield..shield + 2].copy_from_slice(&0x4004u16.to_le_bytes());
        rom[shield + PART_ACCURACY] = 20;
        rom[shield + PART_BONUS] = 20;
        rom[shield + PART_COST] = 2;
        rom[shield + PART_TURNS] = 3;
        let liger = ZOID_RECORDS + 0x39 * ZOID_RECORD_LEN;
        rom[liger + ZOID_PARTS + 4..liger + ZOID_PARTS + 8].copy_from_slice(&[3, 0, 9, 0]);
        rom[liger + ZOID_PARTS + 16..liger + ZOID_PARTS + 20].copy_from_slice(&[0, 0, 0x3C, 1]);
        let mut state = state();
        state[PARTY_LEVEL] = 2;
        mark_member(&mut state, 0);
        form_party(&rom, &mut state, 0).expect("party");

        let raised = part(&rom, &state, 1, 9).expect("part");
        assert_eq!(raised.power, (25 << 16) + (25 << 16) / 100 * 6);
        assert_eq!((raised.accuracy, raised.range), (119, (2, 1)));
        let plain = part(&rom, &state, 0, 9).expect("part");
        assert_eq!((plain.power, plain.accuracy), (25 << 16, 110));
        let support = part(&rom, &state, 1, 0x13C).expect("part");
        assert_eq!(
            (support.power, support.accuracy, support.cost, support.turns),
            (20, 20, 2, 3)
        );

        let slots = unit_parts(&rom, &state, 0).expect("slots");
        assert_eq!((slots[1].rack, slots[1].fitted), (3, true));
        assert_eq!(slots[1].part.map(|part| part.id), Some(9));
        assert_eq!((slots[0].rack, slots[0].part), (0, None));
        assert!(!slots[5].fitted);
        assert_eq!(slots[4].part.map(|part| part.flags), Some(0x4004));
        assert_eq!(unit_parts(&rom, &state, 5), None);
    }

    #[test]
    fn a_part_taken_off_goes_to_stock_and_back() {
        let mut rom = rom();
        let laser = PART_RECORDS + 9 * PART_RECORD_LEN;
        rom[laser] = 0x21;
        let shield = PART_RECORDS + 12 * PART_RECORD_LEN;
        rom[shield..shield + 4].copy_from_slice(&(PART_ACTIVE | 0x4004).to_le_bytes());
        rom[shield + PART_BONUS] = 5;
        let liger = ZOID_RECORDS + 0x39 * ZOID_RECORD_LEN;
        rom[liger + ZOID_PARTS + 4..liger + ZOID_PARTS + 8].copy_from_slice(&[3, 0, 9, 0]);
        let mut state = state();
        mark_member(&mut state, 0);
        form_party(&rom, &mut state, 0).expect("party");

        equip(&rom, &mut state, 0, 1, None, false).expect("taken off");
        assert_eq!(unit_parts(&rom, &state, 0).expect("slots")[1].part, None);
        assert_eq!(stock(&state, 9), 1);
        assert_eq!(stocked_parts(&rom, &state, 15), [9]);
        assert_eq!(stocked_parts(&rom, &state, 2), []);

        state[STOCK + 12] = 2;
        equip(&rom, &mut state, 0, 1, Some(12), false).expect("fitted");
        assert_eq!((stock(&state, 12), stock(&state, 9)), (1, 1));
        let unit = unit_status(&state, 0).expect("unit");
        assert_eq!(unit.df, 10 + 5);
        equip(&rom, &mut state, 0, 1, Some(9), true).expect("replaced");
        assert_eq!((stock(&state, 12), stock(&state, 9)), (1, 0));
        assert_eq!(unit_status(&state, 0).expect("unit").df, 10);
        assert_eq!(stock(&state, 200), 0);
    }

    fn mark_member(state: &mut [u8], character: usize) {
        let entry = CHARACTERS + character * CHARACTER_LEN;
        let flags = half(state, entry) | PARTY_MEMBER;
        set_half(state, entry, flags);
    }

    #[test]
    fn refuses_a_short_state_and_defaults_to_the_shield_liger() {
        let rom = rom();
        assert_eq!(form_party(&rom, &mut [0; 16], 0), None);
        let mut state = state();
        form_party(&rom, &mut state, 3).expect("party");
        assert_eq!(half(&state, UNITS + 6), SHIELD_LIGER);
    }

    /// Characters 0–3 pilot units 0–3; unit 2 is an L unit.
    fn crew() -> Vec<u8> {
        let mut state = vec![0; STATE_LEN];
        for slot in 0..FORMATION_SLOTS {
            state[FORMATION + slot * 4..FORMATION + slot * 4 + 2].fill(NO_UNIT);
        }
        for character in 0..4u8 {
            let at = CHARACTERS + usize::from(character) * CHARACTER_LEN;
            state[at + CHARACTER_UNIT] = character;
            set_half(&mut state, unit_at(character) + 2, IN_USE);
        }
        state[unit_at(2) + UNIT_VARIANT] = LARGE;
        state
    }

    fn slots(state: &[u8]) -> Vec<Option<u8>> {
        formation(state)
            .iter()
            .map(|slot| slot.map(|(_, character)| character))
            .collect()
    }

    #[test]
    fn a_unit_joins_a_slot_and_takes_it_from_another() {
        let mut state = crew();
        join_formation(&mut state, 0, 0);
        join_formation(&mut state, 0, 1);
        assert_eq!(slots(&state), [Some(1), None, None, None, None, None]);
        assert_eq!(formation_slot(&state, 1), Some(0));
        assert_eq!(formation_slot(&state, 0), None);
        assert_eq!(half(&state, CHARACTERS) & CHARACTER_IN_FORMATION, 0);
        assert_eq!(half(&state, unit_at(0) + 2), IN_USE);
        assert_eq!(half(&state, unit_at(1) + 2), IN_USE | IN_FORMATION);
    }

    #[test]
    fn an_l_unit_fills_its_column_and_leaves_it_whole() {
        let mut state = crew();
        join_formation(&mut state, 0, 0);
        join_formation(&mut state, 5, 1);
        join_formation(&mut state, 4, 2);
        assert_eq!(slots(&state), [Some(0), None, None, None, Some(2), None]);
        join_formation(&mut state, 3, 3);
        assert_eq!(slots(&state), [Some(0), None, None, Some(3), None, None]);
        join_formation(&mut state, 1, 2);
        assert_eq!(slots(&state), [None, Some(2), None, Some(3), None, None]);
        leave_formation(&mut state, 2);
        assert_eq!(slots(&state), [None, None, None, Some(3), None, None]);
    }

    #[test]
    fn leaving_an_empty_slot_clears_the_bit_of_character_0xff() {
        let mut state = crew();
        let stray = CHARACTERS + usize::from(NO_UNIT) * CHARACTER_LEN;
        set_half(&mut state, stray, 0x31);
        leave_formation(&mut state, 5);
        assert_eq!(half(&state, stray), 0x21);
        assert_eq!(slots(&state), [None; FORMATION_SLOTS]);
    }

    #[test]
    fn a_development_reads_its_zoids_record() {
        let mut rom = vec![0u8; ZOID_RECORDS + 3 * ZOID_RECORD_LEN];
        let at = ZOID_RECORDS + 2 * ZOID_RECORD_LEN;
        rom[at + DEVELOPMENT_MONEY..at + DEVELOPMENT_MONEY + 4]
            .copy_from_slice(&12_000u32.to_le_bytes());
        rom[at + DEVELOPMENT_ZOID] = 0xFA;
        rom[at + DEVELOPMENT_ITEMS] = 5;
        rom[at + DEVELOPMENT_ITEMS + 1] = NOT_NEEDED;
        assert_eq!(
            development(&rom, 2),
            Some(Development {
                money: 12_000,
                zoid: 0xFA,
                items: [5, NOT_NEEDED],
            })
        );
        assert_eq!(development(&rom, 3), None);
    }

    /// Zoid 5 is developed from a unit of Zoid 3 with 1000 G and item 7;
    /// Zoid 6 from any Zoid of special kind `0xFA`'s list.
    fn development_rom() -> Vec<u8> {
        let mut rom = vec![0u8; SPECIAL_KINDS + 16];
        for zoid in [3usize, 5, 6] {
            let at = ZOID_RECORDS + zoid * ZOID_RECORD_LEN;
            rom[at + ZOID_STATS..at + ZOID_STATS + 4].copy_from_slice(&50u32.to_le_bytes());
            rom[at + ZOID_PARTS..at + ZOID_PARTS + 4].copy_from_slice(&[3, 0, 0xFF, 0xFF]);
            rom[at + DEVELOPMENT_ITEMS..at + DEVELOPMENT_ITEMS + 2].fill(NOT_NEEDED);
        }
        let five = ZOID_RECORDS + 5 * ZOID_RECORD_LEN;
        rom[five + DEVELOPMENT_MONEY..five + DEVELOPMENT_MONEY + 4]
            .copy_from_slice(&1000u32.to_le_bytes());
        rom[five + DEVELOPMENT_ZOID] = 3;
        rom[five + DEVELOPMENT_ITEMS] = 7;
        let six = ZOID_RECORDS + 6 * ZOID_RECORD_LEN;
        rom[six + DEVELOPMENT_ZOID] = SPECIAL_KINDS_FROM;
        rom[SPECIAL_KINDS..SPECIAL_KINDS + 4].copy_from_slice(&[6, 3, 5, LIST_END]);
        rom
    }

    #[test]
    fn a_development_needs_its_money_base_and_items() {
        let rom = development_rom();
        let mut state = vec![0u8; STATE_LEN];
        let lacking = development_shortfall(&rom, &state, 999, 5);
        assert!(lacking.money && lacking.zoid && lacking.items);
        set_half(&mut state, unit_at(2) + 2, IN_USE);
        set_half(&mut state, unit_at(2) + UNIT_ZOID_INDEX, 3);
        set_half(&mut state, unit_at(9) + UNIT_ZOID_INDEX, 5);
        state[ZI_ITEMS + 7] = 1;
        assert!(development_shortfall(&rom, &state, 1000, 5).is_met());
        assert_eq!(development_bases(&rom, &state, 5), [2]);
        assert_eq!(development_bases(&rom, &state, 6), [2, 9]);
        assert!(development_bases(&rom, &state, 3).is_empty());
    }

    #[test]
    fn the_lab_buys_units_at_their_trained_price_while_five_are_left() {
        let mut rom = development_rom();
        let three = ZOID_RECORDS + 3 * ZOID_RECORD_LEN;
        rom[three + ZOID_PRICE..three + ZOID_PRICE + 4].copy_from_slice(&5000u32.to_le_bytes());
        let mut state = vec![0u8; STATE_LEN];
        for unit in [1u8, 4, 7, 8, 9] {
            set_half(&mut state, unit_at(unit) + 2, IN_USE);
            set_half(&mut state, unit_at(unit) + UNIT_ZOID_INDEX, 3);
        }
        state[unit_at(4) + UNIT_TRAINING] = 10;
        state[UNIT_COUNT] = 5;
        assert_eq!(zoid_units(&state), [1, 4, 7, 8, 9]);
        assert_eq!(sellable_count(&state), 5);
        assert_eq!(sale_price(&rom, &state, 4), Some(5500));
        assert_eq!(sale_price(&rom, &state, 2), Some(0));

        mark_member(&mut state, 1);
        state[CHARACTERS + CHARACTER_LEN + CHARACTER_UNIT] = 8;
        let flags = half(&state, CHARACTERS + CHARACTER_LEN) | KEEPS_EQUIPMENT;
        set_half(&mut state, CHARACTERS + CHARACTER_LEN, flags);
        assert_eq!(sellable_count(&state), 4);

        take_apart(&rom, &mut state, 7).expect("sold");
        assert_eq!(zoid_units(&state), [1, 4, 8, 9]);
        assert_eq!(state[UNIT_COUNT], 4);
    }

    #[test]
    fn boarding_an_l_unit_asks_for_room_in_the_formation() {
        let mut state = crew();
        let seat = |state: &mut Vec<u8>, slot: usize, character: u8| {
            state[FORMATION + slot * 4] = character;
            state[FORMATION + slot * 4 + 1] = character;
            let entry = CHARACTERS + usize::from(character) * CHARACTER_LEN;
            let flags = half(state, entry) | CHARACTER_IN_FORMATION;
            set_half(state, entry, flags);
        };
        seat(&mut state, 0, 0);
        seat(&mut state, 1, 1);
        seat(&mut state, 5, 3);
        assert_eq!(
            formation_change(&state, 0, 2),
            Some(FormationChange::Leave(0))
        );
        assert_eq!(formation_change(&state, 0, 1), None);
        assert_eq!(
            formation_change(&state, 3, 2),
            Some(FormationChange::Fix(5))
        );
        seat(&mut state, 4, 3);
        state[FORMATION + 5 * 4..FORMATION + 5 * 4 + 2].fill(NO_UNIT);
        assert_eq!(formation_change(&state, 3, 2), None);
        assert_eq!(formation_change(&state, 2, 2), None);
    }

    #[test]
    fn developing_takes_the_base_apart_and_uses_the_items() {
        let rom = development_rom();
        let mut state = vec![0u8; STATE_LEN];
        mark_member(&mut state, 1);
        let base = unit_at(0);
        set_half(&mut state, base + 2, IN_USE | PILOTED);
        set_half(&mut state, base + UNIT_ZOID_INDEX, 3);
        set_half(&mut state, base + UNIT_PARTS, 3);
        set_half(&mut state, base + UNIT_PARTS + 2, 12);
        set_half(&mut state, base + UNIT_PARTS + 6, NO_PART);
        set_half(&mut state, base + UNIT_PARTS + 10, NO_PART);
        state[CHARACTERS + CHARACTER_LEN + CHARACTER_UNIT] = 0;
        state[UNIT_COUNT] = 1;
        state[ZI_ITEMS + 7] = 2;
        state[STOCK + 12] = 3;
        assert_eq!(rack_weapons(&state, 0), [12]);
        strip_racks(&mut state, 0);
        assert!(rack_weapons(&state, 0).is_empty());
        assert_eq!(stock(&state, 12), 4);

        assert_eq!(develop(&rom, &mut state, 5, Some(0)), Some(0));
        assert_eq!(character_unit(&state, 1), None);
        let unit = unit_status(&state, 0).expect("developed");
        assert_eq!((unit.zoid, unit.hp), (5, (50, 50)));
        assert_eq!(half(&state, base + 2), IN_USE);
        assert_eq!((state[UNIT_COUNT], state[ZI_ITEMS + 7]), (1, 1));
    }

    #[test]
    fn the_core_kinds_count_those_held() {
        let mut state = state();
        assert_eq!(zi_item_kinds(&state), 0);
        state[ZI_ITEMS] = 3;
        state[ZI_ITEMS + 20] = 1;
        state[ZI_ITEMS + ZI_ITEM_KINDS - 1] = 99;
        state[ZI_ITEMS + ZI_ITEM_KINDS] = 9;
        assert_eq!(zi_item_kinds(&state), 3);
        assert_eq!(zi_item_kinds(&[]), 0);
    }
}
