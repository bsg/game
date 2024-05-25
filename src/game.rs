// BUG environment capture by closures assigned to a component member is broken
// boxed closures are fine (see Interactable). is this ecs related?

use std::ops::{Deref, DerefMut};

use ecs::{entity, Component, Entity, Res, ResMut, With, Without, World};
use sdl2::{pixels::Color, rect::Rect};

use crate::{
    math::{Vec2, Vec3},
    Ctx, DepthBuffer, DrawCmd, TextureID,
};

#[derive(Component, Clone, Copy)]
pub struct Position(Vec2<f32>);

impl Position {
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
struct AnimatedSprite {
    // TODO u16
    textures: Vec<Vec<TextureID>>,
    state: u32,
    texture_index: u32,
    width: u32,
    height: u32,
    ticks: u32,
    ticks_per_frame: u32,
    flip_horizontal: bool,
    z_offset: Option<i16>,
}

impl AnimatedSprite {
    pub fn new(
        width: u32,
        height: u32,
        ticks_per_frame: u32,
        textures: Vec<Vec<TextureID>>,
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

impl AnimatedSprite {
    pub fn draw(&self, depth_buffer: &mut DepthBuffer, x: i32, y: i32) {
        // TODO bounds check for indices
        depth_buffer.push(DrawCmd {
            texture_id: self.textures[self.state as usize][self.texture_index as usize],
            pos: Vec3::new(
                x - (self.width / 2) as i32,
                y - (self.height / 2) as i32,
                y + (self.height / 2) as i32,
            ),
            w: self.width,
            h: self.height,
            flip_horizontal: self.flip_horizontal,
        });
    }
}

const CH_NAV: usize = 1;
const CH_PROJECTILE: usize = 1 << 1;

#[derive(Clone)]
struct Collider<'a> {
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
struct ColliderGroup<'a> {
    pub nav: Option<Collider<'a>>,
    pub hitbox: Option<Collider<'a>>,
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
    pub radius: i16,
    pub color: Color,
}

#[derive(Component)]
struct Floor {}

#[derive(Component)]
struct Wall {}

#[derive(Component)]
struct Prop {}

#[derive(Component)]
struct Interactable {
    cooldown: usize, // TODO won't need once we have just_pressed
    on_interact: Box<dyn Fn(&World, Entity)>,
    ticks_left: usize,
}

#[derive(Component)]
struct Spawner {
    is_active: bool,
    cooldown: u32,
    ticks_left: u32,
}

pub fn init(world: &World) {
    spawn_player(world, Vec2::new(400.0, 400.0));

    let ctx = world.get_resource_mut::<Ctx>().unwrap();
    let floor_texture = ctx.floor_texture;

    for tile_x in 0..32 {
        for tile_y in 0..32 {
            world.spawn(entity!(
                Floor {},
                Position(Vec2::new(tile_x as f32 * 64.0, tile_y as f32 * 64.0)),
                AnimatedSprite::new(64, 64, 0, vec![vec![floor_texture]], None)
            ));
        }
    }

    let wall_texture = ctx.wall_texture;
    for tile_x in 0..32 {
        let x = tile_x as f32 * 64.0 - 32.0;
        let y = 32.0;
        world.spawn(entity!(
            Wall {},
            Position(Vec2::new(x, y)),
            AnimatedSprite::new(64, 64, 0, vec![vec![wall_texture]], None),
            ColliderGroup {
                nav: Some(Collider::new(
                    -32,
                    0,
                    64,
                    32,
                    CH_NAV,
                    CH_NAV | CH_PROJECTILE,
                    None
                )),
                hitbox: None
            }
        ));

        if tile_x != 7 && tile_x != 8 {
            world.spawn(entity!(
                Wall {},
                Position(Vec2::new(tile_x as f32 * 64.0 - 32.0, 800.0 - 256.0)),
                AnimatedSprite::new(64, 64, 0, vec![vec![wall_texture]], None),
                ColliderGroup {
                    nav: Some(Collider::new(
                        -32,
                        0,
                        64,
                        32,
                        CH_NAV,
                        CH_NAV | CH_PROJECTILE,
                        None
                    )),
                    hitbox: None
                }
            ));
        }
    }

    let torch_textures = ctx.torch_textures;
    world.spawn(entity!(
        Position(Vec2::new(350.0, 570.0)),
        AnimatedSprite::new(
            64,
            64,
            5,
            vec![vec![
                torch_textures[0],
                torch_textures[1],
                torch_textures[0],
                torch_textures[2],
            ]],
            None
        ),
        Light {
            radius: 100,
            color: Color::RGB(255, 255, 200),
        }
    ));

    let spawner_entity = world.spawn(entity!(
        Prop {},
        Position(Vec2::new(500.0, 400.0)),
        AnimatedSprite::new(32, 32, 0, vec![vec![ctx.spawner_texture]], None),
        Spawner {
            is_active: false,
            cooldown: 60,
            ticks_left: 0
        },
        Light {
            radius: 0,
            color: Color::RGB(150, 255, 150)
        }
    ));

    world.spawn(entity!(
        Position(Vec2::new(200.0, 200.0)),
        AnimatedSprite::new(32, 32, 0, vec![vec![ctx.lever_texture]], None),
        Interactable {
            cooldown: 15,
            on_interact: Box::new(move |world: &World, me: Entity| {
                let sprite = world.get_component_mut::<AnimatedSprite>(me).unwrap();
                sprite.flip_horizontal = !sprite.flip_horizontal;
                let spawner = world
                    .get_component_mut::<Spawner>(spawner_entity)
                    .unwrap();
                spawner.is_active = !spawner.is_active;
                world
                    .get_component_mut::<Light>(spawner_entity)
                    .unwrap()
                    .radius = if spawner.is_active { 60 } else { 0 };
            }),
            ticks_left: 0
        }
    ));
}

pub fn update(world: &World) {
    update_spawners(world);
    update_player(world);
    update_enemies(world);
    update_projectiles(world);
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
    draw_sprites(world);
    debug_draw_colliders(world);
}

fn spawn_player(world: &World, pos: Vec2<f32>) {
    let ctx = world.get_resource::<Ctx>().unwrap();
    let tex = ctx.player_textures;

    world.spawn(entity!(
        Player {
            fire_cooldown: ctx.player_fire_cooldown,
            can_fire_in: 0,
        },
        Position(Vec2::new(pos.x, pos.y)),
        AnimatedSprite::new(
            32,
            64,
            15,
            vec![vec![tex[0], tex[1],], vec![tex[0], tex[2], tex[0], tex[3],]],
            Some(16)
        ),
        ColliderGroup {
            nav: Some(Collider::new(-14, 20, 28, 14, CH_NAV, CH_NAV, None)),
            hitbox: None
        },
        Light {
            radius: 100,
            color: Color::RGB(200, 200, 200),
        }
    ));
}

fn spawn_enemy(world: &World, pos: Vec2<f32>) {
    let ctx = world.get_resource::<Ctx>().unwrap();
    let tex = ctx.enemy_textures;

    world.spawn(entity!(
        Enemy {},
        Position(Vec2::new(pos.x, pos.y)),
        AnimatedSprite::new(32, 32, 30, vec![vec![tex[0], tex[1]]], None),
        ColliderGroup {
            nav: Some(Collider::new(-10, 6, 22, 10, CH_NAV, CH_NAV, None)),
            hitbox: Some(Collider::new(
                -16,
                -16,
                32,
                32,
                CH_PROJECTILE,
                CH_PROJECTILE,
                Some(&|world: &World, me: Entity, other: Entity| {
                    if let Some(_) = world.get_component::<Projectile>(other) {
                        let mut despawn_queue = world
                            .get_resource::<Ctx>()
                            .unwrap()
                            .despawn_queue
                            .write()
                            .unwrap();
                        despawn_queue.push(me);
                    }
                }),
            ))
        },
        Light {
            radius: 30,
            color: Color::RGB(200, 200, 200)
        }
    ));
}

fn spawn_bullet(world: &World, pos: Vec2<f32>, velocity_normal: Vec2<f32>) {
    let ctx = world.get_resource::<Ctx>().unwrap();
    let tex = ctx.bullet_textures;

    world.spawn(entity!(
        Projectile {
            velocity: velocity_normal.scaled(ctx.bullet_speed),
            ticks_left: ctx.bullet_lifetime,
        },
        Position(Vec2::new(pos.x, pos.y)),
        AnimatedSprite::new(16, 16, 30, vec![vec![tex[0], tex[1]]], None),
        ColliderGroup {
            nav: Some(Collider::new(
                -6,
                -6,
                12,
                12,
                CH_PROJECTILE,
                CH_PROJECTILE,
                Some(&|world: &World, me: Entity, _: Entity| {
                    let mut despawn_queue = world
                        .get_resource::<Ctx>()
                        .unwrap()
                        .despawn_queue
                        .write()
                        .unwrap();
                    despawn_queue.push(me);
                }),
            )),
            hitbox: None
        },
        Light {
            radius: 20,
            color: Color::RGB(160, 150, 10),
        }
    ));
}

fn update_player(world: &World) {
    let ctx = world.get_resource::<Ctx>().unwrap();
    let mut player_pos = Position(Vec2::zero());

    world.run(
        |player: &mut Player,
         pos: &mut Position,
         colliders: &ColliderGroup,
         sprite: &mut AnimatedSprite| {
            let collider = colliders.nav.as_ref().unwrap();
            if ctx.input.up | ctx.input.down | ctx.input.left | ctx.input.right {
                sprite.switch_state(1);
                sprite.ticks_per_frame = 5;
            } else {
                sprite.switch_state(0);
                sprite.ticks_per_frame = 30;
            }

            if ctx.input.up && !collider.top {
                pos.y -= ctx.player_speed;
            }
            if ctx.input.down && !collider.bottom {
                pos.y += ctx.player_speed;
            }
            if ctx.input.left {
                sprite.flip_horizontal = false;
                if !collider.left {
                    pos.x -= ctx.player_speed;
                }
            }
            if ctx.input.right {
                sprite.flip_horizontal = true;
                if !collider.right {
                    pos.x += ctx.player_speed;
                }
            }

            player_pos = *pos;

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

    world.run(
        |entity: &Entity, interactable: &mut Interactable, pos: &Position| {
            if interactable.ticks_left == 0 {
                if ctx.input.interact && player_pos.distance(pos) < 32.0 {
                    (interactable.on_interact)(world, *entity);
                    interactable.ticks_left = interactable.cooldown
                }
            } else {
                interactable.ticks_left -= 1;
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
        |_: &Enemy,
         pos: &mut Position,
         colliders: &mut ColliderGroup,
         sprite: &mut AnimatedSprite,
         ctx: Res<Ctx>| {
            let collider = colliders.nav.as_ref().unwrap();
            let mut v = Vec2::<f32>::new(player_pos.x - pos.x, player_pos.y - pos.y);

            v.normalize();
            v.scale(ctx.enemy_speed);

            if v.x > 0.0 {
                sprite.flip_horizontal = true;
            } else if v.x < 0.0 {
                sprite.flip_horizontal = false;
            }

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

fn update_spawners(world: &World) {
    world.run(|spawner: &mut Spawner, pos: &Position| {
        if spawner.is_active {
            if spawner.ticks_left == 0 {
                spawn_enemy(world, pos.0);
                spawner.ticks_left = spawner.cooldown;
            } else {
                spawner.ticks_left -= 1;
            }
        }
    });
}

fn fix_colliders(world: &World) {
    world.run(|colliders: &mut ColliderGroup, pos: &Position| {
        if let Some(collider) = colliders.nav.as_mut() {
            collider.set_pos(
                pos.x.round() as i32 + collider.x_offset,
                pos.y.round() as i32 + collider.y_offset,
            );
        }
        if let Some(collider) = colliders.hitbox.as_mut() {
            collider.set_pos(
                pos.x.round() as i32 + collider.x_offset,
                pos.y.round() as i32 + collider.y_offset,
            );
        }
    });
}

fn detect_collisions(world: &World) {
    fn test(world: &World, e1: &Entity, c1: &mut Collider, e2: &Entity, c2: &mut Collider) {
        if *e1 != *e2 && c1.collides_with & c2.channels != 0 {
            if c1.bounds.has_intersection(c2.bounds) {
                c1.is_colliding = true;

                if let Some(on_collide) = c1.on_collide {
                    on_collide(world, *e1, *e2);
                }

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
    }
    
    fn test_all(world: &World, e1: &Entity, c1: &mut Collider) {
        c1.is_colliding = false;
        c1.left = false;
        c1.right = false;
        c1.top = false;
        c1.bottom = false;

        world.run(|e2: &Entity, cg: &mut ColliderGroup| {
            if let Some(c2) = cg.nav.as_mut() {
                test(world, e1, c1, e2, c2);
            }

            if let Some(c2) = cg.hitbox.as_mut() {
                test(world, e1, c1, e2, c2);
            }
        });
    }

    world.run(|e1: &Entity, cg: &mut ColliderGroup| {
        if let Some(c1) = cg.nav.as_mut() {
            test_all(world, e1, c1);
        }

        if let Some(c1) = cg.hitbox.as_mut() {
            test_all(world, e1, c1);
        }
    });
}

fn draw_sprites(world: &World) {
    // TODO impl draw() for direct drawing and additionally get rid of depth in this signature
    fn push(
        depth_buffer: &mut DepthBuffer,
        sprite: &mut AnimatedSprite,
        pos: &Position,
        depth: Option<i32>,
    ) {
        if sprite.ticks >= sprite.ticks_per_frame {
            sprite.texture_index = if sprite.texture_index
                == sprite.textures[sprite.state as usize].len() as u32 - 1
            {
                0
            } else {
                sprite.texture_index + 1
            };
            sprite.ticks = 0;
        }

        depth_buffer.push(DrawCmd {
            texture_id: sprite.textures[sprite.state as usize][sprite.texture_index as usize],
            pos: Vec3::<i32> {
                x: pos.x.round() as i32 - (sprite.width / 2) as i32,
                y: pos.y.round() as i32 - (sprite.height / 2) as i32,
                z: if depth.is_some() {
                    depth.unwrap()
                } else {
                    pos.y.round() as i32
                        + if sprite.z_offset.is_some() {
                            sprite.z_offset.unwrap() as i32
                        } else {
                            0
                        }
                },
            },
            w: sprite.width,
            h: sprite.height,
            flip_horizontal: sprite.flip_horizontal,
        });
        sprite.ticks += 1;
    }

    // TODO this should draw directly instead of drawing to the depth buffer
    world.run(
        |pos: &mut Position,
         sprite: &mut AnimatedSprite,
         mut depth_buffer: ResMut<DepthBuffer>,
         _: With<Floor>| {
            push(&mut depth_buffer, sprite, pos, Some(-100));
        },
    );

    // TODO this should draw directly instead of drawing to the depth buffer
    world.run(
        |pos: &mut Position,
         sprite: &mut AnimatedSprite,
         mut depth_buffer: ResMut<DepthBuffer>,
         _: With<Prop>| {
            push(&mut depth_buffer, sprite, pos, Some(-99));
        },
    );

    world.run(
        |pos: &mut Position,
         sprite: &mut AnimatedSprite,
         mut depth_buffer: ResMut<DepthBuffer>,
         _: Without<Floor>,
         _: Without<Prop>| {
            push(&mut depth_buffer, sprite, pos, None);
        },
    );

    let ctx = world.get_resource_mut::<Ctx>().unwrap();
    let depth_buffer = world.get_resource_mut::<DepthBuffer>().unwrap();
    depth_buffer.draw_to_canvas(&mut ctx.canvas, &ctx.textures);
}

// ----------------------------------------------------------------------------
// DEBUG
// ----------------------------------------------------------------------------

fn debug_draw_colliders(world: &World) {
    world.run(|cg: &ColliderGroup, mut ctx: ResMut<Ctx>| {
        if ctx.debug_draw_nav_colliders {
            if let Some(collider) = cg.nav.as_ref() {
                if collider.is_colliding {
                    ctx.canvas.set_draw_color(Color::RGB(255, 0, 0));
                    let _ = ctx.canvas.draw_rect(collider.bounds);
                } else {
                    ctx.canvas.set_draw_color(Color::RGB(0, 255, 0));
                    let _ = ctx.canvas.draw_rect(collider.bounds);
                }
            }
        }

        if ctx.debug_draw_hitboxes {
            if let Some(collider) = cg.hitbox.as_ref() {
                if collider.is_colliding {
                    ctx.canvas.set_draw_color(Color::RGB(255, 0, 0));
                    let _ = ctx.canvas.draw_rect(collider.bounds);
                } else {
                    ctx.canvas.set_draw_color(Color::RGB(255, 255, 0));
                    let _ = ctx.canvas.draw_rect(collider.bounds);
                }
            }
        }
    });
}
