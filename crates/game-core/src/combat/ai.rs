//! Who acts when, and what the enemy does.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the turn
//! order (`0x08032410` with the sorts at `0x08032564`, `0x080325C8` and
//! `0x0803262C`), the table of what each weapon can reach (`0x08038B00`),
//! the enemies' first way of choosing (`0x0805959C`, with `0x08059528`,
//! `0x08059A98`, `0x08059C80`, `0x08059D10`, `0x08059D60` and the support
//! filters `0x0805AA3C`, `0x0805AC1C`, `0x0805AE2C`); checked against the
//! order and the choices of a battle on the world map.

use extraction::saga_combat::SLOTS;

use super::units::{BattleUnit, Derived, damage, distance, hit_chance};
use crate::rng::Rng;

/// Both sides' units by slot: the party's, then the enemy's.
pub type Sides = [[Option<BattleUnit>; SLOTS]; 2];

/// The party's side.
pub const PARTY: usize = 0;
/// The enemy's side.
pub const ENEMY: usize = 1;

/// How the turn is ordered (the battle flags at `0x0200EB86`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// The fastest first, ties in slot order, the party's before the
    /// enemy's.
    Fastest,
    /// The slowest first.
    Slowest,
    /// At random.
    Shuffled,
}

/// The units that act this turn, in order, as (side, slot).
pub fn turn_order(sides: &Sides, order: Order, rng: &mut Rng, frame: u16) -> Vec<(usize, usize)> {
    let mut entries: Vec<((usize, usize), i16)> = Vec::new();
    for (side, units) in sides.iter().enumerate() {
        for (slot, unit) in units.iter().enumerate() {
            if let Some(unit) = unit.as_ref().filter(|unit| unit.fighting()) {
                entries.push(((side, slot), unit.derived().speed));
            }
        }
    }
    match order {
        Order::Fastest => entries.sort_by_key(|entry| std::cmp::Reverse(entry.1)),
        Order::Slowest => entries.sort_by_key(|entry| entry.1),
        Order::Shuffled => {
            let count = u16::try_from(entries.len()).unwrap_or(u16::MAX);
            for index in 0..entries.len() {
                let other = usize::from(rng.next(frame) % count.max(1));
                entries.swap(index, other);
            }
        }
    }
    entries.into_iter().map(|(unit, _)| unit).collect()
}

/// What a unit does with its turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// Uses the weapon in slot `weapon` on the slots `targets` of side
    /// `side`.
    Weapon {
        /// The weapon's part slot.
        weapon: usize,
        /// The side it aims at.
        side: usize,
        /// The slots it takes.
        targets: Vec<usize>,
    },
    /// Defends.
    Defend,
}

/// What a weapon can do from where its unit stands (a record of
/// `0x02010E40`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The weapon's part slot.
    pub weapon: usize,
    /// The side it aims at.
    pub side: usize,
    /// Its groups of slots, each with the targets it reaches.
    pub groups: Vec<Vec<Target>>,
}

/// A slot a weapon reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    /// The slot.
    pub slot: usize,
    /// The chance to hit in percent.
    pub hit: u16,
    /// The damage in 16.16.
    pub damage: i32,
    /// Whether the damage would beat it.
    pub lethal: bool,
}

/// Energy-restoring parts (`0x400000`) and repairing parts (`0x80000`).
const RESTORES: u32 = 0x40_0000;
const REPAIRS: u32 = 0x8_0000;
const ALWAYS_REACHES: u32 = 0x800;

