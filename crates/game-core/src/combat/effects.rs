//! The sprites of the attack scenes: the shots each weapon fires and the
//! weapons mounted on a Zoid, as entities of the game's sprite system.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! entity table at IWRAM `0x03004BBC` (0x88 bytes an entity), the sprite
//! system's animation step (`0x08000AF8`, `0x08000BD8`) and OAM builder
//! (`0x0800055C`), the shot spawner (`0x080485C4`, `0x080483B4`), the
//! mounted weapons (`0x0804588C`, `0x08045678`) and the shot behaviors (the
//! routines at ROM `0x6D4784`, sharing `0x08049BE4`, `0x08049C04`,
//! `0x08049BA8`, `0x08049AF0`, `0x08049B08` and `0x08049B4C`, and the
//! palettes' fades `0x08047E48` and `0x08047E88`); checked against the
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
/// How far a behavior sends a shot in before it flies back (`0x0804B2E0`,
/// `0x0804B804`), and how far one starts off to the side
/// (`0x0804BC74`).
const FAR_JUMP: i32 = 0xF0_0000;
const SIDE_JUMP: i32 = 0x80_0000;
/// One pixel, in 16.16.
const PIXEL: i32 = 0x1_0000;
/// The Zoid's steps in the lunges (`0x0804A044`, `0x0804A1B4`,
/// `0x0804B934`): a unit of speed moves it an eighth of a pixel.
const LUNGE_STEP: i32 = 0x2000;
/// A dash's first speed, and the one its return starts from: -64 in a
/// half-word, which the game adds as an unsigned one.
const DASH_SPEED: u16 = 0x24;
const DASH_RETURN: u16 = 0xFFC0;
/// Where a charge stops, past the camera (`0x0804A044`).
const CHARGE_REACH: i32 = 0x108_0000;
/// A pan's speed and length (`0x0804ACE4`).
const PAN_STEP: i32 = 0x4_0000;
const PAN_FRAMES: u16 = 0x20;
/// The levels of a flash of the palettes (`0x08047E88`): 16 is the
/// palette as loaded, 32 white.
const FLASH_NORMAL: u8 = crate::windows::NORMAL_LEVEL;
const FLASH_WHITE: u16 = 0x20;
/// The mounted weapons' animations start at this entity (`0x0804B4D0`).
const MOUNT_ENTITY: usize = FIRST_MOUNT;

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
    /// What its behavior keeps: [`ACTIVE`] (`+0x4C`), [`ALIVE`] (`+0x64`)
    /// and [`PAUSED`] (`+0x6E`).
    marks: u8,
    /// Its place in the table (`+0x4E`).
    index: usize,
    /// The animation step at which it sets off the next one (`+0x52`).
    link: u16,
    /// Its behavior's value (`+0x54`).
    parameter: u16,
    /// Pixels it moved (`+0x56`).
    travelled: u16,
    /// Frames since it started (`+0x58`), which some behaviors use as a
    /// speed.
    age: u16,
    /// Whether it started (`+0x50`): 1 once shown, 2 once it set off what
    /// it names.
    started: u8,
    /// A second speed of the lunges (`+0x5A`).
    counter: u16,
    /// Where a waving shot started (`+0x80`).
    base_y: i32,
    /// Frames it waits once set off (`+0x5E`).
    delay: u16,
    /// Frames since it was set off (`+0x62`).
    timer: u16,
    /// The screen shake it starts (`+0x6A`).
    shake: u8,
    /// The sound it plays as it starts (`+0x6C`).
    sound: u8,
    /// Where its fade of the blend stands (`+0x68`), 0 once done.
    fading: u8,
    /// Its own affine transform (flag `0x20`, `+0x44`): the horizontal and
    /// vertical ratios in 8.8 and the angle, which every piece takes.
    pub(super) affine: Option<(i16, i16, u16)>,
}

/// A step of the blend's fade in (`0x080481D4` with 2): the next step,
/// and the blend it sets. It starts from none of the sprite and all of the
/// layer below and goes up in 32 steps, the last one back to 0.
fn fade_step(step: u8) -> (u8, (u8, u8)) {
    let blend = if step == 0 {
        (0, FADE_BELOW)
    } else {
        let half = u16::from(step >> 1);
        let eva = (half * 15) >> 4;
        let evb = u16::from(FADE_BELOW) - ((half * 7) >> 4);
        (
            u8::try_from(eva).unwrap_or(0),
            u8::try_from(evb).unwrap_or(0),
        )
    };
    let next = step.wrapping_add(1);
    (if next >> 1 == FADE_STEPS { 0 } else { next }, blend)
}

