//! Deck commands in battle: the round menu's コマンド (task `0x0803B7D0` in
//! slot 5): the battle's deck, the Zoid a command needs, its check and
//! effect, the message that issues it, its description and its displays.
//!
//! A command is taken out of the battle's deck once issued. Its record at
//! ROM `0x683AC0` (twelve bytes a command) holds flags (`0x100`: it needs
//! a Zoid of the party) and two display descriptors, each played by the
//! effect task (`0x0803C138`): its low bits pick the units, its high bits
//! the display. The command's routine (the table at ROM `0x683C4C`) checks
//! it and makes its changes as it is chosen: bits of the battle's flags,
//! of the units' state and effects that last the round.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the task
//! (`0x0803B7D0`), the effect task (`0x0803C138`), the command routines
//! (`0x0803CAAC` to `0x0803DBCC`), their helpers (`0x080335B4`,
//! `0x08033628`, `0x08032B54`, `0x08032AC4`), the displays (`0x0802C420`,
//! `0x0802CA54`, `0x0802DE74`, `0x0802DCB4`, `0x0802E40C`) and the
//! round's start and end (`0x0802F5D8`, `0x08033570`); checked against a
//! battle on the world map in a reference emulator, commands put in the
//! battle's deck by hand. See `docs/combat.md`.

use super::ai::{ENEMY, PARTY};
use super::attack::{self, Blow};
use super::turn::{Glow, Task};
use super::units::{BattleUnit, Effect};
use super::{Act, Call, Combat};
use extraction::revision::locate;
use extraction::saga_combat::SLOTS;
use platform::Button;

/// The deck's slots.
pub(super) const DECK_SLOTS: usize = 6;
const EMPTY: u8 = 0xFF;
const DECK_WINDOW: u8 = 2;
// Scripts of the battle-menu table.
const MENU_RESET: u16 = 1;
const MENU_MESSAGE_WINDOW: u16 = 2;
const MENU_PRESENT: u16 = 5;
const MENU_DRAW: u16 = 6;
const MENU_CLEAR: u16 = 7;
const MENU_PRESENT_DECK: u16 = 9;
const MENU_CLEAR_DECK: u16 = 0xB;
const MENU_DECK: u16 = 0xF;
const MENU_OPEN_DECK: u16 = 0x10;
const MENU_KEY: u16 = 0x14;
// battle-text: a line break, the player's name, は「, 」を発令しました,
// ゾイドを選択してください, the digits from ０, ：, －－－－－－－, and the
// errors from {name}がいません.
const TEXT_NEW_LINE: u16 = 0;
const TEXT_PLAYER: u16 = 0x33;
const TEXT_ISSUED_BEFORE: u16 = 0x21;
const TEXT_ISSUED_AFTER: u16 = 0x22;
const TEXT_WHICH: u16 = 0x1E;
const TEXT_DIGITS: u16 = 94;
const TEXT_COLON: u16 = 0x24;
const TEXT_EMPTY: u16 = 0x23;
const TEXT_ERRORS: u16 = 0x34;
/// The commands' names (`item` 77 + n) and descriptions (`item` 110 + n).
const COMMAND_NAMES: u16 = 77;
const COMMAND_HELP: u16 = 110;
const SOUND_TAKEN: u16 = 0x3E;
const SOUND_BACK: u16 = 0x3F;
const SOUND_MOVE: u16 = 0x40;
const SOUND_ISSUED: u16 = 0x54;
/// What the deck's menu leaves in var 0 (`battle-menu` 0xF, a `MoveMenu`
/// of mode 6).
const CHOSE: u16 = 1;
const CANCELLED: u16 = 0;
/// The command records (ROM `0x683AC0`): flags, then the two displays'
/// descriptors.
pub(super) const RECORDS: usize = 0x0068_3AC0;
pub(super) const RECORD_SIZE: usize = 12;
pub(super) const COMMANDS: usize = 33;
const NEEDS_ZOID: u32 = 0x100;
// The descriptors' units: both sides (the party's first), the party's
// (the enemy's otherwise), the chosen Zoid, all, the player, the front row,
// the back row, the swimmers, the fliers, all but the L size, all but the S
// size, all but the chosen Zoid, all but the player.
const BOTH_SIDES: u32 = 4;
const PARTY_SIDE: u32 = 1;
const ALL: u32 = 0x10;
const PLAYER: u32 = 0x20;
const FRONT: u32 = 0x40;
const BACK: u32 = 0x80;
const SWIMMERS: u32 = 0x100;
const FLIERS: u32 = 0x200;
const NOT_LARGE: u32 = 0x400;
const NOT_SMALL: u32 = 0x800;
const NOT_CHOSEN: u32 = 0x1000;
const NOT_PLAYER: u32 = 0x2000;
// The descriptors' displays.
const SHOW_LOWERED: u32 = 0x8000_0000;
const SHOW_RAISED: u32 = 0x4000_0000;
const SHOW_STOPPED: u32 = 0x2000_0000;
const SHOW_SACRIFICE: u32 = 0x1000_0000;
const SHOW_REVIVAL: u32 = 0x0800_0000;
const SHOW_MARKED: u32 = 0x0400_0000;
const SHOW_REPAIRED: u32 = 0x0200_0000;
/// The battle's flags (`0x0200EB84 + 2`), which the round's end clears: Zi
/// data, twice the money and twice the experience for a win this round,
/// the slowest first, a random order, no fighting weapons, only fighting
/// weapons.
pub(super) const ZI_DATA: u16 = 1;
pub(super) const DOUBLE_MONEY: u16 = 2;
pub(super) const DOUBLE_EXPERIENCE: u16 = 4;
pub(super) const SLOWEST_FIRST: u16 = 8;
pub(super) const RANDOM_ORDER: u16 = 0x10;
pub(super) const NO_MELEE: u16 = 0x20;
pub(super) const MELEE_ONLY: u16 = 0x40;
/// The units' round state (`+0`): cannot act, cannot be hit, always hits,
/// spends twice the energy.
pub(super) const STOPPED: u16 = 1;
const UNTOUCHABLE: u16 = 2;
const SURE_TO_HIT: u16 = 4;
pub(super) const DOUBLE_ENERGY: u16 = 8;
/// A unit's traits: paralysed, flying, swimming.
const PARALYSED: u16 = 0x4000;
const FLYING: u16 = 2;
const SWIMMING: u16 = 4;
/// The sizes (`+0x3D`).
const SMALL: u8 = 0;
const LARGE: u8 = 2;
// The effects' changes (see `Effect::changes`), bit 0 raising.
const POWER_UP: u16 = 0x11;
const ACCURACY_UP: u16 = 0x21;
const DEFENSE_UP: u16 = 0x101;
const DEFENSE_DOWN: u16 = 0x102;
const SPEED_UP: u16 = 0x801;
const SPEED_DOWN: u16 = 0x802;
/// Effects in percent on the unit's statistics, and on its pilot's.
const OWN_PERCENT: u8 = 0;
const PILOT_PERCENT: u8 = 1;
/// The frames between the display's end and the task's report, in the
/// effect task and the command's (states `0x1B58` to `0x2328`), measured:
/// the sacrifice's display ends a step sooner than the glows'.
const DISPLAY_HOPS: u32 = 1;
const SACRIFICE_HOPS: u32 = 0;

