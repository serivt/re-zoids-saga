//! The battle's results (task `0x08035778` in slot 5): the messages of a
//! win, a loss or a retreat, the money, the spoils and the experience the
//! party takes, its levels, and the units' hit and energy points written
//! back into the game state.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! results task (`0x08035778`) and its routines: the money and the
//! experience (`0x08037100`, `0x08037124`, the table at ROM `0x66BB58`,
//! `0x08035FE0`), the spoils (`0x0803666C`, `0x0803680C`, ROM `0x66BB10`
//! and `0x66BB24`, `0x08037014`, `0x08037040`, `0x0803706C`,
//! `0x08037098`), their names (`0x08032800`, `0x08032840`, `0x080328E4`),
//! the write-back (`0x080364DC`, `0x08037B1C`) and the loss's
//! (`0x08032B18`, `0x08037AB4`); checked against a battle won on the world
//! map in a reference emulator (its calls and task states frame by frame).
//! See `docs/combat.md`.

use extraction::saga::EXPERIENCE_TABLE;
use extraction::saga_combat::SLOTS;
use extraction::saga_encounter::{self, Formation};
use extraction::saga_party;

use super::ai::PARTY;
use super::turn::{number, number_in};
use super::{Act, Call, Combat, Outcome};

// Sounds.
const WIN_SOUND: u16 = 0x33;
const SPOILS_SOUND: u16 = 0x35;
const LEVEL_SOUND: u16 = 0x34;
// `battle-menu` 0x14: wait for a key.
const MENU_KEY: u16 = 0x14;
// `battle-text` strings.
const TEXT_EXPERIENCE: u16 = 7;
const TEXT_LEVEL: u16 = 8;
const TEXT_OBTAINED: u16 = 9;
const TEXT_MONEY: u16 = 0x0A;
const TEXT_WON: u16 = 0x0B;
const TEXT_LOST: u16 = 0x0C;
const TEXT_ITEM: u16 = 0x27;
const TEXT_CORE: u16 = 0x28;
const TEXT_PART: u16 = 0x29;
const TEXT_ZI_DATA: u16 = 0x2A;
const TEXT_ZI_DATA_OBTAINED: u16 = 0x2B;
const TEXT_ZI_DATA_KNOWN: u16 = 0x2C;
const TEXT_LEVEL_REACHED: u16 = 0x2E;
const TEXT_PLAYER: u16 = 0x33;

// The game state.
const LEVEL: usize = 0xCD2;
const EXPERIENCE: usize = 0xCD4;
const MONEY: usize = 0xD28;
const MOST: u32 = 9_999_999;
const TOP_LEVEL: u8 = 99;
const ITEM_COUNTS: usize = 0x3305;
const CORE_COUNTS: usize = 0x330C;
const PART_COUNTS: usize = 0x334C;
const ZI_DATA: usize = 0x33E2;
const MOST_ITEMS: u8 = 99;
const MOST_PARTS: u8 = 9;
const UNITS: usize = 0xD2C;
const UNIT_LEN: usize = 0x38;
const UNIT_FLAGS: usize = 2;
const UNIT_HP: usize = 8;
const UNIT_EP: usize = 0xC;
const UNIT_MOST_HP: usize = 0x28;
const UNIT_MOST_EP: usize = 0x2C;
const IN_USE: u16 = 1;
const IN_FORMATION: u16 = 2;
/// A unit beaten for good: no hit points, out of the formation.
const WRECKED: u16 = 0x800;
const FORMATION: usize = 0x3600;
const CHARACTERS: usize = 0x34A4;
const CHARACTER_IN_FORMATION: u16 = 0x10;
/// The slot the player takes again after a loss.
const PLAYER_SLOT: usize = 1;
const CHAPTER: usize = 2;
/// The chapter whose spoils come from the second table.
const LATE_CHAPTER: u8 = 10;

