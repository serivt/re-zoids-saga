//! Who acts when, and what the enemy does.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the turn
//! order (`0x08032410` with the sorts at `0x08032564`, `0x080325C8` and
//! `0x0803262C`), the table of what each weapon can reach (`0x08038B00`),
//! the enemies' seventeen ways of choosing (the table at `0x0875C048`,
//! `0x0805959C` to `0x080599FC`, with `0x08059528`, `0x08059A98`,
//! `0x08059C80`, `0x08059D10`, `0x08059D60`, the filters `0x08059DB0`,
//! `0x08059E78`, `0x0805A284`, `0x0805AF68`, `0x0805B068`, `0x0805B0BC` and
//! the support filters `0x0805AA3C`, `0x0805AC1C`, `0x0805AE2C`); the first
//! way checked against the order and the choices of a battle on the world
//! map.

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
/// The bit of the chance to hit the original's estimate takes for the
/// damage routine's critical flag (see [`reach`]).
const ESTIMATE_CRITICAL: u16 = 2;

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

/// What a weapon does to one target: its chance to hit and the damage the
/// choice weighs. `0x08038B00` calls the damage routine without its flags,
/// which then are the chance to hit it has just computed: the damage is a
/// critical hit's when the chance has bit 1 (99 and 78 do, 77 does not).
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
    let critical = hit & ESTIMATE_CRITICAL != 0;
    let damage = damage((unit, stats), (target, &target_stats), weapon, critical);
    Some(Target {
        hit,
        damage,
        lethal: target.hp <= damage >> 16,
        ..plain
    })
}

/// How an enemy chooses its action (`0x0875C048[U+0xCD]`, 17 ways, each a
/// chain of filters on the list of what its weapons can do): the list is
/// narrowed step by step, the next step taken only when a step leaves
/// nothing, and the last list picks the weapon and the group at random
/// (`0x08059C80`). A way that tries support first (`0x08059528`) does so one
/// time in two.
pub fn choose(
    sides: &Sides,
    side: usize,
    slot: usize,
    terrain: u8,
    rng: &mut Rng,
    frame: u16,
) -> Choice {
    let all = candidates(sides, side, slot, terrain);
    let Some(unit) = sides[side][slot].as_ref() else {
        return Choice::Defend;
    };
    let ways = Ways {
        sides,
        side,
        unit,
        all: &all,
    };
    let list = match unit.ai {
        1 => ways.support(rng, frame).unwrap_or_else(|| {
            let lethal = lethal(ways.offensive());
            if lethal.is_empty() {
                most_targets(ways.offensive())
            } else {
                or_else(flagged(lethal, unit, MELEE), || {
                    most_targets(ways.offensive())
                })
            }
        }),
        2 => or_else(ways.repair(ways.own()), || {
            or_else(ways.buff(ways.own()), || ways.offensive())
        }),
        3 => or_else(ways.buff(ways.own()), || {
            or_else(flagged(ways.offensive(), unit, SURE), || ways.offensive())
        }),
        5 | 10 | 13 => ways.support(rng, frame).unwrap_or_else(|| {
            or_else(lethal(ways.offensive()), || most_targets(ways.offensive()))
        }),
        6 | 7 | 11 => {
            let character = match unit.ai {
                6 => 0x13,
                7 => 0x12,
                _ => 0x1C,
            };
            if ways.own().is_empty() {
                ways.offensive()
            } else {
                or_else(ways.repair(ways.piloted(ways.own(), character)), || {
                    or_else(ways.buff(ways.piloted(ways.own(), character)), || {
                        ways.offensive()
                    })
                })
            }
        }
        8 => ways.support(rng, frame).unwrap_or_else(|| {
            or_else(carrying(ways.offensive(), unit, MISSILE_POD), || {
                strongest(ways.offensive())
            })
        }),
        9 => or_else(ways.buff(ways.own()), || {
            or_else(lethal(ways.offensive()), || ways.offensive())
        }),
        12 => ways
            .support(rng, frame)
            .unwrap_or_else(|| most_targets(ways.offensive())),
        14 => ways.support(rng, frame).unwrap_or_else(|| {
            or_else(flagged(ways.offensive(), unit, PIERCING), || {
                ways.offensive()
            })
        }),
        _ => ways.support(rng, frame).unwrap_or_else(|| ways.offensive()),
    };
    if !list.is_empty() {
        return pick(&list, rng, frame);
    }
    let own = ways.own();
    if own.is_empty() {
        Choice::Defend
    } else {
        pick(&own, rng, frame)
    }
}

/// Weapon flags some ways look for: `0x100`, a fighting (格闘) weapon, as
/// the deck commands' descriptions name it; one that always lands
/// (`0x800`), a piercing one (`0x400`).
const MELEE: u32 = 0x100;
const SURE: u32 = 0x800;
const PIERCING: u32 = 0x400;
/// The part way 8 looks for first (`0x0805B0BC` with `0x224`).
const MISSILE_POD: u16 = 0x224;
/// Parts that restore or repair, which the other support filter leaves out
/// (`0x0805AE2C`).
const HEALS: u32 = RESTORES | REPAIRS;

