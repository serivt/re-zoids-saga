//! The towns' shops: what each sells and what the party holds of it.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the shop
//! dispatcher at `0x08008F58` and the talk handlers that call it
//! (`0x080090F0` on), the item shop's task at `0x0805378C` with its sell
//! list at `0x0804E2C4`, and the armaments shop's task at `0x0805451C`;
//! checked against Arcana's shops in a reference emulator.
//!
//! The item shops sell two kinds of goods, each with its own names, texts,
//! prices and counts:
//!
//! | Kind | Names | Texts | Prices (ROM) | Counts (game state) |
//! |---|---|---|---|---|
//! | 0, consumables | `name` 241 + id | `item` 63 + id | `0x75BE24` | `+0x3305` |
//! | 1, Zoid cores | `item` id | `pause-menu` 572 + id | `0x75BE40` | `+0x330C` |
//!
//! The armaments shops sell parts at their record's price (`+4` of the
//! 24-byte records at ROM `0x66C8F8`) into the parts' stock.

use crate::saga_party::{STOCK_LIMIT, STOCKED_PARTS};

const ITEM_SHOPS: usize = 0x0075_BCD4;
const ITEM_SHOP_LEN: usize = 16;
const ARMS_SHOPS: usize = 0x0075_BF40;
const ARMS_SHOP_LEN: usize = 8;
const SHOP_GOODS: usize = 4;
const END_OF_ITEMS: u8 = 0xFF;
const END_OF_PARTS: u16 = 0xFFFF;
const CONSUMABLE_PRICES: usize = 0x0075_BE24;
const CORE_PRICES: usize = 0x0075_BE40;
const CONSUMABLE_COUNTS: usize = 0x3305;
const CORE_COUNTS: usize = 0x330C;
const PARTS_STOCK: usize = 0x334C;
const PART_RECORDS: usize = 0x0066_C8F8;
const PART_RECORD_LEN: usize = 24;
const PART_PRICE: usize = 4;
/// The armaments shops the keeper of area 10's map 337 (`0x080093FC`)
/// picks from: twelve bytes at ROM `0x666E74`.
const ROTATING_ARMS_SHOPS: usize = 0x0066_6E74;
const ROTATING_ARMS_SHOP_COUNT: u16 = 12;

/// Consumables there are: the ones the item shops sell back.
pub const CONSUMABLES: u8 = 7;
/// The most of an item the party carries.
pub const ITEM_LIMIT: u8 = 99;
/// The most of a part the stock keeps.
pub const PART_LIMIT: u8 = STOCK_LIMIT;
/// The most money the party keeps.
pub const MONEY_LIMIT: u32 = 9_999_999;
/// Where the consumables' names start in the `name` table.
pub const CONSUMABLE_NAMES: usize = 241;
/// Where the consumables' texts start in the `item` table.
pub const CONSUMABLE_TEXTS: usize = 63;
/// Where the cores' texts start in the pause menu's scripts.
pub const CORE_TEXTS: usize = 572;

/// The two kinds of goods an item shop sells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// Kind 0: recovery items and the like, which the shops buy back.
    Consumable,
    /// Kind 1: the Zoid cores the lab builds Zoids from.
    Core,
}

/// An item: its kind and its index among that kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item {
    /// Which table the item's name, text, price and count come from.
    pub kind: ItemKind,
    /// The item's index within its kind.
    pub id: u8,
}

/// The goods of item shop `shop`: up to four entries of the 16-byte
/// records at ROM `0x75BCD4`, each a kind and an index, ended by kind
/// `0xFF`. `None` when the table lies outside `rom`.
#[must_use]
pub fn item_shop(rom: &[u8], shop: u8) -> Option<Vec<Item>> {
    let at = ITEM_SHOPS + usize::from(shop) * ITEM_SHOP_LEN;
    let record = rom.get(at..at + ITEM_SHOP_LEN)?;
    Some(
        record
            .chunks_exact(ITEM_SHOP_LEN / SHOP_GOODS)
            .take_while(|entry| entry[0] != END_OF_ITEMS)
            .map(|entry| Item {
                kind: if entry[0] == 0 {
                    ItemKind::Consumable
                } else {
                    ItemKind::Core
                },
                id: entry[1],
            })
            .collect(),
    )
}

/// The parts armaments shop `shop` sells: up to four half-words of the
/// 8-byte records at ROM `0x75BF40`, ended by `0xFFFF`. `None` when the
/// table lies outside `rom`.
#[must_use]
pub fn arms_shop(rom: &[u8], shop: u8) -> Option<Vec<u16>> {
    let at = ARMS_SHOPS + usize::from(shop) * ARMS_SHOP_LEN;
    let record = rom.get(at..at + ARMS_SHOP_LEN)?;
    Some(
        record
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|&part| part != END_OF_PARTS)
            .collect(),
    )
}