/// The command task's states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CommandStep {
    /// State 0, which goes on to 100 in the next frame.
    Start,
    /// State 100: the deck's window, the slot's description and the deck.
    Open,
    /// 0x44C: the deck's menu, and what it returned.
    Choose,
    AwaitChoice,
    /// 2000 and 0x834: the Zoid the command needs.
    Target,
    ChooseTarget,
    /// 0xBB8: the command's check and changes; 4000, the link battles'.
    Check,
    Link,
    /// 0x1B58: the check failed, its message and the key.
    Failed(u8),
    AwaitFailed,
    /// 0x1004 and 0x100E: 〜は「〜」を発令しました and the wait.
    Issue,
    AwaitIssue,
    /// 0x1068: the description.
    Describe,
    /// 0x1388 and 0x1450: display `n`; its passes, side by side, the
    /// second a frame after the first ends.
    Show(usize),
    AwaitShow(usize, usize),
    SecondPass(usize),
    /// A sacrifice's display runs from the frame after (`0x0802DCB4`).
    Sacrifice(usize),
    /// 0xBB8: a revival on a side whose back row moved up waits for its
    /// rows to move back (`0x0802E144`); 0xC1C, the revival's display.
    Retreat(usize, super::rows::Retreat),
    Revive(usize),
    /// Frames since the display's end.
    Shown(usize, u32),
    /// 0x2328: issued; 0x238C: given up; then the windows close and the
    /// task reports.
    Issued,
    Cancelled,
    Closed(bool),
}

/// The round's commands: the battle's deck (`0x0200EB84 + 0x10D4`, the
/// game state's at the start, a command taken out once issued), the
/// command records (ROM `0x683AC0`), the round's flags (`0x0200EB84 + 2`),
/// the task's menu and result, and the units' weapons before the round's
/// commands took some away.
#[derive(Debug, Clone, Default)]
pub(super) struct CommandState {
    pub(super) deck: [u8; DECK_SLOTS],
    pub(super) records: Vec<[u32; 3]>,
    pub(super) flags: u16,
    pub(super) menu: CommandMenu,
    pub(super) issued: Option<bool>,
    pub(super) round_weapons: Option<[[[Option<super::units::Weapon>; SLOTS]; SLOTS]; 2]>,
}

impl CommandState {
    /// The deck the game state holds and the records of `rom`.
    pub(super) fn new(state: &[u8], rom: &[u8]) -> Self {
        Self {
            deck: deck_of(state),
            records: command_records(rom),
            ..Self::default()
        }
    }
}

/// The game state's deck (`+0x349C`, `0x0802BC6C`).
pub(super) fn deck_of(state: &[u8]) -> [u8; DECK_SLOTS] {
    std::array::from_fn(|slot| state.get(super::deck::DECK + slot).copied().unwrap_or(0xFF))
}

/// The command records at ROM `0x683AC0`.
fn command_records(rom: &[u8]) -> Vec<[u32; 3]> {
    (0..COMMANDS)
        .map(|command| {
            let at = locate(rom, RECORDS + command * RECORD_SIZE);
            std::array::from_fn(|index| {
                rom.get(at + index * 4..at + index * 4 + 4)
                    .and_then(|bytes| bytes.try_into().ok())
                    .map_or(0, u32::from_le_bytes)
            })
        })
        .collect()
}

/// The side display `descriptor`'s pass `pass` takes (0 the party's first
/// when it takes both sides).
fn display_side(descriptor: u32, pass: usize) -> usize {
    if descriptor & BOTH_SIDES != 0 {
        pass.min(1)
    } else if descriptor & PARTY_SIDE != 0 {
        PARTY
    } else {
        ENEMY
    }
}

/// The command task's own state: the deck, the cursor, the command and
/// the Zoid.
#[derive(Debug, Clone, Default)]
pub(super) struct CommandMenu {
    line: usize,
    /// The command whose description the message window shows.
    shown: Option<u8>,
    command: u8,
    target: usize,
}

