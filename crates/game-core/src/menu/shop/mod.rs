//! The towns' shops (`0x08008F58`): the item shops (`0x0805378C`) and the
//! armaments shops (`0x0805451C`), built from the pause menu's scripts and
//! wallpaper.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the two
//! shop tasks with the quantity chooser at `0x08053600`, the sell list at
//! `0x0804E2C4`, the part description at `0x080544A8` and the name padding
//! at `0x0804D698` (full-width spaces up to a cell, which `pad_to` gives); checked against Arcana's shops in a reference emulator
//! (breakpoints on the script runner and the sound call, screenshots frame
//! by frame); see `docs/shop.md`.

use extraction::saga_party;
use extraction::saga_shop::{self, Item, ItemKind};

use super::parts::put_value;
use super::{
    CONFIRMED, EMPTY_BACK_SOUND, EMPTY_SOUND, LEAVE_SOUND, LEFT_ALIGNED, MENU_MOVE_SOUND,
    MONEY_WINDOW, MenuState, PauseMenu, SCRIPT_CLEAR_HELP, SCRIPT_MEMBER_MENU, SCRIPT_MONEY_UNIT,
    SCRIPT_MONEY_WINDOW, SCRIPT_PRESENT, SCRIPT_PRESENT_ALL, SCRIPT_SPACE, SCRIPT_WAIT_KEY,
    SCRIPT_YES_NO,
};
use crate::ScriptHost;
use crate::script::ScriptError;
use crate::windows::ScriptWindows;
use platform::{Button, Input};

mod wares;

/// A shop a keeper opens (`0x08008F58`'s kind and number).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shop {
    /// Item shop `n` (kind 0): its goods are record `n` at ROM `0x75BCD4`.
    Items(u8),
    /// Armaments shop `n` (kind 1): its parts are record `n` at ROM
    /// `0x75BF40`.
    Arms(u8),
}

/// Which list, question or notice a shop is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ShopStep {
    /// 買う or 売る (state `0x20`).
    Choice,
    /// The notice that the party has nothing to sell.
    NothingToSell,
    /// The goods on sale (state `0x101`).
    Goods,
    /// A notice over the goods: too little money, or no room.
    GoodsNotice,
    /// How many, chosen with the pad (`0x08053600`).
    Quantity,
    /// Whether to buy (state `0x120`).
    ConfirmBuy,
    /// The keeper's answer to a purchase.
    Bought,
    /// What the party can sell (state `0x201`).
    Wares,
    /// Whether to sell (state `0x220`).
    ConfirmSell,
    /// The keeper's answer to a sale.
    Sold,
}

/// A shop's scripts, by the index its task passes: the item shops' start
/// at pause-menu script 223, the armaments shops' at 242.
#[derive(Debug, Clone, Copy)]
struct Scripts {
    windows: usize,
    welcome: usize,
    choice: usize,
    nothing_to_sell: usize,
    goods_windows: usize,
    held_label: usize,
    held_unit: usize,
    no_money: usize,
    no_room: usize,
    subject: usize,
    buy_question: usize,
    thanks: usize,
    refused: usize,
    wares_window: usize,
    sell_question: usize,
    how_many_to_buy: usize,
    how_many_to_sell: usize,
    quantity_window: usize,
    count_unit: usize,
}

const ITEM_SCRIPTS: Scripts = Scripts {
    windows: 223,
    welcome: 224,
    choice: 225,
    nothing_to_sell: 226,
    goods_windows: 227,
    held_label: 228,
    held_unit: 229,
    how_many_to_buy: 230,
    quantity_window: 231,
    no_money: 232,
    no_room: 233,
    subject: 234,
    count_unit: 235,
    buy_question: 236,
    thanks: 237,
    refused: 238,
    wares_window: 239,
    how_many_to_sell: 240,
    sell_question: 241,
};

const ARMS_SCRIPTS: Scripts = Scripts {
    windows: 242,
    welcome: 243,
    choice: 244,
    nothing_to_sell: 245,
    goods_windows: 246,
    held_label: 247,
    held_unit: 248,
    no_money: 249,
    no_room: 250,
    subject: 251,
    buy_question: 252,
    thanks: 253,
    refused: 254,
    wares_window: 255,
    sell_question: 256,
    how_many_to_buy: 0,
    how_many_to_sell: 0,
    quantity_window: 0,
    count_unit: 0,
};