// The spoils: the formation's item and core, and the enemy records' Zi
// data and weapons.
const SPOIL_KINDS: usize = 0x6_6BB10;
const LATE_SPOIL_KINDS: usize = 0x6_6BB24;
const SPOIL_ROLLS: u16 = 20;
const FORMATION_ITEM: usize = 0x20;
const FORMATION_CORE: usize = 0x21;
const MEMBER_RECORD: usize = 5;
const MEMBER_LEN: usize = 4;
const WEAPON_DROPS: u16 = 3;
const NO_SPOIL: u8 = 0xFF;
const CONSUMABLE_NAMES: u16 = 241;

// The points the levels bring (task `0x080360A1`).
const POINTS_PER_LEVEL: u16 = 10;
const MOST_STAT: u16 = 200;
/// The player's five bonuses in the screen's order: 耐久力, 攻撃力, 防御力,
/// 反応値, 命中値.
const PLAYER_STATS: [usize; 5] = [0xCDC, 0xCE2, 0xCE0, 0xCDE, 0xCE4];
/// The other characters' blocks, which the level sets by the growth table
/// at ROM `0x66BB38` (`0x080368BC`, `0x08036904`).
const COMPANIONS: [usize; 3] = [0xCE8, 0xCF8, 0xD08];
const GROWTH: usize = 0x6_6BB38;
const COMPANION_STATS: usize = 4;
const CHARACTER_UNITS: usize = 0x34A6;
const REFRESHED_CHARACTERS: u8 = 4;
// `battle-menu` scripts of the screen: the statistics' window, the menu,
// its choice (`0x36` in mode 6, which also ends on a move), and the
// statistics' window drawn, cleared and presented.
const MENU_STATS_WINDOW: u16 = 0x16;
const MENU_STATS_MENU: u16 = 0x17;
const MENU_STATS_CHOICE: u16 = 0x0F;
const MENU_STATS_DRAW: u16 = 0x0E;
const MENU_STATS_CLEAR: u16 = 0x0C;
const MENU_STATS_PRESENT: u16 = 0x0A;
// `battle-text` strings: →, a line break, the prompt and its end, the
// digits' table's space (94 + 12), the statistics' names (`item` 143 on).
const TEXT_ARROW: u16 = 0x2D;
const TEXT_LINE: u16 = 0;
const TEXT_CHOOSE: u16 = 0x2F;
const TEXT_CHOOSE_END: u16 = 0x30;
const TEXT_ALLOCATED: u16 = 0x31;
const TEXT_SPACE: u16 = 94 + 12;
const STAT_NAMES: u16 = 143;
const CHOSEN: u16 = 1;
/// The statistics' window.
const STATS_WINDOW: u8 = 3;

/// The points left to share out, and the bonuses before.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Allocation {
    points: u16,
    before: [u16; 5],
}

/// A spoil the battle leaves (EWRAM `0x0200EB84 + 0x22AC`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Spoil {
    Item(u8),
    Core(u8),
    Part(u8),
    ZiData(u8),
}

/// The results task's states, by the original's numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Results {
    Idle,
    /// 0: the outcome picks the messages.
    Start,
    /// 1000: the win, the money, then the spoils' roll.
    Victory,
    /// `0x834`, `0x898`, `0x8FC` and `0xA8C`: the spoil.
    Spoil(Spoil),
    /// `0xBB8`: the experience, and the levels it brings.
    Experience,
    /// 4000 and `0x1004`: a level's message, `left` of them still to come.
    Level {
        level: u8,
        left: u8,
    },
    /// `0x1388`: the points of the levels, shared out on the task
    /// `0x080360A1` it starts (its states 0, 1000, `0x44C` and `0x2328`).
    Allocate,
    AllocateDraw,
    AllocateMenu,
    AllocateEnd,
    /// `0x1392`: the characters' statistics by the new level.
    Recompute,
    /// `0x1B58`: the loss.
    Defeat,
    /// 8000 and `0x1FA4`: the retreat.
    Retreat,
    AwaitRetreat,
    /// `0x2328`: the write-back.
    Finish,
    /// `0x238C`: the results have ended.
    Done,
}

