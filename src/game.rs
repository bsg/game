// TODO serializable entity definitions
// TODO serializable room definitions
// TODO how do we wanna scale sprites around entity centerpoint?
// FIXME fix shadows
// FIXME colliders are still fucky

use ecs::{Entity, Res, ResMut, With, Without, World};
use rand::{thread_rng, Rng};
use sdl2::{pixels::Color, render::Canvas, video::Window};

use crate::{
    components::{
        AnimatedSprite, Chemlight, Collider, ColliderGroup, Enemy, Floor, Interactable, Light,
        ParticleEmitter, PerfectlyGenericItem, Player, Pos, Projectile, Prop, ProximityIndicator,
        Static, TestItem, Torch, Wall, CH_HITBOX, CH_NAV, CH_NONE,
    },
    math::{Vec2, Vec3},
    Ctx, DepthBuffer, DrawCmd,
};

const TILE_SIZE: f32 = 32.0;

#[inline(always)]
fn tile_to_pos(x: i32, y: i32) -> Pos {
    Pos::new(
        x as f32 * TILE_SIZE + (TILE_SIZE / 2.),
        y as f32 * TILE_SIZE + (TILE_SIZE / 2.),
    )
}

pub fn init(world: &World) {
    for x in 0..64 {
        for y in 0..64 {
            spawn_floor(world, tile_to_pos(x, y));
        }
    }

    for x in 0..64 {
        spawn_wall(world, tile_to_pos(x, 1));

        if x != 7 && x != 8 {
            spawn_wall(world, tile_to_pos(x, 8));
        }
    }

    for x in 0..64 {
        if x != 12 {
            spawn_wall(world, tile_to_pos(x, 18));
        }
    }

    spawn_wall(world, tile_to_pos(16, 15));
    spawn_wall(world, tile_to_pos(16, 16));
    spawn_wall(world, tile_to_pos(16, 17));

    spawn_torch(world, Pos::new(350.0, 570.0));
    spawn_torch(world, Pos::new(600.0, 200.0));

    world.resource_mut::<Ctx>().unwrap().particle_emitter_entity =
        Some(spawn_particle_emitter(world, Pos::new(540.0, 640.0)));

    spawn_lever(
        world,
        Pos::new(200.0, 200.0),
        move |world: &World, me: Entity| {
            let sprite = world.component_mut::<AnimatedSprite>(me).unwrap();
            sprite.flip_horizontal = !sprite.flip_horizontal;
            let particle_emitter_entity = world
                .resource_mut::<Ctx>()
                .unwrap()
                .particle_emitter_entity
                .unwrap();
            let particle_emitter = world
                .component_mut::<ParticleEmitter>(particle_emitter_entity)
                .unwrap();
            particle_emitter.is_active = !particle_emitter.is_active;
            world
                .component_mut::<Light>(particle_emitter_entity)
                .unwrap()
                .radius = if particle_emitter.is_active { 60 } else { 0 };
        },
    );

    spawn_player(world, Vec2::new(400.0, 400.0));
}

pub fn update(world: &World) {
    update_spawners(world);
    update_player(world);
    update_enemies(world);
    update_projectiles(world);
    fix_colliders(world);
    detect_collisions(world);

    let mut despawn_queue = world
        .resource_mut::<Ctx>()
        .unwrap()
        .despawn_queue
        .write()
        .unwrap();

    for e in despawn_queue.iter() {
        world.despawn(*e);
    }

    despawn_queue.clear();
}

fn spawn_player(world: &World, pos: Vec2<f32>) {
    let ctx = world.resource_mut::<Ctx>().unwrap();
    world.spawn(&[
        &Player {
            fire_cooldown: ctx.player_fire_cooldown,
            can_fire_in: 0,
        },
        &Pos::new(pos.x, pos.y),
        &AnimatedSprite::new(
            (-16, -48, 32, 64),
            15,
            ctx.animations.get("player_idle").unwrap(),
            None,
        ),
        &ColliderGroup {
            nav: Some(Collider::new((-13, 0, 26, 16), CH_NAV, CH_NAV, None)),
            hitbox: None,
        },
        &Light {
            radius: 0,
            color: Color::RGB(255, 255, 255),
            intensity: 0.,
        },
    ]);

    assert!(ctx.player_inventory.insert(TestItem {}, world));
    assert!(ctx.player_inventory.insert(PerfectlyGenericItem {}, world));
    assert!(ctx.player_inventory.insert(Torch::new(), world));
    assert!(ctx.player_inventory.insert(Chemlight::new(), world));
}