const HELP_WINDOW: u8 = 0;
const HELD_WINDOW: u8 = 4;
const GOODS_WINDOW: u8 = 5;
const WARES_WINDOW: u8 = 4;
const QUANTITY_WINDOW: u8 = 7;
const SCRIPT_CLEAR_MONEY: usize = 1;
const SCRIPT_CLEAR_HELD: usize = 4;
/// The wares' list is window 4 as well.
const SCRIPT_CLEAR_WARES: usize = SCRIPT_CLEAR_HELD;
const SCRIPT_CLEAR_QUANTITY: usize = 7;
const SCRIPT_CLOSE_HELD: usize = 12;
const SCRIPT_CLOSE_GOODS: usize = 13;
const SCRIPT_CLOSE_QUANTITY: usize = 15;
const SCRIPT_PRESENT_QUANTITY: usize = 23;
const SCRIPT_DRAW_HELP: usize = 25;
const SCRIPT_DRAW_MONEY: usize = 26;
const SCRIPT_DRAW_WARES: usize = 29;
const GOODS_NAME_CELLS: usize = 9;
const WARES_NAME_CELLS: usize = 8;
const PRICE_CELLS: usize = 7;
const ITEM_COUNT_CELLS: usize = 2;
const PART_COUNT_CELLS: usize = 1;
const PAGE_LINES: usize = 4;
const QUANTITY_CELLS: usize = 2;
const QUANTITY_STEP: u32 = 10;
const CHOSEN_SOUND: u8 = 0x47;
const SELL: u16 = 1;
const MOVED: u16 = 1;

/// Something a shop sells or buys back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Goods {
    Item(Item),
    Part(u16),
}

/// What the help line and window 4 describe needs doing again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Redraw {
    /// Only what changed.
    #[default]
    Changed,
    /// The help, as a window drew over it.
    Help,
    /// The list's page as well, as a sale changed it.
    List,
}

/// A shop's own state: its goods, the list the party sells from and where
/// the cursor stands.
#[derive(Debug, Clone)]
pub(super) struct ShopSession {
    shop: Shop,
    goods: Vec<Goods>,
    wares: Vec<Goods>,
    /// The goods' line, or the wares' line within the page.
    line: usize,
    /// The entry the help describes.
    shown: Option<usize>,
    page: Option<usize>,
    shown_page: Option<usize>,
    redraw: Redraw,
    quantity: u32,
    most: u32,
    selling: bool,
    /// The keys held last frame, for the quantity chooser's presses.
    keys: Input,
}

impl ShopSession {
    /// Keeps the keys of a frame the game stood still in: the quantity
    /// chooser reads only presses made while it polls.
    pub(super) fn forget_keys(&mut self, input: Input) {
        self.keys = input;
    }

    /// Frames the welcome and the question overrun: the item shops' longer
    /// text takes the original past a frame's CPU time, which delays them
    /// a frame and stands the wallpaper still for it (measured in
    /// Arcana).
    pub(super) fn welcome_lag(&self) -> u32 {
        match self.shop {
            Shop::Items(_) => 1,
            Shop::Arms(_) => 0,
        }
    }

    fn scripts(&self) -> Scripts {
        match self.shop {
            Shop::Items(_) => ITEM_SCRIPTS,
            Shop::Arms(_) => ARMS_SCRIPTS,
        }
    }

    fn items(&self) -> bool {
        matches!(self.shop, Shop::Items(_))
    }

    fn chosen(&self) -> Option<Goods> {
        if self.selling {
            self.wares.get(self.current()).copied()
        } else {
            self.goods.get(self.line).copied()
        }
    }

    /// The wares entry under the cursor.
    fn current(&self) -> usize {
        self.page.unwrap_or(0) * PAGE_LINES + self.line
    }
}