impl Combat {
    /// One step of the results task (`0x08035778`), when none of its calls
    /// is running.
    pub(super) fn step_results(&mut self, rom: &[u8]) {
        match self.results {
            Results::Start => {
                self.results = match self.outcome {
                    Some(Outcome::Won) => {
                        self.acts.push_back(Act::Sound(WIN_SOUND));
                        Results::Victory
                    }
                    Some(Outcome::Lost) => Results::Defeat,
                    Some(Outcome::Retreated) => {
                        self.acts.push_back(Act::Sound(super::RETREAT_SOUND));
                        Results::Retreat
                    }
                    None => Results::Finish,
                };
            }
            Results::Victory => {
                let money = self.fight.money;
                add(&mut self.state, MONEY, money);
                self.message_key(&[Call::Text(TEXT_WON)]);
                let mut calls = number(i32::try_from(money).unwrap_or(i32::MAX));
                calls.push(Call::Text(TEXT_MONEY));
                self.message_key(&calls);
                self.acts.push_back(Act::Spoils);
                self.results = Results::Idle;
            }
            Results::Spoil(spoil) => {
                self.take_spoil(spoil);
                self.results = Results::Experience;
            }
            Results::Experience => self.step_experience(),
            Results::Level { level, left } => self.step_level(level, left),
            Results::Allocate => {
                self.allocation = Allocation {
                    points: u16::from(self.levels) * POINTS_PER_LEVEL,
                    before: PLAYER_STATS.map(|at| half(&self.state, at)),
                };
                self.acts
                    .push_back(Act::Call(Call::Menu(MENU_STATS_WINDOW)));
                self.acts.push_back(Act::Call(Call::Menu(MENU_STATS_MENU)));
                // The menu's names take the original a frame more; its
                // state 0 then goes on into 1000.
                self.acts.push_back(Act::Pause(1));
                self.draw_allocation(true);
            }
            Results::Recompute => {
                self.recompute(rom);
                self.results = Results::Finish;
            }
            Results::Defeat => {
                self.message_key(&[Call::Text(TEXT_PLAYER), Call::Text(TEXT_LOST)]);
                self.results = Results::Finish;
            }
            Results::Retreat => {
                self.message(&[Call::Text(super::TEXT_RETREATED)]);
                self.results = Results::AwaitRetreat;
            }
            Results::AwaitRetreat => {
                if self.wait_done() {
                    self.results = Results::Finish;
                }
            }
            Results::Finish => {
                self.write_back();
                if self.outcome == Some(Outcome::Lost) {
                    self.recover_player();
                }
                self.results = Results::Done;
            }
            Results::AllocateDraw
            | Results::AllocateMenu
            | Results::AllocateEnd
            | Results::Idle
            | Results::Done => {}
        }
    }

    /// State `0xBB8`: the experience, and the levels it brings.
    fn step_experience(&mut self) {
        let experience = self.fight.experience;
        add(&mut self.state, EXPERIENCE, experience);
        let mut calls = number(i32::try_from(experience).unwrap_or(i32::MAX));
        calls.push(Call::Text(TEXT_EXPERIENCE));
        self.message_key(&calls);
        let level = self.state.get(LEVEL).copied().unwrap_or(0);
        let mut left = 0;
        while self.level_up() {
            left += 1;
        }
        self.levels = left;
        self.results = if left == 0 {
            Results::Finish
        } else {
            Results::Level { level, left }
        };
    }

    /// States 4000 and `0x1004`: a level's message.
    fn step_level(&mut self, level: u8, left: u8) {
        let level = level.saturating_add(1);
        self.acts.push_back(Act::Sound(LEVEL_SOUND));
        let mut calls = vec![Call::Text(TEXT_PLAYER), Call::Text(TEXT_LEVEL)];
        calls.extend(number(i32::from(level)));
        calls.push(Call::Text(TEXT_LEVEL_REACHED));
        self.message_key(&calls);
        self.results = if left > 1 {
            Results::Level {
                level,
                left: left - 1,
            }
        } else {
            Results::Allocate
        };
    }

    /// A message, then a key (`battle-menu` 6, 7, the calls, 5 and 0x14).
    fn message_key(&mut self, calls: &[Call]) {
        self.acts.push_back(Act::Call(Call::Menu(super::MENU_DRAW)));
        self.acts
            .push_back(Act::Call(Call::Menu(super::MENU_CLEAR)));
        for call in calls {
            self.acts.push_back(Act::Call(*call));
        }
        self.acts
            .push_back(Act::Call(Call::Menu(super::MENU_PRESENT)));
        self.acts.push_back(Act::Call(Call::Menu(MENU_KEY)));
    }

