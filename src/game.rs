use std::ops::{Deref, DerefMut};

use ecs::{Component, Entity, Res, ResMut, World};
use rand::Rng;
use sdl2::{pixels::Color, rect::Rect};

use crate::{
    math::{Vec2, Vec3},
    Ctx, DepthBuffer, TextureID, DrawCmd,
};

#[derive(Component, Clone, Copy)]
struct Position(Vec2<f32>);

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
struct Sprite {
    texture_ids: [TextureID; 4],
    texture_index: u32,
    width: u32,
    height: u32,
    frames: u32,
    ticks: u32,
    swap_at: u32,
}

impl Sprite {
    pub fn draw(&self, depth_buffer: &mut DepthBuffer, x: i32, y: i32) {
        depth_buffer.push(DrawCmd {
            texture_id: self.texture_ids[self.texture_index as usize],
            pos: Vec3::new(
                x - (self.width / 2) as i32,
                y - (self.height / 2) as i32,
                y + (self.height / 2) as i32,
            ),
            w: self.width,
            h: self.height,
        });
    }
}

const CH_NAV: usize = 1;
const CH_PROJECTILE: usize = 1 << 1;

#[derive(Component, Clone)]
struct Collider<'a> {
    pub channels: usize,
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
    pub fn new(x: i32, y: i32, w: u32, h: u32, channels: usize) -> Self {
        Collider {
            channels,
            x_offset: x,
            y_offset: y,
            bounds: Rect::new(0, 0, w, h),
            is_colliding: false,
            left: false,
            right: false,
            top: false,
            bottom: false,
            on_collide: None,
        }
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.bounds.set_x(x);
        self.bounds.set_y(y);
    }
}

#[derive(Component)]
struct Hitbox<'a>(Collider<'a>);

impl<'a> Deref for Hitbox<'a> {
    type Target = Collider<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> DerefMut for Hitbox<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Component)]
struct Player {
    pub fire_cooldown: usize,
    pub can_fire_in: usize,
}

#[derive(Component)]
struct Enemy {}

#[derive(Component)]
struct Projectile {
    pub velocity: Vec2<f32>,
    pub ticks_left: usize,
}

#[derive(Component)]
pub struct Light {
    pub pos: Vec2<i32>,
    pub radius: i16,
    pub color: Color,
}

pub fn init(world: &World) {
    spawn_player(world, Vec2::new(400.0, 400.0));
}

pub fn update(world: &World) {
    let ctx = world.get_resource_mut::<Ctx>().unwrap();

    if ctx.enemy_spawn_in == 0 {
        let side = rand::thread_rng().gen_range(0..=3);
        let (x_range, y_range) = match side {
            0 => (-64..=800, -64..=-64),
            1 => (-64..=-64, -64..=800),
            2 => (-64..=800, 800..=800),
            3 => (800..=800, -64..=800),
            _ => unreachable!(),
        };

        spawn_enemy(
            world,
            Vec2::new(
                rand::thread_rng().gen_range(x_range) as f32,
                rand::thread_rng().gen_range(y_range) as f32,
            ),
        );
        ctx.enemy_spawn_in = ctx.enemy_spawn_cooldown;
    } else {
        ctx.enemy_spawn_in -= 1;
    }

    update_player(world);
    update_enemies(world);
    update_projectiles(world);
    update_lights(world);
    fix_colliders(world);
    detect_collisions(world);

    let mut despawn_queue = world
        .get_resource_mut::<Ctx>()
        .unwrap()
        .despawn_queue
        .write()
        .unwrap();

    for e in despawn_queue.iter() {
        world.despawn(*e);
    }

    despawn_queue.clear();
}

pub fn render(world: &World) {
    let ctx = world.get_resource_mut::<Ctx>().unwrap();
    draw_sprites(world);

    if ctx.debug_draw_colliders {
        debug_draw_colliders(world);
    }

    if ctx.debug_draw_hitboxes {
        debug_draw_hitboxes(world);
    }
}

fn spawn_player(world: &World, pos: Vec2<f32>) {
    let ctx = world.get_resource::<Ctx>().unwrap();

    let collider = Collider::new(-20, 40, 40, 24, CH_NAV);

    world.spawn(&[
        &Player {
            fire_cooldown: ctx.player_fire_cooldown,
            can_fire_in: 0,
        },
        &Position(Vec2::new(pos.x, pos.y)),
        &Sprite {
            texture_ids: [
                ctx.player_textures[0],
                ctx.player_textures[1],
                ctx.player_textures[0],
                ctx.player_textures[2],
            ],
            texture_index: 0,
            width: 64,
            height: 128,
            frames: 4,
            ticks: 0,
            swap_at: 20,
        },
        &collider,
        &Light {
            pos: Vec2::<i32> {x: pos.x.round() as i32, y: pos.y.round() as i32},
            radius: 100,
            color: Color::RGB(255, 255, 255)
        }
    ]);
}

