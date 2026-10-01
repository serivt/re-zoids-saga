//! The weapons' reach in the enhanced mode, a port feature: the armaments
//! shops, the pause menu's weapons list (武器) and its equipment screen
//! (武装) draw, beside the part under the cursor, what it reaches on the
//! battle's grid: the other side on the left and the user's on the right,
//! their front rows facing, as the battle lays them out. SELECT turns the
//! user from the front row to the back and back.
//!
//! Source of knowledge: this project's own design over the aim's reading of
//! the grid (see [`crate::combat::reach`]); the plate takes the windows'
//! background and ink, as the auto text's mark does.

use extraction::saga_party;
use platform::{Button, Frame, Input, Rgb};

use super::{MENU_MOVE_SOUND, MenuState, PauseMenu, equipment, shop};
use crate::combat::reach::{self, CELLS, Mark, Reach};
use crate::port_text::{REACH_BACK, REACH_FRONT, port_text};
use crate::windows::ScriptWindows;
use crate::{ScriptHost, TextPainter, WindowPainter};

/// Where the plate goes, its top at the row given: in the weapons list,
/// its right edge on the left window's inner edge, under the part's values;
/// in the equipment screen, from the left over the wallpaper above the
/// Zoid; in a shop, centred on the gap of wallpaper between the count and
/// the money. The plate widens for a long row name.
const STOCK_AT: Anchor = Anchor::Right(129, 74);
const EQUIP_AT: Anchor = Anchor::Left(4, 4);
const SHOP_AT: Anchor = Anchor::Center(120, 81);
/// The cell the user stands on when no formation says: the front row's
/// middle.
const MIDDLE_FRONT: usize = 1;
/// The plate's look: a cell's side, the gap between cells, between the two
/// sides and around them inside the frame, and the label's rows.
const CELL: usize = 5;
const CELL_GAP: usize = 1;
const SIDES_GAP: usize = 5;
const PADDING: usize = 2;
const LABEL_ROWS: usize = 5;
const LABEL_GAP: usize = 2;
/// The rows of the small capitals above their letters, where the accents
/// go.
const LABEL_TOP: usize = 4;
/// The skin's colors the plate takes: the windows' background and ink.
const BACKGROUND_COLOR: u8 = 1;
const INK_COLOR: u8 = 15;
/// The cells' colors: an empty cell a shade under the plate, the user, a
/// cell within reach, and the cells one shot takes.
const EMPTY_SHADE: u16 = 72;
const USER: Rgb = Rgb::new(96, 160, 248);
const REACHED: Rgb = Rgb::new(248, 208, 112);
const SHOT: Rgb = Rgb::new(248, 112, 40);

/// Where a plate goes: its left edge, its right edge or its middle at the
/// column given, its top at the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Anchor {
    Left(usize, usize),
    Right(usize, usize),
    Center(usize, usize),
}

impl Anchor {
    /// The top-left corner of a plate `width` pixels wide.
    fn corner(self, width: usize) -> (usize, usize) {
        match self {
            Self::Left(x, y) => (x, y),
            Self::Right(x, y) => (x.saturating_sub(width), y),
            Self::Center(x, y) => (x.saturating_sub(width / 2), y),
        }
    }
}

/// The picture of the part under the cursor, where it goes, and whether
/// its user stands in the back row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReachPanel {
    reach: Reach,
    at: Anchor,
    back_row: bool,
}

impl PauseMenu {
    /// SELECT turns the weapons' reach to the other row while it shows,
    /// with the menus' move sound; `input` is this frame's buttons and the
    /// menu still holds the last frame's.
    pub(super) fn turn_reach_row(&mut self, input: Input, windows: &mut ScriptWindows<'_>) {
        let pressed = input.is_held(Button::Select) && !self.held.is_held(Button::Select);
        if pressed && self.reach.is_some() {
            self.reach_back = !self.reach_back;
            windows.play_sound(MENU_MOVE_SOUND);
        }
    }

    /// Works out the weapons' reach for the part under the cursor, when the
    /// enhanced mode shows it and the screen has one.
    pub(super) fn refresh_reach(&mut self, rom: &[u8]) {
        let shown = self
            .enhancements()
            .is_some_and(|enhancements| enhancements.weapon_reach);
        self.reach =
            shown
                .then(|| self.reach_target())
                .flatten()
                .and_then(|(part, standing, at)| {
                    let user = if self.reach_back {
                        (standing + CELLS / 2) % CELLS
                    } else {
                        standing
                    };
                    let reach = reach::reach(rom, part, user)?;
                    Some(ReachPanel {
                        reach,
                        at,
                        back_row: user >= CELLS / 2,
                    })
                });
    }

