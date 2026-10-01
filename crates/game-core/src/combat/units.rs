//! The units of a battle: what each side fights with, and the rules an
//! attack follows.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! party's units (`0x0802B5D0`, `0x0802B6E0`), the enemy's (`0x0802B728`,
//! `0x0802B978`, `0x0802B9DC`, `0x08036EF4`), the weapons (`0x0802BA24`),
//! the passive parts (`0x0803376C`), the statistics of each turn
//! (`0x08032CA4`), the reach (`0x08038F10`, `0x08038F60`), the targets a
//! weapon can take (`0x08038B00`), the hit chance (`0x08034500`) and the
//! damage (`0x080345EC`); checked against the units RAM held in a battle
//! on the world map and the damage of its three attacks.

use extraction::saga_combat::{self, SLOTS};
use extraction::saga_encounter::Lineup;
use extraction::saga_party::{self, PILOT_VALUES, percent};

/// The character that stands for no pilot in the statistics routine.
const NO_CHARACTER: u8 = 0xFF;
const PASSIVE: u32 = 0x2000_0000;
const OFFENSIVE: u32 = 1;
const SELF_ONLY: u32 = 4;
/// A weapon aimed at its own side (bit 1), rather than at its user.
const OWN_SIDE: u32 = 2;
/// The kind of a weapon its user takes on itself.
const SELF_KIND: u8 = 0xF;
const BEAM: u32 = 0x30;
const ANTI_AIR: u32 = 0x200;
/// The part slots a story battle's mode 2 empties: the three racks.
const RACKS: usize = 3;
/// The trait a story battle's flagged enemies take (`0x0802B728`): no
/// critical hit lands on them.
const GUARDED: u16 = 0x200;
const PIERCING: u32 = 0x400;
const SURE_HIT: u32 = 0x4000_0800;
const BEAM_DEFENSE_PARTS: u32 = 0xC000;
const TRAIT_PARTS: u32 = 0x20_0000;
const TRAIT_BITS: u32 = 0x3E;
const STAT_PARTS: [(u32, Stat); 6] = [
    (0x4000, Stat::Defense),
    (0x8000, Stat::Defense),
    (0x1_0000, Stat::Speed),
    (0x2_0000, Stat::Speed),
    (0x80_0000, Stat::HitPoints),
    (0x100_0000, Stat::Energy),
];
const MAX_WORDS: (i32, i32) = (9999, 999);
const MAX_HALVES: (i16, i16) = (9999, 999);
/// The unit flies: harder to hit but for anti-air weapons.
const FLYING: u16 = 2;
/// The unit swims: harder to hit on water.
const SWIMMING: u16 = 4;
/// The unit is defending this turn: it takes half the damage.
const DEFENDING: u16 = 0x100;
const SURE_TO_HIT: u16 = 4;
const UNTOUCHABLE: u16 = 2;
const NO_PILOT: u16 = 0x10;
/// An effect that makes its unit ignore its pilot.
const NO_PILOT_EFFECT: u16 = 0x2000;
/// The unit has left the battle: beaten, or retreated.
pub const OUT: u16 = 0x2000;
/// The unit left the battle beaten: the write-back takes its Zoid for
/// broken (`0x080364DC`).
pub const WRECKED: u16 = 0x800;
const HARMLESS: u16 = 0x20;
const WATER: u8 = 6;
const TRAIT_EVASION: i16 = 20;
const MIN_HIT: u16 = 50;
const MAX_HIT: u16 = 99;
const SURE: u16 = 100;
const DEFENSE_CAP: i16 = 0x41;
const CRITICAL_BONUS: i32 = 50;
const PILOT_DURABILITY: usize = 0;
const PILOT_REACTION: usize = 1;
const PILOT_DEFENSE: usize = 2;
const PILOT_ATTACK: usize = 3;
const PILOT_ACCURACY: usize = 4;
// Offsets in a unit's record of the game-state block.
const UNIT_TRAITS: usize = 0;
const UNIT_ZOID: usize = 6;
const UNIT_HP: usize = 8;
const UNIT_EP: usize = 0xC;
const UNIT_PARTS: usize = 0x10;
const UNIT_STATS: usize = 0x28;
const UNIT_SIZE: usize = 0x35;
// Offsets in an enemy record.
const ENEMY_PARTS: usize = 4;
const ENEMY_PARTS_OVERRIDE: usize = 3;
const ENEMY_PILOT: usize = 0x10;
const ENEMY_AI: usize = 0x11;
const ENEMY_EXPERIENCE: usize = 0x14;
const ENEMY_MONEY: usize = 0x18;

