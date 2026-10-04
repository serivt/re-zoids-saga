//! The two releases of Zoids Saga's cartridge, and where each keeps what
//! the port reads.
//!
//! The port names every table by where Rev 1 keeps it. The first release
//! (Rev 0) runs the same program over the same data, but keeps the data a
//! few hundred bytes earlier: Rev 1 rewrote some code and some text, and
//! everything after each change moved. A table's own pointers already lead
//! to the release's data, so only the places the port knows by heart need
//! moving: [`locate`] turns a Rev 1 offset into the loaded image's own.
//!
//! Rev 1 also added one script to the menus' table (163 of the pause
//! menu's table); [`lacks`] tells such an entry apart, and the scripts
//! after it follow one place earlier.
//!
//! Rev 1 retouched the text of some messages, which moved the messages
//! after them within their strings; [`Revision::message_offset`] gives a
//! message the offset Rev 1 keeps it at, so translations, which name
//! messages by it, read the same in both releases.
//!
//! The release is read from the image's own header (the game code `ATZJ`
//! and the software version at `0xBC`), so every reader of a `&[u8]`
//! follows the release it was given.
//!
//! Source of knowledge: own comparison of Zoids Saga (Japan) and Zoids Saga
//! (Japan, Rev 1): a word-by-word alignment of the two images, the table
//! addresses each release's code holds in its literal pools, and each
//! table's records read from both (see `docs/formats/revisions.md`).

use core::ops::Range;

/// A release of Zoids Saga's cartridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Revision {
    /// The first release, version 0 in the header.
    Rev0,
    /// The revised release, version 1, whose layout the port names.
    Rev1,
}

const GAME_CODE: Range<usize> = 0xAC..0xB0;
const SAGA_GAME_CODE: &[u8] = b"ATZJ";
const VERSION: usize = 0xBC;
const FIRST_VERSION: u8 = 0;
const ROM_BASE: u32 = 0x0800_0000;

/// From where, in Rev 1's offsets, the first release keeps its bytes `by`
/// bytes away; each shift holds until the next one.
#[derive(Debug, Clone, Copy)]
struct Shift {
    from: usize,
    by: isize,
}

const fn shift(from: usize, by: isize) -> Shift {
    Shift { from, by }
}

/// The first release's shifts. The code up to `0x59528` moved with each
/// rewritten function; the data after it moved with the code, then with
/// each stretch Rev 1 changed: at `0x534FEC`, the strings at `0x674710`
/// and `0x6C8BA0`, at `0x6FC158`, the guides' scripts at `0x702CE4`, the
/// dialogue at `0x70852C`, the battle and menu scripts at `0x756D54` and
/// the menu script Rev 1 added at `0x75B448`.
const REV0_SHIFTS: &[Shift] = &[
    shift(0x0003_6A30, -148),
    shift(0x0004_EAC8, -152),
    shift(0x0005_2190, -180),
    shift(0x0005_534C, -236),
    shift(0x0005_70E8, -216),
    shift(0x0005_8D8C, -200),
    shift(0x0005_9528, -280),
    shift(0x0053_5E6C, -420),
    shift(0x0067_5D10, -424),
    shift(0x006C_8BBC, -420),
    shift(0x006F_ED54, -416),
    shift(0x0070_2CE8, -380),
    shift(0x0070_852C, -452),
    shift(0x0075_A1A0, -516),
    shift(0x0075_B44C, -520),
];

/// Rev 1's bytes the first release does not have: the menus' table entry
/// Rev 1 added.
const REV0_ADDED: Range<usize> = 0x0075_B448..0x0075_B44C;

/// From which offset of a script string the first release keeps its
/// messages `by` bytes away from Rev 1's; each holds to the string's end
/// or the next one.
#[derive(Debug, Clone, Copy)]
struct MessageShift {
    table: &'static str,
    index: usize,
    from: usize,
    by: isize,
}

const fn message_shift(table: &'static str, index: usize, from: usize, by: isize) -> MessageShift {
    MessageShift {
        table,
        index,
        from,
        by,
    }
}