impl Combat {
    /// A frame of the command task (`0x0803B7D0`).
    pub(super) fn step_command(&mut self, step: CommandStep) -> Task {
        match step {
            CommandStep::Start => Task::Command(CommandStep::Open),
            CommandStep::Open => {
                self.command_state.menu = CommandMenu::default();
                for script in [MENU_OPEN_DECK, MENU_PRESENT_DECK] {
                    self.acts.push_back(Act::Call(Call::Menu(script)));
                }
                self.describe_slot();
                self.print_deck();
                Task::Command(CommandStep::Choose)
            }
            CommandStep::Choose => {
                self.acts.push_back(Act::Call(Call::Menu(MENU_DECK)));
                self.acts.push_back(Act::CommandChoice);
                Task::Command(CommandStep::AwaitChoice)
            }
            CommandStep::AwaitChoice => Task::Command(CommandStep::AwaitChoice),
            CommandStep::Target => {
                self.command_state.menu.target = self.first_fighting();
                self.targeting = Some(self.command_state.menu.target);
                self.palette_writes += 1;
                self.message(&[Call::Text(TEXT_WHICH)]);
                self.acts.pop_back();
                Task::Command(CommandStep::ChooseTarget)
            }
            CommandStep::ChooseTarget => self.choose_command_target(),
            CommandStep::Check => {
                let command = self.command_state.menu.command;
                if let Some(slot) = self
                    .command_state
                    .deck
                    .iter_mut()
                    .find(|slot| **slot == command)
                {
                    *slot = EMPTY;
                }
                match self.apply_command(command) {
                    Some(error) => Task::Command(CommandStep::Failed(error)),
                    None => Task::Command(CommandStep::Link),
                }
            }
            CommandStep::Link => Task::Command(CommandStep::Issue),
            CommandStep::Failed(error) => {
                self.acts.push_back(Act::Call(Call::Menu(MENU_DRAW)));
                self.acts.push_back(Act::Call(Call::Menu(MENU_CLEAR)));
                self.acts
                    .push_back(Act::Call(Call::Text(TEXT_ERRORS + u16::from(error))));
                self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
                self.acts.push_back(Act::Call(Call::Menu(MENU_KEY)));
                Task::Command(CommandStep::AwaitFailed)
            }
            CommandStep::AwaitFailed => Task::Command(CommandStep::Cancelled),
            CommandStep::Issue => {
                let command = u16::from(self.command_state.menu.command);
                self.sounds.push(SOUND_ISSUED);
                self.acts.push_back(Act::Call(Call::Menu(MENU_CLEAR)));
                self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
                for call in [
                    Call::Text(TEXT_PLAYER),
                    Call::Text(TEXT_ISSUED_BEFORE),
                    Call::Item(COMMAND_NAMES + command),
                    Call::Text(TEXT_ISSUED_AFTER),
                ] {
                    self.acts.push_back(Act::Call(call));
                }
                self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
                self.acts.push_back(Act::Wait);
                Task::Command(CommandStep::AwaitIssue)
            }
            CommandStep::AwaitIssue => {
                if self.wait_done() {
                    Task::Command(CommandStep::Describe)
                } else {
                    Task::Command(CommandStep::AwaitIssue)
                }
            }
            CommandStep::Describe => {
                let command = u16::from(self.command_state.menu.command);
                self.acts.push_back(Act::Call(Call::Menu(MENU_RESET)));
                self.acts
                    .push_back(Act::Call(Call::Menu(MENU_MESSAGE_WINDOW)));
                self.acts
                    .push_back(Act::Call(Call::Item(COMMAND_HELP + command)));
                self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
                Task::Command(CommandStep::Show(0))
            }
            _ => self.step_command_display(step),
        }
    }

    /// A frame of the command task's displays and its end (states `0x1388`
    /// to `0x238C`).
    fn step_command_display(&mut self, step: CommandStep) -> Task {
        match step {
            CommandStep::Show(display) => {
                self.figures_held = true;
                let descriptor = self.descriptor(display);
                if descriptor == 0 {
                    return Task::Command(CommandStep::Shown(display, DISPLAY_HOPS));
                }
                if descriptor & SHOW_SACRIFICE != 0 {
                    return Task::Command(CommandStep::Sacrifice(display));
                }
                if descriptor & SHOW_REVIVAL != 0 {
                    let side = display_side(descriptor, 0);
                    if self.fight.advanced[side] && self.retreat_rows(side) {
                        let moving = super::rows::Retreat::Moving(side, 0);
                        return Task::Command(CommandStep::Retreat(display, moving));
                    }
                }
                self.start_command_display(descriptor, 0);
                // A display with no unit to show ends at once.
                if self.fight.display.is_none() && descriptor & BOTH_SIDES == 0 {
                    return Task::Command(CommandStep::Shown(display, 0));
                }
                Task::Command(CommandStep::AwaitShow(display, 0))
            }
            CommandStep::Sacrifice(display) | CommandStep::Revive(display) => {
                self.start_command_display(self.descriptor(display), 0);
                Task::Command(CommandStep::AwaitShow(display, 0))
            }
            CommandStep::Retreat(display, retreat) => match self.step_retreat(retreat) {
                Some(retreat) => Task::Command(CommandStep::Retreat(display, retreat)),
                None => Task::Command(CommandStep::Revive(display)),
            },
            CommandStep::AwaitShow(display, pass) => {
                if self.fight.display.is_some() {
                    self.step_report();
                    return Task::Command(CommandStep::AwaitShow(display, pass));
                }
                // The sacrificed units leave the battle (`0x0802C014`).
                if self.descriptor(display) & SHOW_SACRIFICE != 0 {
                    for unit in self.sides.iter_mut().flatten().flatten() {
                        if unit.traits & attack::DESTROYED != 0 {
                            unit.traits |= super::units::OUT;
                        }
                    }
                }
                if pass == 0 && self.descriptor(display) & BOTH_SIDES != 0 {
                    return Task::Command(CommandStep::SecondPass(display));
                }
                Task::Command(CommandStep::Shown(display, 0))
            }
            CommandStep::SecondPass(display) => {
                self.start_command_display(self.descriptor(display), 1);
                Task::Command(CommandStep::AwaitShow(display, 1))
            }
            CommandStep::Shown(display, frames) if frames < self.display_hops(display) => {
                Task::Command(CommandStep::Shown(display, frames + 1))
            }
            CommandStep::Shown(0, _) => Task::Command(CommandStep::Show(1)),
            CommandStep::Shown(..) => Task::Command(CommandStep::Issued),
            CommandStep::Issued => {
                self.figures_held = false;
                // A party's display has each fighting unit's panel shown
                // again and lit in turn (`0x0803CA20`).
                if self
                    .fight
                    .blows
                    .first()
                    .is_some_and(|blow| blow.side == PARTY)
                {
                    for slot in 0..SLOTS {
                        if self.command_fighting(PARTY, slot) {
                            self.refresh_panel(slot);
                            self.light_panel(slot);
                        }
                    }
                    self.panels_held = Some(super::MESSAGE_WINDOW);
                }
                self.close_command_task();
                Task::Command(CommandStep::Closed(true))
            }
            CommandStep::Cancelled => {
                self.close_command_task();
                Task::Command(CommandStep::Closed(false))
            }
            CommandStep::Closed(issued) => {
                self.command_state.issued = Some(issued);
                Task::Reported
            }
            _ => Task::Command(step),
        }
    }