#[derive(Debug, Clone, Copy)]
enum Stat {
    HitPoints,
    Energy,
    Speed,
    Defense,
}

/// A weapon a unit carries: the part in one of its six slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weapon {
    /// The part.
    pub part: u16,
    /// What it does: bit 0 aims at the other side, `0x30` beams, and so on.
    pub flags: u32,
    /// Accuracy before the pilot.
    pub accuracy: i16,
    /// Power in 16.16 before the pilot.
    pub power: i32,
    /// Energy it costs.
    pub cost: i16,
    /// How far it reaches: which distances between the rows it hits.
    pub reach: u8,
    /// How many it hits: one, a row, a column, a square or all.
    pub spread: u8,
    /// The turns the effects it causes last.
    pub turns: u8,
}

impl Weapon {
    pub(crate) fn from_part(rom: &[u8], id: u16) -> Option<Self> {
        let part = saga_party::part_record(rom, id)?;
        Some(Self {
            part: id,
            flags: part.flags,
            accuracy: part.accuracy,
            power: part.power,
            cost: part.cost,
            reach: part.range.0,
            spread: part.range.1,
            turns: part.turns,
        })
    }

    /// Whether it aims at the other side; otherwise at its own.
    #[must_use]
    pub fn offensive(&self) -> bool {
        self.flags & OFFENSIVE != 0
    }

    /// Its kind (`0x0802BAF8`, the weapon's `+0xF`), which names its reach
    /// in the aim's window and picks its shape on the grid: `0xF` for one
    /// its user takes on itself; for one aimed at its side, `0x10`, `0x11`
    /// or `0x12` by its spread 0, 2 or 4; otherwise, for a spread of 0 or 2,
    /// a code by its reach (5, 6, 0, 7, 8, 9 or 10, 11, 2, 12, 13, 14), and
    /// its spread itself for the rest.
    #[must_use]
    pub fn kind(&self) -> u8 {
        if self.flags & SELF_ONLY != 0 {
            return SELF_KIND;
        }
        let by_reach = |codes: [u8; 6]| {
            codes
                .get(usize::from(self.reach))
                .copied()
                .unwrap_or(self.spread)
        };
        match (self.flags & OWN_SIDE != 0, self.spread) {
            (true, 0) => 0x10,
            (true, 2) => 0x11,
            (true, 4) => 0x12,
            (false, 0) => by_reach([5, 6, 0, 7, 8, 9]),
            (false, 2) => by_reach([10, 11, 2, 12, 13, 14]),
            (_, spread) => spread,
        }
    }

    /// Whether it reaches a target `distance` rows away (`0x08038F60`).
    #[must_use]
    pub fn reaches(&self, distance: u8) -> bool {
        let (near, far) = match self.reach {
            0 => (1, 1),
            1 => (1, 2),
            2 => (1, 3),
            3 => (2, 2),
            4 => (2, 3),
            5 => (3, 3),
            _ => return false,
        };
        (near..=far).contains(&distance)
    }

    /// The groups of slots it can take at once (`0x08038B00`): one slot,
    /// a slot with the one behind it, a column of three, a square of four
    /// or the whole side; a weapon for its own user takes only its slot.
    #[must_use]
    pub fn groups(&self, own_slot: usize) -> Vec<Vec<usize>> {
        if self.flags & SELF_ONLY != 0 {
            return vec![vec![own_slot]];
        }
        match self.spread {
            0 => (0..SLOTS).map(|slot| vec![slot]).collect(),
            1 => (0..3).map(|slot| vec![slot, slot + 3]).collect(),
            2 => vec![vec![0, 1, 2], vec![3, 4, 5]],
            3 => vec![vec![0, 1, 3, 4], vec![1, 2, 4, 5]],
            4 => vec![(0..SLOTS).collect()],
            _ => Vec::new(),
        }
    }
}