/// The armaments shop the keeper of area 10's map 337 opens (`0x080093FC`):
/// entry `wins % 12` of the bytes at ROM `0x666E74`, `wins` being the
/// roaming battles won as the game was started or continued (IWRAM
/// `0x030022DC`, a copy of the game state's `+0x0A` the continue makes).
/// `None` when the table lies outside `rom`.
#[must_use]
pub fn rotating_arms_shop(rom: &[u8], wins: u16) -> Option<u8> {
    rom.get(ROTATING_ARMS_SHOPS + usize::from(wins % ROTATING_ARMS_SHOP_COUNT))
        .copied()
}

/// What `item` costs; the shops buy it back for half.
#[must_use]
pub fn item_price(rom: &[u8], item: Item) -> u32 {
    let table = match item.kind {
        ItemKind::Consumable => CONSUMABLE_PRICES,
        ItemKind::Core => CORE_PRICES,
    };
    word_at(rom, table + usize::from(item.id) * 4)
}

/// What part `id` costs; the shops buy it back for half.
#[must_use]
pub fn part_price(rom: &[u8], id: u16) -> u32 {
    word_at(
        rom,
        PART_RECORDS + usize::from(id) * PART_RECORD_LEN + PART_PRICE,
    )
}

/// How many of `item` the party carries.
#[must_use]
pub fn item_count(state: &[u8], item: Item) -> u8 {
    state.get(count_at(item)).copied().unwrap_or(0)
}

/// Sets how many of `item` the party carries.
pub fn set_item_count(state: &mut [u8], item: Item, count: u8) {
    if let Some(slot) = state.get_mut(count_at(item)) {
        *slot = count;
    }
}

/// Sets how many of part `id` the stock keeps; parts beyond the stock's
/// 150 have no count.
pub fn set_stock(state: &mut [u8], id: u16, count: u8) {
    if usize::from(id) >= STOCKED_PARTS {
        return;
    }
    if let Some(slot) = state.get_mut(PARTS_STOCK + usize::from(id)) {
        *slot = count;
    }
}

/// The consumables the party carries, in id order (`0x0804E2C4`): what
/// an item shop offers to buy.
#[must_use]
pub fn held_consumables(state: &[u8]) -> Vec<u8> {
    (0..CONSUMABLES)
        .filter(|&id| {
            item_count(
                state,
                Item {
                    kind: ItemKind::Consumable,
                    id,
                },
            ) > 0
        })
        .collect()
}

fn count_at(item: Item) -> usize {
    let table = match item.kind {
        ItemKind::Consumable => CONSUMABLE_COUNTS,
        ItemKind::Core => CORE_COUNTS,
    };
    table + usize::from(item.id)
}

fn word_at(rom: &[u8], at: usize) -> u32 {
    rom.get(at..at + 4).map_or(0, |bytes| {
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn rom_with(at: usize, bytes: &[u8]) -> Vec<u8> {
        let mut rom = vec![0; at + bytes.len()];
        rom[at..].copy_from_slice(bytes);
        rom
    }

    #[test]
    fn the_rotating_shop_follows_the_wins_round_twelve() {
        let table: Vec<u8> = (1..=12).collect();
        let rom = rom_with(ROTATING_ARMS_SHOPS, &table);
        assert_eq!(rotating_arms_shop(&rom, 0), Some(1));
        assert_eq!(rotating_arms_shop(&rom, 11), Some(12));
        assert_eq!(rotating_arms_shop(&rom, 25), Some(2));
        assert_eq!(rotating_arms_shop(&[], 0), None);
    }

    #[test]
    fn an_item_shops_goods_end_at_kind_ff() {
        let rom = rom_with(
            ITEM_SHOPS + ITEM_SHOP_LEN,
            &[0, 2, 0, 0, 1, 5, 0, 0, 0xFF, 0xFF, 0, 0, 0, 3, 0, 0],
        );
        let goods = item_shop(&rom, 1).unwrap();
        assert_eq!(
            goods,
            vec![
                Item {
                    kind: ItemKind::Consumable,
                    id: 2
                },
                Item {
                    kind: ItemKind::Core,
                    id: 5
                },
            ]
        );
    }

    #[test]
    fn an_arms_shop_lists_up_to_four_parts() {
        let rom = rom_with(ARMS_SHOPS, &[0x15, 0, 0x29, 0, 0xFF, 0xFF, 0x11, 0]);
        assert_eq!(arms_shop(&rom, 0).unwrap(), vec![0x15, 0x29]);
    }

    #[test]
    fn counts_live_in_the_game_state() {
        let mut state = vec![0; 0x3400];
        let item = Item {
            kind: ItemKind::Consumable,
            id: 4,
        };
        set_item_count(&mut state, item, 3);
        assert_eq!(state[CONSUMABLE_COUNTS + 4], 3);
        assert_eq!(item_count(&state, item), 3);
        assert_eq!(held_consumables(&state), vec![4]);
        set_stock(&mut state, 0x15, 2);
        assert_eq!(state[PARTS_STOCK + 0x15], 2);
    }
}
