//! The damage numbers, a port feature of the enhanced mode: as the hit
//! display starts, each unit a blow landed on shows the damage it took
//! under it for a moment, besides the message, in the orange digits of the
//! figures L shows (see `docs/combat.md`).
//!
//! Source of knowledge: this project's own design; the digits are the
//! figures' tiles and palette, the places the slots' (see [`super`]).

use gba_runtime::ppu::Palette;
use platform::Frame;

use super::attack::Blow;
use super::{Combat, HEIGHT, LABEL_HP_DIGITS, WIDTH};

/// Frames a number stays on screen, and the last of them, in which it
/// blinks before it goes.
const SHOWN_FRAMES: u32 = 90;
const BLINK_FRAMES: u32 = 20;
/// Frames of a blink's half.
const BLINK_HALF: u32 = 4;
/// Pixels a number rises as it shows, one a frame.
const RISE: u32 = 6;
/// The largest damage shown; the figures' four digits.
const LARGEST: u32 = 9999;
/// Pixels from one digit to the next.
const ADVANCE: i32 = 7;
/// The lowest a number's top rests, so that it and its outline end above
/// the message window's top (row 128); it rests right under the unit's
/// place, the bottom of its sprite, when that is higher.
const LOWEST_TOP: i32 = 119;
/// The digits' shadow color, which outlines them.
const OUTLINE: u8 = 14;

/// Whether the numbers show, those on screen, and those the frame shows
/// (the sprites' OAM of the frame before).
#[derive(Debug, Default)]
pub(super) struct DamageNumbers {
    shown: bool,
    on_screen: Vec<DamageNumber>,
    latched: Vec<DamageNumber>,
}

impl DamageNumbers {
    /// Keeps the numbers the frame shows.
    pub(super) fn latch(&mut self) {
        self.latched.clone_from(&self.on_screen);
    }

    /// A frame of the numbers: each ages, and goes once its time is over.
    fn age(&mut self) {
        for number in &mut self.on_screen {
            number.age += 1;
        }
        self.on_screen.retain(|number| number.age < SHOWN_FRAMES);
    }
}

/// A unit's damage on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DamageNumber {
    side: usize,
    slot: usize,
    damage: u32,
    /// Frames since it showed.
    age: u32,
}

impl DamageNumber {
    /// Whether it shows this frame: all along, then blinking.
    fn visible(self) -> bool {
        let blinking = self.age.saturating_sub(SHOWN_FRAMES - BLINK_FRAMES);
        (blinking / BLINK_HALF) % 2 == 0
    }

    /// How far below its resting place it is, as it rises.
    fn rise_left(self) -> i32 {
        i32::try_from(RISE.saturating_sub(self.age)).unwrap_or(0)
    }
}

impl Combat {
    /// Shows the damage each unit takes as a number under it, as the
    /// enhanced mode can (a port feature), or only the messages, as the
    /// original.
    pub fn set_damage_numbers(&mut self, shown: bool) {
        let numbers = &mut self.fight.numbers;
        numbers.shown = shown;
        if !shown {
            numbers.on_screen.clear();
        }
    }

    /// The numbers of the blows that landed, in place of the last ones.
    pub(super) fn show_damage(&mut self, blows: &[Blow]) {
        let numbers = &mut self.fight.numbers;
        if !numbers.shown {
            return;
        }
        numbers.on_screen = blows
            .iter()
            .filter(|blow| blow.landed())
            .map(|blow| DamageNumber {
                side: blow.side,
                slot: blow.slot,
                damage: u32::try_from(blow.damage >> 16).unwrap_or(0).min(LARGEST),
                age: 0,
            })
            .collect();
    }

    /// A frame of the numbers.
    pub(super) fn age_numbers(&mut self) {
        self.fight.numbers.age();
    }

    /// The numbers, each centered under its unit's place and outlined in
    /// the digits' shadow color, over the units and their sparks; the
    /// message window covers what would reach it.
    pub(super) fn draw_numbers(&self, frame: &mut Frame) {
        let Some(glyphs) = self.figure_glyphs.as_ref() else {
            return;
        };
        let palette = Palette::new(glyphs.palette.map(Palette::from_bgr555));
        let shown = self.fight.numbers.latched.iter();
        for number in shown.filter(|number| number.visible()) {
            let (x, y) = self
                .row_move_at(number.side, number.slot)
                .unwrap_or(self.anchors[number.side][number.slot]);
            let digits = number.damage.to_string();
            let count = i32::try_from(digits.len()).unwrap_or(0);
            let left = x - count * ADVANCE / 2;
            let top = (y - self.shown_scrolls.1).min(LOWEST_TOP) + number.rise_left();
            let mut pixels: Vec<(i32, i32, u8)> = Vec::new();
            for (place, digit) in digits.bytes().enumerate() {
                let tile = LABEL_HP_DIGITS + usize::from(digit - b'0');
                let Some(tile) = glyphs.tiles.tile(tile) else {
                    continue;
                };
                let left = left + ADVANCE * i32::try_from(place).unwrap_or(0);
                pixels.extend(
                    tile.iter()
                        .enumerate()
                        .filter(|(_, color)| **color != 0)
                        .map(|(index, &color)| {
                            (
                                left + i32::try_from(index % 8).unwrap_or(0),
                                top + i32::try_from(index / 8).unwrap_or(0),
                                color,
                            )
                        }),
                );
            }
            let outline = pixels.iter().flat_map(|&(px, py, _)| {
                (-1..=1).flat_map(move |dy| (-1..=1).map(move |dx| (px + dx, py + dy, OUTLINE)))
            });
            for (px, py, color) in outline.collect::<Vec<_>>().into_iter().chain(pixels) {
                if let (Ok(px), Ok(py)) = (usize::try_from(px), usize::try_from(py))
                    && px < WIDTH
                    && py < HEIGHT
                {
                    frame.set_pixel(px, py, palette.color(color));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(age: u32) -> DamageNumber {
        DamageNumber {
            side: 0,
            slot: 0,
            damage: 79,
            age,
        }
    }

    #[test]
    fn a_number_goes_once_its_time_is_over() {
        let mut numbers = DamageNumbers {
            shown: true,
            on_screen: vec![number(SHOWN_FRAMES - 2)],
            latched: Vec::new(),
        };
        numbers.age();
        numbers.latch();
        assert_eq!(numbers.latched, [number(SHOWN_FRAMES - 1)]);
        numbers.age();
        assert!(numbers.on_screen.is_empty());
    }

    #[test]
    fn a_number_rises_then_rests_and_blinks_before_it_goes() {
        assert_eq!(number(0).rise_left(), 6);
        assert_eq!(number(RISE).rise_left(), 0);
        assert!(number(0).visible());
        let blinking = SHOWN_FRAMES - BLINK_FRAMES;
        assert!(number(blinking - 1).visible());
        assert!(number(blinking).visible());
        assert!(!number(blinking + BLINK_HALF).visible());
        assert!(number(blinking + 2 * BLINK_HALF).visible());
    }
}
