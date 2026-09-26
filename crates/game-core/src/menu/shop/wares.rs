//! A shop's wares: what the party sells back (states `0x200` to `0x220`
//! of the shop tasks).
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the item
//! shop's states at `0x08053F4C` to `0x080543F4` with the sell list at
//! `0x0804E2C4`, and the armaments shop's at `0x08054A8A` on with the
//! stock list at `0x0804E24C`; see `docs/shop.md`.

use extraction::saga_party;
use extraction::saga_shop::{self, Item, ItemKind};

use super::super::parts::put_value;
use super::super::{
    CONFIRMED, LEAVE_SOUND, MENU_MOVE_SOUND, MenuState, PAGE_LEFT, PAGE_RIGHT, PauseMenu,
    SCRIPT_CLEAR_HELP, SCRIPT_MEMBER_MENU, SCRIPT_MONEY_UNIT, SCRIPT_PRESENT_ALL, SCRIPT_SPACE,
    SCRIPT_TIMES, STOCK_MASK, ZERO_PADDED,
};
use super::{
    Goods, PAGE_LINES, PRICE_CELLS, Redraw, SCRIPT_CLEAR_WARES, SCRIPT_CLOSE_GOODS,
    SCRIPT_CLOSE_HELD, SCRIPT_DRAW_WARES, Scripts, Shop, ShopStep, WARES_NAME_CELLS, WARES_WINDOW,
    price, price_shown, sale_total,
};
use crate::ScriptHost;
use crate::script::ScriptError;
use crate::windows::ScriptWindows;