/// `list`, or what `next` gives when it is empty.
fn or_else(list: Vec<Candidate>, next: impl FnOnce() -> Vec<Candidate>) -> Vec<Candidate> {
    if list.is_empty() { next() } else { list }
}

/// Keeps the groups that `keep` accepts, and the candidates left with any
/// (`0x08059BA0` removes the rest).
fn keep_groups(
    list: Vec<Candidate>,
    keep: impl Fn(&Candidate, &[Target]) -> bool,
) -> Vec<Candidate> {
    list.into_iter()
        .filter_map(|candidate| {
            let groups: Vec<Vec<Target>> = candidate
                .groups
                .iter()
                .filter(|group| keep(&candidate, group))
                .cloned()
                .collect();
            (!groups.is_empty()).then_some(Candidate {
                groups,
                ..candidate
            })
        })
        .collect()
}

/// The groups with a target the weapon would beat (`0x08059DB0`).
fn lethal(list: Vec<Candidate>) -> Vec<Candidate> {
    keep_groups(list, |_, group| group.iter().any(|target| target.lethal))
}

/// The groups with a target taking the most damage any takes
/// (`0x08059E78`).
fn strongest(list: Vec<Candidate>) -> Vec<Candidate> {
    let most = list
        .iter()
        .flat_map(|candidate| candidate.groups.iter().flatten())
        .map(|target| target.damage)
        .fold(0, i32::max);
    keep_groups(list, |_, group| {
        group.iter().any(|target| target.damage == most)
    })
}

/// The groups with as many targets as any (`0x0805A284`).
fn most_targets(list: Vec<Candidate>) -> Vec<Candidate> {
    let most = list
        .iter()
        .flat_map(|candidate| candidate.groups.iter())
        .map(Vec::len)
        .max()
        .unwrap_or(0);
    keep_groups(list, |_, group| group.len() == most)
}

/// The weapons with all the flags of `mask` (`0x0805B068`).
fn flagged(list: Vec<Candidate>, unit: &BattleUnit, mask: u32) -> Vec<Candidate> {
    list.into_iter()
        .filter(|candidate| {
            unit.weapons[candidate.weapon].is_some_and(|weapon| weapon.flags & mask == mask)
        })
        .collect()
}

/// The weapons of part `part` (`0x0805B0BC`).
fn carrying(list: Vec<Candidate>, unit: &BattleUnit, part: u16) -> Vec<Candidate> {
    list.into_iter()
        .filter(|candidate| unit.parts.get(candidate.weapon) == Some(&part))
        .collect()
}

/// What the ways of choosing work on: the sides, the chooser and the list
/// of what its weapons can do.
struct Ways<'a> {
    sides: &'a Sides,
    side: usize,
    unit: &'a BattleUnit,
    all: &'a [Candidate],
}

