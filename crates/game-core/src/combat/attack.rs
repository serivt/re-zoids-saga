//! An attack as it applies (`0x08033E40`): the energy it costs, whether
//! each target is hit, critically or not, the damage and the units it
//! beats.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! attack's routine (`0x08033E40`), the target list it compacts
//! (`0x08034448`), the turn's rolls (`0x08033D94`), the hit chance
//! (`0x08034500`) and the damage (`0x080345EC`); checked against the three
//! attacks of a battle on the world map in a reference emulator. See
//! `docs/combat.md`.

use super::ai::Sides;
use super::units::{damage, hit_chance};

/// Rolls a turn draws (`0x08033D94`): each actor's, `% 100`, at EWRAM
/// `0x020143BC`.
pub const ROLLS: usize = 16;
/// The unit has been beaten this turn (`U+8`); it leaves the battle once
/// the actor's turn ends.
pub const DESTROYED: u16 = 0x400;
/// The target can't be hit critically from chapter 6 on.
const NO_CRITICAL: u16 = 0x200;
const CRITICAL_FROM_CHAPTER: u8 = 6;
const SURE: u16 = 100;
const OFFENSIVE: u32 = 1;
/// A weapon twice as likely to hit critically, ten in a hundred at least.
const KEEN: u32 = 0x80;
const KEEN_FLOOR: u16 = 10;
/// A weapon that hurts the pilot too.
const HURTS_PILOT: u32 = 0x1000;
/// A weapon that shakes the target.
const STUNS: u32 = 0x2000;
const ALWAYS: u32 = 0x800;

/// What an attack did to one target (a record at `0x0200EB84 + 0x220C`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Blow {
    /// The target's side.
    pub side: usize,
    /// Its slot.
    pub slot: usize,
    /// The record's flags: [`LANDED`], [`CRITICAL`], [`STUNNED`],
    /// [`PILOT_HURT`] and [`BEATEN`].
    pub flags: u16,
    /// The damage in 16.16.
    pub damage: i32,
    /// How the battle screen shows it (`+8`): 0 for damage; the support
    /// parts' own kinds otherwise.
    pub kind: u8,
}

/// The attack landed.
pub const LANDED: u16 = 1;
/// A critical hit.
pub const CRITICAL: u16 = 2;
/// The target was shaken.
pub const STUNNED: u16 = 4;
/// The pilot was hurt too.
pub const PILOT_HURT: u16 = 8;
/// The target was beaten.
pub const BEATEN: u16 = 0x8000;

impl Blow {
    /// Whether it landed.
    #[must_use]
    pub fn landed(&self) -> bool {
        self.flags & LANDED != 0
    }

    /// Whether it was a critical hit.
    #[must_use]
    pub fn critical(&self) -> bool {
        self.flags & CRITICAL != 0
    }

    /// Whether it beat the target.
    #[must_use]
    pub fn destroyed(&self) -> bool {
        self.flags & BEATEN != 0
    }
}

/// An attack's outcome: each target's blow, and the experience and money
/// the units it beat are worth (`0x0200EB84 + 0x22A0`, `+0x22A4`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    /// The blows, in the targets' order.
    pub blows: Vec<Blow>,
    /// Experience won.
    pub experience: u32,
    /// Money won.
    pub money: u32,
}

