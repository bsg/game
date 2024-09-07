extern crate sdl2;

mod components;
mod game;
mod math;

use std::{
    collections::{BinaryHeap, HashMap},
    mem::MaybeUninit,
    ops::Deref,
    sync::RwLock,
    time::{Duration, Instant},
};

use components::{ColliderGroup, Inventory, Wall};
use ecs::{Entity, Resource, With, World};
use math::Vec3;
use sdl2::{
    event::Event,
    gfx::primitives::DrawRenderer,
    image::{InitFlag, LoadTexture},
    keyboard::{Keycode, Scancode},
    pixels::Color,
    rect::Rect,
    render::{BlendMode, Canvas, Texture, TextureCreator},
    video::{Window, WindowContext},
};

use crate::components::{Light, Pos};

#[derive(Clone, Copy)]
pub struct TextureId(usize);

impl Deref for TextureId {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AnimationId(usize);

struct AnimationRepository {
    animations: Vec<Vec<Sprite>>,
    lookup: HashMap<&'static str, AnimationId>,
}

impl AnimationRepository {
    pub fn new() -> Self {
        AnimationRepository {
            animations: vec![vec![]],
            lookup: HashMap::new(),
        }
    }

    pub fn push(&mut self, name: &'static str, frames: &[Sprite]) {
        let id = AnimationId(self.animations.len());
        self.animations.push(Vec::from(frames));
        self.lookup.insert(name, id);
    }

    pub fn get_frames(&self, anim_id: AnimationId) -> &[Sprite] {
        // TODO unwrap_unchecked is probably safe unless AnimationId's are constructed elsewhere
        self.animations.get(anim_id.0).unwrap()
    }

