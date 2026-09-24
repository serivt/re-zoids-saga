//! The real battles (`0x0800C3AC`): the screen the two sides face each
//! other on, the opening messages, the battle menu and the retreat.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! battle controller (task `0x0802AC0C` in slot 4), the opening and menu
//! task (`0x0802E9CC`, slot 5), the results task (`0x08035778`, slot 5),
//! the message wait (`0x08003F70`, slot 9, armed by `0x08004018`), the fade
//! task (`0x08004034`, slot 10), the screen's builder (`0x08031074`) and the
//! result routine (`0x08032684`); checked frame by frame against a battle
//! met on the world map in a reference emulator. See `docs/combat.md`.
//!
//! The original runs these as tasks, each one step a frame in slot order;
//! a script call blocks its task until it returns, and the task goes on in
//! the frame it does. The port keeps the same tasks and steps, so the
//! screen changes on the same frames.

use std::collections::VecDeque;

use extraction::saga::SpriteSheet;
use extraction::saga_combat::{self, Grounds, PanelGraphics, SLOTS};
use extraction::saga_encounter::{self, Formation};
use extraction::saga_formation::FieldLayer;
use extraction::saga_party;
use formats::tile::Tileset;
use gba_runtime::ppu::{FADE_STEPS, PaletteBank, darken, draw_background};
use platform::{Frame, Input, Rgb};

use crate::data::GameData;
use crate::script::{ScriptError, ScriptRunner};
use crate::translation::{BATTLE_MENU_TABLE, BATTLE_TEXT_TABLE, NAME_TABLE};
use crate::windows::ScriptWindows;
use crate::{TextPainter, WindowPainter, draw_sprite};

/// The song the battles play (`0x02005C70 + 3` after `0x080329E4`).
pub const BATTLE_SONG: u16 = 0x17;
/// The song the opening starts (`0x0802E9CC`, state 0).
const OPENING_SONG: u16 = 0x16;
/// The sound a retreat plays (`0x08035778`, state 0).
const RETREAT_SOUND: u16 = 0x53;
/// Frames from the controller's start to the opening task's.
const OPENING_START: u32 = 5;
/// Frames the screen's build takes before the fade task starts, besides
/// [`BUILD_PER_PANEL`] a panel: the scripts that name each panel's unit.
const BUILD_FRAMES: u32 = 9;
const BUILD_PER_PANEL: u32 = 8;
const BLACK: u8 = 31;
const MESSAGE_WINDOW: u8 = 1;
/// The opening's waits after each message, by message speed (the
/// game-state block's `+0x3618`, speed − 1): frames at ROM `0x664668`;
/// the slowest waits for A.
const MESSAGE_WAITS: [u16; 5] = [1, 15, 30, 45, 60];
const KEY_WAIT_SPEED: u8 = 5;
const SPEED_BYTE: usize = 0x3618;
const PLAYER: usize = 0;
const ENEMY: usize = 1;
const PANEL_SCROLL_ROWS: usize = 2;
const SECOND_BAR: u16 = 0x13;
const PANEL_BANK: u8 = 14;
/// The tile the text system loads the window frame's tiles from on BG1
/// (`0x0803E1C8`).
const FRAME_FIRST_TILE: usize = 0x56;
const TEXT_BANK: u8 = 15;

// Scripts of the battle-menu table.
const MENU_MAIN: u16 = 0;
const MENU_RESET: u16 = 1;
const MENU_MESSAGE_WINDOW: u16 = 2;
const MENU_PRESENT: u16 = 5;
const MENU_DRAW: u16 = 6;
const MENU_CLEAR: u16 = 7;
// Scripts of the battle-text table.
const TEXT_MET: u16 = 1;
const TEXT_RETREATED: u16 = 0x0D;
const TEXT_CONFIRMED: u16 = 0x0F;
const TEXT_READY: u16 = 0x10;
/// Where the names of the Zoids start in the name table (`0x08676020`).
const ZOID_NAMES: usize = 1;

