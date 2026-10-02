//! The player's statistics, a port feature: what the battles brought, how
//! long the game has been played and the records set, counted as the game
//! goes and kept with each save in a note of this port's own (see
//! `docs/formats/save.md`); the enhanced mode shows them, with what the
//! game state says of the collection, in the pause menu's コンフィグ.
//!
//! Source of knowledge: this project's own design; the outcome of a
//! battle, its blows and its money are the port's battle rules (see
//! `docs/combat.md`).

use crate::combat::{BattleTally, Outcome};
use extraction::saga_encounter::STORY_BATTLE_COUNT;

/// Game frames a second.
pub const FRAMES_PER_SECOND: u32 = 60;
/// The bytes the statistics take in a save's note.
pub const STATS_LEN: usize = 9 * 4 + 8;

/// What the player has done since the game was started.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Battles won, roaming and story alike.
    pub battles_won: u32,
    /// Battles lost.
    pub battles_lost: u32,
    /// Battles the party retreated from.
    pub battles_retreated: u32,
    /// Enemy Zoids destroyed.
    pub enemies_destroyed: u32,
    /// The party's Zoids destroyed.
    pub party_destroyed: u32,
    /// Game frames played, on the field, in its menus and in battle.
    pub play_frames: u32,
    /// Money the won battles added.
    pub money_earned: u32,
    /// The most damage one blow of the party dealt.
    pub best_hit: u32,
    /// The most rounds a battle lasted.
    pub longest_battle: u32,
    /// The story battles won, a bit each (bit `n` for battle `n`).
    pub story_won: u64,
}

impl Stats {
    /// Counts a battle that ended with `outcome`, what `tally` saw of it
    /// and, for a story battle, its number.
    pub fn record_battle(&mut self, outcome: Outcome, tally: &BattleTally, story: Option<u8>) {
        let count = match outcome {
            Outcome::Won => &mut self.battles_won,
            Outcome::Lost => &mut self.battles_lost,
            Outcome::Retreated => &mut self.battles_retreated,
        };
        *count = count.saturating_add(1);
        self.enemies_destroyed = self
            .enemies_destroyed
            .saturating_add(tally.enemies_destroyed);
        self.party_destroyed = self.party_destroyed.saturating_add(tally.party_destroyed);
        self.money_earned = self.money_earned.saturating_add(tally.money);
        self.best_hit = self.best_hit.max(tally.best_hit);
        self.longest_battle = self.longest_battle.max(tally.rounds);
        if let Some(battle) = story.filter(|battle| usize::from(*battle) < STORY_BATTLE_COUNT)
            && outcome == Outcome::Won
        {
            self.story_won |= 1 << battle;
        }
    }

    /// Counts a frame of play.
    pub fn tick(&mut self) {
        self.play_frames = self.play_frames.saturating_add(1);
    }

    /// The story battles won.
    #[must_use]
    pub fn story_battles_won(&self) -> u32 {
        self.story_won.count_ones()
    }

    /// The time played: hours, minutes and seconds.
    #[must_use]
    pub fn play_time(&self) -> (u32, u32, u32) {
        let seconds = self.play_frames / FRAMES_PER_SECOND;
        (seconds / 3600, seconds / 60 % 60, seconds % 60)
    }

    /// The statistics as a save's note keeps them, little-endian.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; STATS_LEN] {
        let mut bytes = [0; STATS_LEN];
        let words = [
            self.battles_won,
            self.battles_lost,
            self.battles_retreated,
            self.enemies_destroyed,
            self.party_destroyed,
            self.play_frames,
            self.money_earned,
            self.best_hit,
            self.longest_battle,
        ];
        for (chunk, word) in bytes.chunks_exact_mut(4).zip(words) {
            chunk.copy_from_slice(&word.to_le_bytes());
        }
        bytes[words.len() * 4..].copy_from_slice(&self.story_won.to_le_bytes());
        bytes
    }

    /// The statistics `bytes` keep, `None` when they are too short.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let bytes = bytes.get(..STATS_LEN)?;
        let word = |index: usize| {
            let at = index * 4;
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        };
        let mut story = [0; 8];
        story.copy_from_slice(&bytes[9 * 4..]);
        Some(Self {
            battles_won: word(0),
            battles_lost: word(1),
            battles_retreated: word(2),
            enemies_destroyed: word(3),
            party_destroyed: word(4),
            play_frames: word(5),
            money_earned: word(6),
            best_hit: word(7),
            longest_battle: word(8),
            story_won: u64::from_le_bytes(story),
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn tally(enemies: u32, party: u32, money: u32, best: u32, rounds: u32) -> BattleTally {
        BattleTally {
            enemies_destroyed: enemies,
            party_destroyed: party,
            money,
            best_hit: best,
            rounds,
        }
    }

    #[test]
    fn battles_add_up_and_records_keep_the_most() {
        let mut stats = Stats::default();
        stats.record_battle(Outcome::Won, &tally(3, 1, 500, 120, 4), None);
        stats.record_battle(Outcome::Lost, &tally(1, 4, 0, 300, 9), Some(5));
        stats.record_battle(Outcome::Retreated, &tally(0, 0, 0, 10, 2), None);
        stats.record_battle(Outcome::Won, &tally(2, 0, 800, 90, 3), Some(41));
        assert_eq!(
            (
                stats.battles_won,
                stats.battles_lost,
                stats.battles_retreated
            ),
            (2, 1, 1)
        );
        assert_eq!((stats.enemies_destroyed, stats.party_destroyed), (6, 5));
        assert_eq!(stats.money_earned, 1300);
        assert_eq!((stats.best_hit, stats.longest_battle), (300, 9));
        assert_eq!(stats.story_battles_won(), 1, "battle 5 was lost");
        assert_eq!(stats.story_won, 1 << 41);
    }

    #[test]
    fn the_time_played_counts_frames_at_sixty_a_second() {
        let mut stats = Stats {
            play_frames: (3600 + 2 * 60 + 5) * FRAMES_PER_SECOND - 1,
            ..Stats::default()
        };
        assert_eq!(stats.play_time(), (1, 2, 4));
        stats.tick();
        assert_eq!(stats.play_time(), (1, 2, 5));
    }

    #[test]
    fn the_note_bytes_give_back_the_same_statistics() {
        let stats = Stats {
            battles_won: 1,
            battles_lost: 2,
            battles_retreated: 3,
            enemies_destroyed: 4,
            party_destroyed: 5,
            play_frames: 6,
            money_earned: 7,
            best_hit: 8,
            longest_battle: 9,
            story_won: 0x0000_0201_0000_0001,
        };
        assert_eq!(Stats::from_bytes(&stats.to_bytes()), Some(stats));
        assert_eq!(Stats::from_bytes(&[0; STATS_LEN - 1]), None);
    }
}