    pub fn get(&self, name: &'static str) -> Option<AnimationId> {
        self.lookup.get(name).copied()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Sprite(u16, u16, u16, u16);

impl From<(u16, u16, u16, u16)> for Sprite {
    fn from(value: (u16, u16, u16, u16)) -> Self {
        Sprite(value.0, value.1, value.2, value.3)
    }
}

struct Spritesheet {
    texture: MaybeUninit<Texture>,
    specular: MaybeUninit<Texture>,
    tile_size: u16,
}

impl Spritesheet {
    pub fn new_from_file(
        texture_creator: &TextureCreator<WindowContext>,
        spritesheet_path: &'static str,
        specular_path: &'static str,
        tile_size: u16,
    ) -> Self {
        if let Ok(spritesheet) = texture_creator.load_texture(spritesheet_path) {
            if let Ok(specular) = texture_creator.load_texture(specular_path) {
                Spritesheet {
                    texture: MaybeUninit::new(spritesheet),
                    specular: MaybeUninit::new(specular),
                    tile_size,
                }
            } else {
                panic!("Failed to load texture {}", specular_path)
            }
        } else {
            panic!("Failed to load texture {}", spritesheet_path)
        }
    }

    pub fn draw_to_canvas(
        &self,
        canvas: &mut Canvas<Window>,
        src: Sprite,
        dst: (i32, i32),
        angle: f64,
        flip_horizontal: bool,
        flip_vertical: bool,
    ) {
        canvas
            .copy_ex(
                unsafe { self.texture.assume_init_ref() },
                Some(Rect::new(
                    (src.0 * self.tile_size) as i32,
                    (src.1 * self.tile_size) as i32,
                    (self.tile_size * src.2) as u32,
                    (self.tile_size * src.3) as u32,
                )),
                Some(Rect::new(
                    dst.0,
                    dst.1,
                    (self.tile_size * src.2 * 2) as u32,
                    (self.tile_size * src.3 * 2) as u32,
                )),
                angle,
                None,
                flip_horizontal,
                flip_vertical,
            )
            .unwrap();
    }
}

impl Drop for Spritesheet {
    fn drop(&mut self) {
        unsafe { self.texture.assume_init_read().destroy() }
    }
}

// TODO dunno what to call this
struct DrawCmd {
    sprite: Sprite,
    pos: Vec3<i32>,
    flip_horizontal: bool,
}

impl PartialEq for DrawCmd {
    fn eq(&self, other: &Self) -> bool {
        self.pos.z == other.pos.z
    }
}

impl Eq for DrawCmd {}

impl PartialOrd for DrawCmd {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering::*;
        match self.pos.z.cmp(&other.pos.z) {
            Less => Some(Greater),
            Equal => Some(Equal),
            Greater => Some(Less),
        }
    }
}

impl Ord for DrawCmd {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        match self.pos.z.cmp(&other.pos.z) {
            Less => Greater,
            Equal => Equal,
            Greater => Less,
        }
    }
}

#[derive(Resource)]
struct DepthBuffer {
    buffer: BinaryHeap<DrawCmd>,
}

impl DepthBuffer {
    pub fn new() -> Self {
        DepthBuffer {
            buffer: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, texture: DrawCmd) {
        self.buffer.push(texture);
    }

    pub fn draw_to_canvas(&mut self, canvas: &mut Canvas<Window>, spritesheet: &Spritesheet) {
        while let Some(draw_cmd) = self.buffer.pop() {
            spritesheet.draw_to_canvas(
                canvas,
                draw_cmd.sprite,
                (draw_cmd.pos.x, draw_cmd.pos.y),
                0.,
                draw_cmd.flip_horizontal,
                false,
            )
        }
    }
}

pub struct InputState {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub shift: bool,
    pub fire_up: bool,
    pub fire_down: bool,
    pub fire_left: bool,
    pub fire_right: bool,
    pub interact: bool,
    pub q: bool,
    pub e: bool,
    pub use_item: bool,
}

pub struct Input {
    pressed: InputState,
    just_pressed: InputState,
}

pub struct Lightmap {
    lights: MaybeUninit<Texture>,
    per_light_tex: MaybeUninit<Texture>,
    shadow_mask: MaybeUninit<Texture>,
    specular_map: MaybeUninit<Texture>,
}

impl Lightmap {
    pub fn new(canvas: &Canvas<Window>, w: u32, h: u32) -> Lightmap {
        let mut lights = canvas
            .texture_creator()
            .create_texture_target(canvas.default_pixel_format(), w, h)
            .unwrap();
        lights.set_blend_mode(sdl2::render::BlendMode::Mul);

        let mut per_light_tex = canvas
            .texture_creator()
            .create_texture_target(canvas.default_pixel_format(), w, h)
            .unwrap();
        per_light_tex.set_blend_mode(sdl2::render::BlendMode::Mul);

        let mut shadow_mask = canvas
            .texture_creator()
            .create_texture_target(canvas.default_pixel_format(), w, h)
            .unwrap();
        shadow_mask.set_blend_mode(sdl2::render::BlendMode::Mul);

        let mut specular_map = canvas
            .texture_creator()
            .create_texture_target(canvas.default_pixel_format(), w, h)
            .unwrap();
        specular_map.set_blend_mode(sdl2::render::BlendMode::Mul);

        Lightmap {
            lights: MaybeUninit::new(lights),
            per_light_tex: MaybeUninit::new(per_light_tex),
            shadow_mask: MaybeUninit::new(shadow_mask),
            specular_map: MaybeUninit::new(specular_map),
        }
    }

    pub fn lights(&self) -> Texture {
        unsafe { self.lights.assume_init_read() }
    }

    pub fn per_light_tex(&self) -> Texture {
        unsafe { self.per_light_tex.assume_init_read() }
    }

    pub fn mask(&self) -> Texture {
        unsafe { self.shadow_mask.assume_init_read() }
    }