    /// The spoils' roll once the money's message is read (`0x0803666C`,
    /// `0x0803680C`): a unit of the formation, the leader or one of its
    /// members, gives its Zi data and one of its three weapons; the
    /// formation gives an item and a core. A roll of 20 picks which the
    /// battle leaves.
    pub(super) fn roll_spoils(&mut self, rom: &[u8]) {
        let formation = self.formation;
        let item = formation[FORMATION_ITEM];
        let core = formation[FORMATION_CORE];
        let members: Vec<usize> = (0..SLOTS)
            .filter(|member| formation.get(MEMBER_RECORD + member * MEMBER_LEN) != Some(&NO_SPOIL))
            .collect();
        let pick = if members.is_empty() {
            None
        } else {
            let count = u16::try_from(members.len() + 1).unwrap_or(1);
            let roll = usize::from(self.rng.next(self.vblank) % count);
            roll.checked_sub(1).map(|at| members[at])
        };
        let record = unit_record(rom, &formation, pick);
        let zi_data = record.map_or(NO_SPOIL, |record| record[0]);
        let drop = self.rng.next(self.vblank) % WEAPON_DROPS;
        let part = record.and_then(|record| {
            let at = 4 + usize::from(drop) * 4;
            let chance = u16::from_le_bytes([record[at], record[at + 1]]);
            let part = u16::from_le_bytes([record[at + 2], record[at + 3]]);
            (part != 0xFFFF && chance != 0).then(|| part.to_le_bytes()[0])
        });
        let roll = self.rng.next(self.vblank) % SPOIL_ROLLS;
        let table = if self.state.get(CHAPTER) == Some(&LATE_CHAPTER) {
            LATE_SPOIL_KINDS
        } else {
            SPOIL_KINDS
        };
        let kind = rom.get(table + usize::from(roll)).copied().unwrap_or(0);
        let spoil = match kind {
            1 if item != NO_SPOIL => Some(Spoil::Item(item)),
            2 if core != NO_SPOIL => Some(Spoil::Core(core)),
            3 => part.map(Spoil::Part),
            4 if zi_data != NO_SPOIL => Some(Spoil::ZiData(zi_data)),
            _ => None,
        };
        self.results = match spoil {
            Some(spoil) => {
                self.sounds.push(SPOILS_SOUND);
                Results::Spoil(spoil)
            }
            None => Results::Experience,
        };
    }

    /// The spoil's message, and the spoil into the game state.
    fn take_spoil(&mut self, spoil: Spoil) {
        match spoil {
            Spoil::Item(item) => {
                count_up(&mut self.state, ITEM_COUNTS + usize::from(item), MOST_ITEMS);
                self.message_key(&[
                    Call::Text(TEXT_ITEM),
                    Call::NameIndex(CONSUMABLE_NAMES + u16::from(item)),
                    Call::Text(TEXT_OBTAINED),
                ]);
            }
            Spoil::Core(core) => {
                count_up(&mut self.state, CORE_COUNTS + usize::from(core), MOST_ITEMS);
                self.message_key(&[
                    Call::Text(TEXT_CORE),
                    Call::Item(u16::from(core)),
                    Call::Text(TEXT_OBTAINED),
                ]);
            }
            Spoil::Part(part) => {
                count_up(&mut self.state, PART_COUNTS + usize::from(part), MOST_PARTS);
                self.message_key(&[
                    Call::Text(TEXT_PART),
                    Call::Part(u16::from(part)),
                    Call::Text(TEXT_OBTAINED),
                ]);
            }
            Spoil::ZiData(zi_data) => {
                self.message_key(&[
                    Call::Text(TEXT_ZI_DATA),
                    Call::Name(u16::from(zi_data)),
                    Call::Text(TEXT_ZI_DATA_OBTAINED),
                ]);
                if let Some(known) = self.state.get_mut(ZI_DATA + usize::from(zi_data)) {
                    if *known != 0 {
                        self.message_key(&[Call::Text(TEXT_ZI_DATA_KNOWN)]);
                    } else {
                        *known = 1;
                    }
                }
            }
        }
    }