impl PauseMenu {
    /// Opens `shop` on `windows` in the dark (`0x0805378C`, `0x0805451C`):
    /// the title, 買う / 売る and the money; the welcome and the choice
    /// follow once it has brightened.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a script cannot run.
    pub fn open_shop(
        &mut self,
        rom: &[u8],
        shop: Shop,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let goods = match shop {
            Shop::Items(number) => saga_shop::item_shop(rom, number)
                .unwrap_or_default()
                .into_iter()
                .map(Goods::Item)
                .collect(),
            Shop::Arms(number) => saga_shop::arms_shop(rom, number)
                .unwrap_or_default()
                .into_iter()
                .map(Goods::Part)
                .collect(),
        };
        let session = ShopSession {
            shop,
            goods,
            wares: Vec::new(),
            line: 0,
            shown: None,
            page: None,
            shown_page: None,
            redraw: Redraw::Changed,
            quantity: 1,
            most: 1,
            selling: false,
            keys: Input::default(),
        };
        let scripts = session.scripts();
        self.shop = Some(session);
        self.held = Input::default();
        windows.close_window(None);
        self.run_now(rom, scripts.windows, windows)?;
        self.run_now(rom, SCRIPT_MONEY_WINDOW, windows)?;
        self.print_shop_money(rom, windows)?;
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.state = MenuState::Shop(ShopStep::Choice);
        self.scroll = 0;
        self.busy = 0;
        self.shop_intro = Some(0);
        Ok(())
    }

    /// Whether the menu is a shop.
    #[must_use]
    pub fn is_shop(&self) -> bool {
        self.shop.is_some()
    }

    /// The welcome, once the shop has brightened, and the question of the
    /// choice in the same frame; its menu runs from the next.
    pub(super) fn welcome(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(scripts) = self.shop.as_ref().map(ShopSession::scripts) else {
            return Ok(());
        };
        self.run_now(rom, scripts.welcome, windows)?;
        self.back_to_choice(scripts)?;
        self.runner.update(rom, self.held, windows)?;
        Ok(())
    }

    fn back_to_choice(&mut self, scripts: Scripts) -> Result<(), ScriptError> {
        self.runner.start(scripts.choice)?;
        self.state = MenuState::Shop(ShopStep::Choice);
        Ok(())
    }

    /// A frame of the quantity chooser, which reads the pad itself.
    pub(super) fn shop_frame(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(session) = self.shop.as_mut() else {
            return Ok(());
        };
        let keys = session.keys;
        session.keys = input;
        if self.state != MenuState::Shop(ShopStep::Quantity) {
            return Ok(());
        }
        let pressed = |button: Button| input.is_held(button) && !keys.is_held(button);
        let (most, value) = (session.most, session.quantity);
        let step = [
            (Button::L, Step::DownTen),
            (Button::R, Step::UpTen),
            (Button::Left, Step::Down),
            (Button::Right, Step::Up),
        ]
        .into_iter()
        .find(|&(button, _)| pressed(button));
        if let Some((_, step)) = step {
            windows.play_sound(MENU_MOVE_SOUND);
            if let Some(session) = self.shop.as_mut() {
                session.quantity = next_quantity(value, most, step);
            }
            self.show_quantity(rom, windows)?;
        } else if pressed(Button::A) {
            windows.play_sound(CHOSEN_SOUND);
            self.run_now(rom, SCRIPT_CLOSE_QUANTITY, windows)?;
            return self.quantity_chosen(rom, windows);
        } else if pressed(Button::B) {
            windows.play_sound(LEAVE_SOUND);
            self.run_now(rom, SCRIPT_CLOSE_QUANTITY, windows)?;
            return self.quantity_canceled(rom, windows);
        }
        Ok(())
    }

    /// What a shop's script ended with.
    pub(super) fn shop_choice(
        &mut self,
        rom: &[u8],
        step: ShopStep,
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(scripts) = self.shop.as_ref().map(ShopSession::scripts) else {
            return Ok(());
        };
        match step {
            ShopStep::Choice if code != CONFIRMED => {
                windows.play_sound(LEAVE_SOUND);
                self.state = MenuState::Closing(0);
                Ok(())
            }
            ShopStep::Choice => self.choose(rom, scripts, line, windows),
            ShopStep::NothingToSell => {
                windows.play_sound(EMPTY_BACK_SOUND);
                self.back_to_choice(scripts)
            }
            ShopStep::Goods => self.goods_choice(rom, scripts, code, line, windows),
            ShopStep::GoodsNotice => {
                windows.play_sound(EMPTY_BACK_SOUND);
                self.describe_goods(rom, windows)?;
                self.goods_menu()
            }
            ShopStep::ConfirmBuy => self.buy_answer(rom, scripts, code, line, windows),
            ShopStep::Bought => {
                windows.play_sound(EMPTY_BACK_SOUND);
                self.set_redraw(Redraw::Help);
                self.show_goods(rom, windows)
            }
            ShopStep::Wares => self.wares_choice(rom, scripts, code, line, windows),
            ShopStep::ConfirmSell => self.sell_answer(rom, scripts, code, line, windows),
            ShopStep::Sold => {
                windows.play_sound(EMPTY_BACK_SOUND);
                self.set_redraw(Redraw::List);
                self.enter_wares(rom, windows)
            }
            ShopStep::Quantity => Ok(()),
        }
    }