    pub fn specular_map(&self) -> Texture {
        unsafe { self.specular_map.assume_init_read() }
    }
}

impl Drop for Lightmap {
    fn drop(&mut self) {
        unsafe { self.lights.assume_init_read().destroy() }
        unsafe { self.shadow_mask.assume_init_read().destroy() }
        unsafe { self.specular_map.assume_init_read().destroy() }
    }
}

#[derive(Resource)]
pub struct Ctx {
    canvas: Canvas<Window>,
    spritesheet: Spritesheet,
    animations: AnimationRepository,
    light_tex: Texture,
    ui_tex: Texture,
    ui_active_item_bg: Sprite,
    lightmap: Lightmap,
    despawn_queue: RwLock<Vec<Entity>>,
    input: Input,
    player_speed: f32,
    enemy_speed: f32,
    bullet_speed: f32,
    bullet_lifetime: usize,
    player_fire_cooldown: usize,
    debug_draw_nav_colliders: bool,
    debug_draw_hitboxes: bool,
    debug_draw_centerpoints: bool,
    shadows_enabled: bool,
    player_pos: Pos,
    room_size: (u16, u16),
    player_inventory: Inventory,
    particle_emitter_entity: Option<Entity>,
}

impl Ctx {
    pub fn camera_pos(&self) -> (i32, i32) {
        let window_w = self.canvas.window().size().0 as i32;
        let window_h = self.canvas.window().size().1 as i32;

        (
            ((self.player_pos.x as i32) - window_w / 2)
                .clamp(0, self.room_size.0 as i32 - window_w / 2),
            ((self.player_pos.y as i32) - window_h / 2)
                .clamp(0, self.room_size.1 as i32 - window_h / 2),
        )
    }
}

pub fn main() {
    let mut is_fullscreen = false;
    let world = World::new();
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let _image_context = sdl2::image::init(InitFlag::PNG).unwrap();
    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string()).unwrap();
    let window = video_subsystem
        .window("gaem", 800, 800)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())
        .unwrap();

    let canvas = window
        .into_canvas()
        .accelerated()
        .build()
        .map_err(|e| e.to_string())
        .unwrap();

    let texture_creator = canvas.texture_creator();

    let mut font = ttf_context
        .load_font("assets/fonts/vcr_osd_mono.ttf", 18)
        .unwrap();
    font.set_style(sdl2::ttf::FontStyle::NORMAL);

    let mut animations = AnimationRepository::new();

    animations.push("player_idle", &[(0, 0, 1, 2).into(), (1, 0, 1, 2).into()]);
    animations.push(
        "player_walk",
        &[
            (0, 0, 1, 2).into(),
            (2, 0, 1, 2).into(),
            (0, 0, 1, 2).into(),
            (3, 0, 1, 2).into(),
        ],
    );

    animations.push("enemy_walk", &[(4, 0, 2, 2).into(), (6, 0, 2, 2).into()]);

    animations.push("bang", &[(10, 0, 1, 1).into(), (11, 0, 1, 1).into()]);

    animations.push("floor", &[(8, 0, 1, 1).into()]);

    animations.push("wall", &[(0, 2, 1, 2).into()]);

    animations.push(
        "torch",
        &[
            (9, 1, 1, 1).into(),
            (10, 1, 1, 1).into(),
            (11, 1, 1, 1).into(),
        ],
    );

    animations.push("lever", &[(8, 1, 1, 1).into()]);

    animations.push("particle_emitter", &[(9, 0, 1, 1).into()]);

    animations.push("chemlight", &[(12, 1, 1, 1).into()]);