    /// The task's state 1000: the statistics, the points left, then the
    /// menu (state `0x44C`).
    fn draw_allocation(&mut self, first: bool) {
        self.draw_stats(first);
        self.acts.push_back(Act::Call(Call::Menu(super::MENU_DRAW)));
        self.acts
            .push_back(Act::Call(Call::Menu(super::MENU_CLEAR)));
        self.acts.push_back(Act::Call(Call::Text(TEXT_CHOOSE)));
        for call in number_in(i32::from(self.allocation.points), 7, 1) {
            self.acts.push_back(Act::Call(call));
        }
        self.acts.push_back(Act::Call(Call::Text(TEXT_CHOOSE_END)));
        self.acts
            .push_back(Act::Call(Call::Menu(super::MENU_PRESENT)));
        self.acts
            .push_back(Act::Call(Call::Menu(MENU_STATS_CHOICE)));
        self.acts.push_back(Act::Allocation);
        self.results = Results::Idle;
    }

    /// The statistics' window (`0x08036270`): each bonus before and now.
    /// Its text takes the original about two lines a frame, and its
    /// presenting a frame more.
    fn draw_stats(&mut self, first: bool) {
        self.acts.push_back(Act::Call(Call::Menu(MENU_STATS_DRAW)));
        self.acts.push_back(Act::Call(Call::Menu(MENU_STATS_CLEAR)));
        self.acts.push_back(Act::PrintWindow(STATS_WINDOW));
        for (line, &at) in PLAYER_STATS.iter().enumerate() {
            let mut calls = vec![
                Call::Text(TEXT_SPACE),
                Call::Item(STAT_NAMES + u16::try_from(line).unwrap_or(0)),
                Call::Text(TEXT_SPACE),
            ];
            calls.extend(number_in(i32::from(self.allocation.before[line]), 3, 0));
            calls.push(Call::Text(TEXT_ARROW));
            calls.extend(number_in(i32::from(half(&self.state, at)), 3, 0));
            if line + 1 < PLAYER_STATS.len() {
                calls.push(Call::Text(TEXT_LINE));
            }
            for call in calls {
                self.acts.push_back(Act::Call(call));
            }
            if line % 2 == 1 {
                self.acts.push_back(Act::Pause(1));
            }
        }
        self.acts.push_back(Act::PrintWindow(super::MESSAGE_WINDOW));
        self.acts
            .push_back(Act::Call(Call::Menu(MENU_STATS_PRESENT)));
        if first {
            self.acts.push_back(Act::Pause(1));
        }
    }

    /// What the menu returned (the task's state `0x44C`): A raises the
    /// bonus of its line by one, up to 200, for a point.
    pub(super) fn allocate(&mut self) {
        let [chosen, line, ..] = *self.menu.vars();
        let mut next = Results::AllocateMenu;
        if chosen == CHOSEN
            && let Some(&at) = PLAYER_STATS.get(usize::from(line))
        {
            let value = half(&self.state, at);
            if value < MOST_STAT {
                set_half(&mut self.state, at, value + 1);
                self.allocation.points = self.allocation.points.saturating_sub(1);
                next = Results::AllocateDraw;
            }
        }
        let capped = PLAYER_STATS
            .iter()
            .all(|&at| half(&self.state, at) == MOST_STAT);
        if self.allocation.points == 0 || capped {
            next = Results::AllocateEnd;
        }
        // The original's menu returns in the key's frame and the task goes
        // on in the next; the port's returns a frame later, so the task's
        // next state is queued at once.
        match next {
            Results::AllocateDraw => self.draw_allocation(false),
            Results::AllocateEnd => {
                self.draw_stats(false);
                self.message_key(&[Call::Text(TEXT_ALLOCATED)]);
                self.results = Results::Recompute;
            }
            _ => {
                self.acts
                    .push_back(Act::Call(Call::Menu(MENU_STATS_CHOICE)));
                self.acts.push_back(Act::Allocation);
            }
        }
    }