    /// The command's record: its flags and its two displays.
    fn command_record(&self, command: u8) -> [u32; 3] {
        self.command_state
            .records
            .get(usize::from(command))
            .copied()
            .unwrap_or_default()
    }

    fn display_hops(&self, display: usize) -> u32 {
        if self.descriptor(display) & SHOW_SACRIFICE != 0 {
            SACRIFICE_HOPS
        } else {
            DISPLAY_HOPS
        }
    }

    fn descriptor(&self, display: usize) -> u32 {
        self.command_record(self.command_state.menu.command)[1 + display.min(1)]
    }

    /// The description of the command in the slot under the cursor, or
    /// none for an empty slot, when it is not the one shown (`0x0803BFE8`).
    fn describe_slot(&mut self) {
        let command = self
            .command_state
            .deck
            .get(self.command_state.menu.line)
            .copied()
            .filter(|&command| command != EMPTY);
        if command == self.command_state.menu.shown {
            return;
        }
        self.command_state.menu.shown = command;
        self.acts.push_back(Act::Call(Call::Menu(MENU_CLEAR)));
        self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
        if let Some(command) = command {
            self.acts
                .push_back(Act::Call(Call::Item(COMMAND_HELP + u16::from(command))));
            self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
        }
    }

    /// The deck (`0x0803C05C`): per slot its number, ：, and its command's
    /// name or －－－－－－－.
    fn print_deck(&mut self) {
        self.acts.push_back(Act::Call(Call::Menu(MENU_CLEAR_DECK)));
        self.acts
            .push_back(Act::Call(Call::Menu(MENU_PRESENT_DECK)));
        self.acts.push_back(Act::PrintWindow(DECK_WINDOW));
        for slot in 0..DECK_SLOTS {
            if slot > 0 {
                self.acts.push_back(Act::Call(Call::Text(TEXT_NEW_LINE)));
            }
            let digit = TEXT_DIGITS + u16::try_from(slot + 1).unwrap_or(0);
            self.acts.push_back(Act::Call(Call::Text(digit)));
            self.acts.push_back(Act::Call(Call::Text(TEXT_COLON)));
            let call = match self
                .command_state
                .deck
                .get(slot)
                .copied()
                .filter(|&command| command != EMPTY)
            {
                Some(command) => Call::Item(COMMAND_NAMES + u16::from(command)),
                None => Call::Text(TEXT_EMPTY),
            };
            self.acts.push_back(Act::Call(call));
        }
        self.acts.push_back(Act::PrintWindow(super::MESSAGE_WINDOW));
        self.acts
            .push_back(Act::Call(Call::Menu(MENU_PRESENT_DECK)));
    }

    /// What the deck's menu returned, as its script ends, going on at once
    /// as the original's menu does in the key's frame.
    pub(super) fn take_command_choice(&mut self) {
        if self.task != Task::Command(CommandStep::AwaitChoice) {
            return;
        }
        let [code, line, ..] = *self.menu.vars();
        self.task = match code {
            CHOSE => {
                self.sounds.push(SOUND_TAKEN);
                self.command_state.menu.line = usize::from(line);
                match self
                    .command_state
                    .deck
                    .get(self.command_state.menu.line)
                    .copied()
                {
                    Some(command) if command != EMPTY => {
                        self.command_state.menu.command = command;
                        let [flags, ..] = self.command_record(command);
                        if flags & NEEDS_ZOID != 0 {
                            self.step_command(CommandStep::Target)
                        } else {
                            Task::Command(CommandStep::Check)
                        }
                    }
                    _ => self.step_command(CommandStep::Choose),
                }
            }
            CANCELLED => {
                self.sounds.push(SOUND_BACK);
                self.step_command(CommandStep::Cancelled)
            }
            _ => {
                self.command_state.menu.line = usize::from(line);
                self.acts.push_back(Act::Call(Call::Menu(MENU_DRAW)));
                self.describe_slot();
                self.step_command(CommandStep::Choose)
            }
        };
    }