    let mut ctx = Ctx {
        despawn_queue: RwLock::new(Vec::new()),
        light_tex: texture_creator
            .load_texture("assets/textures/light.png")
            .unwrap(),
        ui_tex: texture_creator
            .create_texture(
                None,
                sdl2::render::TextureAccess::Target,
                canvas.window().drawable_size().0,
                canvas.window().drawable_size().1,
            )
            .unwrap(),
        ui_active_item_bg: (13, 0, 1, 1).into(),
        lightmap: Lightmap::new(
            &canvas,
            canvas.window().drawable_size().0,
            canvas.window().drawable_size().1,
        ),
        spritesheet: Spritesheet::new_from_file(
            &texture_creator,
            "assets/textures/spritesheet.png",
            "assets/textures/specular.png",
            16,
        ),
        animations,
        canvas,
        input: Input {
            pressed: InputState {
                up: false,
                down: false,
                left: false,
                right: false,
                shift: false,
                fire_up: false,
                fire_down: false,
                fire_left: false,
                fire_right: false,
                interact: false,
                q: false,
                e: false,
                use_item: false,
            },
            just_pressed: InputState {
                up: false,
                down: false,
                left: false,
                right: false,
                shift: false,
                fire_up: false,
                fire_down: false,
                fire_left: false,
                fire_right: false,
                interact: false,
                q: false,
                e: false,
                use_item: false,
            },
        },
        player_speed: 3.0,
        enemy_speed: 1.2,
        bullet_speed: 4.0,
        debug_draw_nav_colliders: false,
        debug_draw_hitboxes: false,
        debug_draw_centerpoints: false,
        bullet_lifetime: 60,
        player_fire_cooldown: 20,
        shadows_enabled: false,
        player_pos: Pos::zero(),
        room_size: (2048, 2048),
        player_inventory: Inventory::new(),
        particle_emitter_entity: None,
    };

    ctx.ui_tex.set_blend_mode(BlendMode::Add);

    world.add_resource(ctx);
    world.add_resource(DepthBuffer::new());
    let ctx = world.resource_mut::<Ctx>().unwrap();

    game::init(&world);