    /// The characters' statistics once the levels are shared out
    /// (`0x080368BC`, `0x08036960`): the others' bonuses grow with the
    /// level, and the units of the first four characters are computed
    /// again.
    fn recompute(&mut self, rom: &[u8]) {
        let level = u16::from(self.state.get(LEVEL).copied().unwrap_or(0));
        for (companion, &block) in COMPANIONS.iter().enumerate() {
            for stat in 0..5 {
                let at = GROWTH + companion * 10 + stat * 2;
                let growth = rom
                    .get(at..at + 2)
                    .map_or(0, |bytes| u16::from_le_bytes([bytes[0], bytes[1]]));
                set_half(
                    &mut self.state,
                    block + COMPANION_STATS + stat * 2,
                    level.wrapping_mul(growth),
                );
            }
        }
        for character in 0..REFRESHED_CHARACTERS {
            let unit = self
                .state
                .get(CHARACTER_UNITS + usize::from(character) * 4)
                .copied()
                .unwrap_or(NO_SPOIL);
            if unit != NO_SPOIL {
                saga_party::refresh_stats(rom, &mut self.state, character, unit);
            }
        }
    }

    /// A level more when the experience reaches the table's next
    /// (`0x08035FE0`).
    fn level_up(&mut self) -> bool {
        let level = self.state.get(LEVEL).copied().unwrap_or(TOP_LEVEL);
        let experience = word(&self.state, EXPERIENCE);
        let needed = self
            .rom_experience
            .get(usize::from(level))
            .copied()
            .unwrap_or(u32::MAX);
        if level < TOP_LEVEL && needed <= experience {
            self.state[LEVEL] = level + 1;
            return true;
        }
        false
    }

    /// The units' hit and energy points back into the game state
    /// (`0x080364DC`): a formation slot whose unit did not fight is left
    /// wrecked and taken out.
    fn write_back(&mut self) {
        for (slot, entry) in saga_party::formation(&self.state).iter().enumerate() {
            let Some((unit, _)) = *entry else {
                continue;
            };
            let at = UNITS + usize::from(unit) * UNIT_LEN;
            if let Some(fighter) = self.sides[PARTY][slot].as_ref() {
                let (hp, ep) = (fighter.hp.max(0), fighter.ep.max(0));
                set_word(
                    &mut self.state,
                    at + UNIT_HP,
                    u32::try_from(hp).unwrap_or(0),
                );
                set_word(
                    &mut self.state,
                    at + UNIT_EP,
                    u32::try_from(ep).unwrap_or(0),
                );
            } else {
                set_word(&mut self.state, at + UNIT_HP, 0);
                set_word(&mut self.state, at + UNIT_EP, 0);
                let flags = half(&self.state, at) | WRECKED;
                set_half(&mut self.state, at, flags);
                self.leave_formation(slot);
            }
        }
    }

    /// Takes formation slot `slot`'s unit and pilot out (`0x08037B1C`).
    fn leave_formation(&mut self, slot: usize) {
        let at = FORMATION + slot * 4;
        let (unit, character) = (self.state[at], self.state[at + 1]);
        self.state[at] = NO_SPOIL;
        self.state[at + 1] = NO_SPOIL;
        let unit_flags = UNITS + usize::from(unit) * UNIT_LEN + UNIT_FLAGS;
        if unit_flags + 2 <= self.state.len() {
            let flags = half(&self.state, unit_flags) & !IN_FORMATION;
            set_half(&mut self.state, unit_flags, flags);
        }
        let pilot = CHARACTERS + usize::from(character) * 4;
        if pilot + 2 <= self.state.len() {
            let flags = half(&self.state, pilot) & !CHARACTER_IN_FORMATION;
            set_half(&mut self.state, pilot, flags);
        }
    }