    /// 買う opens the goods; 売る the wares, or a notice when the party has
    /// nothing the shop buys.
    fn choose(
        &mut self,
        rom: &[u8],
        scripts: Scripts,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if line == SELL && self.wares(rom).is_empty() {
            windows.play_sound(EMPTY_SOUND);
            self.runner.start(scripts.nothing_to_sell)?;
            self.state = MenuState::Shop(ShopStep::NothingToSell);
            return Ok(());
        }
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        if line == SELL {
            return self.open_wares(rom, scripts, windows);
        }
        self.run_now(rom, scripts.goods_windows, windows)?;
        let goods = self
            .shop
            .as_ref()
            .map(|session| session.goods.clone())
            .unwrap_or_default();
        for (index, entry) in goods.iter().enumerate() {
            self.print_goods_name(rom, *entry, GOODS_WINDOW, windows)?;
            windows.pad_to(GOODS_WINDOW, GOODS_NAME_CELLS);
            put_value(
                windows,
                GOODS_WINDOW,
                price_shown(price(rom, *entry)),
                PRICE_CELLS,
                0,
            );
            self.run_in(rom, GOODS_WINDOW, SCRIPT_MONEY_UNIT, windows)?;
            if index + 1 < PAGE_LINES && index + 1 < goods.len() {
                windows.line_break(GOODS_WINDOW);
            }
        }
        if let Some(session) = self.shop.as_mut() {
            session.selling = false;
            session.line = 0;
            session.shown = None;
            session.redraw = Redraw::Changed;
        }
        self.show_goods(rom, windows)
    }

    /// The goods' loop: the help again when the cursor moved or a window
    /// covered it, then the list's menu.
    fn show_goods(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let stale = self.shop.as_ref().is_some_and(|session| {
            session.redraw != Redraw::Changed || session.shown != Some(session.line)
        });
        if stale {
            self.describe_goods(rom, windows)?;
        }
        self.goods_menu()
    }

    fn goods_menu(&mut self) -> Result<(), ScriptError> {
        self.runner.start(SCRIPT_MEMBER_MENU)?;
        self.state = MenuState::Shop(ShopStep::Goods);
        Ok(())
    }

    /// How many of the goods under the cursor the party holds, and in the
    /// help line what they are.
    fn describe_goods(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(session) = self.shop.as_ref() else {
            return Ok(());
        };
        let scripts = session.scripts();
        let (line, shown) = (session.line, session.shown);
        let Some(entry) = session.goods.get(line).copied() else {
            return Ok(());
        };
        if shown.is_some() {
            self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
            self.run_now(rom, SCRIPT_CLEAR_HELD, windows)?;
        }
        self.print_held(rom, scripts, entry, windows)?;
        self.describe(rom, entry, windows)?;
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        if let Some(session) = self.shop.as_mut() {
            session.shown = Some(line);
            session.redraw = Redraw::Changed;
        }
        Ok(())
    }

