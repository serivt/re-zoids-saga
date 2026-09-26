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
use super::units::{BattleUnit, Effect, Weapon, accuracy_excess, damage, hit_chance};
use extraction::saga_party::percent;

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
/// A part used up by its use (bit 31): its slot empties.
const USED_UP: u32 = 0x8000_0000;
/// The support parts' effects (`0x08033E40`).
const DEFENSES: u32 = 0xC000;
const BEAM_SHIELD: u32 = 0x4000;
const SPEED_UP: u32 = 0x2_0000;
const EVASION_UP: u32 = 0x1_0000;
const ACCURACY_UP: u32 = 0x4_0000;
const FULL_REPAIR: u32 = 0x40_0000;
const STATE: u32 = 0x10_0000;
const REPAIR: u32 = 0x8_0000;
/// Effects by the amount on the unit's own statistics, and on its pilot's
/// bonuses.
const OWN_AMOUNT: u8 = 2;
const PILOT_AMOUNT: u8 = 3;
/// What an effect raises (bit 0 raising): defense, beam defense, both,
/// speed, evasion, accuracy; the shields' bit `0x1000`; the state
/// `0x4000`; and what a sure weapon lowers.
const RAISE_DEFENSE: u16 = 0x201;
const RAISE_BEAM_DEFENSE: u16 = 0x401;
const RAISE_DEFENSES: u16 = 0x101;
const SHIELD: u16 = 0x1000;
const RAISE_SPEED: u16 = 0x801;
const RAISE_EVASION: u16 = 0x8001;
const RAISE_ACCURACY: u16 = 0x21;
const STATE_EFFECT: u16 = 0x4000;
/// What a weapon that hurts the pilot leaves on the target
/// (`0x08033E40`): its unit ignores its pilot for the weapon's turns.
const DAZED: u16 = 0x2000;
const SURE_EFFECT: u16 = 0x22;
/// What a full repair raises speed and defense by, in percent.
const RESTORE_PERCENT: i32 = 50;
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
    /// The damage in 16.16; for the other kinds, the amount the message
    /// gives (`+4`).
    pub damage: i32,
    /// What changed, for the message (`+2`): the effect's bits (see
    /// [`Effect::changes`]).
    pub code: u16,
    /// How the battle screen shows it (`+8`): [`DAMAGE`], [`RAISED`],
    /// [`LOWERED`], [`REPAIRED`], [`AFFLICTED`] or [`RESTORED`].
    pub kind: u8,
}

/// A blow's kinds (`+8`), which pick the return's display
/// (`0x0802BCA4`, the table at `0x0802BEB0`).
pub const DAMAGE: u8 = 0;
/// A statistic raised.
pub const RAISED: u8 = 1;
/// A statistic lowered by a weapon that always lands.
pub const LOWERED: u8 = 2;
/// Hit points repaired.
pub const REPAIRED: u8 = 3;
/// A state set on the unit.
pub const AFFLICTED: u8 = 4;
/// Hit points and energy restored in full, speed and defense raised.
pub const RESTORED: u8 = 5;

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
/// below `(bonus >> 1) + 2`, `bonus` being the accuracy past 100 of the
/// last chance to hit that went past it this turn, which each target's
/// updates. `terrain` is the attacker's side's ground, `chapter` the
/// game's.
#[must_use]
pub fn attack(
    sides: &mut Sides,
    (side, slot): (usize, usize),
    weapon: usize,
    targets: &[usize],
    rolls: &[u16; ROLLS],
    (bonus, terrain, chapter): (&mut u16, u8, u8),
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
        let (units, other) = ((&attacker, &stats), (&target, &target_stats));
        let chance = hit_chance(units, other, weapon, terrain);
        if let Some(excess) = accuracy_excess(units, other, weapon, terrain) {
            *bonus = excess;
        }
        let immune = target.traits & NO_CRITICAL != 0 && chapter >= CRITICAL_FROM_CHAPTER;
        blow.flags |= landing(
            chance,
            rolls[ROLLS - 1 - k.min(ROLLS - 1)],
            *bonus,
            (arms.flags & KEEN != 0, immune),
        );
        if arms.flags & OFFENSIVE == 0 {
            if let Some(unit) = sides[aimed][target_slot].as_mut() {
                support(unit, &arms, &mut blow);
            }
        } else if arms.flags & ALWAYS != 0 {
            blow.flags |= LANDED;
            blow.kind = LOWERED;
            blow.damage = arms.power;
            blow.code = SURE_EFFECT;
            if let Some(unit) = sides[aimed][target_slot].as_mut() {
                unit.affect(effect(&arms, SURE_EFFECT, arms.power, OWN_AMOUNT));
            }
        } else if blow.landed() {
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
                if let Some(unit) = sides[aimed][target_slot].as_mut() {
                    unit.affect(effect(&arms, DAZED, arms.power, PILOT_AMOUNT));
                }
            }
            if arms.flags & STUNS != 0 {
                blow.flags |= STUNNED;
            }
        }
        outcome.blows.push(blow);
    }
    if arms.flags & USED_UP != 0
        && let Some(unit) = sides[side][slot].as_mut()
    {
        unit.use_up(weapon);
    }
    outcome
}