/// The menu's lines.
const CHOICE_RETREAT: u16 = 4;

/// How a battle ended, as `0x0800C3AC` tells its caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The enemies were beaten or fled (result 1 or 2).
    Won,
    /// The party was beaten (result 3).
    Lost,
    /// The party retreated (result 4).
    Retreated,
}

/// A unit on the battle screen.
#[derive(Debug, Clone)]
struct Unit {
    zoid: u16,
    sheet: Option<SpriteSheet>,
}

/// A panel of the party's side: its unit's bars.
#[derive(Debug, Clone, Copy)]
struct Panel {
    hp: (u32, u32),
    ep: (u32, u32),
}

/// A script call the running task makes.
#[derive(Debug, Clone, Copy)]
enum Call {
    Menu(u16),
    Text(u16),
    Name(u16),
}

/// What the running slot-5 task does next once its calls return.
#[derive(Debug, Clone, Copy)]
enum Act {
    Call(Call),
    /// Arms the message wait (`0x08004018`).
    Wait,
    /// Starts the fade task in; fading out when `out`.
    Fade {
        out: bool,
    },
    Sound(u16),
    Music(u16),
}

/// The opening and menu task (`0x0802E9CC`), by the state it waits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opening {
    Build,
    FadeIn,
    AwaitFade,
    Met,
    AwaitMet,
    Check(usize),
    Next(usize),
    AwaitConfirmed(usize),
    Ready,
    AwaitReady,
    Menu,
    Retreat,
    Done,
}

/// The results task (`0x08035778`) after a retreat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Results {
    Idle,
    Start,
    Message,
    AwaitMessage,
    Finish,
    Done,
}

/// The controller (`0x0802AC0C`) after the opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Controller {
    Opening,
    StartResults,
    AwaitResults,
    FadeOut,
    AwaitFade,
    Finish,
    Done,
}

/// The message wait (`0x08003F70`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wait {
    Idle,
    Armed,
    Counting(u16),
    Key,
    Done,
}

/// The fade task (`0x08004034`): frames since it started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fade {
    out: bool,
    age: u32,
}

/// The frame, from the fade task's start, the tasks before it in the slot
/// order see that it ended.
const FADE_SEEN_AGE: u32 = 35;

/// A battle met on the field.
pub struct Combat {
    grid: Option<FieldLayer>,
    grounds: (Option<FieldLayer>, Option<FieldLayer>),
    panel_graphics: Option<PanelGraphics>,
    /// The window frame's tiles, which the text system loads from tile
    /// `0x56` on and the panels' frames use.
    frame_tiles: Option<Tileset>,
    banks: PaletteBank,
    panels: Vec<Panel>,
    units: [[Option<Unit>; SLOTS]; 2],
    anchors: [[(i32, i32); SLOTS]; 2],
    menu: ScriptRunner,
    text: ScriptRunner,
    names: ScriptRunner,
    acts: VecDeque<Act>,
    running: Option<Call>,
    frame: u32,
    build_frames: u32,
    controller: Controller,
    opening: Opening,
    results: Results,
    wait: Wait,
    wait_frames: Option<u16>,
    fade: Option<Fade>,
    level: u8,
    shown_level: u8,
    outcome: Option<Outcome>,
    choice: u16,
    sounds: Vec<u16>,
    music: Option<u16>,
    input: Input,
    previous: Input,
}