    fn print_held(
        &mut self,
        rom: &[u8],
        scripts: Scripts,
        entry: Goods,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, scripts.held_label, windows)?;
        let (count, cells) = self.count(entry);
        put_value(windows, HELD_WINDOW, i32::from(count), cells, 0);
        self.run_in(rom, HELD_WINDOW, scripts.held_unit, windows)
    }

    /// The help line: an item's text, or a part's values (`0x080544A8`).
    fn describe(
        &mut self,
        rom: &[u8],
        entry: Goods,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        match entry {
            Goods::Item(item) => match item.kind {
                ItemKind::Consumable => {
                    let text = saga_shop::CONSUMABLE_TEXTS + usize::from(item.id);
                    self.run_item_script(rom, text, HELP_WINDOW, windows)
                }
                ItemKind::Core => {
                    let text = saga_shop::CORE_TEXTS + usize::from(item.id);
                    self.run_in(rom, HELP_WINDOW, text, windows)
                }
            },
            Goods::Part(id) => match saga_party::part_record(rom, id) {
                Some(part) => self.describe_weapon(rom, HELP_WINDOW, part, windows),
                None => Ok(()),
            },
        }
    }

    fn goods_choice(
        &mut self,
        rom: &[u8],
        scripts: Scripts,
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if let Some(session) = self.shop.as_mut() {
            session.line = usize::from(line);
        }
        if code > MOVED {
            return self.show_goods(rom, windows);
        }
        if code != CONFIRMED {
            windows.play_sound(LEAVE_SOUND);
            return self.leave_list(rom, scripts, windows);
        }
        let Some(entry) = self.shop.as_ref().and_then(ShopSession::chosen) else {
            return self.show_goods(rom, windows);
        };
        let price = price(rom, entry);
        let (count, _) = self.count(entry);
        let notice = if self.party.money < price {
            Some(scripts.no_money)
        } else if count >= limit(entry) {
            Some(scripts.no_room)
        } else {
            None
        };
        if let Some(notice) = notice {
            windows.play_sound(EMPTY_SOUND);
            self.runner.start(notice)?;
            self.state = MenuState::Shop(ShopStep::GoodsNotice);
            return Ok(());
        }
        if let Some(session) = self.shop.as_mut() {
            session.selling = false;
        }
        match entry {
            Goods::Item(_) => {
                let most = (self.party.money / price.max(1))
                    .min(u32::from(saga_shop::ITEM_LIMIT.saturating_sub(count)));
                self.ask_quantity(rom, scripts.how_many_to_buy, most, windows)
            }
            Goods::Part(_) => self.ask_to_buy(rom, windows),
        }
    }

    /// Back from a list to 買う / 売る: the list's windows closed and the
    /// help line cleared.
    fn leave_list(
        &mut self,
        rom: &[u8],
        scripts: Scripts,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_CLOSE_GOODS, windows)?;
        self.run_now(rom, SCRIPT_CLOSE_HELD, windows)?;
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.back_to_choice(scripts)
    }

    /// The question, then the chooser from 1 to `most` (`0x08053600`).
    fn ask_quantity(
        &mut self,
        rom: &[u8],
        question: usize,
        most: u32,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, question, windows)?;
        let scripts = ITEM_SCRIPTS;
        self.run_now(rom, scripts.quantity_window, windows)?;
        if let Some(session) = self.shop.as_mut() {
            session.quantity = 1;
            session.most = most.max(1);
        }
        self.show_quantity(rom, windows)?;
        self.state = MenuState::Shop(ShopStep::Quantity);
        Ok(())
    }

    fn show_quantity(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let quantity = self.shop.as_ref().map_or(1, |session| session.quantity);
        self.run_now(rom, SCRIPT_CLEAR_QUANTITY, windows)?;
        self.run_in(rom, QUANTITY_WINDOW, SCRIPT_SPACE, windows)?;
        put_value(
            windows,
            QUANTITY_WINDOW,
            i32::try_from(quantity).unwrap_or(0),
            QUANTITY_CELLS,
            0,
        );
        self.run_now(rom, SCRIPT_PRESENT_QUANTITY, windows)
    }

    fn quantity_chosen(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if self.shop.as_ref().is_some_and(|session| session.selling) {
            self.ask_to_sell(rom, windows)
        } else {
            self.ask_to_buy(rom, windows)
        }
    }

    fn quantity_canceled(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.set_redraw(Redraw::Help);
        if self.shop.as_ref().is_some_and(|session| session.selling) {
            self.enter_wares(rom, windows)
        } else {
            self.show_goods(rom, windows)
        }
    }

    /// …が…個で…Ｇになります　お買い上げになりますか？, or for a part
    /// …は…Ｇになるな　買うかい？, then はい / いいえ.
    fn ask_to_buy(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(session) = self.shop.as_ref() else {
            return Ok(());
        };
        let scripts = session.scripts();
        let quantity = session.quantity;
        let Some(entry) = session.chosen() else {
            return Ok(());
        };
        let total = price(rom, entry).saturating_mul(quantity);
        self.ask(
            rom,
            scripts,
            entry,
            quantity,
            total,
            scripts.buy_question,
            windows,
        )?;
        self.state = MenuState::Shop(ShopStep::ConfirmBuy);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn ask(
        &mut self,
        rom: &[u8],
        scripts: Scripts,
        entry: Goods,
        quantity: u32,
        total: u32,
        question: usize,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        self.print_goods_name(rom, entry, HELP_WINDOW, windows)?;
        self.run_in(rom, HELP_WINDOW, scripts.subject, windows)?;
        if matches!(entry, Goods::Item(_)) {
            put_value(
                windows,
                HELP_WINDOW,
                i32::try_from(quantity).unwrap_or(0),
                QUANTITY_CELLS,
                LEFT_ALIGNED,
            );
            self.run_in(rom, HELP_WINDOW, scripts.count_unit, windows)?;
        }
        put_value(
            windows,
            HELP_WINDOW,
            price_shown(total),
            PRICE_CELLS,
            LEFT_ALIGNED,
        );
        self.run_in(rom, HELP_WINDOW, question, windows)?;
        self.run_now(rom, SCRIPT_PRESENT + usize::from(HELP_WINDOW), windows)?;
        self.runner.start(SCRIPT_YES_NO)
    }

    /// はい buys: the goods join the party's, the money and the count are
    /// printed again, and the keeper thanks; いいえ gets …… or a jibe. B
    /// asks how many again, or for a part goes back to the goods.
    fn buy_answer(
        &mut self,
        rom: &[u8],
        scripts: Scripts,
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(session) = self.shop.as_ref() else {
            return Ok(());
        };
        let quantity = session.quantity;
        let Some(entry) = session.chosen() else {
            return Ok(());
        };
        if code != CONFIRMED {
            windows.play_sound(LEAVE_SOUND);
            if let Goods::Item(item) = entry {
                let price = saga_shop::item_price(rom, item).max(1);
                let count = saga_shop::item_count(&self.game_state, item);
                let most = (self.party.money / price)
                    .min(u32::from(saga_shop::ITEM_LIMIT.saturating_sub(count)));
                return self.ask_quantity(rom, scripts.how_many_to_buy, most, windows);
            }
            self.set_redraw(Redraw::Help);
            return self.show_goods(rom, windows);
        }
        if line == 0 {
            let total = price(rom, entry).saturating_mul(quantity);
            let (count, _) = self.count(entry);
            self.set_count(
                entry,
                count.saturating_add(u8::try_from(quantity).unwrap_or(0)),
            );
            self.party.money = self.party.money.saturating_sub(total);
            self.reprint_money(rom, windows)?;
            self.run_now(rom, SCRIPT_CLEAR_HELD, windows)?;
            self.print_held(rom, scripts, entry, windows)?;
            self.run_now(rom, scripts.thanks, windows)?;
        } else {
            self.run_now(rom, scripts.refused, windows)?;
        }
        self.answered(rom, ShopStep::Bought, windows)
    }

    /// The answer shown, then a key.
    fn answered(
        &mut self,
        rom: &[u8],
        step: ShopStep,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
        self.run_now(rom, SCRIPT_DRAW_HELP, windows)?;
        self.runner.start(SCRIPT_WAIT_KEY)?;
        self.state = MenuState::Shop(step);
        Ok(())
    }

    fn reprint_money(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, SCRIPT_CLEAR_MONEY, windows)?;
        self.run_now(rom, SCRIPT_DRAW_MONEY, windows)?;
        self.print_shop_money(rom, windows)
    }

    /// A space, the money in seven cells and Ｇ, in window 1.
    fn print_shop_money(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_in(rom, MONEY_WINDOW, SCRIPT_SPACE, windows)?;
        put_value(
            windows,
            MONEY_WINDOW,
            price_shown(self.party.money),
            PRICE_CELLS,
            0,
        );
        self.run_in(rom, MONEY_WINDOW, SCRIPT_MONEY_UNIT, windows)
    }

    fn set_redraw(&mut self, redraw: Redraw) {
        if let Some(session) = self.shop.as_mut() {
            session.redraw = redraw;
        }
    }

    /// How many of `entry` the party holds, and the cells the shop prints
    /// it in.
    fn count(&self, entry: Goods) -> (u8, usize) {
        match entry {
            Goods::Item(item) => (
                saga_shop::item_count(&self.game_state, item),
                ITEM_COUNT_CELLS,
            ),
            Goods::Part(id) => (saga_party::stock(&self.game_state, id), PART_COUNT_CELLS),
        }
    }

    fn set_count(&mut self, entry: Goods, count: u8) {
        match entry {
            Goods::Item(item) => saga_shop::set_item_count(&mut self.game_state, item, count),
            Goods::Part(id) => saga_shop::set_stock(&mut self.game_state, id, count),
        }
    }

    /// The name of `entry` in `window`: a consumable's from the `name`
    /// table, a core's from the `item` table, a part's from the `part`
    /// table.
    fn print_goods_name(
        &mut self,
        rom: &[u8],
        entry: Goods,
        window: u8,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        match entry {
            Goods::Item(Item {
                kind: ItemKind::Consumable,
                id,
            }) => self.print_name(
                rom,
                saga_shop::CONSUMABLE_NAMES + usize::from(id),
                window,
                windows,
            ),
            Goods::Item(Item {
                kind: ItemKind::Core,
                id,
            }) => self.run_item_script(rom, usize::from(id), window, windows),
            Goods::Part(id) => self.print_part_name(rom, window, id, windows),
        }
    }

    /// Runs string `index` of the `item` table into `window`.
    fn run_item_script(
        &mut self,
        rom: &[u8],
        index: usize,
        window: u8,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.items.select_window(window);
        self.items.start(index)?;
        while !self.items.update(rom, self.held, windows)? {
            if self.items.is_waiting_for_key() {
                break;
            }
            self.busy += 1;
        }
        Ok(())
    }
}