/// How far apart two slots are for a weapon (`0x08038F10`): 1 between the
/// front rows (slots 0–2), 2 between a front and a back row, 3 between the
/// back rows; 0 for a weapon aimed at its own side.
#[must_use]
pub fn distance(weapon: &Weapon, from: usize, to: usize) -> u8 {
    if !weapon.offensive() || from >= SLOTS || to >= SLOTS {
        return 0;
    }
    match (from < 3, to < 3) {
        (true, true) => 1,
        (false, false) => 3,
        _ => 2,
    }
}

/// One of the 48 effects a unit can be under (`U+0x144`, eight bytes
/// each, set by `0x08032B54`): what it changes, by how much and for how
/// many more turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effect {
    /// What it changes (`0x300` defense, `0x500` beam defense, `0x800`
    /// speed, `0x8000` evasion, `0x10` power, `0x20` accuracy, and with
    /// kinds 1 and 3 the pilot's bonuses), bit 0 raising rather than
    /// lowering; `0x1000`, `0x2000`, `0x4000` set flags on the unit.
    pub changes: u16,
    /// How much.
    pub value: u16,
    /// 0 and 1 in percent, 2 and 3 by the amount; 0 and 2 change the
    /// unit's own statistics, 1 and 3 its pilot's bonuses.
    pub kind: u8,
    /// Turns left.
    pub turns: u8,
    /// The part that caused it.
    pub part: u16,
}

/// Effects a unit holds at most.
pub const EFFECTS: usize = 48;

/// A unit in battle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleUnit {
    /// Its Zoid.
    pub zoid: u16,
    /// The Zoid record's traits, with the battle's own bits
    /// (`0x100` defending).
    pub traits: u16,
    /// Its state in the battle.
    pub status: u16,
    /// Hit points left.
    pub hp: i32,
    /// Energy left.
    pub ep: i32,
    /// Full hit points.
    pub max_hp: i32,
    /// Full energy.
    pub max_ep: i32,
    /// SP: speed and evasion.
    pub sp: i16,
    /// DF: defense in percent.
    pub df: i16,
    /// Defense against beams in percent.
    pub beam_df: i16,
    /// Evasion added to the pilot's.
    pub evasion_bonus: i16,
    /// The pilot's bonuses in percent: 耐久, 反応, 防御, 攻撃, 命中.
    pub pilot: [i32; PILOT_VALUES],
    /// The weapons, by part slot.
    pub weapons: [Option<Weapon>; SLOTS],
    /// The enemy's way of choosing (its record's `+0x11`).
    pub ai: u8,
    /// Experience the party gains for beating it.
    pub experience: u32,
    /// Money the party gains for beating it.
    pub money: u32,
    /// The effects it is under.
    pub effects: Vec<Effect>,
    /// The parts in its six slots, [`NO_PART`] for none: what the attack
    /// scenes mount on its Zoid.
    pub parts: [u16; SLOTS],
    /// Its Zoid's size class: 0 S, 1 M, 2 L.
    pub size: u8,
    /// Its pilot to the attack scenes: the portrait and the lines
    /// ([`saga_party::pilot_face`]).
    pub face: u8,
    /// Its pilot (`U+0x42`, the pilot record's `+2`), which some of the
    /// enemies' ways of choosing aim at first.
    pub character: u8,
}

/// A part slot without a part.
pub const NO_PART: u16 = 0xFFFF;

/// A unit's statistics for a turn (`0x08032CA4`), its pilot's bonuses
/// applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Derived {
    /// Speed, which orders the turn, and evasion.
    pub speed: i16,
    /// Defense in percent.
    pub defense: i16,
    /// Defense against beams in percent.
    pub beam_defense: i16,
    /// Evasion added.
    pub evasion_bonus: i16,
    /// Each weapon's accuracy.
    pub accuracy: [i16; SLOTS],
    /// Each weapon's power in 16.16.
    pub power: [i32; SLOTS],
}