impl Combat {
    /// A battle against `formation`, the party standing on terrain
    /// `player_terrain` and the enemy on `enemy_terrain`; `state` is the
    /// game-state block the party comes from, whose units' statistics the
    /// battle computes again. It starts in the frame the field has gone
    /// black.
    #[must_use]
    pub fn new(
        data: &GameData<'_>,
        state: &mut [u8],
        formation: &Formation,
        (player_terrain, enemy_terrain): (u8, u8),
    ) -> Self {
        let rom = data.bytes();
        let offsets = |table| {
            data.script_offsets(table)
                .ok()
                .flatten()
                .unwrap_or_default()
        };
        let mut sheets: Vec<(u16, SpriteSheet)> = Vec::new();
        let mut unit = |zoid: u16| {
            if let Some((_, sheet)) = sheets.iter().find(|(known, _)| *known == zoid) {
                return Unit {
                    zoid,
                    sheet: Some(sheet.clone()),
                };
            }
            let sheet = data.zoid_status_sprite(usize::from(zoid)).ok();
            if let Some(sheet) = &sheet {
                sheets.push((zoid, sheet.clone()));
            }
            Unit { zoid, sheet }
        };
        let mut panels = Vec::new();
        let mut player: [Option<Unit>; SLOTS] = Default::default();
        for (slot, entry) in saga_party::formation(state).iter().enumerate() {
            let Some((unit_index, character)) = *entry else {
                continue;
            };
            saga_party::refresh_stats(rom, state, character, unit_index);
            let status = saga_party::unit_status(state, unit_index);
            panels.push(Panel {
                hp: status.map_or((0, 1), |status| status.hp),
                ep: status.map_or((0, 1), |status| status.ep),
            });
            if let Some(status) = status {
                player[slot] = Some(unit(status.zoid));
            }
        }
        let enemy: [Option<Unit>; SLOTS] = std::array::from_fn(|slot| {
            saga_encounter::enemy_record(rom, formation, slot)
                .map(|record| unit(u16::from(record[0])))
        });
        let anchors = std::array::from_fn(|side| {
            std::array::from_fn(|slot| {
                saga_combat::slot_anchor(rom, side == ENEMY, slot).unwrap_or_default()
            })
        });
        let grounds = (
            saga_combat::grounds(rom, player_terrain).map(|Grounds { player, .. }| player),
            saga_combat::grounds(rom, enemy_terrain).map(|Grounds { enemy, .. }| enemy),
        );
        let panel_graphics = saga_combat::panel_graphics(rom);
        let mut bank = [[0u16; 16]; 16];
        if let Some(graphics) = &panel_graphics {
            bank[usize::from(PANEL_BANK)] = graphics.palette;
        }
        let skin = data.window_skin().ok();
        if let Some(skin) = &skin {
            bank[usize::from(TEXT_BANK)] = skin.palette;
        }
        let speed = state.get(SPEED_BYTE).copied().unwrap_or(2);
        let wait_frames = MESSAGE_WAITS
            .get(usize::from(speed))
            .copied()
            .filter(|_| speed != KEY_WAIT_SPEED);
        let build_frames =
            BUILD_FRAMES + BUILD_PER_PANEL * u32::try_from(panels.len()).unwrap_or(0);
        Self {
            grid: data.battle_field().map(|field| field.grid),
            grounds,
            panel_graphics,
            frame_tiles: skin.map(|skin| skin.tiles),
            banks: PaletteBank::from_bgr555(&bank),
            panels,
            units: [player, enemy],
            anchors,
            menu: ScriptRunner::named(BATTLE_MENU_TABLE, offsets(BATTLE_MENU_TABLE))
                .with_quick_redraws(),
            text: ScriptRunner::named(BATTLE_TEXT_TABLE, offsets(BATTLE_TEXT_TABLE))
                .with_quick_redraws(),
            names: ScriptRunner::named(NAME_TABLE, offsets(NAME_TABLE)).with_quick_redraws(),
            acts: VecDeque::new(),
            running: None,
            frame: 0,
            build_frames,
            controller: Controller::Opening,
            opening: Opening::Build,
            results: Results::Idle,
            wait: Wait::Idle,
            wait_frames,
            fade: None,
            level: BLACK,
            shown_level: BLACK,
            outcome: None,
            choice: 0,
            sounds: Vec::new(),
            music: None,
            input: Input::default(),
            previous: Input::default(),
        }
    }

