//! What a weapon reaches on the battle's grid, a port feature for the
//! enhanced mode's menus: the cells of both sides its user and its targets
//! take, from the front row or the back, as the aim lays them out.
//!
//! Source of knowledge: the aim's own reading of Zoids Saga (Japan, Rev 1)
//! (see [`super::aim`]): the weapon's kind by its reach and spread
//! (`0x0802BAF8`), its shape from the user's row (`0x0804593C`) and the
//! groups each shape takes (ROM `0x6D43BC`); the picture of it is this
//! project's own design.

use super::aim::{self, ORDER, group_cells};
use super::units::Weapon;

/// Cells a side has: 0–2 the front row, 3–5 the back row, each from the
/// bottom up.
pub const CELLS: usize = 6;
/// What a weapon's user and its targets take: bit 0 aims at the other
/// side, bit 1 at the user's own, bit 2 at the user alone.
const AIMED: u32 = 7;
const OTHER_SIDE: u32 = 1;
const USER_ALONE: u32 = 4;
const OWN_SIDE_KIND: u8 = 0x12;
const WHOLE_SIDE: u8 = 13;
const TOO_CLOSE: u8 = 0xFE;
const TOO_FAR: u8 = 0xFF;

/// A cell of the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mark {
    /// Nothing the weapon does.
    #[default]
    Empty,
    /// The weapon's user.
    User,
    /// A cell the weapon can take.
    Reached,
    /// A cell of the weapon's first group of targets, as one shot takes.
    Shot,
}

/// The picture of what a weapon reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Reach {
    /// The user's side.
    pub own: [Mark; CELLS],
    /// The other side.
    pub enemy: [Mark; CELLS],
}

/// What part `part` reaches when its user stands on cell `user` (0–5, the
/// back row from 3): `None` for a part that is no weapon, as the support
/// parts that only raise their user's values. A weapon that reaches
/// nothing from there leaves the other side empty.
#[must_use]
pub fn reach(rom: &[u8], part: u16, user: usize) -> Option<Reach> {
    let weapon = Weapon::from_part(rom, part)?;
    if weapon.flags & AIMED == 0 || user >= CELLS {
        return None;
    }
    let mut picture = Reach::default();
    picture.own[user] = Mark::User;
    let back_row = user >= CELLS / 2;
    let kind = weapon.kind();
    let shape = match kind {
        OWN_SIDE_KIND => WHOLE_SIDE,
        kind => aim::shape(kind, back_row),
    };
    let groups: Vec<u8> = match shape {
        TOO_CLOSE | TOO_FAR => Vec::new(),
        WHOLE_SIDE | 4 => vec![WHOLE_SIDE],
        shape => ORDER
            .get(usize::from(shape))
            .map(|order| order.to_vec())
            .unwrap_or_default(),
    };
    let side = if weapon.flags & OTHER_SIDE != 0 {
        &mut picture.enemy
    } else {
        &mut picture.own
    };
    if weapon.flags & USER_ALONE != 0 {
        side[user] = Mark::Shot;
        return Some(picture);
    }
    for &group in &groups {
        for cell in group_cells(group) {
            if side[cell] == Mark::Empty {
                side[cell] = Mark::Reached;
            }
        }
    }
    if let Some(&first) = groups.first() {
        for cell in group_cells(first) {
            side[cell] = Mark::Shot;
        }
    }
    Some(picture)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    const FRONT: usize = 1;
    const BACK: usize = 4;

    /// A ROM with one part record, id 0, at the parts' table.
    fn rom(flags: u32, reach: u8, spread: u8) -> Vec<u8> {
        let at = 0x0066_C8F8;
        let mut rom = vec![0; at + 24];
        rom[at..at + 4].copy_from_slice(&flags.to_le_bytes());
        rom[at + 0x12] = reach;
        rom[at + 0x13] = spread;
        rom
    }

    fn marks(side: [Mark; CELLS], mark: Mark) -> Vec<usize> {
        (0..CELLS).filter(|&cell| side[cell] == mark).collect()
    }

    #[test]
    fn a_melee_weapon_reaches_the_front_row_from_the_front_and_nothing_from_the_back() {
        let rom = rom(1, 0, 0);
        let front = reach(&rom, 0, FRONT).expect("a weapon");
        assert_eq!(marks(front.own, Mark::User), [FRONT]);
        assert_eq!(marks(front.enemy, Mark::Reached), [0, 1]);
        assert_eq!(marks(front.enemy, Mark::Shot), [2]);
        let back = reach(&rom, 0, BACK).expect("a weapon");
        assert_eq!(back.enemy, [Mark::Empty; CELLS]);
    }

    #[test]
    fn a_long_gun_takes_either_row_and_a_column_takes_three() {
        let singles = reach(&rom(1, 2, 0), 0, BACK).expect("a weapon");
        assert_eq!(marks(singles.enemy, Mark::Reached), [0, 1, 3, 4, 5]);
        let column = reach(&rom(1, 1, 2), 0, FRONT).expect("a weapon");
        assert_eq!(marks(column.enemy, Mark::Shot), [0, 1, 2]);
        assert_eq!(marks(column.enemy, Mark::Reached), [3, 4, 5]);
        let all = reach(&rom(1, 0, 4), 0, BACK).expect("a weapon");
        assert_eq!(marks(all.enemy, Mark::Shot), [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn a_repair_aims_at_its_own_side_and_a_passive_part_at_nothing() {
        let own = reach(&rom(2, 0, 4), 0, FRONT).expect("a weapon");
        assert_eq!(own.enemy, [Mark::Empty; CELLS]);
        assert_eq!(marks(own.own, Mark::Shot), [0, 1, 2, 3, 4, 5]);
        let user = reach(&rom(4, 0, 0), 0, BACK).expect("a weapon");
        assert_eq!(marks(user.own, Mark::Shot), [BACK]);
        assert_eq!(reach(&rom(0x4000, 0, 0), 0, FRONT), None);
    }
}