impl BattleUnit {
    /// Unit `unit` of the game-state block, piloted by `character`, as the
    /// battle takes it once its statistics are computed again: its full hit
    /// points with the pilot's durability, its energy, SP and DF without
    /// the pilot, whose other bonuses each turn adds (`0x0802B5D0`).
    /// `unarmed` takes the parts of its first three slots off before the
    /// passive ones apply, as a story battle's mode 2 does when the fight
    /// starts (`0x080337F0`, before `0x08033718`).
    #[must_use]
    pub fn party(rom: &[u8], state: &[u8], unit: u8, character: u8, unarmed: bool) -> Option<Self> {
        let record = saga_party::unit_record(state, unit)?;
        let mut unpiloted = state.to_vec();
        saga_party::refresh_stats(rom, &mut unpiloted, NO_CHARACTER, unit);
        let bare = saga_party::unit_record(&unpiloted, unit)?;
        let bare_short = |at: usize| i16::from_le_bytes([bare[at], bare[at + 1]]);
        let bare_word =
            |at: usize| i32::from_le_bytes([bare[at], bare[at + 1], bare[at + 2], bare[at + 3]]);
        let half = |at: usize| u16::from_le_bytes([record[at], record[at + 1]]);
        let word = |at: usize| {
            i32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]])
        };
        let parts: [u16; SLOTS] = std::array::from_fn(|slot| {
            if unarmed && slot < RACKS {
                NO_PART
            } else {
                half(UNIT_PARTS + slot * 4 + 2)
            }
        });
        let df = bare_short(UNIT_STATS + 10);
        let mut unit = Self {
            zoid: half(UNIT_ZOID),
            traits: half(UNIT_TRAITS),
            status: 0,
            hp: word(UNIT_HP),
            ep: word(UNIT_EP),
            max_hp: word(UNIT_STATS),
            max_ep: bare_word(UNIT_STATS + 4),
            sp: bare_short(UNIT_STATS + 8),
            df,
            beam_df: df >> 1,
            evasion_bonus: 0,
            pilot: saga_party::pilot(rom, state, character).unwrap_or_default(),
            weapons: [None; SLOTS],
            ai: 0,
            experience: 0,
            money: 0,
            effects: Vec::new(),
            parts,
            size: record[UNIT_SIZE],
            face: saga_party::pilot_face(rom, state, character).unwrap_or(character),
            character,
        };
        unit.fit(rom, parts);
        Some(unit)
    }

    /// The enemy of slot `slot` of `lineup`, if it has one.
    #[must_use]
    pub fn enemy(rom: &[u8], state: &[u8], lineup: &Lineup, slot: usize) -> Option<Self> {
        let record = lineup.enemy_record(rom, slot)?;
        let zoid = u16::from(record[0]);
        let zoid_record = saga_combat::zoid_battle_record(rom, zoid)?;
        let half = |at: usize| u16::from_le_bytes([record[at], record[at + 1]]);
        let word = |at: usize| {
            u32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]])
        };
        let mut slots = zoid_record.parts;
        for (index, slot) in slots.iter_mut().take(ENEMY_PARTS_OVERRIDE).enumerate() {
            if half(ENEMY_PARTS + index * 4 + 2) != NO_PART {
                *slot = word(ENEMY_PARTS + index * 4);
            }
        }
        let parts: [u16; SLOTS] = slots.map(|slot| u16::try_from(slot >> 16).unwrap_or(NO_PART));
        let character = record[ENEMY_PILOT];
        let pilot = saga_party::pilot(rom, state, character).unwrap_or_default();
        let (mut hp, mut ep, mut sp, mut df) = zoid_record.stats;
        for part in parts.iter().filter(|part| **part != NO_PART) {
            let Some(record) = saga_party::part_record(rom, *part) else {
                continue;
            };
            if record.flags & PASSIVE == 0 {
                continue;
            }
            let value = record.power;
            for (bit, stat) in STAT_PARTS {
                if record.flags & bit == 0 {
                    continue;
                }
                match stat {
                    Stat::HitPoints => hp = hp.wrapping_add(value),
                    Stat::Energy => ep = ep.wrapping_add(value),
                    Stat::Speed => sp = sp.wrapping_add(low_half(value)),
                    Stat::Defense => df = df.wrapping_add(low_half(value)),
                }
            }
        }
        hp = hp.wrapping_add(percent(hp, pilot[PILOT_DURABILITY]));
        let hp = hp.min(MAX_WORDS.0);
        let ep = ep.min(MAX_WORDS.1);
        let sp = sp.min(MAX_HALVES.0);
        let df = df.min(MAX_HALVES.1);
        let mut unit = Self {
            zoid,
            traits: if lineup.guarded(slot) {
                zoid_record.traits | GUARDED
            } else {
                zoid_record.traits
            },
            status: 0,
            hp,
            ep,
            max_hp: hp,
            max_ep: ep,
            sp,
            df,
            beam_df: df >> 1,
            evasion_bonus: 0,
            pilot,
            weapons: [None; SLOTS],
            ai: record[ENEMY_AI],
            experience: word(ENEMY_EXPERIENCE),
            money: word(ENEMY_MONEY),
            effects: Vec::new(),
            parts,
            size: zoid_record.size,
            face: saga_party::pilot_face(rom, state, character).unwrap_or(character),
            character,
        };
        unit.fit(rom, parts);
        Some(unit)
    }

    /// Takes the weapons from the part slots; a passive part instead adds
    /// its beam defense or its traits and leaves its slot empty
    /// (`0x0803376C`).
    fn fit(&mut self, rom: &[u8], parts: [u16; SLOTS]) {
        for (slot, part) in parts.into_iter().enumerate() {
            if part == NO_PART {
                continue;
            }
            let Some(weapon) = Weapon::from_part(rom, part) else {
                continue;
            };
            if weapon.flags & PASSIVE == 0 {
                self.weapons[slot] = Some(weapon);
                continue;
            }
            self.parts[slot] = NO_PART;
            if weapon.flags & BEAM_DEFENSE_PARTS != 0 {
                self.beam_df = self.beam_df.wrapping_add(weapon.accuracy);
            }
            if weapon.flags & TRAIT_PARTS != 0 {
                let bits = u32::from_ne_bytes(weapon.power.to_ne_bytes()) & TRAIT_BITS;
                self.traits |= u16::try_from(bits).unwrap_or(0);
            }
        }
    }

    /// Empties the slot of a part used up by its use (`0x08033E40`, bit 31
    /// of its flags).
    pub fn use_up(&mut self, slot: usize) {
        if let Some(weapon) = self.weapons.get_mut(slot) {
            *weapon = None;
        }
        if let Some(part) = self.parts.get_mut(slot) {
            *part = NO_PART;
        }
    }

    /// Adds an effect in the first free place, one whose turns have run
    /// out (`0x08032B54`); `false` when all 48 are taken.
    pub fn affect(&mut self, effect: Effect) -> bool {
        if let Some(free) = self.effects.iter_mut().find(|held| held.turns == 0) {
            *free = effect;
            return true;
        }
        if self.effects.len() >= EFFECTS {
            return false;
        }
        self.effects.push(effect);
        true
    }

    /// The statistics of a turn (`0x08032CA4`): the unit's own, changed by
    /// its effects (by amounts first, then in percent), then raised by its
    /// pilot's defense, reaction, attack and accuracy bonuses in percent,
    /// themselves changed by the effects on the pilot, unless an effect
    /// makes the unit ignore its pilot.
    #[must_use]
    pub fn derived(&self) -> Derived {
        let mut stats = Stats {
            speed: self.sp,
            defense: self.df,
            beam_defense: self.beam_df,
            evasion_bonus: self.evasion_bonus,
            accuracy: self
                .weapons
                .map(|weapon| weapon.map_or(0, |weapon| weapon.accuracy)),
            power: self
                .weapons
                .map(|weapon| weapon.map_or(0, |weapon| weapon.power)),
            pilot: self.pilot.map(low_half),
        };
        for pass in [EffectPass::Amounts, EffectPass::Percents] {
            for effect in self.effects.iter().filter(|effect| effect.turns > 0) {
                stats.apply(*effect, pass);
            }
        }
        let ignores_pilot = self
            .effects
            .iter()
            .any(|effect| effect.turns > 0 && effect.changes & NO_PILOT_EFFECT != 0)
            || self.status & NO_PILOT != 0;
        stats.clamp();
        let mut derived = Derived {
            speed: stats.speed,
            defense: stats.defense,
            beam_defense: stats.beam_defense,
            evasion_bonus: stats.evasion_bonus,
            accuracy: stats.accuracy,
            power: stats.power,
        };
        if ignores_pilot {
            return derived;
        }
        let raise = |value: i16, bonus: i16| {
            low_half(i32::from(value).wrapping_add(percent(i32::from(value), i32::from(bonus))))
        };
        let [_, reaction, defense, attack, accuracy] = stats.pilot;
        derived.defense = raise(derived.defense, defense);
        derived.beam_defense = raise(derived.beam_defense, defense);
        derived.speed = raise(derived.speed, reaction);
        for slot in 0..SLOTS {
            derived.power[slot] =
                derived.power[slot].wrapping_add(percent(derived.power[slot], i32::from(attack)));
            derived.accuracy[slot] = raise(derived.accuracy[slot], accuracy);
        }
        derived
    }

    /// Whether it still fights (`0x08032A88`): it has a Zoid and is not out
    /// of the battle.
    #[must_use]
    pub fn fighting(&self) -> bool {
        self.traits & OUT == 0
    }
}