    let mut event_pump = sdl_context.event_pump().unwrap();
    'mainloop: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'mainloop,
                Event::KeyDown {
                    keycode: Some(Keycode::F1),
                    ..
                } => ctx.debug_draw_nav_colliders = !ctx.debug_draw_nav_colliders,
                Event::KeyDown {
                    keycode: Some(Keycode::F2),
                    ..
                } => ctx.debug_draw_hitboxes = !ctx.debug_draw_hitboxes,
                Event::KeyDown {
                    keycode: Some(Keycode::F3),
                    ..
                } => ctx.debug_draw_centerpoints = !ctx.debug_draw_centerpoints,
                Event::KeyDown {
                    keycode: Some(Keycode::F5),
                    ..
                } => ctx.shadows_enabled = !ctx.shadows_enabled,
                Event::KeyDown {
                    keycode: Some(Keycode::F9),
                    ..
                } => {
                    is_fullscreen = !is_fullscreen;
                    ctx.canvas
                        .window_mut()
                        .set_fullscreen(if is_fullscreen {
                            sdl2::video::FullscreenType::Desktop
                        } else {
                            sdl2::video::FullscreenType::Off
                        })
                        .unwrap();
                    ctx.lightmap = Lightmap::new(
                        &ctx.canvas,
                        ctx.canvas.window().drawable_size().0,
                        ctx.canvas.window().drawable_size().1,
                    );
                    ctx.ui_tex = ctx
                        .canvas
                        .texture_creator()
                        .create_texture(
                            None,
                            sdl2::render::TextureAccess::Target,
                            ctx.canvas.window().drawable_size().0,
                            ctx.canvas.window().drawable_size().1,
                        )
                        .unwrap();
                    ctx.ui_tex.set_blend_mode(BlendMode::Add);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::F12),
                    ..
                } => {
                    ctx.spritesheet = Spritesheet::new_from_file(
                        &ctx.canvas.texture_creator(),
                        "assets/textures/spritesheet.png",
                        "assets/textures/specular.png",
                        16,
                    );
                    println!("Assets reloaded");
                }
                _ => {}
            }
        }

        let kb = event_pump.keyboard_state();
        let input = &mut ctx.input;
        // TODO just_pressed for all
        input.pressed.up = kb.is_scancode_pressed(Scancode::W);
        input.pressed.down = kb.is_scancode_pressed(Scancode::S);
        input.pressed.left = kb.is_scancode_pressed(Scancode::A);
        input.pressed.right = kb.is_scancode_pressed(Scancode::D);
        input.pressed.fire_right = kb.is_scancode_pressed(Scancode::Right);
        input.pressed.fire_left = kb.is_scancode_pressed(Scancode::Left);
        input.pressed.fire_up = kb.is_scancode_pressed(Scancode::Up);
        input.pressed.fire_down = kb.is_scancode_pressed(Scancode::Down);
        input.pressed.shift = kb.is_scancode_pressed(Scancode::LShift);
        input.just_pressed.interact =
            !input.pressed.interact && kb.is_scancode_pressed(Scancode::F);
        input.pressed.interact = kb.is_scancode_pressed(Scancode::F);
        input.just_pressed.q = !input.pressed.q && kb.is_scancode_pressed(Scancode::Q);
        input.pressed.q = kb.is_scancode_pressed(Scancode::Q);
        input.just_pressed.e = !input.pressed.e && kb.is_scancode_pressed(Scancode::E);
        input.pressed.e = kb.is_scancode_pressed(Scancode::E);
        input.just_pressed.use_item =
            !input.pressed.use_item && kb.is_scancode_pressed(Scancode::Space);
        input.pressed.use_item = kb.is_scancode_pressed(Scancode::Space);

        let update_start = Instant::now();
        game::update(&world);
        let end = Instant::now().duration_since(update_start);
        let update_time = end.as_micros();

        let render_start = Instant::now();
        ctx.canvas.set_draw_color(Color::RGB(0, 0, 0));
        ctx.canvas.clear();

        game::render(&world);
        build_lightmap(&world, ctx);
        ctx.lightmap.lights().set_blend_mode(BlendMode::Mul);
        ctx.canvas.copy(&ctx.lightmap.lights(), None, None).unwrap();
        ctx.canvas.copy(&ctx.ui_tex, None, None).unwrap();

        let end = Instant::now().duration_since(render_start);
        let render_time = end.as_micros();
        let frame_time = update_time + render_time;

        let sleep_duration = Duration::new(0, 1_000_000_000u32 / 60)
            .saturating_sub(Instant::now().duration_since(update_start));
        ::std::thread::sleep(sleep_duration);

        use memory_stats::memory_stats;
        let mut mem_usage = 0;
        if let Some(usage) = memory_stats() {
            mem_usage = usage.physical_mem;
        }

        let surface = font
            .render(
                format!(
                    "MEM: {:.2} MB | FRAME: {:.2}ms | UPDATE: {:.2}ms | RENDER: {:.2}ms",
                    mem_usage as f32 / (1024 * 1204) as f32,
                    frame_time as f32 / 1000.,
                    update_time as f32 / 1000.,
                    render_time as f32 / 1000.
                )
                .as_str(),
            )
            .shaded(
                if sleep_duration.is_zero() {
                    Color::RGBA(255, 0, 0, 255)
                } else {
                    Color::RGBA(255, 255, 255, 255)
                },
                Color::RGBA(0, 0, 0, 255),
            )
            .map_err(|e| e.to_string())
            .unwrap();
        let texture = texture_creator
            .create_texture_from_surface(&surface)
            .map_err(|e| e.to_string())
            .unwrap();

        let sdl2::render::TextureQuery { width, height, .. } = texture.query();
        ctx.canvas
            .copy(&texture, None, Rect::new(0, 0, width, height))
            .unwrap();
        unsafe { texture.destroy() };

        ctx.canvas.present();
    }
}

