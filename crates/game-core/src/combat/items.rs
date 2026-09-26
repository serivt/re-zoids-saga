//! Items in battle: the action menu's アイテム (task `0x08038FC4` in slot
//! 5): the list of the party's battle items, the unit to use one on, its
//! effect, its display and its message.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the item
//! task (`0x08038FC4`), its list (`0x080393AC`, `0x0803944C`,
//! `0x080394A0`, `0x08039518`), the target's choice (`0x08033844`,
//! `0x08032390` with the table at ROM `0x66BA90`, `0x080385B0`), the use
//! (`0x08039580` and the effects at ROM `0x683AA8`), the repair display
//! and its messages (`0x0802C420`, `0x0802C588`, `0x0802C708`); checked
//! against a battle on the world map in a reference emulator, the party's
//! items set by hand. See `docs/combat.md`.

use super::ai::PARTY;
use super::attack::{self, Blow};
use super::turn::PARALYSED;
use super::turn::{Glow, Report, Task};
use super::units::BattleUnit;
use super::{Act, Call, Combat};
use extraction::saga_combat::SLOTS;
use platform::Button;

/// The battle items: the first six of the item counts (`+0x3305`), less
/// the seventh.
const ITEM_COUNTS: usize = 0x3305;
const ITEMS: u8 = 6;
/// The list shows six a page.
const PAGE_LINES: usize = 6;
/// Where an item's count is printed, after its name.
const COUNT_COLUMN: usize = 9;
/// The list's window (`battle-menu` 0x10 opens it) and the message
/// window.
pub(super) const LIST_WINDOW: u8 = 2;
const MESSAGE_WINDOW: u8 = 1;
// Scripts of the battle-menu table.
const MENU_RESET: u16 = 1;
const MENU_MESSAGE_WINDOW: u16 = 2;
const MENU_PRESENT: u16 = 5;
const MENU_DRAW: u16 = 6;
const MENU_CLEAR: u16 = 7;
const MENU_SHOW_LIST: u16 = 9;
const MENU_CLEAR_LIST: u16 = 0xB;
const MENU_CHOOSE: u16 = 0xF;
const MENU_OPEN_LIST: u16 = 0x10;
/// The texts: battle-text 0 (a new line), 27 (どのゾイドに使いますか？),
/// a full-width space (the digits' 12th); the items' names (`name` 241 on),
/// help (`item` 63 on) and the messages of those that repair nothing
/// (`item` 70 on).
const TEXT_NEW_LINE: u16 = 0;
const TEXT_WHICH: u16 = 27;
const TEXT_SPACE: u16 = 106;
const ITEM_NAMES: u16 = 241;
const ITEM_HELP: u16 = 63;
const ITEM_MESSAGES: u16 = 70;
/// Frames from a message's last wait to the item task's end.
const MESSAGE_TAIL: u32 = 4;
const SOUND_TAKEN: u16 = 0x3E;
const SOUND_BACK: u16 = 0x3F;
const SOUND_MOVE: u16 = 0x40;
/// What the menu's script leaves in var 0 (`battle-menu` 0xF, a
/// `MoveMenu` of mode 6): A, B, or a move up or down.
const CHOSE: u16 = 1;
const CANCELLED: u16 = 0;

/// The item task's states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemStep {
    /// State 0: the list's window opens.
    Open,
    /// 1000: the page's lines, then the menu.
    Page,
    /// 0x44C: the list's menu, and what it returned.
    Choose,
    AwaitChoice,
    /// 2000 and 0x834: the unit to use it on.
    Target,
    ChooseTarget,
    /// 0x898: the figures hide and the repair's glow starts; 0x8FC: it
    /// plays.
    Use,
    Display,
    /// 0xBB8 and 0x960: the message.
    Message,
    AwaitMessage,
    /// 0x2328: the item is used; 0x238C: given up; then the windows close
    /// and the task reports.
    Used,
    Cancelled,
    Closed(bool),
}