/// The statistics an effect can change, during [`BattleUnit::derived`].
struct Stats {
    speed: i16,
    defense: i16,
    beam_defense: i16,
    evasion_bonus: i16,
    accuracy: [i16; SLOTS],
    power: [i32; SLOTS],
    pilot: [i16; PILOT_VALUES],
}

/// The two passes over the effects: amounts (kinds 2 and 3), then
/// percents (kinds 0 and 1).
#[derive(Clone, Copy, PartialEq, Eq)]
enum EffectPass {
    Amounts,
    Percents,
}

impl Stats {
    fn apply(&mut self, effect: Effect, pass: EffectPass) {
        let own = match (pass, effect.kind) {
            (EffectPass::Amounts, 2) | (EffectPass::Percents, 0) => true,
            (EffectPass::Amounts, 3) | (EffectPass::Percents, 1) => false,
            _ => return,
        };
        let raise = effect.changes & 1 != 0;
        let value = i32::from(effect.value);
        let change = |current: i32| -> i32 {
            let amount = if pass == EffectPass::Percents {
                percent(current, value)
            } else {
                value
            };
            if raise {
                current.wrapping_add(amount)
            } else {
                current.wrapping_sub(amount)
            }
        };
        let change_half = |current: i16| low_half(change(i32::from(current)));
        let bits = effect.changes;
        if own {
            if bits & 0x300 != 0 {
                self.defense = change_half(self.defense);
            }
            if bits & 0x500 != 0 {
                self.beam_defense = change_half(self.beam_defense);
            }
            if bits & 0x800 != 0 {
                self.speed = change_half(self.speed);
            }
            if bits & 0x8000 != 0 {
                self.evasion_bonus = change_half(self.evasion_bonus);
            }
            if bits & 0x10 != 0 {
                self.power = self.power.map(change);
            }
            if bits & 0x20 != 0 {
                self.accuracy = self.accuracy.map(change_half);
            }
        } else {
            if bits & 0x100 != 0 {
                self.pilot[PILOT_DEFENSE] = change_half(self.pilot[PILOT_DEFENSE]);
                if pass == EffectPass::Amounts {
                    self.beam_defense = change_half(self.beam_defense);
                }
            }
            if bits & 0x800 != 0 {
                self.pilot[PILOT_REACTION] = change_half(self.pilot[PILOT_REACTION]);
            }
            if bits & 0x10 != 0 {
                self.pilot[PILOT_ATTACK] = change_half(self.pilot[PILOT_ATTACK]);
            }
            if bits & 0x20 != 0 {
                self.pilot[PILOT_ACCURACY] = change_half(self.pilot[PILOT_ACCURACY]);
            }
        }
    }