    /// A frame of the Zoid's choice (state `0x834`): the pad moves the
    /// marker, A takes the Zoid, B goes back to the deck.
    fn choose_command_target(&mut self) -> Task {
        let pressed = |button: Button| self.input.is_held(button) && !self.previous.is_held(button);
        let way = [Button::Up, Button::Down, Button::Left, Button::Right]
            .into_iter()
            .position(pressed);
        if let Some(way) = way {
            let before = self.command_state.menu.target;
            let next = self
                .neighbour_slots
                .get(before)
                .and_then(|ways| ways.get(way))
                .and_then(|slots| {
                    slots
                        .iter()
                        .map(|&slot| usize::from(slot))
                        .find(|&slot| self.command_fighting(PARTY, slot))
                });
            if let Some(next) = next {
                self.command_state.menu.target = next;
            }
            if self.command_state.menu.target != before {
                self.sounds.push(SOUND_MOVE);
            }
            self.targeting = Some(self.command_state.menu.target);
            self.palette_writes += 1;
        }
        if pressed(Button::A) {
            self.sounds.push(SOUND_TAKEN);
            self.targeting = None;
            return Task::Command(CommandStep::Check);
        }
        if pressed(Button::B) {
            self.sounds.push(SOUND_BACK);
            self.targeting = None;
            self.command_state.menu.shown = None;
            self.describe_slot();
            self.print_deck();
            return Task::Command(CommandStep::Choose);
        }
        Task::Command(CommandStep::ChooseTarget)
    }

    fn command_fighting(&self, side: usize, slot: usize) -> bool {
        self.sides[side][slot]
            .as_ref()
            .is_some_and(BattleUnit::fighting)
    }

    fn first_fighting(&self) -> usize {
        (0..SLOTS)
            .find(|&slot| self.command_fighting(PARTY, slot))
            .unwrap_or(0)
    }

    /// The player's slot on `side` (`0x08032AC4`): the unit of character 0.
    fn player_slot(&self, side: usize) -> Option<usize> {
        (0..SLOTS).find(|&slot| {
            self.sides[side][slot]
                .as_ref()
                .is_some_and(|unit| unit.character == 0)
        })
    }

    /// The command's routine (the table at ROM `0x683C4C`): its check,
    /// which gives an error (1 to 4: no player, the player not in the
    /// formation, no front row, no back row), and its changes.
    fn apply_command(&mut self, command: u8) -> Option<u8> {
        match command {
            0 => self.command_state.flags |= ZI_DATA,
            1 => self.command_state.flags |= DOUBLE_EXPERIENCE,
            2 => self.command_state.flags |= DOUBLE_MONEY,
            9 => self.command_state.flags |= NO_MELEE,
            10 => self.command_state.flags |= MELEE_ONLY,
            15 => self.command_state.flags |= SLOWEST_FIRST,
            16 => self.command_state.flags |= RANDOM_ORDER,
            3..=5 | 19..=22 => return self.apply_party_command(command),
            6..=8 | 11..=14 | 17 | 18 => return self.apply_stop_command(command),
            _ => self.apply_effect_command(command),
        }
        None
    }

    /// The commands on the party's hit points and on the player.
    fn apply_party_command(&mut self, command: u8) -> Option<u8> {
        let chosen = self.command_state.menu.target;
        match command {
            3 => {
                let Ok(player) = self.fighting_player() else {
                    return Some(1);
                };
                self.set_status(PARTY, player, STOPPED);
                for slot in self.fighting_slots(PARTY, 0..SLOTS) {
                    self.pilot_twice(PARTY, slot);
                }
            }
            4 => {
                if self.fighting_player().is_err() {
                    return Some(1);
                }
            }
            5 => {
                if extraction::saga_party::formation_slot(&self.state, 0).is_none() {
                    return Some(2);
                }
            }
            19 => {
                for slot in self.fighting_slots(PARTY, 0..SLOTS) {
                    if let Some(unit) = self.sides[PARTY][slot].as_mut() {
                        let amount = unit.max_hp >> 2;
                        unit.hp = unit.hp.saturating_add(amount).min(unit.max_hp.max(unit.hp));
                    }
                }
            }
            20 => {
                let Ok(player) = self.fighting_player() else {
                    return Some(1);
                };
                self.set_status(PARTY, player, STOPPED);
                for slot in self.fighting_slots(PARTY, 0..SLOTS) {
                    if let Some(unit) = self.sides[PARTY][slot].as_mut() {
                        unit.traits &= !PARALYSED;
                        unit.hp = unit.max_hp;
                    }
                }
            }
            21 => {
                let player = self.player_slot(PARTY);
                self.set_status(PARTY, chosen, STOPPED);
                let player = player.filter(|&slot| self.command_fighting(PARTY, slot));
                let Some(player) = player else {
                    return Some(1);
                };
                if let Some(unit) = self.sides[PARTY][player].as_mut() {
                    unit.traits &= !PARALYSED;
                    unit.hp = unit.max_hp;
                }
                self.pilot_twice(PARTY, player);
            }
            _ => {
                for slot in self.fighting_slots(PARTY, 0..SLOTS) {
                    self.set_status(PARTY, slot, STOPPED);
                }
                if let Some(unit) = self.sides[PARTY][chosen].as_mut() {
                    unit.ep = unit.ep.saturating_add(unit.max_ep >> 1).min(unit.max_ep);
                }
            }
        }
        None
    }