impl PauseMenu {
    /// 売る: the wares window, its first page.
    pub(super) fn open_wares(
        &mut self,
        rom: &[u8],
        scripts: Scripts,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.run_now(rom, scripts.wares_window, windows)?;
        if let Some(session) = self.shop.as_mut() {
            session.selling = true;
            session.page = Some(0);
            session.shown_page = None;
            session.shown = None;
            session.line = 0;
            session.redraw = Redraw::Changed;
        }
        self.enter_wares(rom, windows)
    }
    pub(super) fn wares(&self, rom: &[u8]) -> Vec<Goods> {
        match self.shop.as_ref().map(|session| session.shop) {
            Some(Shop::Items(_)) => saga_shop::held_consumables(&self.game_state)
                .into_iter()
                .map(|id| {
                    Goods::Item(Item {
                        kind: ItemKind::Consumable,
                        id,
                    })
                })
                .collect(),
            Some(Shop::Arms(_)) => saga_party::stocked_parts(rom, &self.game_state, STOCK_MASK)
                .into_iter()
                .map(Goods::Part)
                .collect(),
            Some(Shop::Lab(_)) | None => Vec::new(),
        }
    }
    /// State `0x201` from its start: the wares listed again from the game
    /// state (`0x0804E2C4`, `0x0804E24C`), then their loop.
    pub(super) fn enter_wares(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let wares = self.wares(rom);
        if let Some(session) = self.shop.as_mut() {
            session.wares = wares;
        }
        self.show_wares(rom, windows)
    }
    /// The wares' loop: the page printed again when it changed or a sale
    /// changed it, a page back when it is left empty; the help again when
    /// the entry changed or a window covered it; then the list's menu.
    pub(super) fn show_wares(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(session) = self.shop.as_ref() else {
            return Ok(());
        };
        let scripts = session.scripts();
        if session.redraw == Redraw::List || session.page != session.shown_page {
            if session.redraw != Redraw::Changed {
                self.run_now(rom, SCRIPT_DRAW_WARES, windows)?;
            }
            if self
                .shop
                .as_ref()
                .is_some_and(|session| session.shown_page.is_some())
            {
                self.run_now(rom, SCRIPT_CLEAR_WARES, windows)?;
            }
            self.print_wares_page(rom, windows)?;
        }
        let Some(session) = self.shop.as_ref() else {
            return Ok(());
        };
        if session.page.is_none() {
            self.run_now(rom, SCRIPT_CLOSE_GOODS, windows)?;
            self.run_now(rom, SCRIPT_CLOSE_HELD, windows)?;
            self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
            self.runner.start(scripts.nothing_to_sell)?;
            self.state = MenuState::Shop(ShopStep::NothingToSell);
            return Ok(());
        }
        let current = session.current();
        if session.redraw != Redraw::Changed || session.shown != Some(current) {
            let shown = session.shown;
            if let Some(entry) = session.wares.get(current).copied() {
                if shown.is_some() {
                    self.run_now(rom, SCRIPT_CLEAR_HELP, windows)?;
                }
                self.describe(rom, entry, windows)?;
                self.run_now(rom, SCRIPT_PRESENT_ALL, windows)?;
            }
            if let Some(session) = self.shop.as_mut() {
                session.shown = Some(current);
                session.redraw = Redraw::Changed;
            }
        }
        self.runner.start(SCRIPT_MEMBER_MENU)?;
        self.state = MenuState::Shop(ShopStep::Wares);
        Ok(())
    }
    /// Prints the wares' page: four a page, each its name, ×, the count,
    /// and half the price; an empty page steps back to the one before
    /// with the cursor on its last line.
    pub(super) fn print_wares_page(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(session) = self.shop.as_ref() else {
            return Ok(());
        };
        let wares = session.wares.clone();
        let mut page = session.page;
        let mut line = session.line;
        let mut last = 0;
        let mut printed = 0;
        while let Some(current) = page {
            let start = current * PAGE_LINES;
            printed = 0;
            for (index, entry) in wares.iter().enumerate().skip(start).take(PAGE_LINES) {
                last = index;
                self.print_goods_name(rom, *entry, WARES_WINDOW, windows)?;
                windows.pad_to(WARES_WINDOW, WARES_NAME_CELLS);
                self.run_in(rom, WARES_WINDOW, SCRIPT_TIMES, windows)?;
                let (count, cells) = self.count(*entry);
                let mode = if matches!(entry, Goods::Item(_)) {
                    ZERO_PADDED
                } else {
                    0
                };
                put_value(windows, WARES_WINDOW, i32::from(count), cells, mode);
                self.run_in(rom, WARES_WINDOW, SCRIPT_SPACE, windows)?;
                put_value(
                    windows,
                    WARES_WINDOW,
                    price_shown(price(rom, *entry) / 2),
                    PRICE_CELLS,
                    0,
                );
                self.run_in(rom, WARES_WINDOW, SCRIPT_MONEY_UNIT, windows)?;
                printed += 1;
                if printed < PAGE_LINES && index + 1 < wares.len() {
                    windows.line_break(WARES_WINDOW);
                }
            }
            if printed > 0 {
                break;
            }
            page = current.checked_sub(1);
            if page.is_some() {
                line = PAGE_LINES - 1;
                windows.set_cursor(WARES_WINDOW, Some(line));
                windows.set_cursor(WARES_WINDOW, None);
            }
        }
        if page.is_some() && printed <= line {
            line = printed.saturating_sub(1);
            windows.set_cursor(WARES_WINDOW, Some(line));
            windows.set_cursor(WARES_WINDOW, None);
        }
        let more = last + 1 < wares.len();
        windows.set_scroll_marks(
            WARES_WINDOW,
            (page.is_some_and(|page| page > 0), page.is_some() && more),
        );
        if let Some(session) = self.shop.as_mut() {
            session.page = page;
            session.shown_page = page;
            session.line = line;
        }
        Ok(())
    }
    pub(super) fn wares_choice(
        &mut self,
        rom: &[u8],
        scripts: Scripts,
        code: u16,
        line: u16,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(session) = self.shop.as_mut() else {
            return Ok(());
        };
        session.line = usize::from(line);
        let page = session.page.unwrap_or(0);
        let more = (page + 1) * PAGE_LINES < session.wares.len();
        let entry = session.chosen();
        match code {
            PAGE_LEFT if page > 0 => {
                windows.play_sound(MENU_MOVE_SOUND);
                session.page = Some(page - 1);
                self.show_wares(rom, windows)
            }
            PAGE_RIGHT if more => {
                windows.play_sound(MENU_MOVE_SOUND);
                session.page = Some(page + 1);
                self.show_wares(rom, windows)
            }
            CONFIRMED => match entry {
                Some(Goods::Item(item)) => {
                    let count = saga_shop::item_count(&self.game_state, item);
                    self.ask_quantity(rom, scripts.how_many_to_sell, u32::from(count), windows)
                }
                Some(Goods::Part(_)) => {
                    session.quantity = 1;
                    self.ask_to_sell(rom, windows)
                }
                None => self.show_wares(rom, windows),
            },
            0 => {
                windows.play_sound(LEAVE_SOUND);
                self.leave_list(rom, scripts, windows)
            }
            _ => self.show_wares(rom, windows),
        }
    }
    /// …が…個で…Ｇになります　お売りになりますか？, or for a part
    /// …は…Ｇってとこか　売るかい？, at half the price, then はい / いいえ.
    pub(super) fn ask_to_sell(
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
        let total = sale_total(price(rom, entry), quantity, session.items());
        self.ask(
            rom,
            scripts,
            entry,
            quantity,
            total,
            scripts.sell_question,
            windows,
        )?;
        self.state = MenuState::Shop(ShopStep::ConfirmSell);
        Ok(())
    }
    /// はい sells: the goods leave, the money is printed again and the
    /// keeper thanks; いいえ gets …… or a jibe. B asks how many again, or
    /// for a part goes back to the wares.
    pub(super) fn sell_answer(
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
        let items = session.items();
        let Some(entry) = session.chosen() else {
            return Ok(());
        };
        if code != CONFIRMED {
            windows.play_sound(LEAVE_SOUND);
            if let Goods::Item(item) = entry {
                let count = saga_shop::item_count(&self.game_state, item);
                return self.ask_quantity(rom, scripts.how_many_to_sell, u32::from(count), windows);
            }
            self.set_redraw(Redraw::Help);
            return self.enter_wares(rom, windows);
        }
        if line == 0 {
            let total = sale_total(price(rom, entry), quantity, items);
            let (count, _) = self.count(entry);
            self.set_count(
                entry,
                count.saturating_sub(u8::try_from(quantity).unwrap_or(u8::MAX)),
            );
            self.party.money = self
                .party
                .money
                .saturating_add(total)
                .min(saga_shop::MONEY_LIMIT);
            self.reprint_money(rom, windows)?;
            self.run_now(rom, scripts.thanks, windows)?;
        } else {
            self.run_now(rom, scripts.refused, windows)?;
        }
        self.answered(rom, ShopStep::Sold, windows)
    }
}