/// The first release's message shifts, by its own offsets, ordered by
/// table, string and offset.
const REV0_MESSAGE_SHIFTS: &[MessageShift] = &[
    message_shift("battle", 12, 0x7A, -2),
    message_shift("battle", 221, 0x411, 2),
    message_shift("battle-label", 21, 0xD7F, 2),
    message_shift("battle-label", 31, 0x1159, 2),
    message_shift("battle-text", 133, 0x6D3, 2),
    message_shift("dialogue", 63, 0xA3, 1),
    message_shift("dialogue", 71, 0x4E, 1),
    message_shift("dialogue", 207, 0x46, 2),
    message_shift("dialogue", 216, 0x321, 1),
    message_shift("dialogue", 299, 0x7F2, -2),
    message_shift("dialogue", 301, 0x619, -2),
    message_shift("dialogue", 301, 0x97B, -4),
    message_shift("dialogue", 330, 0x50, -2),
    message_shift("dialogue", 334, 0x4A, 1),
    message_shift("dialogue", 338, 0x7E, -1),
    message_shift("dialogue", 349, 0x7C, 1),
    message_shift("dialogue", 367, 0x365, -2),
    message_shift("dialogue", 372, 0x8C, -1),
    message_shift("dialogue", 376, 0x90, -1),
    message_shift("dialogue", 380, 0x8C, -1),
    message_shift("dialogue", 384, 0x250, -1),
    message_shift("dialogue", 511, 0x377, 1),
    message_shift("dialogue", 558, 0x485, 1),
    message_shift("dialogue", 608, 0x64, 1),
    message_shift("dialogue", 621, 0x253, 1),
    message_shift("item", 147, 0xD32, 2),
    message_shift("system", 4, 0xBE, -4),
    message_shift("zoid-guide", 428, 0x12, -2),
];

impl Revision {
    /// The release of the image `rom`. Anything but the first release's
    /// header reads as Rev 1, so an image without the game's header (a
    /// test's) keeps the offsets as they are.
    #[must_use]
    pub fn of(rom: &[u8]) -> Self {
        let first =
            rom.get(GAME_CODE) == Some(SAGA_GAME_CODE) && rom.get(VERSION) == Some(&FIRST_VERSION);
        if first { Self::Rev0 } else { Self::Rev1 }
    }

    /// Where this release keeps the byte Rev 1 keeps at `offset`.
    #[must_use]
    pub fn locate(self, offset: usize) -> usize {
        match self {
            Self::Rev1 => offset,
            Self::Rev0 => REV0_SHIFTS
                .iter()
                .rev()
                .find(|shift| shift.from <= offset)
                .map_or(offset, |shift| offset.saturating_add_signed(shift.by)),
        }
    }

    /// The offset Rev 1 keeps the message this release keeps `offset`
    /// bytes into string `index` of script table `table` at.
    #[must_use]
    pub fn message_offset(self, table: &str, index: usize, offset: usize) -> usize {
        match self {
            Self::Rev1 => offset,
            Self::Rev0 => REV0_MESSAGE_SHIFTS
                .iter()
                .rev()
                .find(|shift| shift.table == table && shift.index == index && shift.from <= offset)
                .map_or(offset, |shift| offset.saturating_add_signed(shift.by)),
        }
    }

    /// Whether this release lacks the byte Rev 1 keeps at `offset`.
    #[must_use]
    pub fn lacks(self, offset: usize) -> bool {
        match self {
            Self::Rev1 => false,
            Self::Rev0 => REV0_ADDED.contains(&offset),
        }
    }
}

/// Where the image `rom` keeps the byte Rev 1 keeps at `offset`.
#[must_use]
pub fn locate(rom: &[u8], offset: usize) -> usize {
    Revision::of(rom).locate(offset)
}

/// Where the image `rom` keeps what Rev 1 keeps at the ROM address
/// `address` (`0x08000000` and up); other addresses stay as they are.
#[must_use]
pub fn locate_address(rom: &[u8], address: u32) -> u32 {
    let Some(offset) = address
        .checked_sub(ROM_BASE)
        .and_then(|offset| usize::try_from(offset).ok())
    else {
        return address;
    };
    u32::try_from(locate(rom, offset))
        .ok()
        .and_then(|offset| offset.checked_add(ROM_BASE))
        .unwrap_or(address)
}