    /// The commands that stop units for the round.
    fn apply_stop_command(&mut self, command: u8) -> Option<u8> {
        let chosen = self.command_state.menu.target;
        match command {
            6 | 7 => {
                let trait_bit = if command == 6 { FLYING } else { SWIMMING };
                self.stop_where(|unit| unit.traits & trait_bit != 0);
            }
            8 => {
                for side in [PARTY, ENEMY] {
                    for slot in self.fighting_slots(side, 0..SLOTS) {
                        if (side, slot) != (PARTY, chosen) {
                            self.set_status(side, slot, STOPPED);
                        }
                    }
                }
            }
            11 => {
                self.set_status(PARTY, chosen, STOPPED);
                self.stop_row(ENEMY, 3..SLOTS);
            }
            12..=14 => {
                if !self.stop_row(PARTY, 3..SLOTS) {
                    return Some(4);
                }
                match command {
                    12 => self.affect_side(PARTY, 0..3, POWER_UP, 100),
                    13 => self.affect_side(PARTY, 0..SLOTS, DEFENSE_UP, 100),
                    _ => self.affect_side(ENEMY, 0..SLOTS, DEFENSE_DOWN, 50),
                }
            }
            17 => self.stop_where(|unit| unit.size != LARGE),
            _ => self.stop_where(|unit| unit.size != SMALL),
        }
        None
    }

    /// The commands that change statistics and the units' round state.
    fn apply_effect_command(&mut self, command: u8) {
        let chosen = self.command_state.menu.target;
        match command {
            23 => self.set_status(PARTY, chosen, UNTOUCHABLE),
            24 => {
                self.affect_side(PARTY, 0..SLOTS, SPEED_UP, 100);
                self.affect_side(PARTY, 0..SLOTS, DEFENSE_DOWN, 50);
            }
            25 => {
                self.affect_side(PARTY, 0..SLOTS, SPEED_DOWN, 50);
                self.affect_side(ENEMY, 0..SLOTS, SPEED_DOWN, 50);
            }
            26 => {
                self.affect_side(PARTY, 0..SLOTS, DEFENSE_DOWN, 50);
                self.affect_side(ENEMY, 0..SLOTS, SPEED_DOWN, 50);
            }
            27 => {
                for slot in self.fighting_slots(PARTY, 0..SLOTS) {
                    self.set_status(PARTY, slot, DOUBLE_ENERGY);
                }
                self.affect_side(ENEMY, 0..SLOTS, DEFENSE_DOWN, 100);
            }
            28 => {
                self.set_status(PARTY, chosen, STOPPED);
                self.affect_side(ENEMY, 0..SLOTS, DEFENSE_DOWN, 50);
            }
            29 => self.affect_side(PARTY, 0..SLOTS, POWER_UP, 50),
            30 => {
                self.affect_side(PARTY, 0..SLOTS, POWER_UP, 100);
                self.affect_side(PARTY, 0..SLOTS, SPEED_DOWN, 50);
            }
            31 => self.set_status(PARTY, chosen, SURE_TO_HIT),
            _ => {
                self.affect_side(PARTY, 0..SLOTS, SPEED_DOWN, 50);
                for slot in self.fighting_slots(PARTY, 0..SLOTS) {
                    self.set_status(PARTY, slot, SURE_TO_HIT);
                }
            }
        }
    }

    /// The player's slot while it fights; error 1 otherwise.
    fn fighting_player(&self) -> Result<usize, u8> {
        self.player_slot(PARTY)
            .filter(|&slot| self.command_fighting(PARTY, slot))
            .ok_or(1)
    }

    fn fighting_slots(&self, side: usize, slots: std::ops::Range<usize>) -> Vec<usize> {
        slots
            .filter(|&slot| self.command_fighting(side, slot))
            .collect()
    }

    fn set_status(&mut self, side: usize, slot: usize, bit: u16) {
        if let Some(unit) = self.sides[side][slot].as_mut() {
            unit.status |= bit;
        }
    }

    /// Stops the fighting units of both sides `stopped` picks.
    fn stop_where(&mut self, stopped: impl Fn(&BattleUnit) -> bool) {
        for side in [PARTY, ENEMY] {
            for slot in self.fighting_slots(side, 0..SLOTS) {
                if self.sides[side][slot].as_ref().is_some_and(&stopped) {
                    self.set_status(side, slot, STOPPED);
                }
            }
        }
    }

    /// Stops a row's fighting units (`0x080335B4`): whether there were any.
    fn stop_row(&mut self, side: usize, slots: std::ops::Range<usize>) -> bool {
        let row = self.fighting_slots(side, slots);
        for &slot in &row {
            self.set_status(side, slot, STOPPED);
        }
        !row.is_empty()
    }

    /// An effect for the round on a side's fighting units (`0x08033628`).
    fn affect_side(
        &mut self,
        side: usize,
        slots: std::ops::Range<usize>,
        changes: u16,
        value: u16,
    ) {
        for slot in self.fighting_slots(side, slots) {
            self.affect_unit(side, slot, changes, value, OWN_PERCENT);
        }
    }

    fn affect_unit(&mut self, side: usize, slot: usize, changes: u16, value: u16, kind: u8) {
        if let Some(unit) = self.sides[side][slot].as_mut() {
            unit.affect(Effect {
                changes,
                value,
                kind,
                turns: 1,
                part: 0,
            });
        }
    }

    /// The pilot's bonuses but 耐久 twice for the round (`0x08032B54` four
    /// times).
    fn pilot_twice(&mut self, side: usize, slot: usize) {
        for changes in [POWER_UP, ACCURACY_UP, DEFENSE_UP, SPEED_UP] {
            self.affect_unit(side, slot, changes, 100, PILOT_PERCENT);
        }
    }

