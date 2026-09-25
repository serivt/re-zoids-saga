//! The sprites of the attack scenes: the shots each weapon fires and the
//! weapons mounted on a Zoid, as entities of the game's sprite system.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! entity table at IWRAM `0x03004BBC` (0x88 bytes an entity), the sprite
//! system's animation step (`0x08000AF8`, `0x08000BD8`) and OAM builder
//! (`0x0800055C`), the shot spawner (`0x080485C4`, `0x080483B4`), the
//! mounted weapons (`0x0804588C`, `0x08045678`) and the shot behaviors (the
//! routines at ROM `0x6D4784`, sharing `0x08049BE4`, `0x08049C04`,
//! `0x08049BA8`, `0x08049AF0` and `0x08049B08`); checked against the
//! entity table frame by frame in a battle on the world map in a reference
//! emulator. See `docs/combat.md`.

use extraction::saga::AnimationStep;
use extraction::saga_battle::{EffectSprite, FirePart, SHOT_SPAWNS, ShotSpawn};

/// Entities the scenes use: the portrait's, 18 shots and three mounts.
pub const ENTITIES: usize = 30;
/// The first shot entity.
pub const FIRST_SHOT: usize = 1;
/// The first mounted weapon's entity (`0x08045678`): one for each of the
/// first three racks.
pub const FIRST_MOUNT: usize = 20;
/// Racks whose weapon shows on the Zoid.
pub const MOUNTED_RACKS: usize = 3;
/// The rack on the Zoid's back, whose weapons have their own pictures.
pub const BACK_RACK: usize = 1;

const RACK_FLAG: u32 = 4;
const ANIMATE: u32 = 1;
const LOOP: u32 = 2;
const SEMI_TRANSPARENT: u32 = 8;
const STARTS_AT_ONCE: u32 = 0x10;
const SPREAD_X: u32 = 0x4000;
const SPREAD_Y: u32 = 0x8000;
const BEHIND: u32 = 0x10_0000;
const SPREAD_CENTER: u16 = 0x40;
/// The sprite id that keeps the record's position on a rack.
const PLACED: u8 = b'.';
const PARTY_TARGET_MIRROR: u16 = 0xE0;
const ENEMY_ATTACKER_MIRROR: u16 = 0xF0;
const ENEMY_RACK_MIRROR: u16 = 0x80;
const PRIORITY_FRONT: u8 = 2;
const PRIORITY_BEHIND: u8 = 3;
/// The weapon slot whose shots the game draws behind the Zoid: the third
/// rack's, when the player chose it (`0x080483B4`).
const BEHIND_SLOT: u16 = 2;
/// A bullet's speed, 24 pixels a frame (`0x0804A5A8`, `0x0804A6B8`).
const BULLET_SPEED: i32 = 0x18_0000;
/// How far a returning bullet first jumps (`0x0804A6B8`).
const BULLET_JUMP: i32 = 0xF0_0000;
const SCREEN_WIDTH: u16 = 0xF0;
/// Bytes of a 4bpp tile.
const TILE_BYTES: usize = 32;

/// Which side's view a scene shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// The unit that attacks.
    Attacker,
    /// A unit it hits.
    Target,
}

/// One entity: where it is, how it animates and what its behavior keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    /// Its sprite, an index into the scene's sprites.
    pub sprite: usize,
    /// The sprite system's flags (`+0`): [`VISIBLE`], [`ANIMATES`],
    /// [`HOLDS_END`], [`ENDED`], [`MIRRORED`] and [`ON_LAYER`].
    pub flags: u16,
    /// Blended over the layers below (OBJ mode 1).
    pub semi_transparent: bool,
    /// OBJ priority: 2 in front of the Zoid, 3 behind it.
    pub priority: u8,
    /// Position in 16.16.
    pub x: i32,
    /// See [`Entity::x`].
    pub y: i32,
    animation: usize,
    step: usize,
    ticks: u32,
    /// Where it returns to when it resets (`+0x78`, `+0x7C`).
    home: (i32, i32),
    behavior: Option<u8>,
    /// What its behavior keeps: [`ACTIVE`] (`+0x4C`), [`STARTED`]
    /// (`+0x50`), [`ALIVE`] (`+0x64`) and [`PAUSED`] (`+0x6E`).
    marks: u8,
    /// Its place in the table (`+0x4E`).
    index: usize,
    /// The animation step at which it sets off the next one (`+0x52`).
    link: u16,
    /// Its behavior's value (`+0x54`).
    parameter: u16,
    /// Pixels it moved (`+0x56`).
    travelled: u16,
    /// Frames since it started (`+0x58`).
    age: u16,
    /// Frames it waits once set off (`+0x5E`).
    delay: u16,
    /// Frames since it was set off (`+0x62`).
    timer: u16,
    /// The screen shake it starts (`+0x6A`).
    shake: u8,
    /// The sound it plays as it starts (`+0x6C`).
    sound: u8,
    /// Its own affine transform (flag `0x20`, `+0x44`): the horizontal and
    /// vertical ratios in 8.8 and the angle, which every piece takes.
    pub(super) affine: Option<(i16, i16, u16)>,
}

