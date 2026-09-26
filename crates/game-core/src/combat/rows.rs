//! The row advance at a round's start (`0x0802F09C`): a side whose front
//! row has no unit left fighting moves its back row up.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! task `0x0802F09C`, its units' moves (`0x0802F408`), the side's sprites'
//! placing (`0x080317E4`) and the check of a slot (`0x08032A88`). See
//! `docs/combat.md`.

use super::Combat;
use super::ai::PARTY;
use super::turn::{PARALYSED, Task};
use extraction::saga_combat::SLOTS;

/// The back row's first slot: slot `n` of it moves to slot `n − 3`.
const BACK_ROW: usize = 3;
/// Frames a unit takes to move up (`0x0802F408`).
const MOVE_FRAMES: i32 = 15;
/// How a paralysed unit's colors are darkened once put back in place.
const DIM_PARALYSED: u8 = 24;

/// A step of the row advance's task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Rows {
    /// State 1000: whether side `n`'s front row is empty.
    Check(usize),
    /// States 2000 and `0x7DA`: side `n`'s back row moves up.
    Advance(usize),
    /// State `0x834`: the moves play, `n` frames of them done.
    Moving(usize, i32),
    /// State `0x898`: the side's sprites are put in their slots.
    Placed(usize),
    /// State `0xBB8`: the enemy's side next, or the end.
    Next(usize),
    /// State `0x2328`: the figures come back and the task reports.
    Done,
}

/// A unit's sprite on its way to its new slot, in 16.16.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RowMove {
    pub(super) side: usize,
    pub(super) slot: usize,
    pub(super) at: (i32, i32),
    step: (i32, i32),
    target: (i32, i32),
}

impl Combat {
    /// One frame of the row advance's task.
    pub(super) fn step_rows(&mut self, rows: Rows) -> Task {
        Task::Rows(match rows {
            Rows::Check(side) => {
                if (0..BACK_ROW).any(|slot| self.fighting_in(side, slot)) {
                    Rows::Next(side)
                } else {
                    Rows::Advance(side)
                }
            }
            Rows::Advance(side) => {
                self.figures_held = true;
                if self.advance_rows(side) {
                    Rows::Moving(side, 0)
                } else {
                    Rows::Next(side)
                }
            }
            Rows::Moving(side, done) if done >= MOVE_FRAMES => Rows::Placed(side),
            Rows::Moving(side, done) => {
                for row_move in &mut self.fight.row_moves {
                    row_move.at = if done + 1 >= MOVE_FRAMES {
                        row_move.target
                    } else {
                        (
                            row_move.at.0.wrapping_add(row_move.step.0),
                            row_move.at.1.wrapping_add(row_move.step.1),
                        )
                    };
                }
                Rows::Moving(side, done + 1)
            }
            Rows::Placed(side) => {
                self.fight
                    .row_moves
                    .retain(|row_move| row_move.side != side);
                for slot in 0..SLOTS {
                    let paralysed = self.sides[side][slot]
                        .as_ref()
                        .is_some_and(|unit| unit.fighting() && unit.traits & PARALYSED != 0);
                    if paralysed {
                        self.dims[side][slot] = DIM_PARALYSED;
                    }
                }
                self.fight.advanced[side] = true;
                Rows::Next(side)
            }
            Rows::Next(PARTY) => Rows::Check(1),
            Rows::Next(_) => Rows::Done,
            Rows::Done => {
                self.figures_held = false;
                return Task::Reported;
            }
        })
    }

    /// Whether side `side`'s slot `slot` holds a unit still fighting
    /// (`0x08032A88`).
    fn fighting_in(&self, side: usize, slot: usize) -> bool {
        self.sides[side][slot]
            .as_ref()
            .is_some_and(super::units::BattleUnit::fighting)
    }

    /// Moves each unit of side `side`'s back row still fighting to the
    /// slot in front of it, its sprite starting on its way; whether any
    /// moved.
    fn advance_rows(&mut self, side: usize) -> bool {
        let mut moved = false;
        for back in BACK_ROW..SLOTS {
            if !self.fighting_in(side, back) {
                continue;
            }
            let front = back - BACK_ROW;
            self.sides[side][front] = self.sides[side][back].take();
            self.units[side][front] = self.units[side][back].take();
            self.dims[side][front] = std::mem::take(&mut self.dims[side][back]);
            self.fight.gone[side][front] = false;
            self.fight.gone[side][back] = false;
            if side == PARTY {
                for panel in self.panels.iter_mut().filter(|panel| panel.slot == back) {
                    panel.slot = front;
                }
            }
            let fixed = |(x, y): (i32, i32)| (x << 16, y << 16);
            let (at, target) = (
                fixed(self.anchors[side][back]),
                fixed(self.anchors[side][front]),
            );
            let row_move = RowMove {
                side,
                slot: front,
                at,
                step: (
                    (target.0 - at.0) / MOVE_FRAMES,
                    (target.1 - at.1) / MOVE_FRAMES,
                ),
                target,
            };
            self.fight.row_moves.push(row_move);
            // The sprite that moves is the back slot's, on the screen
            // already where it starts: the frame shows it there.
            self.shown_row_moves.push(row_move);
            self.shown_gone[side][front] = false;
            moved = true;
        }
        moved
    }

    /// Where the frame shows a unit's sprite while its row moves up.
    pub(super) fn row_move_at(&self, side: usize, slot: usize) -> Option<(i32, i32)> {
        self.shown_row_moves
            .iter()
            .find(|row_move| row_move.side == side && row_move.slot == slot)
            .map(|row_move| (row_move.at.0 >> 16, row_move.at.1 >> 16))
    }
}