    /// Display `descriptor`'s pass on `side` (0 the party's first when it
    /// takes both sides): its units (`0x0803C1E2`) and its display.
    fn start_command_display(&mut self, descriptor: u32, pass: usize) {
        let side = display_side(descriptor, pass);
        let units = self.command_units(descriptor, side);
        self.fight.blows = units
            .into_iter()
            .map(|slot| Blow {
                side,
                slot,
                flags: attack::LANDED,
                damage: 0,
                code: 0,
                kind: attack::REPAIRED,
            })
            .collect();
        if descriptor & SHOW_REPAIRED != 0 {
            self.start_glows(Glow::Mend);
        } else if descriptor & (SHOW_MARKED | SHOW_RAISED) != 0 {
            self.start_glows(Glow::Raise);
        } else if descriptor & SHOW_LOWERED != 0 {
            self.start_glows(Glow::Lower);
        } else if descriptor & SHOW_STOPPED != 0 {
            self.start_glows(Glow::Stop);
        } else if descriptor & SHOW_SACRIFICE != 0 {
            self.start_blasts();
        } else if descriptor & SHOW_REVIVAL != 0 {
            let revived = self.revive(descriptor);
            // Their panels show them whole again, lit (`0x08031618`,
            // `0x08031598`).
            for &slot in &revived {
                self.refresh_panel(slot);
                self.light_panel(slot);
            }
            self.fight.blows = revived
                .into_iter()
                .map(|slot| Blow {
                    side: PARTY,
                    slot,
                    flags: attack::LANDED,
                    damage: 0,
                    code: 0,
                    kind: attack::REPAIRED,
                })
                .collect();
            self.start_glows(Glow::Revive);
            // The revived units stay hidden from the frame the display
            // starts in.
            if let Some(display) = &self.fight.display {
                self.shown_sparks.clone_from(&display.sparks);
            }
        }
    }

    /// The units display `descriptor` takes on `side`.
    fn command_units(&self, descriptor: u32, side: usize) -> Vec<usize> {
        let fighting = |slot: usize| self.command_fighting(side, slot);
        let unit = |slot: usize| self.sides[side][slot].as_ref();
        let player = self.player_slot(side);
        let all: Vec<usize> = (0..SLOTS).filter(|&slot| fighting(slot)).collect();
        let picked = |keep: &dyn Fn(usize) -> bool| -> Vec<usize> {
            all.iter().copied().filter(|&slot| keep(slot)).collect()
        };
        if descriptor & ALL != 0 {
            all
        } else if descriptor & PLAYER != 0 {
            player.filter(|&slot| fighting(slot)).into_iter().collect()
        } else if descriptor & FRONT != 0 {
            picked(&|slot| slot < 3)
        } else if descriptor & BACK != 0 {
            picked(&|slot| slot >= 3)
        } else if descriptor & FLIERS != 0 {
            picked(&|slot| unit(slot).is_some_and(|unit| unit.traits & FLYING != 0))
        } else if descriptor & SWIMMERS != 0 {
            picked(&|slot| unit(slot).is_some_and(|unit| unit.traits & SWIMMING != 0))
        } else if descriptor & NOT_LARGE != 0 {
            picked(&|slot| unit(slot).is_some_and(|unit| unit.size != LARGE))
        } else if descriptor & NOT_SMALL != 0 {
            picked(&|slot| unit(slot).is_some_and(|unit| unit.size != SMALL))
        } else if descriptor & NOT_CHOSEN != 0 {
            picked(&|slot| (side, slot) != (PARTY, self.command_state.menu.target))
        } else if descriptor & NOT_PLAYER != 0 {
            picked(&|slot| Some(slot) != player)
        } else {
            vec![self.command_state.menu.target]
        }
    }

    /// The revival (`0x0803C8CC`, `0x0802E40C`): the party's formation
    /// slots whose unit no longer fights but for the player's, or the
    /// player's slot, fighting or not, have their units built again from
    /// the game state (`0x0802B5D0`): whole, without their effects, their
    /// used-up parts back; the player with its pilot's bonuses twice.
    fn revive(&mut self, descriptor: u32) -> Vec<usize> {
        let slots: Vec<usize> = (0..SLOTS)
            .filter(|&slot| {
                self.revived[slot].as_ref().is_some_and(|unit| {
                    if descriptor & NOT_PLAYER != 0 {
                        unit.character != 0 && !self.command_fighting(PARTY, slot)
                    } else {
                        unit.character == 0
                    }
                })
            })
            .collect();
        for &slot in &slots {
            let mut unit = self.revived[slot].clone();
            if descriptor & PLAYER != 0
                && let Some(unit) = unit.as_mut()
            {
                for value in &mut unit.pilot[1..] {
                    let [low, high, ..] = value.to_le_bytes();
                    let doubled = i16::from_le_bytes([low, high]).wrapping_shl(1);
                    *value =
                        (*value & !0xFFFF) | i32::from(u16::from_le_bytes(doubled.to_le_bytes()));
                }
            }
            self.sides[PARTY][slot] = unit;
            self.units[PARTY][slot].clone_from(&self.revived_figures[slot]);
            self.fight.gone[PARTY][slot] = false;
        }
        slots
    }