    fn clamp(&mut self) {
        self.speed = self.speed.max(0);
        self.defense = self.defense.max(0);
        self.beam_defense = self.beam_defense.max(0);
        self.evasion_bonus = self.evasion_bonus.max(0);
        self.accuracy = self.accuracy.map(|value| value.max(0));
        self.power = self.power.map(|value| value.max(0));
        for bonus in &mut self.pilot[PILOT_REACTION..] {
            *bonus = (*bonus).max(0);
        }
    }
}

/// The chance in percent that `attacker`'s weapon in slot `weapon` hits
/// `target` (`0x08034500`): its accuracy less the target's evasion, a
/// hundredth of its speed, and more against a flying Zoid or a swimming one
/// on water; between 50 and 99 unless something makes it sure or
/// impossible. `terrain` is the ground the attacker's side stands on.
#[must_use]
pub fn hit_chance(
    attacker: (&BattleUnit, &Derived),
    target: (&BattleUnit, &Derived),
    weapon: usize,
    terrain: u8,
) -> u16 {
    let chance = raw_chance(attacker, target, weapon, terrain);
    let (attacker, _) = attacker;
    let (target, _) = target;
    let flags = attacker
        .weapons
        .get(weapon)
        .copied()
        .flatten()
        .map_or(0, |weapon| weapon.flags);
    let mut chance =
        u16::try_from(chance.clamp(i32::from(MIN_HIT), i32::from(MAX_HIT))).unwrap_or(MIN_HIT);
    if attacker.status & SURE_TO_HIT != 0 {
        chance = SURE;
    }
    if target.status & UNTOUCHABLE != 0 {
        chance = 0;
    }
    if flags & SURE_HIT != 0 {
        chance = SURE;
    }
    chance
}