fn spawn_enemy(world: &World, pos: Vec2<f32>) {
    let ctx = world.get_resource::<Ctx>().unwrap();

    world.spawn(&[
        &Enemy {},
        &Position(Vec2::new(pos.x, pos.y)),
        &Sprite {
            texture_ids: [
                ctx.enemy_textures[0],
                ctx.enemy_textures[1],
                ctx.enemy_textures[0],
                ctx.enemy_textures[1],
            ],
            texture_index: 0,
            width: 64,
            height: 64,
            frames: 2,
            ticks: rand::thread_rng().gen_range(0..30),
            swap_at: 30,
        },
        &Collider::new(-25, 10, 50, 24, CH_NAV),
        &Hitbox(Collider::new(-22, -15, 44, 44, CH_PROJECTILE)),
    ]);
}

fn spawn_bullet(world: &World, pos: Vec2<f32>, velocity_normal: Vec2<f32>) {
    let ctx = world.get_resource::<Ctx>().unwrap();

    let mut collider = Collider::new(-6, -6, 12, 12, CH_PROJECTILE);
    collider.on_collide = Some(&|world: &World, me: Entity, other: Entity| {
        let mut despawn_queue = world
            .get_resource::<Ctx>()
            .unwrap()
            .despawn_queue
            .write()
            .unwrap();
        despawn_queue.push(me);
        despawn_queue.push(other);
    });

    world.spawn(&[
        &Projectile {
            velocity: velocity_normal.scaled(ctx.bullet_speed),
            ticks_left: ctx.bullet_lifetime,
        },
        &Position(Vec2::new(pos.x, pos.y)),
        &Sprite {
            texture_ids: [
                ctx.bullet_textures[0],
                ctx.bullet_textures[1],
                ctx.bullet_textures[0],
                ctx.bullet_textures[1],
            ],
            texture_index: 0,
            width: 16,
            height: 16,
            frames: 2,
            ticks: rand::thread_rng().gen_range(0..30),
            swap_at: 30,
        },
        &collider,
    ]);
}

fn update_player(world: &World) {
    let ctx = world.get_resource::<Ctx>().unwrap();

    world.run(
        |player: &mut Player, pos: &mut Position, collider: &Collider| {
            if ctx.input.up && !collider.top {
                pos.y -= ctx.player_speed;
            }
            if ctx.input.down && !collider.bottom {
                pos.y += ctx.player_speed;
            }
            if ctx.input.left && !collider.left {
                pos.x -= ctx.player_speed;
            }
            if ctx.input.right && !collider.right {
                pos.x += ctx.player_speed;
            }

            if player.can_fire_in > 0 {
                player.can_fire_in -= 1;
            }

            if player.can_fire_in == 0 {
                let mut trajectory = Vec2::zero();

                if ctx.input.fire_right {
                    trajectory.x += 1.0;
                }
                if ctx.input.fire_left {
                    trajectory.x -= 1.0;
                }
                if ctx.input.fire_up {
                    trajectory.y -= 1.0;
                }
                if ctx.input.fire_down {
                    trajectory.y += 1.0;
                }

                if trajectory.magnitude() > 0.0 {
                    spawn_bullet(world, Vec2::new(pos.x, pos.y), trajectory);
                    player.can_fire_in = player.fire_cooldown;
                }
            }
        },
    );
}

fn update_enemies(world: &World) {
    let mut player_pos = Position(Vec2::zero());

    world.run(|_: &Player, pos: &Position| {
        player_pos = *pos;
    });

    world.run(
        |_: &Enemy, pos: &mut Position, collider: &mut Collider, ctx: Res<Ctx>| {
            let mut v = Vec2::<f32>::new(player_pos.x - pos.x, player_pos.y - pos.y);

            v.normalize();
            v.scale(ctx.enemy_speed);

            if v.x > 0.0 && collider.right {
                v.x = 0.0;
            }

            if v.x < 0.0 && collider.left {
                v.x = 0.0;
            }

            if v.y > 0.0 && collider.bottom {
                v.y = 0.0;
            }

            if v.y < 0.0 && collider.top {
                v.y = 0.0;
            }

            pos.x += v.x;
            pos.y += v.y;
        },
    );
}

fn update_projectiles(world: &World) {
    world.run(
        |entity: &Entity, projectile: &mut Projectile, pos: &mut Position| {
            if projectile.ticks_left == 0 {
                world
                    .get_resource::<Ctx>()
                    .unwrap()
                    .despawn_queue
                    .write()
                    .unwrap()
                    .push(*entity);
            } else {
                pos.x += projectile.velocity.x;
                pos.y += projectile.velocity.y;
                projectile.ticks_left -= 1;
            }
        },
    );
}

fn fix_colliders(world: &World) {
    world.run(|collider: &mut Collider, pos: &Position| {
        collider.set_pos(
            pos.x.round() as i32 + collider.x_offset,
            pos.y.round() as i32 + collider.y_offset,
        );
    });

    world.run(|hitbox: &mut Hitbox, pos: &Position| {
        let x_offset = hitbox.x_offset;
        let y_offset = hitbox.y_offset;
        hitbox.set_pos(
            pos.x.round() as i32 + x_offset,
            pos.y.round() as i32 + y_offset,
        );
    });
}

