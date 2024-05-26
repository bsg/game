use std::ops::{Deref, DerefMut};

use crate::{math::Vec2, AnimationId};
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
        x_offset: i16,
        y_offset: i16,
        width: u32,
        height: u32,
        ticks_per_frame: u32,
        anim: AnimationId,
        z_offset: Option<i16>,
    ) -> Self {
        AnimatedSprite {
            x_offset,
            y_offset,
            anim,
            frame: 0,
            width,
            height,
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
pub const CH_PROJECTILE: usize = 1 << 1;

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
        x_offset: i32,
        y_offset: i32,
        w: u32,
        h: u32,
        channels: usize,
        collides_with: usize,
        on_collide: Option<fn(&World, Entity, Entity)>,
    ) -> Self {
        Collider {
            channels,
            collides_with,
            x_offset,
            y_offset,
            bounds: Rect::new(0, 0, w, h),
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
    pub radius: i16,
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
    pub cooldown: usize, // TODO won't need once we have just_pressed
    pub on_interact: fn(&World, Entity),
    pub ticks_left: usize,
}

#[derive(Component)]
pub struct Spawner {
    pub is_active: bool,
    pub cooldown: u32,
    pub ticks_left: u32,
    pub particle_cooldown: u32,
    pub particle_ticks_left: u32,
}