/// Drawn this frame.
pub const VISIBLE: u16 = 0x8000;
/// Steps its animation.
pub const ANIMATES: u16 = 0x800;
/// Stops on the animation's last step rather than looping.
pub const HOLDS_END: u16 = 0x400;
/// Its animation reached its end.
pub const ENDED: u16 = 4;
/// Drawn mirrored, facing right.
pub const MIRRORED: u16 = 0x100;
/// Placed on BG1, so it moves with the Zoid.
pub const ON_LAYER: u16 = 0x1000;
/// Set off, counting its delay.
const ACTIVE: u8 = 1;
/// Started showing.
const STARTED: u8 = 2;
/// Still playing its part.
const ALIVE: u8 = 4;
/// Its animation was paused while its pilot speaks.
const PAUSED: u8 = 8;

/// What an entity asks of the scene as it updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Plays a sound effect.
    Sound(u8),
    /// Starts a screen shake of this kind (`0x0804416C`).
    Shake(u8),
}

/// The entity table of a scene.
#[derive(Debug, Clone, Default)]
pub struct Entities {
    table: Vec<Option<Entity>>,
    /// The sprites the entities draw.
    pub sprites: Vec<EffectSprite>,
    requests: Vec<Request>,
}

/// Where the shots of a view start: what `0x080485C4` reads besides the
/// animation.
#[derive(Debug, Clone, Copy)]
pub struct ShotPlace {
    /// The view.
    pub view: View,
    /// Whether the view shows the enemy's side.
    pub enemy: bool,
    /// The weapon's rack mount on the view's Zoid, and the fire record's
    /// offsets.
    pub mount: (u16, u16),
    /// The fire record.
    pub fire: FirePart,
    /// Whether the weapon sits on the back rack.
    pub back: bool,
    /// How the view's Zoid spreads its shots.
    pub spread: (u8, u8),
    /// The weapon slot the player chose, which puts the third rack's shots
    /// behind the Zoid.
    pub chosen_slot: u16,
}

