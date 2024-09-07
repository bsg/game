// TODO move inventory related stuff elsewhere since inventory is not a component

use std::ops::{Deref, DerefMut};

use crate::{math::Vec2, AnimationId, Ctx, Sprite};
use ecs::{Component, Entity, With, World};
use sdl2::{pixels::Color, rect::Rect};

#[derive(Component)]
pub struct Pos(Vec2<f32>);

impl Pos {
    pub fn new(x: f32, y: f32) -> Self {
        Pos(Vec2::new(x, y))
    }

    pub fn zero() -> Self {
        Pos(Vec2::zero())
    }

    pub fn distance(&self, other: &Pos) -> f32 {
        f32::sqrt((self.0.x - other.x).powi(2) + (self.0.y - other.y).powi(2))
    }
}

impl Deref for Pos {
    type Target = Vec2<f32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Pos {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<(f32, f32)> for Pos {
    fn from(value: (f32, f32)) -> Self {
        Pos::new(value.0, value.1)
    }
}

impl From<(i32, i32)> for Pos {
    fn from(value: (i32, i32)) -> Self {
        Pos::new(value.0 as f32, value.1 as f32)
    }
}

#[derive(Component)]
pub struct AnimatedSprite {
    // TODO u16
    anim: AnimationId,
    pub frame: u32,
    pub width: u32,
    pub height: u32,
    pub ticks: u32,
    pub ticks_per_frame: u32,
    pub flip_horizontal: bool,
    pub x_offset: i16,
    pub y_offset: i16,
    pub z_offset: Option<i16>,
}

impl AnimatedSprite {
    pub fn new(
        rect: (i16, i16, u32, u32),
        ticks_per_frame: u32,
        anim: AnimationId,
        z_offset: Option<i16>,
    ) -> Self {
        AnimatedSprite {
            x_offset: rect.0,
            y_offset: rect.1,
            anim,
            frame: 0,
            width: rect.2,
            height: rect.3,
            ticks: 0,
            ticks_per_frame,
            flip_horizontal: false,
            z_offset,
        }
    }

    pub fn anim(&self) -> AnimationId {
        self.anim
    }

    pub fn switch_anim(&mut self, anim: AnimationId, ticks_per_frame: u32) {
        if self.anim != anim {
            self.anim = anim;
            self.frame = 0;
            self.ticks = 0;
            self.ticks_per_frame = ticks_per_frame;
        }
    }
}

pub const CH_NONE: usize = 0;
pub const CH_NAV: usize = 1;
pub const CH_HITBOX: usize = 1 << 1;

#[derive(Clone, Copy)]
pub struct Collider {
    pub channels: usize,
    pub collides_with: usize,
    pub x_offset: i32,
    pub y_offset: i32,
    pub bounds: Rect,
    pub is_colliding: bool,
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
    pub on_collide: Option<fn(&World, Entity, Entity)>,
}

impl Collider {
    pub fn new(
        rect: (i32, i32, u32, u32),
        channels: usize,
        collides_with: usize,
        on_collide: Option<fn(&World, Entity, Entity)>,
    ) -> Self {
        Collider {
            channels,
            collides_with,
            x_offset: rect.0,
            y_offset: rect.1,
            bounds: Rect::new(0, 0, rect.2, rect.3),
            is_colliding: false,
            left: false,
            right: false,
            top: false,
            bottom: false,
            on_collide,
        }
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.bounds.set_x(x);
        self.bounds.set_y(y);
    }
}

#[derive(Component)]
pub struct ColliderGroup {
    pub nav: Option<Collider>,
    pub hitbox: Option<Collider>,
}

#[derive(Component)]
pub struct Player {
    pub fire_cooldown: usize,
    pub can_fire_in: usize,
}

#[derive(Component)]
pub struct Enemy {}

#[derive(Component)]
pub struct Projectile {
    pub velocity: Vec2<f32>,
    pub ticks_left: usize,
}

#[derive(Component)]
pub struct Light {
    pub radius: u16,
    pub color: Color,
    pub intensity: f32,
}

#[derive(Component)]
pub struct Floor {}

#[derive(Component)]
pub struct Wall {}

#[derive(Component)]
pub struct Prop {}

#[derive(Component)]
pub struct Interactable {
    pub on_interact: fn(&World, Entity),
}

#[derive(Component)]
pub struct ProximityIndicator {
    pub range: f32,
    pub sprite: AnimatedSprite
}

#[derive(Component)]
pub struct ParticleEmitter {
    pub is_active: bool,
    pub particle_cooldown: u32,
    pub particle_ticks_left: u32,
}

#[derive(Component)]
pub struct Static {}

pub trait Item {
    fn name(&self) -> &'static str;
    fn sprite(&self) -> Sprite;
    fn on_tick(&mut self, is_active: bool, world: &World) -> InventoryCmd;
    fn on_use(&mut self, world: &World) -> InventoryCmd;
    fn on_select(&mut self, world: &World);
    fn on_deselect(&mut self, world: &World);
}

pub struct PerfectlyGenericItem {}
impl Item for PerfectlyGenericItem {
    fn name(&self) -> &'static str {
        "perfectly_generic_item"
    }

    fn sprite(&self) -> Sprite {
        (14, 0, 1, 1).into()
    }

    fn on_tick(&mut self, _is_active: bool, _world: &World) -> InventoryCmd {
        InventoryCmd::None
    }

    fn on_use(&mut self, _world: &World) -> InventoryCmd {
        InventoryCmd::None
    }

    fn on_select(&mut self, _world: &World) {}

    fn on_deselect(&mut self, _world: &World) {}
}

pub struct TestItem {}
impl Item for TestItem {
    fn name(&self) -> &'static str {
        "test_item"
    }