/// A change the quantity chooser's pad makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// L.
    DownTen,
    /// R.
    UpTen,
    /// Left.
    Down,
    /// Right.
    Up,
}

/// The quantity after `step`, between 1 and `most` (`0x08053600`): ±1
/// wraps around; ±10 stops at the end it would pass, and from that end
/// goes to the other.
fn next_quantity(value: u32, most: u32, step: Step) -> u32 {
    const LEAST: u32 = 1;
    match step {
        Step::DownTen if value > QUANTITY_STEP => value - QUANTITY_STEP,
        Step::DownTen if value == LEAST => most,
        Step::DownTen => LEAST,
        Step::UpTen if value + QUANTITY_STEP <= most => value + QUANTITY_STEP,
        Step::UpTen if value == most => LEAST,
        Step::UpTen => most,
        Step::Down if value <= LEAST => most,
        Step::Down => value - 1,
        Step::Up if value >= most => LEAST,
        Step::Up => value + 1,
    }
}

/// What `entry` costs.
fn price(rom: &[u8], entry: Goods) -> u32 {
    match entry {
        Goods::Item(item) => saga_shop::item_price(rom, item),
        Goods::Part(id) => saga_shop::part_price(rom, id),
    }
}

/// The most a stock or the party's bag holds of `entry`.
fn limit(entry: Goods) -> u8 {
    match entry {
        Goods::Item(_) => saga_shop::ITEM_LIMIT,
        Goods::Part(_) => saga_shop::PART_LIMIT,
    }
}