    /// How the battle ended, once it has handed back to the field.
    #[must_use]
    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome.filter(|_| self.controller == Controller::Done)
    }

    /// The sound effects requested since the last call.
    pub fn take_sounds(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.sounds)
    }

    /// The song requested since the last call.
    pub fn take_music(&mut self) -> Option<u16> {
        self.music.take()
    }

    /// Keeps what the screen shows this frame: the brightness register is
    /// written at the vertical blank, so a frame shows the level of the one
    /// before.
    pub fn latch(&mut self) {
        self.shown_level = self.level;
    }

    /// Advances one frame, the tasks in slot order.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a script cannot run.
    pub fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.previous = self.input;
        self.input = input;
        self.update_controller();
        self.update_slot5(rom, windows)?;
        self.update_wait();
        self.update_fade();
        self.frame += 1;
        Ok(())
    }

    fn pressed_a(&self) -> bool {
        self.input.is_held(platform::Button::A) && !self.previous.is_held(platform::Button::A)
    }

    /// The controller, slot 4: it waits for the opening, starts the results
    /// once it ends, darkens the screen and hands back.
    fn update_controller(&mut self) {
        match self.controller {
            Controller::Opening => {
                if self.opening == Opening::Done {
                    self.controller = Controller::StartResults;
                }
            }
            Controller::StartResults => {
                self.results = Results::Start;
                self.controller = Controller::AwaitResults;
            }
            Controller::AwaitResults => {
                if self.results == Results::Done {
                    self.controller = Controller::FadeOut;
                }
            }
            Controller::FadeOut => {
                self.fade = Some(Fade { out: true, age: 0 });
                self.controller = Controller::AwaitFade;
            }
            Controller::AwaitFade => {
                if self.fade.is_some_and(|fade| fade.age >= FADE_SEEN_AGE) {
                    self.fade = None;
                    self.controller = Controller::Finish;
                }
            }
            Controller::Finish => {
                self.acts.push_back(Act::Call(Call::Menu(MENU_RESET)));
                self.controller = Controller::Done;
            }
            Controller::Done => {}
        }
    }

    /// Slot 5: the opening, then the results. A call it makes blocks it
    /// until the call returns; it goes on in that frame and steps its state
    /// again the next.
    fn update_slot5(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let busy = self.running.is_some() || !self.acts.is_empty();
        if !busy {
            if self.controller == Controller::Opening {
                self.step_opening();
            } else if self.controller == Controller::AwaitResults {
                self.step_results();
            }
        }
        self.run_acts(rom, windows)
    }

    fn run_acts(&mut self, rom: &[u8], windows: &mut ScriptWindows<'_>) -> Result<(), ScriptError> {
        loop {
            if let Some(call) = self.running {
                let input = self.input;
                let runner = self.runner(call);
                if !runner.update(rom, input, windows)? {
                    return Ok(());
                }
                self.running = None;
                if let Call::Menu(MENU_MAIN) = call {
                    self.choose(self.menu.vars()[1]);
                }
            }
            let Some(act) = self.acts.pop_front() else {
                return Ok(());
            };
            match act {
                Act::Call(call) => {
                    let runner = self.runner(call);
                    let index = match call {
                        Call::Menu(index) | Call::Text(index) => usize::from(index),
                        Call::Name(zoid) => ZOID_NAMES + usize::from(zoid),
                    };
                    if !matches!(call, Call::Menu(_)) {
                        runner.select_window(MESSAGE_WINDOW);
                    }
                    runner.start(index)?;
                    self.running = Some(call);
                }
                Act::Wait => self.wait = Wait::Armed,
                Act::Fade { out } => self.fade = Some(Fade { out, age: 0 }),
                Act::Sound(sound) => self.sounds.push(sound),
                Act::Music(song) => self.music = Some(song),
            }
        }
    }

    fn runner(&mut self, call: Call) -> &mut ScriptRunner {
        match call {
            Call::Menu(_) => &mut self.menu,
            Call::Text(_) => &mut self.text,
            Call::Name(_) => &mut self.names,
        }
    }

    fn message(&mut self, calls: &[Call]) {
        self.acts.push_back(Act::Call(Call::Menu(MENU_DRAW)));
        self.acts.push_back(Act::Call(Call::Menu(MENU_CLEAR)));
        for call in calls {
            self.acts.push_back(Act::Call(*call));
        }
        self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
        self.acts.push_back(Act::Wait);
    }

    fn wait_done(&mut self) -> bool {
        if self.wait == Wait::Done {
            self.wait = Wait::Idle;
            return true;
        }
        false
    }

    /// One step of the opening (`0x0802E9CC`).
    fn step_opening(&mut self) {
        match self.opening {
            Opening::Build => {
                if self.frame == OPENING_START {
                    self.acts.push_back(Act::Music(OPENING_SONG));
                }
                if self.frame >= OPENING_START + self.build_frames {
                    self.opening = Opening::FadeIn;
                }
            }
            Opening::FadeIn => {
                self.acts.push_back(Act::Fade { out: false });
                self.opening = Opening::AwaitFade;
            }
            Opening::AwaitFade => {
                if self.fade.is_some_and(|fade| fade.age >= FADE_SEEN_AGE) {
                    self.fade = None;
                    self.opening = Opening::Met;
                }
            }
            Opening::Met => {
                self.acts
                    .push_back(Act::Call(Call::Menu(MENU_MESSAGE_WINDOW)));
                self.acts.push_back(Act::Call(Call::Text(TEXT_MET)));
                self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
                self.acts.push_back(Act::Wait);
                self.opening = Opening::AwaitMet;
            }
            Opening::AwaitMet => {
                if self.wait_done() {
                    self.opening = Opening::Check(0);
                }
            }
            Opening::Check(slot) => {
                if let Some(unit) = &self.units[ENEMY][slot] {
                    let zoid = unit.zoid;
                    self.message(&[Call::Name(zoid), Call::Text(TEXT_CONFIRMED)]);
                    self.opening = Opening::AwaitConfirmed(slot);
                } else {
                    self.opening = Opening::Next(slot);
                }
            }
            Opening::AwaitConfirmed(slot) => {
                if self.wait_done() {
                    self.opening = Opening::Next(slot);
                }
            }
            Opening::Next(slot) => {
                self.opening = if slot + 1 < SLOTS {
                    Opening::Check(slot + 1)
                } else {
                    Opening::Ready
                };
            }
            Opening::Ready => {
                self.message(&[Call::Text(TEXT_READY)]);
                self.opening = Opening::AwaitReady;
            }
            Opening::AwaitReady => {
                if self.wait_done() {
                    self.opening = Opening::Menu;
                }
            }
            Opening::Menu => {
                self.acts.push_back(Act::Call(Call::Menu(MENU_MAIN)));
            }
            Opening::Retreat => {
                self.outcome = Some(Outcome::Retreated);
                self.opening = Opening::Done;
            }
            Opening::Done => {}
        }
    }

    /// Goes on from the menu's line `line` in the frame the menu returns
    /// (the jump table at `0x0802ED08`): 退却 retreats; the other lines are
    /// not modeled yet and show the menu again.
    fn choose(&mut self, line: u16) {
        self.choice = line;
        self.opening = if line == CHOICE_RETREAT {
            Opening::Retreat
        } else {
            Opening::Menu
        };
    }

    /// One step of the results (`0x08035778`) after a retreat.
    fn step_results(&mut self) {
        match self.results {
            Results::Start => {
                self.acts.push_back(Act::Sound(RETREAT_SOUND));
                self.results = Results::Message;
            }
            Results::Message => {
                self.message(&[Call::Text(TEXT_RETREATED)]);
                self.results = Results::AwaitMessage;
            }
            Results::AwaitMessage => {
                if self.wait_done() {
                    self.results = Results::Finish;
                }
            }
            Results::Finish => self.results = Results::Done,
            Results::Idle | Results::Done => {}
        }
    }

    /// The message wait, slot 9: armed, it counts the speed's frames, or
    /// waits for A at the slowest speed.
    fn update_wait(&mut self) {
        self.wait = match self.wait {
            Wait::Armed => self.wait_frames.map_or(Wait::Key, Wait::Counting),
            Wait::Counting(left) if left <= 1 => Wait::Done,
            Wait::Counting(left) => Wait::Counting(left - 1),
            Wait::Key if self.pressed_a() => Wait::Done,
            other => other,
        };
    }

    /// The fade task, slot 10 (`0x08004034`): a level a frame from its
    /// second frame, to normal or to black.
    fn update_fade(&mut self) {
        let Some(fade) = self.fade.as_mut() else {
            return;
        };
        let level = match (fade.out, fade.age) {
            (_, 0) => None,
            (false, 1) => Some(u32::from(BLACK)),
            (false, age) => u32::from(BLACK).checked_sub(age.saturating_sub(2)),
            (true, 1) => Some(0),
            (true, age) => Some((age - 2).min(u32::from(BLACK))),
        };
        if let Some(level) = level {
            self.level = u8::try_from(level.min(u32::from(BLACK))).unwrap_or(BLACK);
        }
        fade.age += 1;
    }

    /// Draws the battle screen: the grid, the grounds, the units, the
    /// panels and the windows, darkened by the level the frame shows.
    pub fn draw(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
    ) {
        frame.fill(Rgb::default());
        if self.shown_level < FADE_STEPS {
            for layer in [&self.grid, &self.grounds.0, &self.grounds.1]
                .into_iter()
                .flatten()
            {
                draw_layer(frame, layer);
            }
            self.draw_units(frame);
            self.draw_panels(frame);
            windows.draw_shown(frame, skin, painter);
        }
        darken(frame, self.shown_level.min(FADE_STEPS));
    }

    /// The units in their slots, the nearer (lower) ones over the others;
    /// the enemy's drawn mirrored, facing the party.
    fn draw_units(&self, frame: &mut Frame) {
        let mut placed: Vec<((i32, i32), bool, &SpriteSheet)> = Vec::new();
        for side in [PLAYER, ENEMY] {
            for (slot, unit) in self.units[side].iter().enumerate() {
                if let Some(sheet) = unit.as_ref().and_then(|unit| unit.sheet.as_ref()) {
                    placed.push((self.anchors[side][slot], side == ENEMY, sheet));
                }
            }
        }
        placed.sort_by_key(|((_, y), _, _)| *y);
        for ((x, y), mirrored, sheet) in placed {
            let (Some(sprite), Some(image)) = (sheet.frames.first(), sheet.frame_image(0)) else {
                continue;
            };
            let width = i32::try_from(sprite.width).unwrap_or(0);
            let left = if mirrored {
                x - i32::from(sprite.x) - width
            } else {
                x + i32::from(sprite.x)
            };
            draw_sprite(
                frame,
                left,
                y + i32::from(sprite.y),
                &image,
                &sheet.palette,
                sprite.mirrored != mirrored,
            );
        }
    }

    /// The party's panels: BG1 scrolled two rows up, so their bars show and
    /// the names above them stay hidden until L slides them down.
    fn draw_panels(&self, frame: &mut Frame) {
        let Some(graphics) = &self.panel_graphics else {
            return;
        };
        let count = self.panels.len();
        let columns = saga_combat::PANEL_COLUMNS;
        let entry = |column: usize, row: usize| -> u16 {
            let panel = (0..count).find(|index| {
                let first = saga_combat::panel_column(*index, count);
                (first..first + columns).contains(&column)
            });
            let Some(index) = panel else {
                return 0;
            };
            let cell = column - saga_combat::panel_column(index, count);
            let row = row + PANEL_SCROLL_ROWS;
            if row >= saga_combat::PANEL_ROWS {
                return 0;
            }
            let frame_entry = graphics.map[row * columns + cell];
            if cell == 0 || cell == columns - 1 {
                return frame_entry;
            }
            let panel = self.panels[index];
            let (value, base) = if row == 2 {
                (panel.hp, 0)
            } else {
                (panel.ep, SECOND_BAR)
            };
            let level = saga_combat::bar_level(value.0, value.1);
            graphics.bars[usize::from(level)][cell - 1] + base
        };
        let tile = |index: usize| {
            if let Some(frame_tile) = index.checked_sub(FRAME_FIRST_TILE) {
                return self
                    .frame_tiles
                    .as_ref()
                    .and_then(|tiles| tiles.tile(frame_tile));
            }
            index
                .checked_sub(saga_combat::PANEL_FIRST_TILE)
                .and_then(|index| graphics.tiles.tile(index))
        };
        draw_background(frame, entry, tile, &self.banks, (0, 0), true);
    }
}