fn build_lightmap(world: &World, ctx: &mut Ctx) {
    // TODO cull off-screen lights
    ctx.canvas
        .with_texture_canvas(&mut ctx.lightmap.lights(), |lightmap_canvas| {
            // clear lightmap to ambient
            lightmap_canvas.set_draw_color(Color::RGB(70, 70, 70));
            lightmap_canvas.clear();
        })
        .unwrap();

    world.run(|light: &mut Light, lp: &Pos| {
        let camera_pos = world.resource::<Ctx>().unwrap().camera_pos();
        let x = lp.x - camera_pos.0 as f32;
        let y = lp.y - camera_pos.1 as f32;

        build_shadow_mask(
            light,
            *lp,
            camera_pos.into(),
            &ctx.lightmap,
            world,
            &mut ctx.canvas,
        );

        ctx.canvas
            .with_texture_canvas(&mut ctx.lightmap.per_light_tex(), |per_light_canvas| {
                per_light_canvas.set_draw_color(Color::RGB(0, 0, 0));
                per_light_canvas.clear();

                if light.radius > 0 && light.intensity > 0. {
                    ctx.light_tex.set_blend_mode(BlendMode::Add);
                    ctx.light_tex.set_color_mod(
                        (light.color.r as f32 * light.intensity) as u8,
                        (light.color.g as f32 * light.intensity) as u8,
                        (light.color.b as f32 * light.intensity) as u8,
                    );
                    per_light_canvas
                        .copy(
                            &ctx.light_tex,
                            None,
                            Rect::from_center(
                                (x as i32, y as i32),
                                (light.radius as u32) * 2,
                                (light.radius as u32) * 2,
                            ),
                        )
                        .unwrap();
                }

                per_light_canvas
                    .copy(&ctx.lightmap.mask(), None, None)
                    .unwrap();
            })
            .unwrap();

        ctx.canvas
            .with_texture_canvas(&mut ctx.lightmap.lights(), |lightmap_canvas| {
                ctx.lightmap.per_light_tex().set_blend_mode(BlendMode::Add);
                lightmap_canvas
                    .copy(&ctx.lightmap.per_light_tex(), None, None)
                    .unwrap();
            })
            .unwrap();
    });
}

fn build_shadow_mask(
    light: &Light,
    lp: Pos, // light pos
    cp: Pos, // camera pos
    lightmap: &Lightmap,
    world: &World,
    canvas: &mut Canvas<Window>,
) {
    // world space to screen space
    let lp = Pos::new(lp.x - cp.x, lp.y - cp.y);
    
    canvas
        .with_texture_canvas(&mut lightmap.mask(), |shadow_mask_canvas| {
            // clear occlusion mask
            shadow_mask_canvas.set_draw_color(Color::RGB(255, 255, 255));
            shadow_mask_canvas.clear();

            let light_bounds = Rect::new(
                lp.x as i32 - light.radius as i32,
                lp.y as i32 - light.radius as i32,
                light.radius as u32 * 2,
                light.radius as u32 * 2,
            );

            world.run(|cg: &ColliderGroup, _: With<Wall>| {
                let mut cg_bounds = cg.nav.unwrap().bounds;
                cg_bounds.x -= cp.x as i32;
                cg_bounds.y -= cp.y as i32;

                if let Some(rect) = light_bounds.intersection(cg_bounds) {
                    let dx = lp.x as i32 - rect.center().x;
                    let dy = lp.y as i32 - rect.center().y;

                    let p0 = if dx.signum() == dy.signum() {
                        rect.bottom_left()
                    } else {
                        rect.top_left()
                    };

                    let p1 = if dx.signum() == dy.signum() {
                        rect.top_right()
                    } else {
                        rect.bottom_right()
                    };

                    let theta_0 = f32::atan2(lp.y - p0.y as f32, lp.x - p0.x as f32);
                    let theta_1 = f32::atan2(lp.y - p1.y as f32, lp.x - p1.x as f32);

                    // TODO should calculate p0' and p1' on the tangent line at p_t
                    let p0_prime = (
                        lp.x as i32 - (theta_0.cos() * light.radius as f32 * 2.) as i32,
                        lp.y as i32 - (theta_0.sin() * light.radius as f32 * 2.) as i32,
                    );

                    let p1_prime = (
                        lp.x as i32 - (theta_1.cos() * light.radius as f32 * 2.) as i32,
                        lp.y as i32 - (theta_1.sin() * light.radius as f32 * 2.) as i32,
                    );

                    shadow_mask_canvas
                        .filled_polygon(
                            &[
                                p0.x as i16,
                                p1.x as i16,
                                p1_prime.0 as i16,
                                p0_prime.0 as i16,
                            ],
                            &[
                                p0.y as i16,
                                p1.y as i16,
                                p1_prime.1 as i16,
                                p0_prime.1 as i16,
                            ],
                            Color::RGB(0, 0, 0),
                        )
                        .unwrap();
                }
            });
        })
        .unwrap();
}