impl Entities {
    /// An empty table (`0x08000D1C`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            table: vec![None; ENTITIES],
            sprites: Vec::new(),
            requests: Vec::new(),
        }
    }

    /// Empties the table and forgets the sprites.
    pub fn clear(&mut self) {
        self.table = vec![None; ENTITIES];
        self.sprites.clear();
    }

    /// The entity in place `index`.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Entity> {
        self.table.get(index).and_then(Option::as_ref)
    }

    /// The entity in place `index`, to change.
    pub(super) fn get_mut(&mut self, index: usize) -> Option<&mut Entity> {
        self.table.get_mut(index).and_then(Option::as_mut)
    }

    /// Puts `entity` in place `index`, or empties it.
    pub(super) fn put(&mut self, index: usize, entity: Option<Entity>) {
        if let Some(slot) = self.table.get_mut(index) {
            *slot = entity;
        }
    }

    /// Plays animation `animation` of entity `index` from its start
    /// (`0x08000BD8`).
    pub(super) fn animate(&mut self, index: usize, animation: usize) {
        if let Some(Some(entity)) = self.table.get_mut(index)
            && let Some(sprite) = self.sprites.get(entity.sprite)
        {
            entity.set_animation(sprite, animation);
        }
    }

    /// The requests made since the last call.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// Adds a sprite the entities can draw; returns its index.
    pub(super) fn sprite_index(&mut self, sprite: EffectSprite) -> usize {
        self.sprites.push(sprite);
        self.sprites.len() - 1
    }

    /// Puts the weapon of rack `rack` on the view's Zoid (`0x08045678`):
    /// racks 0 and 1 in front of it, rack 2 behind; mirrored, from the
    /// picture's other edge, on the enemy's side.
    pub fn mount(&mut self, rack: usize, sprite: EffectSprite, place: (i16, i16), enemy: bool) {
        let x = if enemy {
            i32::from(ENEMY_RACK_MIRROR) - i32::from(place.0)
        } else {
            i32::from(place.0)
        };
        let sprite = self.sprite_index(sprite);
        let mut entity = Entity::new(sprite, FIRST_MOUNT + rack);
        entity.set(VISIBLE, true);
        entity.set(ON_LAYER, true);
        entity.set(MIRRORED, enemy);
        entity.priority = if rack == 2 {
            PRIORITY_BEHIND
        } else {
            PRIORITY_FRONT
        };
        entity.x = x << 16;
        entity.y = i32::from(place.1) << 16;
        entity.home = (entity.x, entity.y);
        entity.set_animation(&self.sprites[sprite], 0);
        if let Some(slot) = self.table.get_mut(FIRST_MOUNT + rack) {
            *slot = Some(entity);
        }
    }

    /// Spawns a shot animation's sprites into the shot entities
    /// (`0x080485C4`); `sprite` loads the shot sprite of an id. Returns
    /// the tiles' bytes it loaded.
    pub fn spawn(
        &mut self,
        spawns: &[Option<ShotSpawn>],
        place: &ShotPlace,
        mut load: impl FnMut(u8) -> Option<EffectSprite>,
    ) -> usize {
        let mut loaded = 0;
        for (slot, spawn) in spawns.iter().enumerate().take(SHOT_SPAWNS) {
            let index = FIRST_SHOT + slot;
            let Some(spawn) = spawn else {
                if let Some(entry) = self.table.get_mut(index) {
                    *entry = None;
                }
                continue;
            };
            let (x, y) = shot_position(spawn, place);
            let shared = spawn.flags & 0x8000_0000 != 0;
            let sprite = if shared {
                let back = usize::try_from((spawn.flags & 0x7C00_0000) >> 26).unwrap_or(0);
                self.get(FIRST_SHOT + back).map(|entity| entity.sprite)
            } else {
                None
            };
            let sprite = if let Some(sprite) = sprite {
                sprite
            } else {
                let Some(sprite) = load(spawn.sprite) else {
                    if let Some(entry) = self.table.get_mut(index) {
                        *entry = None;
                    }
                    continue;
                };
                loaded += sprite.tiles.len() * TILE_BYTES;
                self.sprite_index(sprite)
            };
            let mut entity = Entity::new(sprite, index);
            entity.set(ANIMATES, spawn.flags & ANIMATE != 0);
            entity.set(HOLDS_END, spawn.flags & LOOP == 0);
            entity.set(ON_LAYER, spawn.flags & RACK_FLAG != 0);
            entity.semi_transparent = spawn.flags & SEMI_TRANSPARENT != 0;
            entity.priority = if spawn.flags & BEHIND != 0 || place.chosen_slot == BEHIND_SLOT {
                PRIORITY_BEHIND
            } else {
                PRIORITY_FRONT
            };
            entity.set(MIRRORED, mirrors(place));
            entity.x = signed(x) << 16;
            entity.y = signed(y) << 16;
            entity.home = (entity.x, entity.y);
            entity.mark(ALIVE, true);
            entity.mark(ACTIVE, spawn.flags & STARTS_AT_ONCE != 0);
            entity.link = u16::try_from((spawn.flags >> 8) & 0x1F).unwrap_or(0);
            entity.parameter = spawn.parameter;
            entity.delay = spawn.timing >> 8;
            entity.sound = spawn.sound;
            entity.shake = u8::try_from((spawn.flags >> 16) & 0xF).unwrap_or(0);
            entity.behavior = Some(spawn.behavior);
            let animation = usize::from(spawn.timing & 0xFF);
            entity.set_animation(&self.sprites[sprite], animation);
            if let Some(entry) = self.table.get_mut(index) {
                *entry = Some(entity);
            }
        }
        loaded
    }

    /// Whether a shot entity still plays its part (`+0x64`).
    #[must_use]
    pub fn shots_alive(&self) -> bool {
        (FIRST_SHOT..=SHOT_SPAWNS)
            .any(|index| self.get(index).is_some_and(|entity| entity.marked(ALIVE)))
    }

    /// Pauses the shots' animations while the target's pilot speaks
    /// (`0x08042C40`), or starts them again.
    pub fn pause_shots(&mut self, paused: bool) {
        for entity in self.table[FIRST_SHOT..=SHOT_SPAWNS].iter_mut().flatten() {
            if paused && entity.is(ANIMATES) {
                entity.mark(PAUSED, true);
                entity.set(ANIMATES, false);
            } else if !paused && entity.marked(PAUSED) {
                entity.set(ANIMATES, true);
            }
        }
    }

    /// Runs the shot entities' behaviors once (`0x08000DF4` on each, in
    /// order); `enemy_view` flips which way they fly.
    pub fn update_shots(&mut self, enemy_view: bool) {
        for index in FIRST_SHOT..=SHOT_SPAWNS {
            self.behave(index, enemy_view);
        }
    }

    fn behave(&mut self, index: usize, enemy_view: bool) {
        let Some(mut entity) = self.table.get_mut(index).and_then(Option::take) else {
            return;
        };
        let behavior = entity.behavior;
        if let Some(behavior) = behavior
            && entity.marked(ACTIVE)
        {
            entity.timer = entity.timer.wrapping_add(1);
            if entity.timer >= entity.delay {
                self.run(&mut entity, behavior, enemy_view);
            }
        }
        if let Some(slot) = self.table.get_mut(index) {
            *slot = Some(entity);
        }
    }

    /// One tick of behavior `behavior` once its delay has run out; the
    /// first shows the entity, with its shake and its sound
    /// (`0x08049C04`).
    fn run(&mut self, entity: &mut Entity, behavior: u8, enemy_view: bool) {
        let starting = !entity.marked(STARTED);
        if starting {
            entity.age = 0;
            entity.set(VISIBLE, true);
            entity.mark(STARTED, true);
            if entity.shake != 0 {
                self.requests.push(Request::Shake(entity.shake));
            }
            if entity.sound != 0 {
                self.requests.push(Request::Sound(entity.sound));
            }
        } else {
            entity.age = entity.age.wrapping_add(1);
        }
        match behavior {
            0 => Self::show_for_parameter(entity),
            1 => self.chain_at_link(entity, starting),
            10 => self.chain_at_steps(entity),
            13 => self.fly_in(entity, starting, enemy_view),
            14 => self.fly_back(entity, starting, enemy_view),
            17 => self.drift(entity, starting, enemy_view),
            _ => self.chain_at_end(entity, starting),
        }
    }

    /// `0x08049C24`: shows for its parameter's frames.
    fn show_for_parameter(entity: &mut Entity) {
        if entity.parameter < entity.age {
            entity.reset();
            entity.mark(ALIVE, false);
        }
    }

    /// `0x08049CA0`: sets off the next at its link step and goes with its
    /// animation.
    fn chain_at_link(&mut self, entity: &mut Entity, starting: bool) {
        if starting {
            return;
        }
        if usize::from(entity.link) <= entity.step {
            self.set_off(entity.index + 1, false);
        }
        if entity.is(ENDED) {
            entity.reset();
            entity.set(VISIBLE, false);
            entity.mark(ALIVE, false);
        }
    }

    /// `0x0804A340`: from its first frame, sets off the next on its link
    /// step and the shot its parameter's low byte names on the step its
    /// high byte gives, and hides once its animation ends.
    fn chain_at_steps(&mut self, entity: &mut Entity) {
        if usize::from(entity.link) == entity.step {
            self.set_off(entity.index + 1, false);
        }
        if usize::from(entity.parameter >> 8) == entity.step {
            self.set_off(FIRST_SHOT + usize::from(entity.parameter & 0xFF), false);
        }
        if entity.is(ENDED) {
            entity.reset();
            entity.set(VISIBLE, false);
            entity.mark(ALIVE, false);
        }
    }

    /// `0x08049D4C` and, until they are read, the behaviors not modeled:
    /// sets off the next as its animation ends.
    fn chain_at_end(&mut self, entity: &mut Entity, starting: bool) {
        if starting || !entity.is(ENDED) {
            return;
        }
        self.set_off(entity.index + 1, false);
        entity.reset();
        entity.set(VISIBLE, false);
        entity.mark(ALIVE, false);
    }

    /// `0x0804A5A8`: a bullet flying in, which sets off the entity its
    /// parameter names, from the start of its animation, once it has
    /// flown past its mark.
    fn fly_in(&mut self, entity: &mut Entity, starting: bool, enemy_view: bool) {
        if starting {
            entity.travelled = 0;
            return;
        }
        if entity.parameter >> 8 < entity.travelled {
            let other = FIRST_SHOT + usize::from(entity.parameter & 0xFF);
            if other != entity.index {
                self.set_off(other, true);
                if let Some(Some(target)) = self.table.get_mut(other) {
                    target.mark(ACTIVE, true);
                    if let Some(sprite) = self.sprites.get(target.sprite) {
                        target.set_animation(sprite, 0);
                    }
                }
            }
            entity.set(VISIBLE, false);
            entity.mark(ACTIVE, false);
            entity.mark(ALIVE, false);
        } else {
            entity.fly(-BULLET_SPEED, 0, enemy_view);
        }
    }

    /// `0x0804A6B8`: a bullet that jumps back a screen and flies in to its
    /// place, then sets off the entity its parameter names.
    fn fly_back(&mut self, entity: &mut Entity, starting: bool, enemy_view: bool) {
        if starting {
            entity.fly(BULLET_JUMP, 0, enemy_view);
            entity.travelled = 0;
            return;
        }
        if i32::from(SCREEN_WIDTH) - i32::from(entity.parameter >> 8) < i32::from(entity.travelled)
        {
            let other = FIRST_SHOT + usize::from(entity.parameter & 0xFF);
            self.set_off(other, false);
            entity.reset();
            entity.set(VISIBLE, false);
            entity.mark(ALIVE, false);
        } else {
            entity.fly(-BULLET_SPEED, 0, enemy_view);
        }
    }

    /// `0x0804A8C8`: drifts by its parameter until its animation ends,
    /// setting off the next at its link step; it moves from its first
    /// frame.
    fn drift(&mut self, entity: &mut Entity, starting: bool, enemy_view: bool) {
        if starting {
            entity.travelled = 0;
        }
        if usize::from(entity.link) <= entity.step {
            self.set_off(entity.index + 1, false);
        }
        if entity.is(ENDED) {
            entity.set(VISIBLE, false);
            entity.mark(ACTIVE, false);
            entity.mark(ALIVE, false);
            return;
        }
        let signed = |value: u16, sign: u16| {
            let magnitude = i32::from(value & 0xF) << 16;
            if sign == 0 {
                magnitude
            } else {
                magnitude | !0xF_FFFF
            }
        };
        let parameter = entity.parameter;
        let dx = signed(parameter >> 5, parameter & 0x200);
        let dy = signed(parameter, parameter & 0x10);
        entity.fly(dx, dy, enemy_view);
    }

    /// Sets off entity `index` (`0x08049BA8`): when it still plays its
    /// part, or anyway when `force`.
    fn set_off(&mut self, index: usize, force: bool) {
        if let Some(Some(entity)) = self.table.get_mut(index)
            && (force || entity.marked(ALIVE))
        {
            entity.mark(ACTIVE, true);
        }
    }

    /// The sprite system's pass over the table at the end of a frame
    /// (`0x08000A90`): each visible entity steps its animation.
    pub fn step_animations(&mut self) {
        for entity in self.table.iter_mut().flatten() {
            if entity.is(VISIBLE)
                && entity.is(ANIMATES)
                && !entity.is(ENDED)
                && let Some(sprite) = self.sprites.get(entity.sprite)
            {
                entity.advance(sprite);
            }
        }
    }

    /// The frames the entities show, in drawing order: each visible one's
    /// sprite, frame, anchor on screen (BG1's scroll taken off those on
    /// it) and how it is drawn.
    #[must_use]
    pub fn shown(&self, layer_scroll: (i32, i32)) -> Vec<Shown> {
        self.table
            .iter()
            .flatten()
            .filter(|entity| entity.is(VISIBLE))
            .filter_map(|entity| {
                let sprite = self.sprites.get(entity.sprite)?;
                let steps = sprite.animations.get(entity.animation)?;
                let frame = steps.get(entity.step)?.frame;
                let (mut x, mut y) = (entity.x >> 16, entity.y >> 16);
                if entity.is(ON_LAYER) {
                    x -= layer_scroll.0;
                    y -= layer_scroll.1;
                }
                Some(Shown {
                    sprite: entity.sprite,
                    frame,
                    anchor: (x, y),
                    mirrored: entity.is(MIRRORED),
                    semi_transparent: entity.semi_transparent,
                    priority: entity.priority,
                    affine: entity.affine,
                })
            })
            .collect()
    }
}