/// Draws a field layer over the frame: its tile 0 is clear.
fn draw_layer(frame: &mut Frame, layer: &FieldLayer) {
    let mut palettes = [[0u16; 16]; 16];
    palettes[usize::from(layer.bank & 0x0F)] = layer.palette;
    let bank = PaletteBank::from_bgr555(&palettes);
    draw_background(
        frame,
        |column, row| layer.entry(column, row),
        |tile| layer.tiles.tile(tile),
        &bank,
        (0, 0),
        true,
    );
}

/// Frames a battle's panels would take to name their units, for a party
/// of `panels`: what the port counts before the fade in.
#[must_use]
pub const fn build_frames(panels: u32) -> u32 {
    BUILD_FRAMES + BUILD_PER_PANEL * panels
}

#[cfg(test)]
mod tests {
    use super::*;

    const FADE_LAST_AGE: u32 = 33;

    #[test]
    fn the_message_wait_ends_the_speeds_frames_after_it_is_armed() {
        let rom = Vec::new();
        let data = GameData::new(&rom);
        let mut state = vec![0; 0x3F10];
        state[SPEED_BYTE] = 2;
        let mut combat = Combat::new(&data, &mut state, &[0xFF; 36], (0, 0));
        combat.wait = Wait::Armed;
        combat.update_wait();
        let frames = (1..100).find(|_| {
            combat.update_wait();
            combat.wait == Wait::Done
        });
        assert_eq!(frames, Some(30));
        state[SPEED_BYTE] = KEY_WAIT_SPEED;
        let mut combat = Combat::new(&data, &mut state, &[0xFF; 36], (0, 0));
        combat.wait = Wait::Armed;
        combat.update_wait();
        assert_eq!(combat.wait, Wait::Key);
    }

