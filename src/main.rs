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

use components::{ColliderGroup, Wall};
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
    tile_size: u16,
}

impl Spritesheet {
    pub fn new_from_file(
        texture_creator: &TextureCreator<WindowContext>,
        path: &'static str,
        tile_size: u16,
    ) -> Self {
        if let Ok(texture) = texture_creator.load_texture(path) {
            Spritesheet {
                texture: MaybeUninit::new(texture),
                tile_size,
            }
        } else {
            panic!("Failed to load texture {}", path)
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

pub struct Input {
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
}

pub struct Lightmap {
    lights: MaybeUninit<Texture>,
    // Occlusion mask
    mask: MaybeUninit<Texture>,
}

impl Lightmap {
    pub fn new(canvas: &Canvas<Window>, w: u32, h: u32) -> Lightmap {
        let mut lights = canvas
            .texture_creator()
            .create_texture_target(canvas.default_pixel_format(), w, h)
            .unwrap();
        lights.set_blend_mode(sdl2::render::BlendMode::Mul);

        let mut mask = canvas
            .texture_creator()
            .create_texture_target(canvas.default_pixel_format(), w, h)
            .unwrap();
        mask.set_blend_mode(sdl2::render::BlendMode::Mul);

        Lightmap {
            lights: MaybeUninit::new(lights),
            mask: MaybeUninit::new(mask),
        }
    }

    pub fn lights(&self) -> Texture {
        unsafe { self.lights.assume_init_read() }
    }

    pub fn lights_mut(&self) -> Texture {
        unsafe { self.lights.assume_init_read() }
    }

    pub fn mask(&self) -> Texture {
        unsafe { self.mask.assume_init_read() }
    }

    pub fn mask_mut(&self) -> Texture {
        unsafe { self.mask.assume_init_read() }
    }
}

impl Drop for Lightmap {
    fn drop(&mut self) {
        unsafe { self.lights.assume_init_read().destroy() }
        unsafe { self.mask.assume_init_read().destroy() }
    }
}

#[derive(Resource)]
pub struct Ctx {
    canvas: Canvas<Window>,
    spritesheet: Spritesheet,
    animations: AnimationRepository,
    light_tex: Texture,
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
    is_shadows_enabled: bool,
    player_pos: Pos,
    spawner_entity: Option<Entity>,
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

    animations.push("bullet", &[(11, 0, 1, 1).into(), (12, 0, 1, 1).into()]);

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

    animations.push("spawner", &[(9, 0, 1, 1).into()]);

    let ctx = Ctx {
        despawn_queue: RwLock::new(Vec::new()),
        light_tex: texture_creator
            .load_texture("assets/textures/light.png")
            .unwrap(),
        lightmap: Lightmap::new(&canvas, 800, 800),
        spritesheet: Spritesheet::new_from_file(
            &texture_creator,
            "assets/textures/spritesheet.png",
            16,
        ),
        animations,
        canvas,
        input: Input {
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
        },
        player_speed: 3.0,
        enemy_speed: 1.2,
        bullet_speed: 4.0,
        debug_draw_nav_colliders: false,
        debug_draw_hitboxes: false,
        debug_draw_centerpoints: false,
        bullet_lifetime: 60,
        player_fire_cooldown: 20,
        is_shadows_enabled: false,
        player_pos: Pos::zero(),
        spawner_entity: None,
    };

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
                } => ctx.is_shadows_enabled = !ctx.is_shadows_enabled,
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
                    )
                }
                Event::KeyDown {
                    keycode: Some(Keycode::F12),
                    ..
                } => {
                    ctx.spritesheet = Spritesheet::new_from_file(
                        &ctx.canvas.texture_creator(),
                        "assets/textures/spritesheet.png",
                        16,
                    );
                    println!("Assets reloaded");
                }
                _ => {}
            }
        }

        let kb = event_pump.keyboard_state();
        let input = &mut ctx.input;
        input.up = kb.is_scancode_pressed(Scancode::W);
        input.down = kb.is_scancode_pressed(Scancode::S);
        input.left = kb.is_scancode_pressed(Scancode::A);
        input.right = kb.is_scancode_pressed(Scancode::D);
        input.fire_right = kb.is_scancode_pressed(Scancode::Right);
        input.fire_left = kb.is_scancode_pressed(Scancode::Left);
        input.fire_up = kb.is_scancode_pressed(Scancode::Up);
        input.fire_down = kb.is_scancode_pressed(Scancode::Down);
        input.shift = kb.is_scancode_pressed(Scancode::LShift);
        input.interact = kb.is_scancode_pressed(Scancode::E);

        let update_start = Instant::now();
        game::update(&world);
        let end = Instant::now().duration_since(update_start);
        let update_time = end.as_micros();

        let render_start = Instant::now();

        ctx.canvas
            .with_texture_canvas(&mut ctx.lightmap.lights_mut(), |canvas| {
                // clear lightmap to ambient
                canvas.set_draw_color(Color::RGB(70, 70, 70));
                canvas.clear();

                // TODO extract the occlusion pass out because this is unreadable as fuck
                world.run(|light: &mut Light, lp: &Pos| {
                    if light.radius > 0 {
                        if ctx.is_shadows_enabled {
                            canvas
                                .with_texture_canvas(&mut ctx.lightmap.mask_mut(), |canvas| {
                                    // clear occlusion mask
                                    canvas.set_draw_color(Color::RGB(255, 255, 255));
                                    canvas.clear();

                                    let light_bounds = Rect::new(
                                        lp.x as i32 - light.radius as i32,
                                        lp.y as i32 - light.radius as i32,
                                        light.radius as u32 * 2,
                                        light.radius as u32 * 2,
                                    );

                                    world.run(|cg: &ColliderGroup, _: With<Wall>| {
                                        if let Some(rect) =
                                            light_bounds.intersection(cg.nav.unwrap().bounds)
                                        {
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

                                            let theta_0 =
                                                f32::atan2(lp.y - p0.y as f32, lp.x - p0.x as f32);

                                            let theta_1 =
                                                f32::atan2(lp.y - p1.y as f32, lp.x - p1.x as f32);

                                            // TODO should calculate p0' and p1' on the tangent line at p_t
                                            let p0_prime = (
                                                lp.x as i32
                                                    - (theta_0.cos() * light.radius as f32 * 2.)
                                                        as i32,
                                                lp.y as i32
                                                    - (theta_0.sin() * light.radius as f32 * 2.)
                                                        as i32,
                                            );

                                            let p1_prime = (
                                                lp.x as i32
                                                    - (theta_1.cos() * light.radius as f32 * 2.)
                                                        as i32,
                                                lp.y as i32
                                                    - (theta_1.sin() * light.radius as f32 * 2.)
                                                        as i32,
                                            );

                                            // TODO filled_trigon x2 would perhaps be faster?
                                            canvas
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

                        ctx.light_tex.set_blend_mode(BlendMode::Add);
                        ctx.light_tex
                            .set_color_mod(light.color.r, light.color.g, light.color.b);
                        canvas
                            .copy(
                                &ctx.light_tex,
                                None,
                                Rect::from_center(
                                    (lp.x as i32, lp.y as i32),
                                    (light.radius as u32) * 2,
                                    (light.radius as u32) * 2,
                                ),
                            )
                            .unwrap();
                    }
                    if ctx.is_shadows_enabled {
                        canvas.copy(&ctx.lightmap.mask(), None, None).unwrap();
                    }
                });
            })
            .unwrap();

        ctx.canvas.set_draw_color(Color::RGB(0, 0, 0));
        ctx.canvas.clear();

        game::render(&world);

        ctx.canvas.copy(&ctx.lightmap.lights(), None, None).unwrap();

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