impl Ways<'_> {
    /// The weapons aimed at the other side (`0x08059A98`, `0x08059D10`).
    fn offensive(&self) -> Vec<Candidate> {
        self.all
            .iter()
            .filter(|candidate| candidate.side != self.side)
            .cloned()
            .collect()
    }

    /// The weapons for its own side (`0x08059A98`, `0x08059D60`).
    fn own(&self) -> Vec<Candidate> {
        self.all
            .iter()
            .filter(|candidate| candidate.side == self.side)
            .cloned()
            .collect()
    }

    fn flags(&self, candidate: &Candidate) -> u32 {
        self.unit.weapons[candidate.weapon].map_or(0, |weapon| weapon.flags)
    }

    /// What a target on the own side lacks: hit points, energy, and their
    /// full amounts.
    fn lacking(&self, target: &Target) -> (i32, i32, i32, i32) {
        self.sides[self.side][target.slot]
            .as_ref()
            .map_or((0, 0, 0, 0), |ally| {
                (
                    ally.max_hp - ally.hp,
                    ally.max_ep - ally.ep,
                    ally.max_hp,
                    ally.max_ep,
                )
            })
    }

    /// Support first (`0x08059528`), one time in two: a restoring part for
    /// an ally badly hurt, else a repair, else other support.
    fn support(&self, rng: &mut Rng, frame: u16) -> Option<Vec<Candidate>> {
        if rng.next(frame) & 1 != 0 {
            return None;
        }
        let found = or_else(self.restore(self.own()), || {
            or_else(self.repair(self.own()), || self.buff(self.own()))
        });
        (!found.is_empty()).then_some(found)
    }

    /// Restoring (`0x0805AC1C`): the restoring parts' groups with the ally
    /// that lacks the most, of those that lack four fifths of both their
    /// hit points and their energy.
    fn restore(&self, list: Vec<Candidate>) -> Vec<Candidate> {
        let mut best = (0, 0);
        for candidate in list.iter().filter(|c| self.flags(c) & RESTORES != 0) {
            for target in candidate.groups.iter().flatten() {
                let (hp, energy, full_hp, full_energy) = self.lacking(target);
                if full_hp * 4 / 5 <= hp
                    && full_energy * 4 / 5 <= energy
                    && best.0 < hp
                    && best.1 < energy
                {
                    best = (hp, energy);
                }
            }
        }
        keep_groups(list, |candidate, group| {
            self.flags(candidate) & RESTORES != 0
                && best != (0, 0)
                && group.iter().any(|target| {
                    let (hp, energy, ..) = self.lacking(target);
                    (hp, energy) == best
                })
        })
    }

    /// Repairing (`0x0805AA3C`): the repairing parts' groups with the ally
    /// that lacks the most hit points, of those that lack two thirds.
    fn repair(&self, list: Vec<Candidate>) -> Vec<Candidate> {
        let mut best = 0;
        for candidate in list.iter().filter(|c| self.flags(c) & REPAIRS != 0) {
            for target in candidate.groups.iter().flatten() {
                let (hp, _, full_hp, _) = self.lacking(target);
                if full_hp * 2 / 3 <= hp && best < hp {
                    best = hp;
                }
            }
        }
        keep_groups(list, |candidate, group| {
            self.flags(candidate) & REPAIRS != 0
                && best != 0
                && group.iter().any(|target| self.lacking(target).0 == best)
        })
    }

    /// Other support (`0x0805AE2C`): the parts that neither restore nor
    /// repair, on groups with an ally not under that part's effect.
    fn buff(&self, list: Vec<Candidate>) -> Vec<Candidate> {
        let list: Vec<Candidate> = list
            .into_iter()
            .filter(|candidate| self.flags(candidate) & HEALS == 0)
            .collect();
        keep_groups(list, |candidate, group| {
            let part = self.unit.weapons[candidate.weapon].map_or(0, |weapon| weapon.part);
            group.iter().any(|target| {
                self.sides[self.side][target.slot]
                    .as_ref()
                    .is_some_and(|ally| {
                        !ally
                            .effects
                            .iter()
                            .any(|effect| effect.turns > 0 && effect.part == part)
                    })
            })
        })
    }

    /// The groups with a unit that `character` pilots (`0x0805AF68`).
    fn piloted(&self, list: Vec<Candidate>, character: u8) -> Vec<Candidate> {
        keep_groups(list, |candidate, group| {
            group.iter().any(|target| {
                self.sides[candidate.side][target.slot]
                    .as_ref()
                    .is_some_and(|unit| unit.character == character)
            })
        })
    }
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
            parts: [0xFFFF; 6],
            size: 0,
            face: 0,
            character: 0,
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
            turns: 0,
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

    /// Two party units in the front row, the first nearly beaten; the
    /// enemy in front of them has a gun for one and a spread for two.
    fn standoff(ai: u8) -> Sides {
        let mut sides: Sides = Default::default();
        let mut weak = unit(100, None);
        weak.hp = 5;
        sides[PARTY][0] = Some(weak);
        sides[PARTY][1] = Some(unit(100, None));
        let spread = Weapon {
            part: 2,
            spread: 2,
            ..short_gun()
        };
        let mut enemy = unit(100, Some(short_gun()));
        enemy.weapons[1] = Some(spread);
        enemy.ai = ai;
        sides[ENEMY][1] = Some(enemy);
        sides
    }

    fn chosen(sides: &Sides) -> Choice {
        choose(sides, ENEMY, 1, 0, &mut Rng::default(), 0)
    }

    #[test]
    fn the_twelfth_way_takes_the_most_targets() {
        assert!(matches!(
            chosen(&standoff(12)),
            Choice::Weapon { weapon: 1, targets, .. } if targets == vec![0, 1]
        ));
    }

    #[test]
    fn the_fifth_way_goes_for_a_unit_it_can_beat() {
        let choice = chosen(&standoff(5));
        assert!(matches!(choice, Choice::Weapon { targets, .. } if targets.contains(&0)));
    }

    #[test]
    fn the_seventh_way_first_repairs_its_pilot_when_badly_hurt() {
        let mut sides = standoff(7);
        let repair = Weapon {
            part: 3,
            flags: 0x8_0002,
            power: 30,
            ..short_gun()
        };
        let mut friend = unit(100, None);
        friend.hp = 5;
        friend.character = 0x12;
        sides[ENEMY][0] = Some(friend);
        if let Some(enemy) = sides[ENEMY][1].as_mut() {
            enemy.weapons[2] = Some(repair);
        }
        assert!(matches!(
            chosen(&sides),
            Choice::Weapon { weapon: 2, side: ENEMY, targets } if targets == vec![0]
        ));
    }
}