/// An entity as the OAM shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown {
    /// Its sprite.
    pub sprite: usize,
    /// The sprite's frame.
    pub frame: usize,
    /// Its anchor on screen.
    pub anchor: (i32, i32),
    /// Drawn mirrored.
    pub mirrored: bool,
    /// Blended over the layers below.
    pub semi_transparent: bool,
    /// OBJ priority.
    pub priority: u8,
    /// The entity's own affine transform, for all its pieces.
    pub affine: Option<(i16, i16, u16)>,
}

impl Entity {
    /// An entity in place `index` drawing sprite `sprite`, hidden.
    pub(super) fn new(sprite: usize, index: usize) -> Self {
        Self {
            sprite,
            flags: 0,
            semi_transparent: false,
            priority: PRIORITY_FRONT,
            x: 0,
            y: 0,
            animation: 0,
            step: 0,
            ticks: 1,
            home: (0, 0),
            behavior: None,
            marks: 0,
            index,
            link: 0,
            parameter: 0,
            travelled: 0,
            age: 0,
            delay: 0,
            timer: 0,
            shake: 0,
            sound: 0,
            affine: None,
        }
    }

    /// Whether the sprite system's `flag` is set.
    #[must_use]
    pub fn is(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }

    /// Sets or clears the sprite system's `flag`.
    pub(super) fn set(&mut self, flag: u16, on: bool) {
        if on {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    fn marked(&self, mark: u8) -> bool {
        self.marks & mark != 0
    }

    fn mark(&mut self, mark: u8, on: bool) {
        if on {
            self.marks |= mark;
        } else {
            self.marks &= !mark;
        }
    }

    /// Plays animation `animation` from its start (`0x08000BD8`).
    pub(super) fn set_animation(&mut self, sprite: &EffectSprite, animation: usize) {
        self.set(ENDED, false);
        self.animation = animation;
        self.step = 0;
        self.ticks = sprite
            .animations
            .get(animation)
            .and_then(|steps| steps.first())
            .map_or(1, step_ticks);
    }

    /// The step of its animation it shows.
    pub(super) fn step(&self) -> usize {
        self.step
    }

    /// Shows step `step` of its animation (`+0x34`), which it holds while
    /// it does not animate.
    pub(super) fn show_step(&mut self, sprite: &EffectSprite, step: usize) {
        self.step = step;
        self.ticks = sprite
            .animations
            .get(self.animation)
            .and_then(|steps| steps.get(step))
            .map_or(1, step_ticks);
    }

    /// One tick of the animation (`0x08000AF8`): the next step when the
    /// current one's ticks run out, the first again after the last unless
    /// the entity holds its end.
    fn advance(&mut self, sprite: &EffectSprite) {
        let Some(steps) = sprite.animations.get(self.animation) else {
            return;
        };
        self.ticks = self.ticks.saturating_sub(1);
        if self.ticks != 0 {
            return;
        }
        if self.step + 1 >= steps.len() {
            if self.is(HOLDS_END) {
                self.set(ENDED, true);
                return;
            }
            self.step = 0;
        } else {
            self.step += 1;
        }
        self.ticks = steps.get(self.step).map_or(1, step_ticks);
    }

    /// Goes back home, waiting to be set off again (`0x08049AF0`).
    fn reset(&mut self) {
        (self.x, self.y) = self.home;
        self.mark(STARTED, false);
        self.mark(ACTIVE, false);
    }

    /// Moves by `dx` (16.16, toward the left on the party's side) and up by
    /// `dy`, counting the pixels (`0x08049B08`).
    fn fly(&mut self, dx: i32, dy: i32, enemy_view: bool) {
        let pixels = u16::try_from(dx.unsigned_abs() >> 16).unwrap_or(u16::MAX);
        self.travelled = self.travelled.wrapping_add(pixels);
        let dx = if enemy_view { dx } else { dx.wrapping_neg() };
        self.x = self.x.wrapping_add(dx);
        self.y = self.y.wrapping_sub(dy);
    }
}

/// Ticks a step lasts: the entities of the scenes use no speed shift.
fn step_ticks(step: &AnimationStep) -> u32 {
    step.duration.max(1)
}

/// A half-word read as signed.
fn signed(value: u16) -> i32 {
    i32::from(i16::from_ne_bytes(value.to_ne_bytes()))
}

/// Whether a view's shots are drawn mirrored: the party's side's target
/// and the enemy's side's attacker face right.
fn mirrors(place: &ShotPlace) -> bool {
    match (place.enemy, place.view) {
        (false, View::Target) | (true, View::Attacker) => true,
        (false, View::Attacker) | (true, View::Target) => false,
    }
}

/// Where a shot sprite starts (`0x080485C4`): on the weapon's rack plus
/// its offsets, or at its own place, spread toward the Zoid's middle by
/// its size, mirrored across the screen for the views that face right.
fn shot_position(spawn: &ShotSpawn, place: &ShotPlace) -> (u16, u16) {
    if spawn.flags & RACK_FLAG == 0 {
        let mut x = spawn.x;
        let mut y = spawn.y;
        if spawn.flags & SPREAD_X != 0 {
            x = spread_x(x, place.spread.0);
        }
        if spawn.flags & SPREAD_Y != 0 {
            y = spread_y(y, place.spread.1);
        }
        match (place.enemy, place.view) {
            (false, View::Target) => x = PARTY_TARGET_MIRROR.wrapping_sub(x),
            (true, View::Attacker) => x = ENEMY_ATTACKER_MIRROR.wrapping_sub(x),
            _ => {}
        }
        return (x, y);
    }
    let (mut x, y) = if spawn.sprite == PLACED {
        (spawn.x, spawn.y)
    } else {
        let mut x = place
            .mount
            .0
            .wrapping_add(spawn.x)
            .wrapping_add(place.fire.mount_offset.0);
        let mut y = place
            .mount
            .1
            .wrapping_add(spawn.y)
            .wrapping_add(place.fire.mount_offset.1);
        if place.back {
            x = x.wrapping_add(place.fire.back_offset.0);
            y = y.wrapping_add(place.fire.back_offset.1);
        }
        (x, y)
    };
    if place.enemy {
        x = ENEMY_RACK_MIRROR.wrapping_sub(x);
    }
    (x, y)
}

/// Pulls `x` toward the middle by the Zoid's size (`0x080480A8`): halfway
/// for mode 1, an eighth of the way for mode 2.
fn spread_x(x: u16, mode: u8) -> u16 {
    let distance = x.abs_diff(SPREAD_CENTER);
    let distance = match mode {
        1 => distance >> 1,
        2 => distance >> 3,
        _ => distance,
    };
    if x <= SPREAD_CENTER {
        SPREAD_CENTER.wrapping_sub(distance)
    } else {
        SPREAD_CENTER.wrapping_add(distance)
    }
}

/// Lowers `y` by the Zoid's size (`0x080480E8`).
fn spread_y(y: u16, mode: u8) -> u16 {
    match mode {
        0 => y,
        1 => y.wrapping_add(0x18),
        2 => y.wrapping_add(0x30),
        _ => SPREAD_CENTER,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shots_spread_toward_the_middle_by_the_zoids_size() {
        assert_eq!(spread_x(0x60, 0), 0x60);
        assert_eq!(spread_x(0x60, 1), 0x50);
        assert_eq!(spread_x(0x20, 2), 0x3C);
        assert_eq!(spread_y(0x20, 1), 0x38);
        assert_eq!(spread_y(0x20, 3), 0x40);
    }
}
