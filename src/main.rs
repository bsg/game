extern crate sdl2;

mod components;
mod game;
mod math;

use std::{
    collections::{BinaryHeap, HashMap},
    ops::Deref,
    sync::RwLock,
    time::{Duration, Instant},
};

use ecs::{Entity, Resource, World};
use math::Vec3;
use sdl2::{
    event::Event,
    gfx::primitives::DrawRenderer,
    image::{InitFlag, LoadTexture},
    keyboard::{Keycode, Scancode},
    pixels::Color,
    rect::Rect,
    render::{Canvas, Texture, TextureCreator},
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

struct TextureRepository {
    textures: Vec<Texture>,
}

impl TextureRepository {
    pub fn new() -> Self {
        TextureRepository {
            textures: Vec::new(),
        }
    }

    pub fn load_texture(
        &mut self,
        texture_creator: &TextureCreator<WindowContext>,
        path: String,
    ) -> TextureId {
        let texture = texture_creator.load_texture(path).unwrap();
        self.textures.push(texture);
        TextureId(self.textures.len() - 1)
    }

    pub fn get(&self, id: TextureId) -> &Texture {
        match self.textures.get(*id) {
            Some(tex) => tex,
            None => panic!("no texture with id {}", *id),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Sprite(u16, u16, u16, u16);

impl Into<Sprite> for (u16, u16, u16, u16) {
    fn into(self) -> Sprite {
        Sprite(self.0, self.1, self.2, self.3)
    }
}

struct Spritesheet {
    texture: Texture,
    tile_size: u16,
}

impl Spritesheet {
    pub fn new_from_file(
        texture_creator: &TextureCreator<WindowContext>,
        path: &'static str,
        tile_size: u16,
    ) -> Self {
        if let Ok(texture) = texture_creator.load_texture(path) {
            Spritesheet { texture, tile_size }
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
                &self.texture,
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
    texture: Texture,
}

impl Lightmap {
    pub fn new(canvas: &Canvas<Window>, w: u32, h: u32) -> Lightmap {
        let mut texture = canvas
            .texture_creator()
            .create_texture_target(canvas.default_pixel_format(), w, h)
            .unwrap();

        texture.set_blend_mode(sdl2::render::BlendMode::Mul);
        Lightmap { texture }
    }
}

#[derive(Resource)]
pub struct Ctx {
    canvas: Canvas<Window>,
    spritesheet: Spritesheet,
    animations: AnimationRepository,
    lightmap: Lightmap,
    despawn_queue: RwLock<Vec<Entity>>,
    input: Input,
    player_speed: f32,
    enemy_speed: f32,
    bullet_speed: f32,
    bullet_lifetime: usize,
    player_fire_cooldown: usize,
    frame_time_avg: u128,
    update_time_avg: u128,
    render_time_avg: u128,
    debug_draw_nav_colliders: bool,
    debug_draw_hitboxes: bool,
    debug_draw_centerpoints: bool,

    spawner_entity: Option<Entity>,
}

pub fn main() {
    let world = World::new();
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let _image_context = sdl2::image::init(InitFlag::PNG).unwrap();
    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string()).unwrap();
    let window = video_subsystem
        .window("gaem", 800, 800)
        // .borderless()
        // .fullscreen_desktop()
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
        .load_font("assets/fonts/comic_sans.ttf", 16)
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
        player_speed: 2.0,
        enemy_speed: 1.2,
        bullet_speed: 4.0,
        debug_draw_nav_colliders: false,
        debug_draw_hitboxes: false,
        debug_draw_centerpoints: false,
        bullet_lifetime: 60,
        player_fire_cooldown: 20,
        frame_time_avg: 0,
        update_time_avg: 0,
        render_time_avg: 0,

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
        ctx.update_time_avg = (ctx.update_time_avg + end.as_micros()) / 2;

        let render_start = Instant::now();

        ctx.canvas
            .with_texture_canvas(&mut ctx.lightmap.texture, |canvas| {
                canvas.set_draw_color(Color::RGBA(0, 0, 0, 180));
                canvas.clear();
                world.run(|light: &Light, pos: &Pos| {
                    let mut color = light.color;
                    color.a = 255;
                    canvas
                        .filled_circle(
                            pos.x as i16,
                            pos.y as i16,
                            (light.radius as f32 * 0.7) as i16,
                            color,
                        )
                        .unwrap();
                    color.a = 127;
                    canvas
                        .filled_circle(pos.x as i16, pos.y as i16, light.radius, color)
                        .unwrap();
                });
            })
            .unwrap();

        ctx.canvas.set_draw_color(Color::RGB(16, 16, 16));
        ctx.canvas.clear();

        game::render(&world);

        ctx.canvas
            .copy(&ctx.lightmap.texture, None, Rect::new(0, 0, 800, 800))
            .unwrap();

        let end = Instant::now().duration_since(render_start);
        ctx.render_time_avg = (ctx.render_time_avg + end.as_micros()) / 2;

        ctx.frame_time_avg = ctx.update_time_avg + ctx.render_time_avg;

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
                    "Mem: {:.2} MB | Frame time: {}us | Update time: {}us | Render time: {}us",
                    mem_usage as f32 / (1024 * 1204) as f32,
                    ctx.frame_time_avg,
                    ctx.update_time_avg,
                    ctx.render_time_avg
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
