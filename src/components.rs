use std::ops::{Deref, DerefMut};

use crate::{math::Vec2, TextureId};
use ecs::{Component, Entity, World};
use sdl2::{pixels::Color, rect::Rect};

#[derive(Component, Clone, Copy)]
pub struct Position(Vec2<f32>);

impl Position {
    pub fn new(x: f32, y: f32) -> Self {
        Position(Vec2::new(x, y))
    }

    pub fn zero() -> Self {
        Position(Vec2::zero())
    }

    pub fn distance(&self, other: &Position) -> f32 {
        f32::sqrt((self.0.x - other.x).powi(2) + (self.0.y - other.y).powi(2))
    }
}

impl Deref for Position {
    type Target = Vec2<f32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Position {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Component)]
pub struct AnimatedSprite {
    // TODO u16
    pub textures: Vec<Vec<TextureId>>,
    pub state: u32,
    pub texture_index: u32,
    pub width: u32,
    pub height: u32,
    pub ticks: u32,
    pub ticks_per_frame: u32,
    pub flip_horizontal: bool,
    pub z_offset: Option<i16>,
}

impl AnimatedSprite {
    pub fn new(
        width: u32,
        height: u32,
        ticks_per_frame: u32,
        textures: Vec<Vec<TextureId>>,
        z_offset: Option<i16>,
    ) -> Self {
        AnimatedSprite {
            textures,
            state: 0,
            texture_index: 0,
            width,
            height,
            ticks: 0,
            ticks_per_frame,
            flip_horizontal: false,
            z_offset,
        }
    }

    pub fn switch_state(&mut self, state: u32) {
        if self.state != state {
            self.state = state;
            self.texture_index = 0;
            self.ticks = 0;
        }
    }
}

pub const CH_NONE: usize = 0;
pub const CH_NAV: usize = 1;
pub const CH_PROJECTILE: usize = 1 << 1;

#[derive(Clone)]
pub struct Collider<'a> {
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
    pub on_collide: Option<&'a dyn Fn(&World, Entity, Entity)>,
}

impl<'a> Collider<'a> {
    pub fn new(
        x_offset: i32,
        y_offset: i32,
        w: u32,
        h: u32,
        channels: usize,
        collides_with: usize,
        on_collide: Option<&'a dyn Fn(&World, Entity, Entity)>,
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
pub struct ColliderGroup<'a> {
    pub nav: Option<Collider<'a>>,
    pub hitbox: Option<Collider<'a>>,
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
    pub on_interact: Box<dyn Fn(&World, Entity)>,
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