    fn sprite(&self) -> Sprite {
        (10, 0, 1, 1).into()
    }

    fn on_tick(&mut self, _is_active: bool, _world: &World) -> InventoryCmd {
        InventoryCmd::None
    }

    fn on_use(&mut self, _world: &World) -> InventoryCmd {
        InventoryCmd::None
    }

    fn on_select(&mut self, _world: &World) {}

    fn on_deselect(&mut self, _world: &World) {}
}

pub struct Torch {
    pub is_lit: bool,
    pub ticks_max: usize,
    pub ticks_left: usize,
}

impl Torch {
    pub fn new() -> Self {
        Torch {
            is_lit: false,
            ticks_max: 3600,
            ticks_left: 3600,
        }
    }
}

impl Item for Torch {
    fn name(&self) -> &'static str {
        "torch"
    }

    fn sprite(&self) -> Sprite {
        (10, 1, 1, 1).into()
    }

    fn on_tick(&mut self, _is_active: bool, world: &World) -> InventoryCmd {
        if self.ticks_left == 0 {
            world.run(|light: &mut Light, _: With<Player>| {
                light.radius = 0;
            });
            return InventoryCmd::Remove;
        } else {
            world.run(|light: &mut Light, _: With<Player>| {
                light.radius = (100. * self.ticks_left as f32 / self.ticks_max as f32) as u16 + 20;
            });
        }

        if self.is_lit {
            self.ticks_left = self.ticks_left.saturating_sub(1);
        }

        InventoryCmd::None
    }

    fn on_use(&mut self, world: &World) -> InventoryCmd {
        self.is_lit = true;
        world.run(|light: &mut Light, _: With<Player>| {
            light.color = Color::RGB(255, 255, 100);
            light.radius = 150;
            light.intensity = 1.;
        });
        InventoryCmd::None
    }

    fn on_select(&mut self, _world: &World) {}

    fn on_deselect(&mut self, _world: &World) {}
}

pub struct Chemlight {
    pub uses_left: u16,
}

impl Chemlight {
    pub fn new() -> Self {
        Chemlight { uses_left: 5 }
    }
}

impl Item for Chemlight {
    fn name(&self) -> &'static str {
        "chemlight"
    }

    fn sprite(&self) -> Sprite {
        (12, 1, 1, 1).into()
    }

    fn on_tick(&mut self, _is_active: bool, _world: &World) -> InventoryCmd {
        InventoryCmd::None
    }

    fn on_use(&mut self, world: &World) -> InventoryCmd {
        let ctx = world.resource::<Ctx>().unwrap();
        world.spawn(&[
            &ctx.player_pos,
            &AnimatedSprite::new(
                (-16, -16, 32, 32),
                0,
                ctx.animations.get("chemlight").unwrap(),
                None,
            ),
            &Light {
                radius: 120,
                color: Color::RGB(0, 255, 0),
                intensity: 1.,
            },
        ]);
        self.uses_left -= 1;
        if self.uses_left == 0 {
            InventoryCmd::Remove
        } else {
            InventoryCmd::None
        }
    }

    fn on_select(&mut self, _world: &World) {}

    fn on_deselect(&mut self, _world: &World) {}
}