/// The layer below's share when a fade starts.
const FADE_BELOW: u8 = 15;
/// Double steps of a blend's fade in.
const FADE_STEPS: u8 = 16;

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
    /// Sets the blend of the semi-transparent sprites over the layers
    /// below, as eighth-sixteenths (`BLDALPHA`, `0x08048168`).
    Blend(u8, u8),
    /// Moves the Zoid and the camera (`0x0200E288`, `0x0200E284`, 16.16).
    Zoid(i32, i32),
    /// Brightens or darkens the first `banks` background palettes, and the
    /// mounted weapons' with `mounts`, to `level` (`0x08047E88`).
    Flash {
        /// Background palettes: 4 the scenery's, 8 the Zoid's too.
        banks: u8,
        /// The mounted weapons' palettes (OBJ 0 to 2) too.
        mounts: bool,
        /// 16 as loaded, above brighter, below darker.
        level: u8,
    },
}

/// The entity table of a scene.
#[derive(Debug, Clone, Default)]
pub struct Entities {
    table: Vec<Option<Entity>>,
    /// The sprites the entities draw.
    pub sprites: Vec<EffectSprite>,
    requests: Vec<Request>,
    /// Where the Zoid is (`0x0200E288`), which a dash starts from.
    pub zoid_x: i32,
    /// The wave table (ROM `0x6D3B94`) a waving shot follows.
    pub wave: Vec<u16>,
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
    /// A weapon for the attacker's own side, whose shots show in front of
    /// the Zoid (`0x0200E24B`).
    pub own_side: bool,
}