/// What a weapon for its own side does to one of its targets
/// (`0x08033E40`): it always lands; shields and the parts that raise a
/// statistic put an effect on the unit for the weapon's turns, a repair
/// part gives back hit points, the full repair everything.
fn support(unit: &mut BattleUnit, arms: &Weapon, blow: &mut Blow) {
    let power = arms.power;
    let second = i32::from(arms.accuracy);
    let flags = arms.flags;
    let raise = |unit: &mut BattleUnit, code: u16, value: i32, kind: u8| {
        unit.affect(effect(arms, code, value, kind));
    };
    blow.flags |= LANDED;
    blow.kind = RAISED;
    if flags & DEFENSES != 0 {
        if flags & BEAM_SHIELD == 0 {
            raise(unit, RAISE_DEFENSE, power, OWN_AMOUNT);
            raise(unit, RAISE_BEAM_DEFENSE, power, OWN_AMOUNT);
        } else {
            raise(unit, SHIELD | RAISE_DEFENSE, power, OWN_AMOUNT);
            raise(unit, SHIELD | RAISE_BEAM_DEFENSE, second, OWN_AMOUNT);
        }
        (blow.code, blow.damage) = if second == 0 {
            (RAISE_DEFENSE, power)
        } else if power == 0 {
            (RAISE_BEAM_DEFENSE, second)
        } else {
            (RAISE_DEFENSES, power + second)
        };
    } else if flags & (SPEED_UP | EVASION_UP) != 0 {
        let code = if flags & SPEED_UP != 0 {
            RAISE_SPEED
        } else {
            RAISE_EVASION
        };
        raise(unit, code, power, OWN_AMOUNT);
        (blow.code, blow.damage) = (code, power);
    } else if flags & ACCURACY_UP != 0 {
        raise(unit, RAISE_ACCURACY, power, PILOT_AMOUNT);
        (blow.code, blow.damage) = (RAISE_ACCURACY, power);
    } else if flags & FULL_REPAIR != 0 {
        blow.kind = RESTORED;
        unit.sp = unit
            .sp
            .wrapping_add(low_half(percent(i32::from(unit.sp), RESTORE_PERCENT)));
        unit.df = unit
            .df
            .wrapping_add(low_half(percent(i32::from(unit.df), RESTORE_PERCENT)));
        unit.hp = unit.max_hp;
        unit.ep = unit.max_ep;
    } else if flags & STATE != 0 {
        blow.kind = AFFLICTED;
        raise(unit, STATE_EFFECT, power, OWN_AMOUNT);
    } else if flags & REPAIR != 0 {
        blow.kind = REPAIRED;
        let before = unit.hp;
        unit.hp = before.saturating_add(power).min(unit.max_hp.max(before));
        blow.damage = unit.hp - before;
    } else {
        blow.flags &= !LANDED;
        blow.kind = DAMAGE;
    }
}

/// Whether a blow lands and is critical: it lands when its roll is at
/// most the chance, or always at [`SURE`]; a roll that lands is critical
/// below `(bonus >> 1) + 2`, twice that and at least 10 for a keen weapon,
/// never on an immune target.
fn landing(chance: u16, roll: u16, bonus: u16, (keen, immune): (bool, bool)) -> u16 {
    if chance == SURE {
        return LANDED;
    }
    if chance == 0 || roll > chance {
        return 0;
    }
    let mut threshold = (bonus >> 1) + 2;
    if keen {
        threshold = (threshold * 2).max(KEEN_FLOOR);
    }
    if !immune && roll < threshold {
        LANDED | CRITICAL
    } else {
        LANDED
    }
}