    /// The part under the cursor, the cell its user stands on and where the
    /// plate goes, on the screens that show a weapon's reach.
    fn reach_target(&self) -> Option<(u16, usize, Anchor)> {
        match self.state {
            MenuState::Stock => {
                let part = self.stock.get(self.stock_shown?).copied()?;
                Some((part, MIDDLE_FRONT, STOCK_AT))
            }
            MenuState::Equip => {
                let part = equipment::shown_part(self)?;
                let character = equipment::character(self);
                let standing = saga_party::formation(&self.game_state)
                    .iter()
                    .position(|slot| slot.is_some_and(|(_, pilot)| pilot == character))
                    .unwrap_or(MIDDLE_FRONT);
                Some((part, standing, EQUIP_AT))
            }
            MenuState::Shop(step) => {
                let part = shop::shown_part(self, step)?;
                Some((part, MIDDLE_FRONT, SHOP_AT))
            }
            _ => None,
        }
    }

    /// Draws the weapons' reach, when it shows.
    pub(super) fn draw_reach(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
    ) {
        let Some(panel) = self.reach else {
            return;
        };
        let key = if panel.back_row {
            REACH_BACK
        } else {
            REACH_FRONT
        };
        let label = port_text(windows.extensions(), key);
        draw_panel(frame, &panel, &label, skin, painter);
    }
}

/// The plate: its frame, the label and the two sides' cells.
fn draw_panel(
    frame: &mut Frame,
    panel: &ReachPanel,
    label: &str,
    skin: &WindowPainter,
    painter: &TextPainter,
) {
    let (background, ink) = (
        skin.palette().color(BACKGROUND_COLOR),
        skin.palette().color(INK_COLOR),
    );
    let grid = (2 * CELL + CELL_GAP, 3 * CELL + 2 * CELL_GAP);
    let sides = 2 * grid.0 + SIDES_GAP;
    let inner = sides.max(painter.metrics().small_width(label));
    let width = inner + 2 * (PADDING + 1);
    let height = LABEL_ROWS + LABEL_GAP + grid.1 + 2 * (PADDING + 1);
    let (x, y) = panel.at.corner(width);
    for row in y..y + height {
        for column in x..x + width {
            let edge = row == y || row == y + height - 1 || column == x || column == x + width - 1;
            frame.set_pixel(column, row, if edge { ink } else { background });
        }
    }
    let left = x + PADDING + 1;
    let cells_left = left + (inner - sides) / 2;
    let top = y + PADDING + 1;
    painter
        .metrics()
        .draw_small(frame, (left, top.saturating_sub(LABEL_TOP)), label, ink);
    let cells_top = top + LABEL_ROWS + LABEL_GAP;
    let empty = shade(background);
    let color = |mark: Mark| match mark {
        Mark::Empty => empty,
        Mark::User => USER,
        Mark::Reached => REACHED,
        Mark::Shot => SHOT,
    };
    for cell in 0..CELLS {
        let row = 2 - cell % 3;
        let back = cell >= CELLS / 2;
        let enemy_column = usize::from(!back);
        let own_column = usize::from(back);
        let place = |origin: usize, column: usize| {
            (
                origin + column * (CELL + CELL_GAP),
                cells_top + row * (CELL + CELL_GAP),
            )
        };
        fill(
            frame,
            place(cells_left, enemy_column),
            color(panel.reach.enemy[cell]),
        );
        let own_left = cells_left + grid.0 + SIDES_GAP;
        fill(
            frame,
            place(own_left, own_column),
            color(panel.reach.own[cell]),
        );
    }
}

fn fill(frame: &mut Frame, (x, y): (usize, usize), color: Rgb) {
    for row in y..y + CELL {
        for column in x..x + CELL {
            frame.set_pixel(column, row, color);
        }
    }
}

/// `color` darkened a little, for the empty cells.
fn shade(color: Rgb) -> Rgb {
    let dim = |channel: u8| {
        let value = u16::from(channel) * (256 - EMPTY_SHADE) / 256;
        u8::try_from(value).unwrap_or(channel)
    };
    Rgb::new(dim(color.r), dim(color.g), dim(color.b))
}