pub enum InventoryCmd {
    None,
    Remove,
}

// FIXME awful everything
pub struct Inventory {
    items: [Option<Box<dyn Item>>; 8],
    num_items: u16,
    active_item_idx: u16,
}

impl Inventory {
    pub fn new() -> Self {
        Inventory {
            // TODO there's supposed to be a way to do this
            // items: [None; 8],
            items: [None, None, None, None, None, None, None, None],
            num_items: 0,
            active_item_idx: 0,
        }
    }

    pub fn insert(&mut self, item: impl Item + 'static, world: &World) -> bool {
        if self.num_items < 8 {
            for slot in self.items.iter_mut() {
                if slot.is_none() {
                    let item = slot.insert(Box::new(item));
                    if self.num_items == 0 {
                        item.on_select(world)
                    }
                    self.num_items += 1;
                    return true;
                }
            }
            false
        } else {
            false
        }
    }

    pub fn has_item(&self, name: &'static str) -> bool {
        if self.num_items > 0 {
            for item in self.items.iter().flatten() {
                if item.name() == name {
                    return true;
                }
            }
        }
        false
    }

    pub fn active_item(&self) -> Option<&dyn Item> {
        self.items[self.active_item_idx as usize].as_deref()
    }

    fn next_idx_right(&self) -> Option<u16> {
        let mut idx = self.active_item_idx;
        let mut i = 0;
        while i < 7 {
            if self.items[idx as usize].is_some() && idx != self.active_item_idx {
                return Some(idx);
            }
            idx = (idx + 1) % 8;
            i += 1;
        }
        None
    }

    fn next_idx_left(&self) -> Option<u16> {
        let mut idx = self.active_item_idx;
        let mut i = 0;
        while i < 7 {
            if self.items[idx as usize].is_some() && idx != self.active_item_idx {
                return Some(idx);
            }
            idx = (idx.wrapping_sub(1)) % 8;
            i += 1;
        }
        None
    }

    pub fn tick(&mut self, world: &World) {
        for i in 0..8 {
            if let Some(item) = self.items[i].as_mut() {
                let cmd = item.on_tick(i == self.active_item_idx as usize, world);
                match cmd {
                    InventoryCmd::None => (),
                    InventoryCmd::Remove => {
                        *self.items.get_mut(i).unwrap() = None;
                    }
                }
            }
        }
    }

    pub fn set_active_offset(&mut self, offset: i16, world: &World) {
        let mut i = 0;
        if offset == 0 {
            return;
        }

        if let Some(item) = self.items.get_mut(self.active_item_idx as usize).unwrap() {
            item.on_deselect(world);
        }

        if offset > 0 {
            self.active_item_idx = (self.active_item_idx as i16 + offset) as u16 % 8;
            while i < 8 && self.items[self.active_item_idx as usize].is_none() {
                i += 1;
                self.active_item_idx = (self.active_item_idx + 1) % 8;
            }
        } else {
            self.active_item_idx = (self.active_item_idx as i16 + 8 + offset) as u16 % 8;
            while i < 8 && self.items[self.active_item_idx as usize].is_none() {
                i += 1;
                self.active_item_idx = self.active_item_idx.wrapping_sub(1) % 8;
            }
        }

        if let Some(item) = self.items.get_mut(self.active_item_idx as usize).unwrap() {
            item.on_select(world);
        }
    }

    pub fn get_left(&self) -> Option<&dyn Item> {
        if let Some(idx) = self.next_idx_left() {
            return self.items[idx as usize].as_deref();
        }
        None
    }

    pub fn get_right(&self) -> Option<&dyn Item> {
        if let Some(idx) = self.next_idx_right() {
            return self.items[idx as usize].as_deref();
        }
        None
    }

    pub fn do_use(&mut self, world: &World) {
        if let Some(item) = self.items.get_mut(self.active_item_idx as usize).unwrap() {
            let cmd = item.on_use(world);
            match cmd {
                InventoryCmd::None => (),
                InventoryCmd::Remove => {
                    *self.items.get_mut(self.active_item_idx as usize).unwrap() = None;
                }
            }
        }
    }
}