impl Entities {
    /// An empty table (`0x08000D1C`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            table: vec![None; ENTITIES],
            sprites: Vec::new(),
            requests: Vec::new(),
            zoid_x: 0,
            wave: Vec::new(),
        }
    }

    /// An empty table whose waving shots follow `wave`.
    #[must_use]
    pub fn with_wave(wave: Vec<u16>) -> Self {
        Self {
            wave,
            ..Self::new()
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
            entity.priority = if place.own_side {
                PRIORITY_FRONT
            } else if spawn.flags & BEHIND != 0 || place.chosen_slot == BEHIND_SLOT {
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
        if let Some(behavior) = entity.behavior {
            match behavior {
                // The dashes run from their spawn, set off or not.
                9 | 33 => self.dash(&mut entity, behavior == 33),
                // The pan counts its delay from its spawn.
                21 => {
                    if entity.tick() {
                        self.pan(&mut entity);
                    }
                }
                _ => {
                    match behavior {
                        23 => entity.mark(ALIVE, false),
                        27 if entity.link == 0 => {
                            entity.mark(ALIVE, false);
                            entity.mark(ACTIVE, false);
                        }
                        32 if entity.link == 0 => entity.mark(ALIVE, false),
                        _ => {}
                    }
                    if entity.marked(ACTIVE) && entity.tick() {
                        self.run(&mut entity, behavior, enemy_view);
                    }
                }
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
        match behavior {
            8 => return self.charge(entity),
            34 => return self.flash(entity, true),
            35 => return self.flash(entity, false),
            _ => {}
        }
        let starting = entity.started == 0;
        if starting {
            entity.age = 0;
            self.start(entity, behavior, enemy_view);
        } else {
            entity.age = entity.age.wrapping_add(1);
        }
        match behavior {
            0 => Self::show_for_parameter(entity),
            1 => self.chain_at_link(entity, starting),
            10 => self.chain_at_steps(entity),
            12 => self.set_off_at_link(entity, starting),
            13 => self.fly_in(entity, starting, enemy_view),
            14 => self.fly_back(entity, starting, enemy_view),
            15 => self.fade_in(entity, starting),
            16 => self.chain_after(entity, starting),
            17 => self.drift(entity, starting, enemy_view),
            18 => self.slide(entity, starting, enemy_view),
            19 => self.shoot(entity, starting, enemy_view),
            20 => self.glide(entity, enemy_view),
            22 => self.wave(entity, enemy_view),
            23 => self.repeat(entity, starting),
            24 => self.replay(entity, starting),
            25 | 31 => self.glide_and_set_off(entity, behavior == 31, enemy_view),
            27 | 32 => self.return_fire(entity, starting, behavior == 32, enemy_view),
            29 => self.recoil(entity, starting),
            36 => self.back_off(entity, starting, enemy_view),
            _ => self.chain_at_end(entity, starting),
        }
    }

    /// What starting does besides showing it, its shake and its sound:
    /// some behaviors start from home or off to the side, restart their
    /// animation or count their way from 0; the recoil animates a mounted
    /// weapon instead of showing.
    fn start(&mut self, entity: &mut Entity, behavior: u8, enemy_view: bool) {
        entity.started = 1;
        if behavior == 29 {
            let mount = MOUNT_ENTITY + usize::from((entity.parameter >> 5) & 0x1F);
            self.animate(mount, usize::from(entity.parameter & 0x1F));
        } else {
            entity.set(VISIBLE, true);
        }
        if entity.shake != 0 {
            self.requests.push(Request::Shake(entity.shake));
        }
        if entity.sound != 0 {
            self.requests.push(Request::Sound(entity.sound));
        }
        match behavior {
            18 | 20 | 22 | 25 | 31 => entity.travelled = 0,
            19 => {
                (entity.x, entity.y) = entity.home;
                entity.travelled = 0;
            }
            27 | 32 => {
                (entity.x, entity.y) = entity.home;
                entity.fly(FAR_JUMP, 0, enemy_view);
                entity.travelled = 0;
            }
            36 => {
                entity.fly(SIDE_JUMP, 0, enemy_view);
                entity.travelled = 0;
            }
            _ => {}
        }
        if behavior == 22 {
            entity.base_y = entity.y;
        }
        if matches!(behavior, 24 | 25 | 31)
            && let Some(sprite) = self.sprites.get(entity.sprite)
        {
            entity.set_animation(sprite, entity.animation);
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

    /// `0x0804A78C`: fades the blend in over 32 frames (`0x080481D4` in its
    /// second way), then plays its part until its parameter's frames have
    /// gone, staying on the screen.
    fn fade_in(&mut self, entity: &mut Entity, starting: bool) {
        if starting || entity.fading != 0 {
            let (next, blend) = fade_step(entity.fading);
            entity.fading = next;
            self.requests.push(Request::Blend(blend.0, blend.1));
        } else if entity.parameter <= entity.age {
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

    /// Sets off entity `index` anyway, from its home and its start
    /// (`0x08049BA8` with 3).
    fn restart(&mut self, index: usize) {
        if let Some(Some(entity)) = self.table.get_mut(index) {
            entity.reset();
            entity.mark(ACTIVE, true);
        }
    }

    /// The shot the low byte of the parameter names, or the bits `shift`
    /// up of it.
    fn named(entity: &Entity, shift: u16) -> usize {
        let mask = if shift == 0 { 0xFF } else { 0x1F };
        FIRST_SHOT + usize::from((entity.parameter >> shift) & mask)
    }

    /// Sets off the next entity once its animation reached its link step.
    fn chain_from_link(&mut self, entity: &Entity, force: bool) {
        if usize::from(entity.link) <= entity.step {
            self.set_off(entity.index + 1, force);
        }
    }

    /// Hides it and ends its part.
    fn finish(entity: &mut Entity) {
        entity.set(VISIBLE, false);
        entity.mark(ACTIVE, false);
        entity.mark(ALIVE, false);
    }

    /// `0x0804A4EC`: on its link step, starts the shot its parameter names
    /// from the start, and leaves the shots to it; hides at its end.
    fn set_off_at_link(&mut self, entity: &mut Entity, starting: bool) {
        if starting {
            return;
        }
        if usize::from(entity.link) == entity.step {
            self.restart(Self::named(entity, 0));
            entity.mark(ACTIVE, true);
            entity.mark(ALIVE, false);
        }
        if entity.is(ENDED) {
            entity.reset();
            entity.set(VISIBLE, false);
        }
    }

    /// `0x0804A834`: sets off the next once it has shown for its
    /// parameter's frames, leaving the shots to the others; it keeps
    /// playing until the view ends.
    fn chain_after(&mut self, entity: &mut Entity, starting: bool) {
        if starting {
            return;
        }
        if entity.parameter <= entity.age {
            self.set_off(entity.index + 1, false);
        }
        entity.mark(ALIVE, false);
    }

    /// `0x0804A9CC`: slides a pixel speed a frame (the parameter's bits 5
    /// to 9 across, its low nibble up or down by bit 4) until it has
    /// travelled its bits 10 on, setting off the next at its link step.
    fn slide(&mut self, entity: &mut Entity, starting: bool, enemy_view: bool) {
        if starting {
            return;
        }
        self.chain_from_link(entity, false);
        let parameter = entity.parameter;
        if entity.travelled < parameter >> 10 {
            let rise = i32::from(parameter & 0xF) << 16;
            let dy = if parameter & 0x10 == 0 { -rise } else { rise };
            entity.fly(i32::from((parameter >> 5) & 0x1F) << 16, dy, enemy_view);
        } else {
            Self::finish(entity);
        }
    }

    /// `0x0804AAB4`: a bullet from its home, flying in at 24 pixels a frame;
    /// past its mark it restarts the shot its parameter names.
    fn shoot(&mut self, entity: &mut Entity, starting: bool, enemy_view: bool) {
        if starting {
            return;
        }
        if entity.parameter >> 8 < entity.travelled {
            let other = Self::named(entity, 0);
            if other != entity.index {
                self.set_off(other, true);
                self.animate(other, 0);
                entity.started = 0;
            }
            Self::finish(entity);
        } else {
            entity.fly(BULLET_SPEED, 0, enemy_view);
        }
    }

    /// The parameter's two signed five-bit speeds in quarter pixels
    /// (`0x0804ABDC`): across from bit 5, up from bit 0.
    fn quarter_speeds(parameter: u16) -> (i32, i32) {
        let speed = |field: u16| {
            let value = i32::from(field & 0x1F);
            (if field & 0x10 == 0 {
                value
            } else {
                value - 0x20
            }) << 14
        };
        (speed(parameter >> 5), speed(parameter))
    }

    /// `0x0804ABDC`: glides by its quarter-pixel speeds until its animation
    /// ends, setting off the next at its link step.
    fn glide(&mut self, entity: &mut Entity, enemy_view: bool) {
        self.chain_from_link(entity, false);
        if entity.is(ENDED) {
            Self::finish(entity);
        } else {
            let (dx, dy) = Self::quarter_speeds(entity.parameter);
            entity.shift(dx, dy, enemy_view);
        }
    }

    /// `0x0804ADC8`: moves across by its speed (bits 5 to 9) and bobs up and
    /// down by the wave table as it goes, until its animation ends.
    fn wave(&mut self, entity: &mut Entity, enemy_view: bool) {
        self.chain_from_link(entity, false);
        if entity.is(ENDED) {
            entity.mark(ACTIVE, false);
            entity.mark(ALIVE, false);
            return;
        }
        let field = (entity.parameter >> 5) & 0x1F;
        let value = i32::from(field & 0xF);
        let dx = (if field & 0x10 == 0 {
            i32::from(field)
        } else {
            value - 0x10
        }) << 16;
        entity.shift(dx, 0, enemy_view);
        let at = usize::from(entity.travelled.wrapping_add(0x80) & 0xFF);
        let wave = self.wave.get(at).copied().unwrap_or(0);
        entity.y = entity
            .base_y
            .wrapping_add(i32::from(wave).wrapping_mul(LUNGE_STEP));
    }

    /// `0x0804AEC8`: sets off the next at its link step, and hides at its
    /// end unless its parameter's top bit makes it loop; it never holds
    /// the shots up.
    fn repeat(&mut self, entity: &mut Entity, starting: bool) {
        if starting {
            return;
        }
        self.chain_from_link(entity, true);
        if entity.parameter & 0x8000 == 0 && entity.is(ENDED) {
            entity.reset();
            entity.set(VISIBLE, false);
        }
    }

    /// `0x0804AF8C`: plays its animation again from the start, sets off
    /// the next once at its link step, and hides at its end.
    fn replay(&mut self, entity: &mut Entity, starting: bool) {
        if starting {
            return;
        }
        if usize::from(entity.link) <= entity.step && entity.started == 1 {
            self.set_off(entity.index + 1, true);
            entity.started = 2;
        }
        if entity.is(ENDED) {
            entity.reset();
            Self::finish(entity);
        }
    }

    /// `0x0804B05C` and `0x0804B6C4`: glides by its quarter-pixel speeds
    /// until its animation ends, and once, on its link step (or from it
    /// with `from_link`), starts the shot its bits 10 on name, which then
    /// holds the shots up no more.
    fn glide_and_set_off(&mut self, entity: &mut Entity, from_link: bool, enemy_view: bool) {
        let link = usize::from(entity.link);
        let reached = if from_link {
            link <= entity.step
        } else {
            link == entity.step
        };
        if reached && entity.started != 2 {
            let other = Self::named(entity, 10);
            self.restart(other);
            if from_link {
                entity.mark(ALIVE, false);
            } else if let Some(other) = self.get_mut(other) {
                other.mark(ALIVE, false);
            }
            entity.started = 2;
        }
        if entity.is(ENDED) {
            entity.set(VISIBLE, false);
            entity.started = 0;
            entity.mark(ACTIVE, false);
            if !from_link {
                entity.mark(ALIVE, false);
            }
        } else {
            let (dx, dy) = Self::quarter_speeds(entity.parameter);
            entity.shift(dx, dy, enemy_view);
        }
    }

    /// `0x0804B2E0` and `0x0804B804`: a bullet that jumps a screen back
    /// and flies in to its place, then starts the shot its parameter names,
    /// as many times as its link step counts.
    fn return_fire(&mut self, entity: &mut Entity, starting: bool, frees: bool, enemy_view: bool) {
        if starting {
            return;
        }
        let reach = i32::from(SCREEN_WIDTH) - i32::from(entity.parameter >> 8);
        if reach < i32::from(entity.travelled) {
            let other = Self::named(entity, 0);
            self.restart(other);
            if frees && let Some(other) = self.get_mut(other) {
                other.mark(ALIVE, false);
            }
            entity.set(VISIBLE, false);
            entity.started = 0;
            entity.mark(ACTIVE, false);
            if !frees || entity.link != 0 {
                entity.link = entity.link.wrapping_sub(1);
            }
        } else {
            entity.fly(-BULLET_SPEED, 0, enemy_view);
        }
    }

    /// `0x0804B4D0`: once it has animated its mounted weapon, sets off the
    /// shot its bits 10 on name.
    fn recoil(&mut self, entity: &mut Entity, starting: bool) {
        if starting {
            return;
        }
        self.set_off(Self::named(entity, 10), false);
        entity.mark(ACTIVE, false);
        entity.mark(ALIVE, false);
    }

    /// `0x0804BC74`: starts off to the side and, once at its link step,
    /// sets off the shot its parameter names; after its animation it
    /// backs off a pixel a frame until it has gone its high byte.
    fn back_off(&mut self, entity: &mut Entity, starting: bool, enemy_view: bool) {
        if starting {
            return;
        }
        if usize::from(entity.link) <= entity.step && entity.started != 2 {
            entity.started = 2;
            self.set_off(Self::named(entity, 0), false);
            entity.mark(ALIVE, false);
        }
        if entity.is(ENDED) {
            entity.fly(-PIXEL, 0, enemy_view);
            if entity.parameter >> 8 < entity.travelled {
                entity.set(VISIBLE, false);
                entity.started = 0;
                entity.mark(ACTIVE, false);
            }
        }
    }

    /// `0x0804A044`: the Zoid charges across the screen, the camera
    /// following ever faster, a stage of the parameter at a time.
    fn charge(&mut self, entity: &mut Entity) {
        let speed = |value: u16| i32::from(value).wrapping_mul(LUNGE_STEP);
        let (mut x, mut camera) = entity.home;
        match entity.parameter {
            0 => {
                entity.travelled = 0;
                entity.age = 0;
                entity.counter = 0;
                (x, camera) = (0, 0);
                entity.parameter = 1;
            }
            1 => {
                if entity.age > 0x20 {
                    entity.parameter = 2;
                }
                entity.age = entity.age.wrapping_add(1);
                entity.counter = entity.counter.wrapping_add(1);
                x = x.wrapping_add(speed(entity.age));
                camera = camera.wrapping_add(speed(entity.counter));
            }
            2 => {
                if entity.travelled > 0xF {
                    entity.parameter = 3;
                    entity.travelled = 0;
                    if entity.sound != 0 {
                        self.requests.push(Request::Sound(entity.sound));
                    }
                }
                entity.travelled = entity.travelled.wrapping_add(1);
                entity.age = entity.age.wrapping_add(1);
                entity.counter = entity.counter.wrapping_add(2);
                x = x.wrapping_add(speed(entity.age));
                camera = camera.wrapping_add(speed(entity.counter));
            }
            3 => {
                let unsigned = |value: i32| u32::from_ne_bytes(value.to_ne_bytes());
                if unsigned(camera.wrapping_add(CHARGE_REACH)) < unsigned(x) {
                    entity.parameter = 4;
                }
                entity.age = entity.age.wrapping_add(2);
                x = x.wrapping_add(speed(entity.age));
                camera = camera.wrapping_add(speed(entity.counter));
            }
            4 | 5 => {
                camera = camera.wrapping_add(speed(entity.counter));
                x = camera.wrapping_add(CHARGE_REACH);
                if entity.parameter == 4 {
                    entity.parameter = 5;
                } else {
                    entity.mark(ALIVE, false);
                }
            }
            _ => {}
        }
        entity.home = (x, camera);
        self.requests.push(Request::Zoid(x, camera));
    }

    /// `0x0804A1B4` and `0x0804B934`: the Zoid dashes back, sets off the
    /// next, waits, then lunges forward with a shake (kind 1, or 5 for the
    /// second) and settles.
    fn dash(&mut self, entity: &mut Entity, short: bool) {
        let speed = |value: u16| i32::from(value).wrapping_mul(LUNGE_STEP);
        let mut moved = true;
        match entity.parameter {
            0 => {
                entity.travelled = 0;
                entity.age = DASH_SPEED;
                entity.counter = 0;
                entity.home = (self.zoid_x, 0);
                entity.parameter = 1;
            }
            1 => {
                if entity.age == 0 {
                    entity.parameter = 2;
                    entity.travelled = 0;
                    moved = false;
                } else {
                    entity.travelled = entity.travelled.wrapping_add(1);
                    entity.age = entity.age.wrapping_sub(2);
                }
            }
            2 => {
                entity.parameter = 3;
                if let Some(next) = self.get_mut(entity.index + 1) {
                    next.mark(ACTIVE, true);
                }
            }
            3 => {
                if entity.travelled == 5 {
                    entity.parameter = 4;
                    entity.age = DASH_RETURN;
                    entity.travelled = 0;
                }
                entity.travelled = entity.travelled.wrapping_add(1);
                moved = false;
            }
            4 => {
                if entity.travelled == 10 {
                    entity.parameter = 5;
                    entity.travelled = 0;
                    self.requests
                        .push(Request::Shake(if short { 5 } else { 1 }));
                    if entity.sound != 0 {
                        self.requests.push(Request::Sound(entity.sound));
                    }
                }
                entity.travelled = entity.travelled.wrapping_add(1);
                entity.age = entity.age.wrapping_add(2);
            }
            5 if !short => {
                if entity.travelled == 10 {
                    entity.parameter = 6;
                    entity.travelled = 0;
                }
                entity.travelled = entity.travelled.wrapping_add(1);
                moved = false;
            }
            6 | 7 if !short => {
                entity.parameter += 1;
                moved = false;
            }
            5 | 8 => {
                entity.mark(ACTIVE, false);
                entity.mark(ALIVE, false);
                moved = false;
            }
            _ => moved = false,
        }
        if moved {
            entity.home.0 = entity.home.0.wrapping_add(speed(entity.age));
        }
        self.requests
            .push(Request::Zoid(entity.home.0, entity.home.1));
    }

    /// `0x0804ACE4`: the camera pans 4 pixels a frame for 33 frames, the
    /// Zoid with it, setting off the next at its link step.
    fn pan(&mut self, entity: &mut Entity) {
        if entity.started == 0 {
            entity.age = 0;
            entity.started = 1;
            if entity.shake != 0 {
                self.requests.push(Request::Shake(entity.shake));
            }
            if entity.sound != 0 {
                self.requests.push(Request::Sound(entity.sound));
            }
        } else {
            entity.age = entity.age.wrapping_add(1);
        }
        self.chain_from_link(entity, false);
        match entity.parameter {
            0 => {
                entity.travelled = 0;
                entity.age = 0;
                entity.home = (0, 0);
                entity.parameter = 1;
            }
            1 => {
                if entity.age > PAN_FRAMES {
                    entity.parameter = 2;
                }
                entity.age = entity.age.wrapping_add(1);
                entity.home.1 = entity.home.1.wrapping_add(PAN_STEP);
            }
            2 => {
                entity.mark(ACTIVE, false);
                entity.mark(ALIVE, false);
            }
            _ => {}
        }
        self.requests
            .push(Request::Zoid(entity.home.0, entity.home.1));
    }

    /// `0x0804BA90` and `0x0804BB60`: the palettes brighten to white a
    /// level every fifth frame (with the Zoid's and the mounted weapons'
    /// too, `all`) and stay so; or the scenery's alone, every second frame,
    /// and come back.
    fn flash(&mut self, entity: &mut Entity, all: bool) {
        let banks = if all { 8 } else { 4 };
        let flash = |requests: &mut Vec<Request>, level: u16| {
            requests.push(Request::Flash {
                banks,
                mounts: all,
                level: u8::try_from(level).unwrap_or(u8::MAX),
            });
        };
        match (entity.parameter, all) {
            (0, _) => {
                entity.age = u16::from(FLASH_NORMAL);
                entity.travelled = 0;
                if all {
                    entity.mark(ALIVE, false);
                }
                if let Some(next) = self.get_mut(entity.index + 1) {
                    next.mark(ACTIVE, true);
                }
                if entity.sound != 0 {
                    self.requests.push(Request::Sound(entity.sound));
                }
                flash(&mut self.requests, u16::from(FLASH_NORMAL));
                entity.parameter = 1;
            }
            (1, _) => {
                if entity.travelled != 0 {
                    entity.travelled -= 1;
                    return;
                }
                flash(&mut self.requests, entity.age);
                entity.travelled = if all { 4 } else { 1 };
                entity.age += 1;
                if entity.age >= FLASH_WHITE {
                    if !all {
                        entity.travelled = 5;
                    }
                    entity.parameter = 2;
                }
            }
            (2, true) => entity.mark(ACTIVE, false),
            (2, false) => {
                if entity.travelled != 0 {
                    entity.travelled -= 1;
                    return;
                }
                flash(&mut self.requests, entity.age);
                entity.age -= 1;
                if entity.age == u16::from(FLASH_NORMAL) {
                    entity.parameter = 3;
                }
            }
            (3, false) => {
                flash(&mut self.requests, u16::from(FLASH_NORMAL));
                entity.parameter = 4;
            }
            (4, false) => {
                entity.mark(ALIVE, false);
                entity.mark(ACTIVE, false);
            }
            _ => {}
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
            started: 0,
            counter: 0,
            base_y: 0,
            delay: 0,
            timer: 0,
            shake: 0,
            sound: 0,
            fading: 0,
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
        self.started = 0;
        self.mark(ACTIVE, false);
    }

    /// Counts a frame of its delay (`0x08049BE4`): whether it has run out.
    fn tick(&mut self) -> bool {
        self.timer = self.timer.wrapping_add(1);
        self.timer >= self.delay
    }

    /// Moves by `dx` (16.16, toward the left on the party's side) and down
    /// by `dy`, counting both ways' pixels (`0x08049B4C`).
    fn shift(&mut self, dx: i32, dy: i32, enemy_view: bool) {
        let pixels = |value: i32| u16::try_from(value.unsigned_abs() >> 16).unwrap_or(u16::MAX);
        self.travelled = self
            .travelled
            .wrapping_add(pixels(dx))
            .wrapping_add(pixels(dy));
        let dx = if enemy_view { dx } else { dx.wrapping_neg() };
        self.x = self.x.wrapping_add(dx);
        self.y = self.y.wrapping_add(dy);
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
    fn a_blend_fades_in_over_32_steps() {
        let mut step = 0;
        let mut blends = Vec::new();
        for _ in 0..32 {
            let (next, blend) = fade_step(step);
            blends.push(blend);
            step = next;
        }
        assert_eq!(step, 0);
        assert_eq!(blends[0], (0, 15));
        assert_eq!(blends[2], (0, 15));
        assert_eq!(blends[4], (1, 15));
        assert_eq!(blends[31], (14, 9));
    }

    #[test]
    fn shots_spread_toward_the_middle_by_the_zoids_size() {
        assert_eq!(spread_x(0x60, 0), 0x60);
        assert_eq!(spread_x(0x60, 1), 0x50);
        assert_eq!(spread_x(0x20, 2), 0x3C);
        assert_eq!(spread_y(0x20, 1), 0x38);
        assert_eq!(spread_y(0x20, 3), 0x40);
    }
}