/// What a sale brings: half the price of all of them for items
/// (`0x080542A0`), half the part's price for a part.
fn sale_total(price: u32, quantity: u32, items: bool) -> u32 {
    if items {
        price.saturating_mul(quantity) / 2
    } else {
        price / 2
    }
}

/// A sum as the number printer takes it.
fn price_shown(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_quantity_wraps_by_one_and_stops_at_the_ends_by_ten() {
        assert_eq!(next_quantity(1, 11, Step::Down), 11);
        assert_eq!(next_quantity(11, 11, Step::Up), 1);
        assert_eq!(next_quantity(3, 11, Step::UpTen), 11);
        assert_eq!(next_quantity(11, 11, Step::UpTen), 1);
        assert_eq!(next_quantity(11, 11, Step::DownTen), 1);
        assert_eq!(next_quantity(1, 11, Step::DownTen), 11);
        assert_eq!(next_quantity(5, 11, Step::DownTen), 1);
        assert_eq!(next_quantity(2, 30, Step::UpTen), 12);
    }

    #[test]
    fn a_sale_brings_half_the_price() {
        assert_eq!(sale_total(500, 3, true), 750);
        assert_eq!(sale_total(501, 1, true), 250);
        assert_eq!(sale_total(3200, 1, false), 1600);
    }

    #[test]
    fn the_shops_scripts_follow_the_pause_menus() {
        assert_eq!(ITEM_SCRIPTS.windows, 223);
        assert_eq!(ITEM_SCRIPTS.sell_question, 241);
        assert_eq!(ARMS_SCRIPTS.windows, 242);
        assert_eq!(ARMS_SCRIPTS.sell_question, 256);
    }
}