/// The accuracy past 100 a chance to hit leaves (`0x08034500` keeps it at
/// `0x0200EB84 + 0x229C`, which the critical hits read), when it goes
/// past.
#[must_use]
pub fn accuracy_excess(
    attacker: (&BattleUnit, &Derived),
    target: (&BattleUnit, &Derived),
    weapon: usize,
    terrain: u8,
) -> Option<u16> {
    let chance = raw_chance(attacker, target, weapon, terrain);
    (chance > i32::from(MAX_HIT)).then(|| u16::try_from(chance - 100).unwrap_or(u16::MAX))
}

/// The weapon's accuracy less the target's evasion, before its bounds.
fn raw_chance(
    (attacker, stats): (&BattleUnit, &Derived),
    (target, target_stats): (&BattleUnit, &Derived),
    weapon: usize,
    terrain: u8,
) -> i32 {
    let flags = attacker
        .weapons
        .get(weapon)
        .copied()
        .flatten()
        .map_or(0, |weapon| weapon.flags);
    let mut evasion = low_half(percent(i32::from(target_stats.speed), 1));
    if target.traits & FLYING != 0 && flags & ANTI_AIR == 0 {
        evasion += TRAIT_EVASION;
    }
    if target.traits & SWIMMING != 0 && terrain == WATER {
        evasion += TRAIT_EVASION;
    }
    let accuracy = i32::from(stats.accuracy.get(weapon).copied().unwrap_or(0));
    accuracy - i32::from(evasion) - i32::from(target_stats.evasion_bonus)
}

/// The damage, in 16.16, of `attacker`'s weapon in slot `weapon` on
/// `target` (`0x080345EC`): its power less the target's defense in percent
/// (against beams for a beam weapon, at most 65); a critical hit adds half
/// the power and pierces the defense; a defending target takes half; at
/// least the smallest amount.
#[must_use]
pub fn damage(
    attacker: (&BattleUnit, &Derived),
    target: (&BattleUnit, &Derived),
    weapon: usize,
    critical: bool,
) -> i32 {
    let (attacker, stats) = attacker;
    let (target, target_stats) = target;
    let flags = attacker
        .weapons
        .get(weapon)
        .copied()
        .flatten()
        .map_or(0, |weapon| weapon.flags);
    let mut defense = if flags & BEAM != 0 {
        target_stats.beam_defense
    } else {
        target_stats.defense
    }
    .min(DEFENSE_CAP);
    if critical || flags & PIERCING != 0 {
        defense = 0;
    }
    let mut power = stats.power.get(weapon).copied().unwrap_or(0);
    if critical {
        power = power.wrapping_add(percent(power, CRITICAL_BONUS));
    }
    let mut damage = power.wrapping_sub(percent(power, i32::from(defense)));
    if target.traits & DEFENDING != 0 {
        damage >>= 1;
    }
    if damage == 0 {
        damage = 1;
    }
    if target.status & HARMLESS != 0 {
        damage = 0;
    }
    damage
}