fn spawn_lever(world: &World, pos: Pos, on_interact: fn(&World, Entity)) {
    let ctx = world.resource::<Ctx>().unwrap();
    world.spawn(&[
        &pos,
        &AnimatedSprite::new(
            (-16, -16, 32, 32),
            0,
            ctx.animations.get("lever").unwrap(),
            None,
        ),
        &Interactable { on_interact },
        &ProximityIndicator {
            range: 64.,
            sprite: AnimatedSprite::new(
                (-16, -16, 16, 16),
                15,
                ctx.animations.get("bang").unwrap(),
                Some(255),
            ),
        },
    ]);
}

fn spawn_particle_emitter(world: &World, pos: Pos) -> Entity {
    let ctx = world.resource::<Ctx>().unwrap();
    world.spawn(&[
        &Prop {},
        &pos,
        &AnimatedSprite::new(
            (-16, -16, 32, 32),
            0,
            ctx.animations.get("particle_emitter").unwrap(),
            None,
        ),
        &ParticleEmitter {
            is_active: false,
            particle_cooldown: 1,
            particle_ticks_left: 0,
        },
        &Light {
            radius: 0,
            color: Color::RGB(150, 150, 150),
            intensity: 1.,
        },
    ])
}

fn spawn_floor(world: &World, pos: Pos) -> Entity {
    let ctx = world.resource::<Ctx>().unwrap();
    world.spawn(&[
        &Floor {},
        &pos,
        &AnimatedSprite::new(
            (-16, -16, TILE_SIZE as u32, TILE_SIZE as u32),
            0,
            ctx.animations.get("floor").unwrap(),
            None,
        ),
    ])
}

fn spawn_wall(world: &World, pos: Pos) -> Entity {
    let ctx = world.resource::<Ctx>().unwrap();
    world.spawn(&[
        &Static {},
        &Wall {},
        &pos,
        &AnimatedSprite::new(
            (-16, -48, TILE_SIZE as u32, (TILE_SIZE * 2.) as u32),
            0,
            ctx.animations.get("wall").unwrap(),
            None,
        ),
        &ColliderGroup {
            nav: Some(Collider::new(
                (-16, -14, 32, 30),
                CH_NAV,
                CH_NAV | CH_HITBOX,
                None,
            )),
            hitbox: None,
        },
    ])
}

fn spawn_torch(world: &World, pos: Pos) {
    let ctx = world.resource::<Ctx>().unwrap();
    world.spawn(&[
        &pos,
        &AnimatedSprite::new(
            (-16, -16, 32, 32),
            5,
            ctx.animations.get("torch").unwrap(),
            None,
        ),
        &Light {
            radius: 120,
            color: Color::RGB(255, 255, 0),
            intensity: 1.,
        },
    ]);
}

fn spawn_enemy(world: &World, pos: Pos) {
    let ctx = world.resource::<Ctx>().unwrap();

    world.spawn(&[
        &Enemy {},
        &Pos::new(pos.x, pos.y),
        &AnimatedSprite::new(
            (-32, -40, 64, 64),
            30,
            ctx.animations.get("enemy_walk").unwrap(),
            None,
        ),
        &ColliderGroup {
            nav: Some(Collider::new((-10, 6, 22, 10), CH_NAV, CH_NAV, None)),
            hitbox: Some(Collider::new(
                (-16, -16, 32, 32),
                CH_HITBOX,
                CH_HITBOX,
                Some(|world: &World, me: Entity, other: Entity| {
                    if world.component::<Projectile>(other).is_some() {
                        let mut despawn_queue = world
                            .resource::<Ctx>()
                            .unwrap()
                            .despawn_queue
                            .write()
                            .unwrap();
                        despawn_queue.push(me);
                    }
                }),
            )),
        },
        &Light {
            radius: 30,
            color: Color::RGB(200, 200, 200),
            intensity: 1.,
        },
    ]);
}