/// The item task's own state beside its step: the list, the cursor, the
/// item and the unit.
#[derive(Debug, Clone, Default)]
pub(super) struct ItemMenu {
    /// The items, page by page (`0x0200E8AA`).
    list: Vec<u8>,
    page: usize,
    line: usize,
    /// The item whose help the message window shows (`0x0200E8A8`).
    shown: Option<u8>,
    item: u8,
    target: usize,
}

impl Combat {
    /// A frame of the item task (`0x08038FC4`).
    pub(super) fn step_item(&mut self, step: ItemStep) -> Task {
        match step {
            ItemStep::Open => {
                for script in [MENU_CLEAR, MENU_PRESENT, MENU_OPEN_LIST, MENU_SHOW_LIST] {
                    self.acts.push_back(Act::Call(Call::Menu(script)));
                }
                self.item_menu = ItemMenu {
                    list: self.battle_items(),
                    ..ItemMenu::default()
                };
                Task::Item(ItemStep::Page)
            }
            ItemStep::Page => {
                self.show_help();
                self.draw_page();
                Task::Item(ItemStep::Choose)
            }
            ItemStep::Choose => {
                self.acts.push_back(Act::Call(Call::Menu(MENU_CHOOSE)));
                self.acts.push_back(Act::ItemChoice);
                Task::Item(ItemStep::AwaitChoice)
            }
            ItemStep::AwaitChoice => Task::Item(ItemStep::AwaitChoice),
            ItemStep::Target => {
                self.item_menu.target = self.first_target();
                self.targeting = Some(self.item_menu.target);
                self.palette_writes += 1;
                self.message(&[Call::Text(TEXT_WHICH)]);
                self.acts.pop_back();
                Task::Item(ItemStep::ChooseTarget)
            }
            ItemStep::ChooseTarget => self.choose_target(),
            ItemStep::Use => {
                self.figures_held = true;
                self.start_glows(Glow::Mend);
                Task::Item(ItemStep::Display)
            }
            ItemStep::Display => {
                if self.fight.display.is_some() {
                    self.step_item_display();
                    return Task::Item(ItemStep::Display);
                }
                self.step_item(ItemStep::Message)
            }
            ItemStep::Message => {
                if self.item_menu.item <= 2 || self.item_menu.item == 5 {
                    self.report_repair(0);
                } else {
                    let mut calls = self.unit_name(Some((PARTY, self.item_menu.target)));
                    calls.push(Call::Item(ITEM_MESSAGES + u16::from(self.item_menu.item)));
                    self.acts.push_back(Act::Call(Call::Menu(MENU_CLEAR)));
                    self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
                    for call in calls {
                        self.acts.push_back(Act::Call(call));
                    }
                    self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
                    self.acts.push_back(Act::Wait);
                    self.fight.report = Some(Report::Item);
                }
                Task::Item(ItemStep::AwaitMessage)
            }
            ItemStep::AwaitMessage => {
                self.step_item_report();
                // The message's task reports two frames after its last wait
                // (`0x0802C708`, states `0x2328`, `0x238C`), and the item
                // task goes on in the frame after it sees that.
                match self.fight.report {
                    Some(Report::Tail(frame)) if frame >= MESSAGE_TAIL => {
                        self.fight.report = None;
                        self.step_item(ItemStep::Used)
                    }
                    Some(Report::Done) => {
                        self.fight.report = None;
                        self.step_item(ItemStep::Used)
                    }
                    _ => Task::Item(ItemStep::AwaitMessage),
                }
            }
            ItemStep::Used => {
                self.finish_item_use();
                Task::Item(ItemStep::Closed(true))
            }
            ItemStep::Cancelled => {
                self.close_item_task();
                Task::Item(ItemStep::Closed(false))
            }
            ItemStep::Closed(used) => {
                self.item_used = Some(used);
                Task::Reported
            }
        }
    }

    /// The battle items the party has, in their order (`0x080393AC`).
    fn battle_items(&self) -> Vec<u8> {
        (0..=ITEMS)
            .filter(|&item| item != ITEMS)
            .filter(|&item| {
                self.state
                    .get(ITEM_COUNTS + usize::from(item))
                    .is_some_and(|&count| count > 0)
            })
            .collect()
    }