fn low_half(value: i32) -> i16 {
    let [low, high, ..] = value.to_le_bytes();
    i16::from_le_bytes([low, high])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(speed: i16, defense: i16, weapon: Option<Weapon>) -> BattleUnit {
        BattleUnit {
            zoid: 0,
            traits: 0,
            status: 0,
            hp: 100,
            ep: 20,
            max_hp: 100,
            max_ep: 20,
            sp: speed,
            df: defense,
            beam_df: defense >> 1,
            evasion_bonus: 0,
            pilot: [0; PILOT_VALUES],
            weapons: [weapon, None, None, None, None, None],
            ai: 0,
            experience: 0,
            money: 0,
            effects: Vec::new(),
            parts: [0xFFFF; 6],
            size: 0,
            face: 0,
            character: 0,
        }
    }

    fn cannon(accuracy: i16, power: i32) -> Weapon {
        Weapon {
            part: 0,
            flags: OFFENSIVE,
            accuracy,
            power,
            cost: 0,
            reach: 2,
            spread: 0,
            turns: 0,
        }
    }

    #[test]
    fn a_weapons_kind_names_its_reach() {
        let weapon = |flags: u32, reach: u8, spread: u8| Weapon {
            part: 1,
            flags,
            accuracy: 0,
            power: 0,
            cost: 0,
            reach,
            spread,
            turns: 0,
        };
        assert_eq!(weapon(0x4004, 0, 0).kind(), 0xF);
        assert_eq!(weapon(0x21, 2, 1).kind(), 1);
        assert_eq!(weapon(0x121, 0, 0).kind(), 5);
        assert_eq!(weapon(0x11, 2, 2).kind(), 2);
        assert_eq!(weapon(0x8_0002, 2, 4).kind(), 0x12);
    }

    #[test]
    fn the_traced_attacks_deal_the_originals_damage() {
        // The Shield Liger's beam cannon (accuracy 110, power 25) on the
        // Gator (beam defense 8) and on the Iguan (2), as a battle on the
        // world map took 23 and 24 hit points.
        let mut beam = cannon(110, 0x19_0000);
        beam.flags = OFFENSIVE | 0x20;
        let liger = unit(250, 10, Some(beam));
        let mut gator = unit(200, 10, None);
        gator.beam_df = 8;
        let iguan = unit(650, 5, None);
        let (liger_stats, gator_stats) = (liger.derived(), gator.derived());
        let iguan_stats = iguan.derived();
        let hit = damage((&liger, &liger_stats), (&gator, &gator_stats), 0, false);
        assert_eq!(hit >> 16, 23);
        let hit = damage((&liger, &liger_stats), (&iguan, &iguan_stats), 0, false);
        assert_eq!(hit >> 16, 24);
        assert_eq!(
            hit_chance((&liger, &liger_stats), (&gator, &gator_stats), 0, 4),
            99
        );
    }

    #[test]
    fn pilots_raise_accuracy_power_speed_and_defense() {
        let mut wolf = unit(210, 5, Some(cannon(95, 0x14_0000)));
        wolf.pilot = [1, 1, 1, 2, 5];
        let stats = wolf.derived();
        assert_eq!(
            (stats.speed, stats.defense, stats.beam_defense),
            (212, 5, 2)
        );
        assert_eq!((stats.accuracy[0], stats.power[0]), (100, 1_336_934));
    }

    #[test]
    fn hits_stay_between_50_and_99_and_critical_hits_pierce() {
        let weak = unit(0, 60, Some(cannon(10, 0x10_0000)));
        let stats = weak.derived();
        assert_eq!(hit_chance((&weak, &stats), (&weak, &stats), 0, 0), MIN_HIT);
        let normal = damage((&weak, &stats), (&weak, &stats), 0, false);
        let critical = damage((&weak, &stats), (&weak, &stats), 0, true);
        assert!(critical > normal);
        assert_eq!(critical >> 16, 23);
    }

    #[test]
    fn weapons_reach_rows_and_take_groups() {
        let mut weapon = cannon(90, 0);
        weapon.reach = 1;
        assert_eq!(distance(&weapon, 1, 4), 2);
        assert!(weapon.reaches(2) && !weapon.reaches(3));
        weapon.spread = 3;
        assert_eq!(weapon.groups(0), vec![vec![0, 1, 3, 4], vec![1, 2, 4, 5]]);
    }
}