fn spawn_bullet(world: &World, pos: Vec2<f32>, velocity_normal: Vec2<f32>) {
    let ctx = world.resource::<Ctx>().unwrap();

    world.spawn(&[
        &Projectile {
            velocity: velocity_normal.scaled(ctx.bullet_speed),
            ticks_left: ctx.bullet_lifetime,
        },
        &Pos::new(pos.x, pos.y),
        &AnimatedSprite::new(
            (-8, -8, 16, 16),
            10,
            ctx.animations.get("bullet").unwrap(),
            None,
        ),
        &ColliderGroup {
            nav: Some(Collider::new(
                (-6, -6, 12, 12),
                CH_NONE,
                CH_HITBOX | CH_NAV,
                Some(|world: &World, me: Entity, _: Entity| {
                    let mut despawn_queue = world
                        .resource::<Ctx>()
                        .unwrap()
                        .despawn_queue
                        .write()
                        .unwrap();
                    despawn_queue.push(me);
                }),
            )),
            hitbox: None,
        },
        &Light {
            radius: 20,
            color: Color::RGB(160, 150, 10),
            intensity: 1.,
        },
    ]);
}

// ███████╗██╗   ██╗███████╗████████╗███████╗███╗   ███╗███████╗
// ██╔════╝╚██╗ ██╔╝██╔════╝╚══██╔══╝██╔════╝████╗ ████║██╔════╝
// ███████╗ ╚████╔╝ ███████╗   ██║   █████╗  ██╔████╔██║███████╗
// ╚════██║  ╚██╔╝  ╚════██║   ██║   ██╔══╝  ██║╚██╔╝██║╚════██║
// ███████║   ██║   ███████║   ██║   ███████╗██║ ╚═╝ ██║███████║
// ╚══════╝   ╚═╝   ╚══════╝   ╚═╝   ╚══════╝╚═╝     ╚═╝╚══════╝

fn update_player(world: &World) {
    world.run(
        |player: &mut Player,
         pos: &mut Pos,
         colliders: &ColliderGroup,
         sprite: &mut AnimatedSprite,
         mut ctx: ResMut<Ctx>| {
            if ctx.input.pressed.up
                | ctx.input.pressed.down
                | ctx.input.pressed.left
                | ctx.input.pressed.right
            {
                sprite.switch_anim(ctx.animations.get("player_walk").unwrap(), 5);
            } else {
                sprite.switch_anim(ctx.animations.get("player_idle").unwrap(), 30);
            }

            let speed = if ctx.input.pressed.shift {
                8.
            } else {
                ctx.player_speed
            };

            let collider = colliders.nav.as_ref().unwrap();
            if ctx.input.pressed.up && !collider.top {
                pos.y -= speed;
            }
            if ctx.input.pressed.down && !collider.bottom {
                pos.y += speed;
            }
            if ctx.input.pressed.left {
                sprite.flip_horizontal = false;
                if !collider.left {
                    pos.x -= speed;
                }
            }
            if ctx.input.pressed.right {
                sprite.flip_horizontal = true;
                if !collider.right {
                    pos.x += speed;
                }
            }

            ctx.player_pos = *pos;

            if player.can_fire_in > 0 {
                player.can_fire_in -= 1;
            }

            if player.can_fire_in == 0 {
                let mut trajectory = Vec2::zero();

                if ctx.input.pressed.fire_right {
                    trajectory.x += 1.0;
                }
                if ctx.input.pressed.fire_left {
                    trajectory.x -= 1.0;
                }
                if ctx.input.pressed.fire_up {
                    trajectory.y -= 1.0;
                }
                if ctx.input.pressed.fire_down {
                    trajectory.y += 1.0;
                }

                if trajectory.magnitude() > 0.0 {
                    spawn_bullet(
                        world,
                        Vec2::new(
                            pos.x + trajectory.normalized().x * 30.,
                            pos.y + trajectory.normalized().y * 30.,
                        ),
                        trajectory,
                    );
                    player.can_fire_in = player.fire_cooldown;
                }
            }

            if ctx.input.just_pressed.q {
                ctx.player_inventory.set_active_offset(-1, world)
            }

            if ctx.input.just_pressed.e {
                ctx.player_inventory.set_active_offset(1, world)
            }

            if ctx.input.just_pressed.use_item {
                ctx.player_inventory.do_use(world)
            }

            ctx.player_inventory.tick(world);
        },
    );

    world.run(
        |entity: &Entity, interactable: &mut Interactable, pos: &Pos, ctx: Res<Ctx>| {
            if ctx.input.just_pressed.interact && ctx.player_pos.distance(pos) < 32.0 {
                (interactable.on_interact)(world, *entity);
            }
        },
    );
}