    fn item_at(&self, page: usize, line: usize) -> Option<u8> {
        self.item_menu.list.get(page * PAGE_LINES + line).copied()
    }

    /// The help of the item under the cursor, when it is not the one shown
    /// (`0x0803944C`).
    fn show_help(&mut self) {
        let item = self.item_at(self.item_menu.page, self.item_menu.line);
        if item == self.item_menu.shown {
            return;
        }
        self.item_menu.shown = item;
        self.acts.push_back(Act::Call(Call::Menu(MENU_CLEAR)));
        self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
        if let Some(item) = item {
            self.acts
                .push_back(Act::Call(Call::Item(ITEM_HELP + u16::from(item))));
            self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
        }
    }

    /// The page's lines (`0x080394A0`): each item's name, spaces to its
    /// count's column and the count in two places.
    fn draw_page(&mut self) {
        self.acts.push_back(Act::Call(Call::Menu(MENU_CLEAR_LIST)));
        self.acts.push_back(Act::Call(Call::Menu(MENU_SHOW_LIST)));
        self.acts.push_back(Act::PrintWindow(LIST_WINDOW));
        for line in 0..PAGE_LINES {
            let Some(item) = self.item_at(self.item_menu.page, line) else {
                continue;
            };
            if line > 0 {
                self.acts.push_back(Act::Call(Call::Text(TEXT_NEW_LINE)));
            }
            self.acts
                .push_back(Act::Call(Call::NameIndex(ITEM_NAMES + u16::from(item))));
            self.acts.push_back(Act::Pad(COUNT_COLUMN, TEXT_SPACE));
            let count = self
                .state
                .get(ITEM_COUNTS + usize::from(item))
                .copied()
                .unwrap_or(0);
            for call in super::turn::number_in(i32::from(count), 2, 0) {
                self.acts.push_back(Act::Call(call));
            }
        }
        self.acts.push_back(Act::PrintWindow(MESSAGE_WINDOW));
        self.acts.push_back(Act::Call(Call::Menu(MENU_SHOW_LIST)));
    }

    /// What the list's menu returned, as its script ends: the original's
    /// menu returns in the key's frame and goes on in it, the port's a frame
    /// later, so the next state starts at once.
    pub(super) fn take_item_choice(&mut self) {
        if self.task != Task::Item(ItemStep::AwaitChoice) {
            return;
        }
        let next = self.chosen_item();
        self.task = match next {
            Task::Item(step @ (ItemStep::Target | ItemStep::Choose | ItemStep::Cancelled)) => {
                self.step_item(step)
            }
            other => other,
        };
    }

    /// What the list's menu returned (state `0x44C`): a move shows the new
    /// item's help, A takes an item, B gives up.
    fn chosen_item(&mut self) -> Task {
        let [code, line, ..] = *self.menu.vars();
        match code {
            CHOSE => {
                self.item_menu.line = usize::from(line);
                let Some(item) = self.item_at(self.item_menu.page, self.item_menu.line) else {
                    return Task::Item(ItemStep::Choose);
                };
                self.sounds.push(SOUND_TAKEN);
                self.item_menu.item = item;
                Task::Item(ItemStep::Target)
            }
            CANCELLED => {
                self.sounds.push(SOUND_BACK);
                Task::Item(ItemStep::Cancelled)
            }
            _ => {
                self.item_menu.line = usize::from(line);
                self.acts.push_back(Act::Call(Call::Menu(MENU_DRAW)));
                self.show_help();
                Task::Item(ItemStep::Choose)
            }
        }
    }

    /// The first party slot with a unit still fighting (`0x08033844`).
    fn first_target(&self) -> usize {
        (0..SLOTS)
            .find(|&slot| self.fighting(PARTY, slot))
            .unwrap_or(0)
    }

    fn fighting(&self, side: usize, slot: usize) -> bool {
        self.sides[side][slot]
            .as_ref()
            .is_some_and(BattleUnit::fighting)
    }