/// The attack of `side`/`slot`'s weapon in slot `weapon` on the slots
/// `targets` of the side it aims at (`0x08033E40`). Target `k` hits when
/// roll `15 − k` is at most the chance, and critically when it is also
/// below `(bonus >> 1) + 2`. `terrain` is the attacker's side's ground,
/// `chapter` the game's.
#[must_use]
pub fn attack(
    sides: &mut Sides,
    (side, slot): (usize, usize),
    weapon: usize,
    targets: &[usize],
    rolls: &[u16; ROLLS],
    (bonus, terrain, chapter): (u16, u8, u8),
) -> Outcome {
    let mut outcome = Outcome::default();
    let Some(attacker) = sides[side][slot].clone() else {
        return outcome;
    };
    let Some(arms) = attacker.weapons.get(weapon).copied().flatten() else {
        return outcome;
    };
    if let Some(unit) = sides[side][slot].as_mut() {
        unit.ep = (unit.ep - i32::from(arms.cost)).max(0);
    }
    let attacker = sides[side][slot].clone().unwrap_or(attacker);
    let stats = attacker.derived();
    let aimed = if arms.flags & OFFENSIVE == 0 {
        side
    } else {
        1 - side
    };
    for (k, &target_slot) in targets.iter().enumerate() {
        let mut blow = Blow {
            side: aimed,
            slot: target_slot,
            ..Blow::default()
        };
        let Some(target) = sides[aimed][target_slot].clone() else {
            outcome.blows.push(blow);
            continue;
        };
        let target_stats = target.derived();
        let chance = hit_chance(
            (&attacker, &stats),
            (&target, &target_stats),
            weapon,
            terrain,
        );
        let roll = rolls[ROLLS - 1 - k.min(ROLLS - 1)];
        if chance == SURE {
            blow.flags |= LANDED;
        } else if chance != 0 && roll <= chance {
            blow.flags |= LANDED;
            let mut threshold = (bonus >> 1) + 2;
            if arms.flags & KEEN != 0 {
                threshold = (threshold * 2).max(KEEN_FLOOR);
            }
            let immune = target.traits & NO_CRITICAL != 0 && chapter >= CRITICAL_FROM_CHAPTER;
            if !immune && roll < threshold {
                blow.flags |= CRITICAL;
            }
        }
        if arms.flags & OFFENSIVE != 0 && arms.flags & ALWAYS == 0 && blow.landed() {
            blow.damage = damage(
                (&attacker, &stats),
                (&target, &target_stats),
                weapon,
                blow.critical(),
            );
            if let Some(unit) = sides[aimed][target_slot].as_mut() {
                unit.hp -= blow.damage >> 16;
                if unit.hp < 1 {
                    unit.hp = 0;
                    unit.ep = 0;
                    unit.traits |= DESTROYED;
                    blow.flags |= BEATEN;
                    outcome.experience = outcome.experience.wrapping_add(unit.experience);
                    outcome.money = outcome.money.wrapping_add(unit.money);
                }
            }
            if arms.flags & HURTS_PILOT != 0 {
                blow.flags |= PILOT_HURT;
            }
            if arms.flags & STUNS != 0 {
                blow.flags |= STUNNED;
            }
        }
        outcome.blows.push(blow);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::units::{BattleUnit, NO_PART, Weapon};

    fn unit(hp: i32, weapon: Option<Weapon>) -> BattleUnit {
        BattleUnit {
            zoid: 0,
            traits: 0,
            status: 0,
            hp,
            ep: 10,
            max_hp: hp,
            max_ep: 10,
            sp: 0,
            df: 0,
            beam_df: 0,
            evasion_bonus: 0,
            pilot: [0; 5],
            weapons: [weapon, None, None, None, None, None],
            ai: 0,
            experience: 7,
            money: 30,
            effects: Vec::new(),
            parts: [NO_PART; 6],
            size: 0,
            face: 0,
        }
    }

    fn gun() -> Weapon {
        Weapon {
            part: 1,
            flags: OFFENSIVE,
            accuracy: 80,
            power: 12 << 16,
            cost: 3,
            reach: 2,
            spread: 0,
        }
    }

    #[test]
    fn a_roll_under_the_chance_hits_and_a_very_low_one_is_critical() {
        let mut sides: Sides = Default::default();
        sides[0][0] = Some(unit(50, Some(gun())));
        sides[1][2] = Some(unit(20, None));
        sides[1][3] = Some(unit(15, None));
        let mut rolls = [99; ROLLS];
        rolls[15] = 40;
        rolls[14] = 1;
        let outcome = attack(&mut sides, (0, 0), 0, &[2, 3], &rolls, (0, 0, 1));
        assert!(outcome.blows[0].landed() && !outcome.blows[0].critical());
        assert_eq!(outcome.blows[0].damage >> 16, 12);
        assert!(outcome.blows[1].critical());
        assert_eq!(outcome.blows[1].damage >> 16, 17);
        assert!(outcome.blows[1].destroyed());
        assert_eq!(outcome.experience, 7);
        assert_eq!(sides[0][0].as_ref().map(|unit| unit.ep), Some(7));
        assert_eq!(sides[1][2].as_ref().map(|unit| unit.hp), Some(8));
    }

    #[test]
    fn a_roll_over_the_chance_misses() {
        let mut sides: Sides = Default::default();
        sides[0][0] = Some(unit(50, Some(gun())));
        sides[1][0] = Some(unit(20, None));
        let rolls = [95; ROLLS];
        let outcome = attack(&mut sides, (0, 0), 0, &[0], &rolls, (0, 0, 1));
        assert!(!outcome.blows[0].landed());
        assert_eq!(sides[1][0].as_ref().map(|unit| unit.hp), Some(20));
    }
}