/// What each of `side`/`slot`'s weapons can do (`0x08038B00`): its groups
/// of slots on the side it aims at, less those out of the battle, each slot
/// within reach with the chance to hit, the damage and whether it beats
/// the target; a weapon the unit has no energy for does nothing.
#[must_use]
pub fn candidates(sides: &Sides, side: usize, slot: usize, terrain: u8) -> Vec<Candidate> {
    let Some(unit) = sides[side][slot].as_ref() else {
        return Vec::new();
    };
    let stats = unit.derived();
    let mut found = Vec::new();
    for (index, weapon) in unit.weapons.iter().enumerate() {
        let Some(weapon) = weapon else {
            continue;
        };
        if unit.ep < i32::from(weapon.cost) {
            continue;
        }
        let aims = if weapon.offensive() { 1 - side } else { side };
        let groups = weapon
            .groups(slot)
            .into_iter()
            .map(|group| {
                group
                    .into_iter()
                    .filter_map(|target_slot| {
                        let target = sides[aims][target_slot]
                            .as_ref()
                            .filter(|target| target.fighting())?;
                        reach(
                            unit,
                            &stats,
                            weapon.flags,
                            index,
                            slot,
                            (target_slot, target),
                            terrain,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|group| !group.is_empty())
            .collect::<Vec<_>>();
        if !groups.is_empty() {
            found.push(Candidate {
                weapon: index,
                side: aims,
                groups,
            });
        }
    }
    found
}

fn reach(
    unit: &BattleUnit,
    stats: &Derived,
    flags: u32,
    weapon: usize,
    slot: usize,
    (target_slot, target): (usize, &BattleUnit),
    terrain: u8,
) -> Option<Target> {
    let arms = unit.weapons[weapon]?;
    let plain = Target {
        slot: target_slot,
        hit: 0,
        damage: 0,
        lethal: false,
    };
    if !arms.offensive() {
        return Some(plain);
    }
    if !arms.reaches(distance(&arms, slot, target_slot)) {
        return None;
    }
    if flags & ALWAYS_REACHES != 0 {
        return Some(plain);
    }
    let target_stats = target.derived();
    let hit = hit_chance((unit, stats), (target, &target_stats), weapon, terrain);
    let damage = damage((unit, stats), (target, &target_stats), weapon, false);
    Some(Target {
        hit,
        damage,
        lethal: target.hp <= damage >> 16,
        ..plain
    })
}

/// The enemies' first way of choosing (`0x0805959C`): one time in two it
/// looks for a support part worth using on its own side; otherwise it
/// picks one of its weapons that reach the party at random, and one of the
/// weapon's groups at random. With nothing to use it takes a support part
/// after all, or defends.
pub fn choose(
    sides: &Sides,
    side: usize,
    slot: usize,
    terrain: u8,
    rng: &mut Rng,
    frame: u16,
) -> Choice {
    let all = candidates(sides, side, slot, terrain);
    let own: Vec<Candidate> = all.iter().filter(|c| c.side == side).cloned().collect();
    let other: Vec<Candidate> = all.iter().filter(|c| c.side != side).cloned().collect();
    if rng.next(frame) & 1 == 0 {
        let support = worth_supporting(sides, side, slot, &own);
        if !support.is_empty() {
            return pick(&support, rng, frame);
        }
    }
    if !other.is_empty() {
        return pick(&other, rng, frame);
    }
    if !own.is_empty() {
        return pick(&own, rng, frame);
    }
    Choice::Defend
}

/// The support parts worth using, in order of preference, each filter on
/// its own: restoring (`0x0805AC1C`) an ally that lacks four fifths of both
/// its hit points and its energy, repairing (`0x0805AA3C`) one that lacks
/// two thirds of its hit points, each on the one that lacks the most; then
/// other support (`0x0805AE2C`) on allies not all under that part's effect.
fn worth_supporting(sides: &Sides, side: usize, slot: usize, own: &[Candidate]) -> Vec<Candidate> {
    let Some(unit) = sides[side][slot].as_ref() else {
        return Vec::new();
    };
    let flags =
        |candidate: &Candidate| unit.weapons[candidate.weapon].map_or(0, |weapon| weapon.flags);
    let lacking = |target: &Target| {
        sides[side][target.slot]
            .as_ref()
            .map_or((0, 0, 0, 0), |ally| {
                (
                    ally.max_hp - ally.hp,
                    ally.max_ep - ally.ep,
                    ally.max_hp,
                    ally.max_ep,
                )
            })
    };
    let keep = |candidates: Vec<Candidate>, wanted: &dyn Fn(&Candidate, &[Target]) -> bool| {
        candidates
            .into_iter()
            .filter_map(|candidate| {
                let groups: Vec<Vec<Target>> = candidate
                    .groups
                    .iter()
                    .filter(|group| wanted(&candidate, group))
                    .cloned()
                    .collect();
                (!groups.is_empty()).then_some(Candidate {
                    groups,
                    ..candidate
                })
            })
            .collect::<Vec<_>>()
    };
    let restoring: Vec<Candidate> = own
        .iter()
        .filter(|c| flags(c) & RESTORES != 0)
        .cloned()
        .collect();
    let mut best = (0, 0);
    for target in restoring.iter().flat_map(|c| c.groups.iter().flatten()) {
        let (hp, energy, full_hp, full_energy) = lacking(target);
        if full_hp * 4 / 5 <= hp && full_energy * 4 / 5 <= energy && best.0 < hp && best.1 < energy
        {
            best = (hp, energy);
        }
    }
    let found = keep(restoring, &|_, group| {
        best != (0, 0)
            && group.iter().any(|target| {
                let (hp, ep, ..) = lacking(target);
                (hp, ep) == best
            })
    });
    if !found.is_empty() {
        return found;
    }
    let repairing: Vec<Candidate> = own
        .iter()
        .filter(|c| flags(c) & REPAIRS != 0)
        .cloned()
        .collect();
    let mut best = 0;
    for target in repairing.iter().flat_map(|c| c.groups.iter().flatten()) {
        let (hp, _, max_hp, _) = lacking(target);
        if max_hp * 2 / 3 <= hp && best < hp {
            best = hp;
        }
    }
    let found = keep(repairing, &|_, group| {
        best != 0 && group.iter().any(|target| lacking(target).0 == best)
    });
    if !found.is_empty() {
        return found;
    }
    let other: Vec<Candidate> = own
        .iter()
        .filter(|c| flags(c) & (RESTORES | REPAIRS) == 0)
        .cloned()
        .collect();
    keep(other, &|candidate, group| {
        let part = unit.weapons[candidate.weapon].map_or(0, |weapon| weapon.part);
        group.iter().any(|target| {
            sides[side][target.slot].as_ref().is_some_and(|ally| {
                !ally
                    .effects
                    .iter()
                    .any(|effect| effect.turns > 0 && effect.part == part)
            })
        })
    })
}

/// Picks one of `candidates` and one of its groups, each a draw scaled to
/// the count (`0x08059C80`).
fn pick(candidates: &[Candidate], rng: &mut Rng, frame: u16) -> Choice {
    let scaled = |draw: u16, count: usize| {
        let count = u32::try_from(count).unwrap_or(1);
        usize::try_from((u32::from(draw) * count) >> 16).unwrap_or(0)
    };
    let candidate =
        &candidates[scaled(rng.next(frame), candidates.len()).min(candidates.len() - 1)];
    let group = &candidate.groups
        [scaled(rng.next(frame), candidate.groups.len()).min(candidate.groups.len() - 1)];
    Choice::Weapon {
        weapon: candidate.weapon,
        side: candidate.side,
        targets: group.iter().map(|target| target.slot).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::units::Weapon;

    fn unit(speed: i16, weapon: Option<Weapon>) -> BattleUnit {
        BattleUnit {
            zoid: 0,
            traits: 0,
            status: 0,
            hp: 30,
            ep: 10,
            max_hp: 30,
            max_ep: 10,
            sp: speed,
            df: 0,
            beam_df: 0,
            evasion_bonus: 0,
            pilot: [0; 5],
            weapons: [weapon, None, None, None, None, None],
            ai: 0,
            experience: 0,
            money: 0,
            effects: Vec::new(),
        }
    }

    fn short_gun() -> Weapon {
        Weapon {
            part: 1,
            flags: 1,
            accuracy: 90,
            power: 0x10_0000,
            cost: 0,
            reach: 0,
            spread: 0,
        }
    }

    #[test]
    fn the_fastest_act_first_and_ties_keep_the_partys_order() {
        let mut sides: Sides = Default::default();
        sides[PARTY][0] = Some(unit(200, None));
        sides[PARTY][2] = Some(unit(200, None));
        sides[ENEMY][4] = Some(unit(650, None));
        let order = turn_order(&sides, Order::Fastest, &mut Rng::default(), 0);
        assert_eq!(order, vec![(ENEMY, 4), (PARTY, 0), (PARTY, 2)]);
    }

    #[test]
    fn a_short_weapon_from_the_back_row_reaches_nothing() {
        let mut sides: Sides = Default::default();
        sides[ENEMY][4] = Some(unit(100, Some(short_gun())));
        sides[ENEMY][1] = Some(unit(100, Some(short_gun())));
        sides[PARTY][0] = Some(unit(100, None));
        sides[PARTY][4] = Some(unit(100, None));
        assert!(candidates(&sides, ENEMY, 4, 0).is_empty());
        let front = candidates(&sides, ENEMY, 1, 0);
        assert_eq!(front.len(), 1);
        assert_eq!(
            front[0].groups,
            vec![vec![Target {
                slot: 0,
                hit: 89,
                damage: 0x10_0000,
                lethal: false
            }]]
        );
        assert_eq!(
            choose(&sides, ENEMY, 4, 0, &mut Rng::default(), 0),
            Choice::Defend
        );
        assert!(matches!(
            choose(&sides, ENEMY, 1, 0, &mut Rng::default(), 0),
            Choice::Weapon { targets, .. } if targets == vec![0]
        ));
    }
}