/// The effect a weapon puts on a unit (`0x08032B54`): what it changes, by
/// how much, how, for the weapon's turns.
fn effect(arms: &Weapon, changes: u16, value: i32, kind: u8) -> Effect {
    Effect {
        changes,
        value: u16::from_ne_bytes(low_half(value).to_ne_bytes()),
        kind,
        turns: arms.turns,
        part: arms.part,
    }
}

/// The low half-word of `value`, as the game stores it.
fn low_half(value: i32) -> i16 {
    i16::from_ne_bytes([value.to_ne_bytes()[0], value.to_ne_bytes()[1]])
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
            character: 0,
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
            turns: 0,
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
        let outcome = attack(&mut sides, (0, 0), 0, &[2, 3], &rolls, (&mut 0, 0, 1));
        assert!(outcome.blows[0].landed() && !outcome.blows[0].critical());
        assert_eq!(outcome.blows[0].damage >> 16, 12);
        assert!(outcome.blows[1].critical());
        assert_eq!(outcome.blows[1].damage >> 16, 17);
        assert!(outcome.blows[1].destroyed());
        assert_eq!(outcome.experience, 7);
        assert_eq!(sides[0][0].as_ref().map(|unit| unit.ep), Some(7));
        assert_eq!(sides[1][2].as_ref().map(|unit| unit.hp), Some(8));
    }

    fn shield() -> Weapon {
        Weapon {
            part: 316,
            flags: 0x4004,
            accuracy: 20,
            power: 20,
            cost: 2,
            reach: 0,
            spread: 0,
            turns: 3,
        }
    }

    #[test]
    fn a_shield_always_raises_both_defenses_for_its_turns() {
        let mut sides: Sides = Default::default();
        sides[0][1] = Some(unit(50, Some(shield())));
        let rolls = [99; ROLLS];
        let outcome = attack(&mut sides, (0, 1), 0, &[1], &rolls, (&mut 0, 0, 1));
        let blow = outcome.blows[0];
        assert!(blow.landed());
        assert_eq!((blow.kind, blow.code, blow.damage), (RAISED, 0x101, 40));
        assert_eq!(sides[0][1].as_ref().map(|unit| unit.ep), Some(8));
        let effects: Vec<_> = sides[0][1]
            .iter()
            .flat_map(|unit| &unit.effects)
            .map(|effect| {
                (
                    effect.changes,
                    effect.value,
                    effect.kind,
                    effect.turns,
                    effect.part,
                )
            })
            .collect();
        assert_eq!(effects, [(0x1201, 20, 2, 3, 316), (0x1401, 20, 2, 3, 316)]);
    }

    #[test]
    fn a_repair_gives_back_what_the_unit_lacks_at_most() {
        let mut sides: Sides = Default::default();
        let repair = Weapon {
            flags: 0x8_0002,
            power: 50,
            ..shield()
        };
        let mut hurt = unit(100, Some(repair));
        hurt.hp = 80;
        sides[0][0] = Some(hurt);
        let outcome = attack(&mut sides, (0, 0), 0, &[0], &[0; ROLLS], (&mut 0, 0, 1));
        assert_eq!(
            (outcome.blows[0].kind, outcome.blows[0].damage),
            (REPAIRED, 20)
        );
        assert_eq!(sides[0][0].as_ref().map(|unit| unit.hp), Some(100));
    }

    #[test]
    fn a_used_up_part_leaves_its_slot() {
        let mut sides: Sides = Default::default();
        let once = Weapon {
            flags: 0x8040_0004,
            ..shield()
        };
        sides[0][0] = Some(unit(100, Some(once)));
        let outcome = attack(&mut sides, (0, 0), 0, &[0], &[0; ROLLS], (&mut 0, 0, 1));
        assert_eq!(outcome.blows[0].kind, RESTORED);
        assert_eq!(sides[0][0].as_ref().and_then(|unit| unit.weapons[0]), None);
    }

    #[test]
    fn a_roll_over_the_chance_misses() {
        let mut sides: Sides = Default::default();
        sides[0][0] = Some(unit(50, Some(gun())));
        sides[1][0] = Some(unit(20, None));
        let rolls = [95; ROLLS];
        let outcome = attack(&mut sides, (0, 0), 0, &[0], &rolls, (&mut 0, 0, 1));
        assert!(!outcome.blows[0].landed());
        assert_eq!(sides[1][0].as_ref().map(|unit| unit.hp), Some(20));
    }
}