fn detect_collisions(world: &World) {
    world.run(|e1: &Entity, c1: &mut Collider| {
        c1.is_colliding = false;
        c1.left = false;
        c1.right = false;
        c1.top = false;
        c1.bottom = false;

        world.run(|e2: &Entity, c2: &Collider| {
            if *e1 != *e2 && c1.channels & c2.channels != 0 {
                if c1.bounds.has_intersection(c2.bounds) {
                    c1.is_colliding = true;

                    if let Some(on_collide) = c1.on_collide {
                        on_collide(world, *e1, *e2);
                    }

                    let d_bottom = c2.bounds.bottom() - c1.bounds.top();
                    let d_top = c1.bounds.bottom() - c2.bounds.top();
                    let d_left = c1.bounds.right() - c2.bounds.left();
                    let d_right = c2.bounds.right() - c1.bounds.left();

                    if d_top < d_bottom && d_top < d_left && d_top < d_right {
                        c1.bottom = true;
                    } else if d_bottom < d_top && d_bottom < d_left && d_bottom < d_right {
                        c1.top = true;
                    } else if d_left < d_right && d_left < d_top && d_left < d_bottom {
                        c1.right = true;
                    } else if d_right < d_left && d_right < d_top && d_right < d_bottom {
                        c1.left = true;
                    }
                }
            }
        });
    });

    world.run(|e1: &Entity, c1: &mut Hitbox| {
        c1.left = false;
        c1.right = false;
        c1.top = false;
        c1.bottom = false;

        world.run(|e2: &Entity, _: &Projectile, c2: &mut Collider| {
            if *e1 != *e2 && c1.channels & c2.channels != 0 {
                if c1.bounds.has_intersection(c2.bounds) {
                    c1.is_colliding = true;

                    if let Some(on_collide) = c2.on_collide {
                        on_collide(world, *e2, *e1);
                    }

                    let d_bottom = c2.bounds.bottom() - c1.bounds.top();
                    let d_top = c1.bounds.bottom() - c2.bounds.top();
                    let d_left = c1.bounds.right() - c2.bounds.left();
                    let d_right = c2.bounds.right() - c1.bounds.left();

                    if d_top < d_bottom && d_top < d_left && d_top < d_right {
                        c1.bottom = true;
                    } else if d_bottom < d_top && d_bottom < d_left && d_bottom < d_right {
                        c1.top = true;
                    } else if d_left < d_right && d_left < d_top && d_left < d_bottom {
                        c1.right = true;
                    } else if d_right < d_left && d_right < d_top && d_right < d_bottom {
                        c1.left = true;
                    }
                }
            }
        });
    });
}

fn draw_sprites(world: &World) {
    world.run(
        |pos: &mut Position, sprite: &mut Sprite, mut depth_buffer: ResMut<DepthBuffer>| {
            sprite.ticks += 1;
            if sprite.ticks >= sprite.swap_at {
                sprite.texture_index = if sprite.texture_index == sprite.frames - 1 {
                    0
                } else {
                    sprite.texture_index + 1
                };
                sprite.ticks = 0;
            }

            sprite.draw(
                &mut depth_buffer,
                pos.x.round() as i32,
                pos.y.round() as i32,
            );
        },
    );

    let ctx = world.get_resource_mut::<Ctx>().unwrap();
    let depth_buffer = world.get_resource_mut::<DepthBuffer>().unwrap();
    depth_buffer.draw_to_canvas(&mut ctx.canvas, &ctx.textures);
}

fn update_lights(world: &World) {
    world.run(|pos: &Position, light: &mut Light| {
        light.pos.x = pos.x.round() as i32;
        light.pos.y = pos.y.round() as i32;
    })
}

// DEBUG

fn debug_draw_colliders(world: &World) {
    world.run(|collider: &Collider, mut ctx: ResMut<Ctx>| {
        if collider.is_colliding {
            ctx.canvas.set_draw_color(Color::RGB(255, 0, 0));
            let _ = ctx.canvas.draw_rect(collider.bounds);
        } else {
            ctx.canvas.set_draw_color(Color::RGB(0, 255, 0));
            let _ = ctx.canvas.draw_rect(collider.bounds);
        }
    });
}

fn debug_draw_hitboxes(world: &World) {
    world.run(|hitbox: &Hitbox, mut ctx: ResMut<Ctx>| {
        if hitbox.is_colliding {
            ctx.canvas.set_draw_color(Color::RGB(255, 0, 0));
            let _ = ctx.canvas.draw_rect(hitbox.bounds);
        } else {
            ctx.canvas.set_draw_color(Color::RGB(255, 255, 0));
            let _ = ctx.canvas.draw_rect(hitbox.bounds);
        }
    });
}