/// Whether the image `rom` lacks the byte Rev 1 keeps at `offset`, one
/// Rev 1 added.
#[must_use]
pub fn lacks(rom: &[u8], offset: usize) -> bool {
    Revision::of(rom).lacks(offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(version: u8) -> Vec<u8> {
        let mut rom = vec![0; 0xC0];
        rom[GAME_CODE].copy_from_slice(SAGA_GAME_CODE);
        rom[VERSION] = version;
        rom
    }

    #[test]
    fn the_header_names_the_release() {
        assert_eq!(Revision::of(&header(0)), Revision::Rev0);
        assert_eq!(Revision::of(&header(1)), Revision::Rev1);
        assert_eq!(Revision::of(&[0; 0xC0]), Revision::Rev1);
        assert_eq!(Revision::of(&[]), Revision::Rev1);
        let mut fuzors = header(0);
        fuzors[GAME_CODE].copy_from_slice(b"BZFJ");
        assert_eq!(Revision::of(&fuzors), Revision::Rev1);
    }

    #[test]
    fn rev1_keeps_every_offset() {
        let rom = header(1);
        for offset in [0, 0x0005_9528, 0x0075_B448, 0x007F_FFFF] {
            assert_eq!(locate(&rom, offset), offset);
            assert!(!lacks(&rom, offset));
        }
    }

    #[test]
    fn the_first_release_moves_each_stretch_by_its_shift() {
        let rom = header(0);
        assert_eq!(locate(&rom, 0x0002_AB08), 0x0002_AB08);
        assert_eq!(locate(&rom, 0x0005_9527), 0x0005_9527 - 200);
        assert_eq!(locate(&rom, 0x0005_9528), 0x0005_9528 - 280);
        assert_eq!(locate(&rom, 0x0074_FC54), 0x0074_FA90);
        assert_eq!(locate(&rom, 0x0075_B1BC), 0x0075_AFB8);
        assert_eq!(locate(&rom, 0x0075_B44C), 0x0075_B244);
    }

    #[test]
    fn an_entry_rev1_added_is_lacking_and_the_next_one_moves_back() {
        let rom = header(0);
        assert!(lacks(&rom, 0x0075_B448));
        assert!(!lacks(&rom, 0x0075_B444));
        assert!(!lacks(&rom, 0x0075_B44C));
        assert_eq!(locate(&rom, 0x0075_B444) + 4, locate(&rom, 0x0075_B44C));
    }

    #[test]
    fn addresses_move_like_offsets() {
        let rom = header(0);
        assert_eq!(locate_address(&rom, 0x0866_6F50), 0x0866_6F50 - 420);
        assert_eq!(locate_address(&rom, 0x0800_C73C), 0x0800_C73C);
        assert_eq!(locate_address(&rom, 0x0200_0000), 0x0200_0000);
    }

    #[test]
    fn the_shifts_are_in_order() {
        assert!(
            REV0_SHIFTS
                .windows(2)
                .all(|pair| pair[0].from < pair[1].from)
        );
        assert!(REV0_MESSAGE_SHIFTS.windows(2).all(|pair| {
            (pair[0].table, pair[0].index, pair[0].from)
                < (pair[1].table, pair[1].index, pair[1].from)
        }));
    }

    #[test]
    fn messages_after_a_retouched_one_take_rev1s_offsets() {
        let first = Revision::Rev0;
        assert_eq!(first.message_offset("dialogue", 301, 0x618), 0x618);
        assert_eq!(first.message_offset("dialogue", 301, 0x619), 0x617);
        assert_eq!(first.message_offset("dialogue", 301, 0x97B), 0x977);
        assert_eq!(first.message_offset("dialogue", 302, 0x97B), 0x97B);
        assert_eq!(first.message_offset("item", 147, 0xD32), 0xD34);
        assert_eq!(Revision::Rev1.message_offset("dialogue", 301, 0x97B), 0x97B);
    }
}