    #[test]
    fn the_menus_retreat_ends_the_opening() {
        let rom = Vec::new();
        let data = GameData::new(&rom);
        let mut state = vec![0; 0x3F10];
        let mut combat = Combat::new(&data, &mut state, &[0xFF; 36], (0, 0));
        combat.choose(0);
        assert_eq!(combat.opening, Opening::Menu);
        combat.choose(CHOICE_RETREAT);
        combat.step_opening();
        assert_eq!(combat.opening, Opening::Done);
        assert_eq!(combat.outcome, Some(Outcome::Retreated));
    }

    #[test]
    fn a_full_party_takes_longer_to_build() {
        assert_eq!(build_frames(4), 41);
        assert!(build_frames(6) > build_frames(4));
    }

    #[test]
    fn the_fade_in_brightens_a_level_a_frame_from_its_third() {
        let mut fade = Fade { out: false, age: 0 };
        let mut levels = Vec::new();
        for _ in 0..=FADE_LAST_AGE {
            let level = match fade.age {
                0 => None,
                1 => Some(31),
                age => 31u32.checked_sub(age - 2),
            };
            levels.push(level);
            fade.age += 1;
        }
        assert_eq!(levels[1], Some(31));
        assert_eq!(levels[2], Some(31));
        assert_eq!(levels[33], Some(0));
    }
}