    /// The deck's window and the message window close (`battle-menu` 1
    /// and 2).
    fn close_command_task(&mut self) {
        self.acts.push_back(Act::Call(Call::Menu(MENU_RESET)));
        self.acts
            .push_back(Act::Call(Call::Menu(MENU_MESSAGE_WINDOW)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use extraction::saga_party::PILOT_VALUES;

    fn unit(character: u8, size: u8) -> BattleUnit {
        BattleUnit {
            zoid: 0,
            traits: 0,
            status: 0,
            hp: 40,
            ep: 10,
            max_hp: 100,
            max_ep: 30,
            sp: 50,
            df: 10,
            beam_df: 10,
            evasion_bonus: 0,
            pilot: [0; PILOT_VALUES],
            weapons: [None; SLOTS],
            ai: 0,
            experience: 0,
            money: 0,
            effects: Vec::new(),
            parts: [0xFFFF; SLOTS],
            size,
            face: 0,
            character,
        }
    }

    fn combat(party: &[usize]) -> Combat {
        let rom = Vec::new();
        let data = GameData::new(&rom);
        let mut state = vec![0; 0x3F10];
        let mut combat = Combat::new(&data, &mut state, &[0xFF; 36], (0, 0));
        for (index, &slot) in party.iter().enumerate() {
            combat.sides[PARTY][slot] = Some(unit(u8::try_from(index).unwrap_or(0), 1));
        }
        combat.sides[ENEMY][0] = Some(unit(0x55, 2));
        combat
    }

    #[test]
    fn the_spoils_and_order_commands_set_the_rounds_flags() {
        let mut combat = combat(&[0, 1]);
        for command in [0, 1, 2, 9, 15] {
            assert_eq!(combat.apply_command(command), None);
        }
        assert_eq!(
            combat.command_state.flags,
            ZI_DATA | DOUBLE_EXPERIENCE | DOUBLE_MONEY | NO_MELEE | SLOWEST_FIRST
        );
    }

    #[test]
    fn a_back_row_command_needs_a_back_row() {
        let mut without = combat(&[0, 1]);
        assert_eq!(without.apply_command(12), Some(4));
        let mut combat = combat(&[0, 1, 3]);
        assert_eq!(combat.apply_command(12), None);
        let status = |slot: usize| combat.sides[PARTY][slot].as_ref().map(|unit| unit.status);
        assert_eq!(status(3), Some(STOPPED));
        assert_eq!(status(0), Some(0));
        let front = combat.sides[PARTY][0]
            .as_ref()
            .map(|unit| unit.effects.len());
        assert_eq!(front, Some(1));
    }

    #[test]
    fn the_princes_cheer_repairs_a_quarter_of_each_units_hit_points() {
        let mut combat = combat(&[0, 2]);
        assert_eq!(combat.apply_command(19), None);
        let hp = combat.sides[PARTY][2].as_ref().map(|unit| unit.hp);
        assert_eq!(hp, Some(65));
    }

    #[test]
    fn size_commands_stop_both_sides_but_the_size_they_keep() {
        let mut combat = combat(&[0]);
        assert_eq!(combat.apply_command(17), None);
        let party = combat.sides[PARTY][0].as_ref().map(|unit| unit.status);
        let enemy = combat.sides[ENEMY][0].as_ref().map(|unit| unit.status);
        assert_eq!((party, enemy), (Some(STOPPED), Some(0)));
    }

    #[test]
    fn a_revival_first_moves_back_the_rows_that_moved_up() {
        let mut combat = combat(&[]);
        let character = |combat: &Combat, slot: usize| {
            combat.sides[PARTY][slot]
                .as_ref()
                .map(|unit| unit.character)
        };
        combat.sides[PARTY][0] = Some(unit(1, 1));
        combat.fight.advanced[PARTY] = true;
        combat.revived[0] = Some(unit(2, 1));
        combat.revived[1] = Some(unit(0, 1));
        combat.revived[3] = Some(unit(1, 1));
        assert!(combat.retreat_rows(PARTY));
        assert_eq!(
            (character(&combat, 0), character(&combat, 3)),
            (None, Some(1))
        );
        let mut retreat = Some(super::super::rows::Retreat::Moving(PARTY, 0));
        let mut frames = 0;
        while let Some(step) = retreat {
            retreat = combat.step_retreat(step);
            frames += 1;
        }
        assert_eq!(frames, 18);
        assert!(!combat.fight.advanced[PARTY]);
        let revived = combat.revive(SHOW_REVIVAL | NOT_PLAYER | PARTY_SIDE);
        assert_eq!(revived, vec![0]);
        assert_eq!(character(&combat, 0), Some(2));
    }

    #[test]
    fn the_players_revival_builds_it_again_even_while_it_fights() {
        let mut combat = combat(&[0]);
        let mut whole = unit(0, 1);
        whole.hp = whole.max_hp;
        combat.revived[0] = Some(whole);
        let revived = combat.revive(SHOW_REVIVAL | PLAYER | PARTY_SIDE);
        assert_eq!(revived, vec![0]);
        let hp = combat.sides[PARTY][0].as_ref().map(|unit| unit.hp);
        assert_eq!(hp, Some(100));
    }

    #[test]
    fn a_player_command_fails_without_the_player() {
        let mut combat = combat(&[0, 1]);
        if let Some(player) = combat.sides[PARTY][0].as_mut() {
            player.traits |= super::super::units::OUT;
        }
        assert_eq!(combat.apply_command(20), Some(1));
        assert_eq!(combat.apply_command(4), Some(1));
    }

    #[test]
    fn the_rounds_rules_take_weapons_and_give_them_back() {
        let mut combat = combat(&[0]);
        let weapon = |flags: u32| super::super::units::Weapon {
            part: 1,
            flags,
            accuracy: 50,
            power: 10,
            cost: 3,
            reach: 1,
            spread: 0,
            turns: 0,
        };
        if let Some(unit) = combat.sides[PARTY][0].as_mut() {
            unit.weapons[0] = Some(weapon(0x101));
            unit.weapons[1] = Some(weapon(1));
            unit.status = DOUBLE_ENERGY;
        }
        combat.command_state.flags = NO_MELEE;
        combat.apply_round_rules();
        let weapons = combat.sides[PARTY][0].as_ref().map(|unit| unit.weapons);
        assert_eq!(weapons.and_then(|weapons| weapons[0]), None);
        assert_eq!(
            weapons
                .and_then(|weapons| weapons[1])
                .map(|weapon| weapon.cost),
            Some(6)
        );
        combat.end_round();
        let weapons = combat.sides[PARTY][0].as_ref().map(|unit| unit.weapons);
        assert_eq!(
            weapons
                .and_then(|weapons| weapons[0])
                .map(|weapon| weapon.cost),
            Some(3)
        );
        assert_eq!(combat.command_state.flags, 0);
    }
}