fn update_enemies(world: &World) {
    let mut player_pos = Pos::zero();

    world.run(|_: &Player, pos: &Pos| {
        player_pos = *pos;
    });

    world.run(
        |_: &Enemy,
         pos: &mut Pos,
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
        |entity: &Entity, projectile: &mut Projectile, pos: &mut Pos| {
            if projectile.ticks_left == 0 {
                world
                    .resource::<Ctx>()
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
    world.run(|spawner: &mut ParticleEmitter, pos: &Pos| {
        if spawner.is_active {
            if spawner.particle_ticks_left == 0 {
                for _ in 0..2 {
                    let mut v = Vec2::new(
                        thread_rng().gen_range(-1.0..1.0),
                        thread_rng().gen_range(-1.0..1.0),
                    );
                    v.scale(2.0);

                    world.spawn(&[
                        pos,
                        &Projectile {
                            velocity: v,
                            ticks_left: 60,
                        },
                        &Light {
                            radius: 2,
                            color: Color::RGB(255, 255, 255),
                            intensity: 1.,
                        },
                        &ColliderGroup {
                            nav: Some(Collider::new(
                                (-2, -2, 4, 4),
                                CH_NONE,
                                CH_NAV | CH_HITBOX,
                                Some(|world: &World, me: Entity, _: Entity| {
                                    world.component_mut::<Projectile>(me).unwrap().velocity =
                                        Vec2::zero();
                                }),
                            )),
                            hitbox: None,
                        },
                    ]);
                }

                spawner.particle_ticks_left = spawner.particle_cooldown;
            } else {
                spawner.particle_ticks_left -= 1;
            }
        }
    });
}

fn fix_colliders(world: &World) {
    world.run(|colliders: &mut ColliderGroup, pos: &Pos| {
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
    fn test(
        world: &World,
        e1: &Entity,
        c1: &mut Collider,
        pos1: &mut Pos,
        e2: &Entity,
        c2: &Collider,
        should_move: bool,
    ) {
        if *e1 != *e2
            && c1.collides_with & c2.channels != 0
            && c1.bounds.has_intersection(c2.bounds)
        {
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
                if should_move && !world.has_component::<Static>(*e1) {
                    pos1.y -= c1.bounds.bottom() as f32 - c2.bounds.top() as f32 - 1.;
                }
            } else if d_bottom < d_top && d_bottom < d_left && d_bottom < d_right {
                c1.top = true;
                if should_move && !world.has_component::<Static>(*e1) {
                    pos1.y += c2.bounds.bottom() as f32 - c1.bounds.top() as f32 - 1.;
                }
            } else if d_left < d_right && d_left < d_top && d_left < d_bottom {
                c1.right = true;
                if should_move && !world.has_component::<Static>(*e1) {
                    pos1.x -= c1.bounds.right() as f32 - c2.bounds.left() as f32 - 1.;
                }
            } else if d_right < d_left && d_right < d_top && d_right < d_bottom {
                c1.left = true;
                if should_move && !world.has_component::<Static>(*e1) {
                    pos1.x += c2.bounds.right() as f32 - c1.bounds.left() as f32 - 1.;
                }
            }
        }
    }

    fn test_all(world: &World, e1: &Entity, c1: &mut Collider, pos1: &mut Pos) {
        c1.is_colliding = false;
        c1.left = false;
        c1.right = false;
        c1.top = false;
        c1.bottom = false;

        world.run(|e2: &Entity, cg: &mut ColliderGroup| {
            if let Some(c2) = cg.nav.as_ref() {
                test(world, e1, c1, pos1, e2, c2, true);
            }

            if let Some(c2) = cg.hitbox.as_ref() {
                test(world, e1, c1, pos1, e2, c2, false);
            }
        });
    }

    world.run(|e1: &Entity, pos1: &mut Pos, cg: &mut ColliderGroup| {
        if let Some(c1) = cg.nav.as_mut() {
            test_all(world, e1, c1, pos1);
        }

        if let Some(c1) = cg.hitbox.as_mut() {
            test_all(world, e1, c1, pos1);
        }
    });
}

// ██████╗ ███████╗███╗   ██╗██████╗ ███████╗██████╗
// ██╔══██╗██╔════╝████╗  ██║██╔══██╗██╔════╝██╔══██╗
// ██████╔╝█████╗  ██╔██╗ ██║██║  ██║█████╗  ██████╔╝
// ██╔══██╗██╔══╝  ██║╚██╗██║██║  ██║██╔══╝  ██╔══██╗
// ██║  ██║███████╗██║ ╚████║██████╔╝███████╗██║  ██║
// ╚═╝  ╚═╝╚══════╝╚═╝  ╚═══╝╚═════╝ ╚══════╝╚═╝  ╚═╝

pub fn render(world: &World) {
    let ctx = world.resource_mut::<Ctx>().unwrap();
    let camera_pos = ctx.camera_pos();

    #[inline(always)]
    fn update_anim(sprite: &mut AnimatedSprite, num_frames: usize) {
        sprite.ticks += 1;
        if sprite.ticks >= sprite.ticks_per_frame {
            sprite.frame = if sprite.frame as usize == num_frames {
                0
            } else {
                sprite.frame + 1
            };
            sprite.ticks = 0;
        }
    }

    #[inline(always)]
    fn draw(ctx: &mut Ctx, anim: &mut AnimatedSprite, pos: &Pos, camera_pos: (i32, i32)) {
        let frames = ctx.animations.get_frames(anim.anim());
        let sprite = frames[anim.frame as usize];

        ctx.spritesheet.draw_to_canvas(
            &mut ctx.canvas,
            sprite,
            (
                pos.x as i32 + anim.x_offset as i32 + camera_pos.0,
                pos.y as i32 + anim.y_offset as i32 + camera_pos.1,
            ),
            0.,
            anim.flip_horizontal,
            false,
        );

        update_anim(anim, frames.len() - 1);
    }

    #[inline(always)]
    fn push(
        ctx: &Ctx,
        depth_buffer: &mut DepthBuffer,
        anim: &mut AnimatedSprite,
        pos: &Pos,
        camera_pos: (i32, i32),
    ) {
        let frames = ctx.animations.get_frames(anim.anim());
        let sprite = frames[anim.frame as usize];
        depth_buffer.push(DrawCmd {
            sprite,
            pos: Vec3::<i32> {
                x: pos.x.round() as i32 + anim.x_offset as i32 + camera_pos.0,
                y: pos.y.round() as i32 + anim.y_offset as i32 + camera_pos.1,
                z: pos.y.round() as i32 + anim.z_offset.map_or(0, |o| o) as i32,
            },
            flip_horizontal: anim.flip_horizontal,
        });

        update_anim(anim, frames.len() - 1);
    }

    world.run(
        |pos: &mut Pos, sprite: &mut AnimatedSprite, mut ctx: ResMut<Ctx>, _: With<Floor>| {
            draw(&mut ctx, sprite, pos, camera_pos);
        },
    );

    world.run(
        |pos: &mut Pos, sprite: &mut AnimatedSprite, mut ctx: ResMut<Ctx>, _: With<Prop>| {
            draw(&mut ctx, sprite, pos, camera_pos);
        },
    );

    world.run(
        |pos: &mut Pos,
         sprite: &mut AnimatedSprite,
         mut depth_buffer: ResMut<DepthBuffer>,
         ctx: Res<Ctx>,
         _: Without<Floor>,
         _: Without<Prop>| {
            push(&ctx, &mut depth_buffer, sprite, pos, camera_pos);
        },
    );

    let ctx = world.resource_mut::<Ctx>().unwrap();
    let depth_buffer = world.resource_mut::<DepthBuffer>().unwrap();
    depth_buffer.draw_to_canvas(&mut ctx.canvas, &ctx.spritesheet);

    if ctx.debug_draw_centerpoints {
        world.run(|pos: &Pos, _: Without<Floor>| {
            let x = pos.x + ctx.camera_pos().0 as f32;
            let y = pos.y + ctx.camera_pos().1 as f32;

            ctx.canvas.set_draw_color(Color::RGBA(0, 255, 0, 255));
            ctx.canvas
                .draw_line(((x - 2.) as i32, y as i32), ((x + 2.) as i32, y as i32))
                .unwrap();
            ctx.canvas
                .draw_line((x as i32, (y - 2.) as i32), (x as i32, (y + 2.) as i32))
                .unwrap();
        });
    }

    ctx.canvas
        .with_texture_canvas(&mut ctx.ui_tex, |canvas| {
            canvas.set_draw_color(Color::RGB(0, 0, 0));
            canvas.clear();

            if let Some(item) = ctx.player_inventory.get_left() {
                ctx.spritesheet.draw_to_canvas(
                    canvas,
                    item.sprite(),
                    (
                        canvas.viewport().width() as i32 / 2 - 58,
                        canvas.viewport().height() as i32 - 40,
                    ),
                    0.,
                    false,
                    false,
                )
            }

            if let Some(item) = ctx.player_inventory.active_item() {
                ctx.spritesheet.draw_to_canvas(
                    canvas,
                    item.sprite(),
                    (
                        canvas.viewport().width() as i32 / 2 - 16,
                        canvas.viewport().height() as i32 - 40,
                    ),
                    0.,
                    false,
                    false,
                )
            }

            if let Some(item) = ctx.player_inventory.get_right() {
                ctx.spritesheet.draw_to_canvas(
                    canvas,
                    item.sprite(),
                    (
                        canvas.viewport().width() as i32 / 2 + 28,
                        canvas.viewport().height() as i32 - 40,
                    ),
                    0.,
                    false,
                    false,
                )
            }

            canvas.set_draw_color(Color::RGB(255, 255, 255));
            ctx.spritesheet.draw_to_canvas(
                canvas,
                ctx.ui_active_item_bg,
                (
                    canvas.viewport().width() as i32 / 2 - 16,
                    canvas.viewport().height() as i32 - 40,
                ),
                0.,
                false,
                false,
            );
        })
        .unwrap();

    world.run(
        |indicator: &mut ProximityIndicator, pos: &Pos, mut ctx: ResMut<Ctx>| {
            if ctx.player_pos.distance(pos) < indicator.range {
                let mut draw_pos = *pos;
                draw_pos.y -= 32.;
                // FIXME don't recreate the AnimatedSprite
                // FIXME needs to be drawn to the ui layer
                draw(&mut ctx, &mut indicator.sprite, &draw_pos, camera_pos);
            }
        },
    );

    // DEBUG
    if ctx.debug_draw_nav_colliders || ctx.debug_draw_hitboxes {
        world.run(|cg: &ColliderGroup| {
            if ctx.debug_draw_nav_colliders {
                if let Some(collider) = cg.nav.as_ref() {
                    let mut rect = collider.bounds;
                    rect.x += ctx.camera_pos().0;
                    rect.y += ctx.camera_pos().1;

                    if collider.is_colliding {
                        ctx.canvas.set_draw_color(Color::RGB(255, 0, 0));
                        ctx.canvas.draw_rect(rect).unwrap();
                    } else {
                        ctx.canvas.set_draw_color(Color::RGB(0, 255, 0));
                        ctx.canvas.draw_rect(rect).unwrap();
                    }
                }
            }

            if ctx.debug_draw_hitboxes {
                if let Some(collider) = cg.hitbox.as_ref() {
                    let mut rect = collider.bounds;
                    rect.x += ctx.camera_pos().0;
                    rect.y += ctx.camera_pos().1;

                    if collider.is_colliding {
                        ctx.canvas.set_draw_color(Color::RGB(255, 0, 0));
                        ctx.canvas.draw_rect(rect).unwrap();
                    } else {
                        ctx.canvas.set_draw_color(Color::RGB(255, 255, 0));
                        ctx.canvas.draw_rect(rect).unwrap();
                    }
                }
            }
        });
    }
}