    /// A frame of the target's choice (state `0x834`): the pad moves the
    /// marker to the first slot with a unit that way (`0x08032390`), A uses
    /// the item on it, B goes back to the list.
    fn choose_target(&mut self) -> Task {
        let pressed = |button: Button| self.input.is_held(button) && !self.previous.is_held(button);
        let way = [Button::Up, Button::Down, Button::Left, Button::Right]
            .into_iter()
            .position(pressed);
        if let Some(way) = way {
            let before = self.item_menu.target;
            if let Some(next) = self
                .neighbours(before, way)
                .into_iter()
                .find(|&slot| self.fighting(PARTY, slot))
            {
                self.item_menu.target = next;
            }
            if self.item_menu.target != before {
                self.sounds.push(SOUND_MOVE);
            }
            self.targeting = Some(self.item_menu.target);
            self.palette_writes += 1;
        }
        if pressed(Button::A) {
            self.use_item();
            self.targeting = None;
            return Task::Item(ItemStep::Use);
        }
        if pressed(Button::B) {
            self.sounds.push(SOUND_BACK);
            self.targeting = None;
            self.item_menu.shown = None;
            return Task::Item(ItemStep::Page);
        }
        Task::Item(ItemStep::ChooseTarget)
    }

    /// The slots the marker tries going `way` from `slot`.
    fn neighbours(&self, slot: usize, way: usize) -> Vec<usize> {
        self.neighbour_slots
            .get(slot)
            .and_then(|ways| ways.get(way))
            .map(|slots| slots.iter().map(|&slot| usize::from(slot)).collect())
            .unwrap_or_default()
    }

    /// The item's effect on the unit (`0x08039580`, the routines at ROM
    /// `0x683AA8`), one fewer of it, and the blow the display shows.
    fn use_item(&mut self) {
        let item = self.item_menu.item;
        let slot = self.item_menu.target;
        let mut amount = 0;
        if let Some(unit) = self.sides[PARTY][slot].as_mut() {
            amount = match item {
                0 => repair(unit, 300),
                1 => repair(unit, 150),
                2 => repair(unit, 50),
                3 => {
                    unit.traits &= !PARALYSED;
                    0
                }
                4 => {
                    unit.traits &= !PARALYSED;
                    unit.hp = unit.max_hp;
                    0
                }
                _ => {
                    let half = unit.max_hp >> 1;
                    repair(unit, half)
                }
            };
        }
        if let Some(count) = self.state.get_mut(ITEM_COUNTS + usize::from(item)) {
            *count = count.saturating_sub(1);
        }
        self.fight.blows = vec![Blow {
            side: PARTY,
            slot,
            flags: attack::LANDED,
            damage: amount,
            code: 0,
            kind: attack::REPAIRED,
        }];
    }

    /// The display's frames while the item's glow plays.
    fn step_item_display(&mut self) {
        self.step_report();
    }

    fn step_item_report(&mut self) {
        self.step_report();
    }

    /// The item's end (state `0x157C`'s last step): the figures come back,
    /// and after a party target each fighting unit's panel is shown again
    /// and lit in turn, the last one staying lit (`0x0803CA20`); they reach
    /// the screen with the list's reset, which copies the screen's maps
    /// again.
    fn finish_item_use(&mut self) {
        self.figures_held = false;
        if self
            .fight
            .blows
            .first()
            .is_some_and(|blow| blow.side == PARTY)
        {
            for slot in 0..SLOTS {
                if self.sides[PARTY][slot]
                    .as_ref()
                    .is_some_and(BattleUnit::fighting)
                {
                    self.refresh_panel(slot);
                    self.light_panel(slot);
                }
            }
            self.panels_held = Some(LIST_WINDOW);
        }
        self.close_item_task();
    }

    /// The list and the message window close (`battle-menu` 1 and 2).
    fn close_item_task(&mut self) {
        self.acts.push_back(Act::Call(Call::Menu(MENU_RESET)));
        self.acts
            .push_back(Act::Call(Call::Menu(MENU_MESSAGE_WINDOW)));
    }
}

/// Hit points back by `amount`, at most the unit's full (`0x0803967C`):
/// the points given back.
fn repair(unit: &mut BattleUnit, amount: i32) -> i32 {
    let before = unit.hp;
    unit.hp = before.saturating_add(amount).min(unit.max_hp.max(before));
    unit.hp - before
}