    /// After a loss the player's unit (or the first one free) comes back
    /// whole and takes the formation's second slot (`0x08035778` state
    /// `0x2328`, `0x08037AB4`).
    fn recover_player(&mut self) {
        let player_unit = CHARACTERS + 2;
        let mut unit = self.state.get(player_unit).copied().unwrap_or(NO_SPOIL);
        if unit == NO_SPOIL {
            let free = (0..=u8::MAX).find(|unit| {
                let at = UNITS + usize::from(*unit) * UNIT_LEN + UNIT_FLAGS;
                at + 2 <= self.state.len() && {
                    let flags = half(&self.state, at);
                    flags & IN_USE != 0 && flags & IN_FORMATION == 0
                }
            });
            let Some(free) = free else {
                return;
            };
            unit = free;
            self.state[player_unit] = unit;
        }
        let at = UNITS + usize::from(unit) * UNIT_LEN;
        let (hp, ep) = (
            word(&self.state, at + UNIT_MOST_HP),
            word(&self.state, at + UNIT_MOST_EP),
        );
        set_word(&mut self.state, at + UNIT_HP, hp);
        set_word(&mut self.state, at + UNIT_EP, ep);
        let flags = half(&self.state, at) & !WRECKED;
        set_half(&mut self.state, at, flags);
        let slot = FORMATION + PLAYER_SLOT * 4;
        self.state[slot + 1] = 0;
        self.state[slot] = unit;
        let unit_flags = half(&self.state, at + UNIT_FLAGS) | IN_FORMATION;
        set_half(&mut self.state, at + UNIT_FLAGS, unit_flags);
        let pilot = half(&self.state, CHARACTERS) | CHARACTER_IN_FORMATION;
        set_half(&mut self.state, CHARACTERS, pilot);
    }
}

/// The record of the formation's unit `member`, or of its leader
/// (`0x0803666C`).
fn unit_record(
    rom: &[u8],
    formation: &Formation,
    member: Option<usize>,
) -> Option<[u8; saga_encounter::ENEMY_RECORD_LEN]> {
    saga_encounter::spoils_record(rom, formation, member)
}

/// The experience each level needs (ROM `0x66BB58`).
pub(super) fn experience_table(rom: &[u8]) -> Vec<u32> {
    (0..usize::from(TOP_LEVEL))
        .map(|level| {
            let at = EXPERIENCE_TABLE + level * 4;
            rom.get(at..at + 4).map_or(u32::MAX, |bytes| {
                u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
            })
        })
        .collect()
}

fn add(state: &mut [u8], at: usize, amount: u32) {
    let value = word(state, at).saturating_add(amount).min(MOST);
    set_word(state, at, value);
}

fn count_up(state: &mut [u8], at: usize, most: u8) {
    if let Some(count) = state.get_mut(at) {
        *count = count.saturating_add(1).min(most);
    }
}

fn word(state: &[u8], at: usize) -> u32 {
    state.get(at..at + 4).map_or(0, |bytes| {
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    })
}

fn set_word(state: &mut [u8], at: usize, value: u32) {
    if let Some(bytes) = state.get_mut(at..at + 4) {
        bytes.copy_from_slice(&value.to_le_bytes());
    }
}

fn half(state: &[u8], at: usize) -> u16 {
    state
        .get(at..at + 2)
        .map_or(0, |bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn set_half(state: &mut [u8], at: usize, value: u16) {
    if let Some(bytes) = state.get_mut(at..at + 2) {
        bytes.copy_from_slice(&value.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::super::turn::number_in;
    use super::*;

    fn texts(calls: &[Call]) -> Vec<u16> {
        calls
            .iter()
            .map(|call| match call {
                Call::Text(text) => *text,
                _ => u16::MAX,
            })
            .collect()
    }

    #[test]
    fn numbers_print_by_their_mode() {
        // 94 + digit, 106 the space.
        assert_eq!(texts(&number_in(5, 3, 0)), vec![106, 106, 99]);
        assert_eq!(texts(&number_in(905, 7, 1)), vec![103, 94, 99]);
        assert_eq!(texts(&number_in(7, 3, 2)), vec![94, 94, 101]);
        assert_eq!(texts(&number_in(-3, 2, 1)), vec![105, 97]);
    }

    #[test]
    fn money_and_counts_stop_at_their_most() {
        let mut state = vec![0u8; 16];
        set_word(&mut state, 0, MOST - 1);
        add(&mut state, 0, 10);
        assert_eq!(word(&state, 0), MOST);
        state[8] = MOST_PARTS;
        count_up(&mut state, 8, MOST_PARTS);
        assert_eq!(state[8], MOST_PARTS);
    }
}
