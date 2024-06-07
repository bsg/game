use std::ops::{Deref, DerefMut};

use crate::{math::Vec2, AnimationId, Sprite};
use ecs::{Component, Entity, World};
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
pub struct Spawner {
    pub is_active: bool,
    pub cooldown: u32,
    pub ticks_left: u32,
    pub particle_cooldown: u32,
    pub particle_ticks_left: u32,
}

#[derive(Component)]
pub struct Static {}

pub trait Item {
    fn name(&self) -> &'static str;
    fn sprite(&self) -> Sprite;
    fn tick(&self);
}

pub struct PerfectlyGenericItem {}
impl Item for PerfectlyGenericItem {
    fn name(&self) -> &'static str {
        "perfectly_generic_item"
    }

    fn sprite(&self) -> Sprite {
        (14, 0, 1, 1).into()
    }

    fn tick(&self) {
        todo!()
    }
}

pub struct TestItem {}
impl Item for TestItem {
    fn name(&self) -> &'static str {
        "test_item"
    }

    fn sprite(&self) -> Sprite {
        (10, 0, 1, 1).into()
    }

    fn tick(&self) {
        todo!()
    }
}

pub struct Inventory {
    items: [Option<&'static dyn Item>; 8],
    num_items: u16,
    active_item_idx: u16,
}

// FIXME awful everything
impl Inventory {
    pub fn new() -> Self {
        Inventory {
            items: [None; 8],
            num_items: 0,
            active_item_idx: 0,
        }
    }

    pub fn insert(&mut self, item: &'static dyn Item) -> bool {
        if self.num_items < 8 {
            for slot in self.items.iter_mut() {
                if slot.is_none() {
                    let _ = slot.insert(item);
                    self.num_items += 1;
                    return true;
                }
            }
            false
        } else {
            false
        }
    }

    pub fn take(&mut self, name: &'static str) -> Option<&'static dyn Item> {
        if self.num_items > 0 {
            for mut slot in self.items {
                if let Some(item) = slot {
                    if item.name() == name {
                        self.num_items -= 1;
                        return slot.take();
                    }
                }
            }
        }
        None
    }

    pub fn has_item(&self, name: &'static str) -> bool {
        if self.num_items > 0 {
            for item in self.items.into_iter().flatten() {
                if item.name() == name {
                    return true;
                }
            }
        }
        false
    }

    pub fn active_item(&self) -> Option<&dyn Item> {
        self.items[self.active_item_idx as usize]
    }

    pub fn tick(&mut self) {
        for item in self.items.iter_mut().flatten() {
            item.tick();
        }
    }

    pub fn set_active(&mut self, idx: u16) {
        self.active_item_idx = idx;
    }

    pub fn set_active_offset(&mut self, offset: i16) {
        let mut i = 0;
        if offset >= 0 {
            self.active_item_idx = (self.active_item_idx as i16 + offset) as u16 % 8;
            while i < 8 && self.items[self.active_item_idx as usize].is_none() {
                i += 1;
                self.active_item_idx = (self.active_item_idx + 1) % 8;
            }
        } else {
            self.active_item_idx = (self.active_item_idx as i16 + 8 + offset) as u16 % 8;
            while i < 8 && self.items[self.active_item_idx as usize].is_none() {
                i += 1;
                self.active_item_idx = (self.active_item_idx - 1) % 8;
            }
        }
    }

    /// Get the item with offset relative to active item
    pub fn get_relative(&self, offset: isize) -> Option<&dyn Item> {
        let mut i = 0;
        if offset >= 0 {
            let mut idx = (self.active_item_idx as isize + offset % 8) as usize;
            while i < 8 && self.items[idx % 8].is_none() {
                i += 1;
                idx += 1;
            }
            self.items[idx % 8]
        } else {
            let mut idx = (self.active_item_idx as isize + 8 + offset) as usize % 8;
            while i < 8 && self.items[idx % 8].is_none() {
                i += 1;
                idx -= 1;
            }
            self.items[idx % 8]
        }
    }
}
