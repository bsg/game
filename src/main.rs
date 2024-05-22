extern crate sdl2;

mod game;

use std::{
    collections::BinaryHeap,
    ops::Deref,
    sync::RwLock,
    time::{Duration, Instant},
};

use ecs::{Entity, Resource, World};
use sdl2::{
    event::Event,
    image::{InitFlag, LoadTexture},
    keyboard::{Keycode, Scancode},
    pixels::Color,
    rect::Rect,
    render::{Canvas, RenderTarget, Texture, TextureCreator},
    video::{Window, WindowContext},
};

#[derive(Clone, Copy)]
struct Vec2F {
    x: f32,
    y: f32,
}

#[allow(dead_code)]
impl Vec2F {
    pub fn new(x: f32, y: f32) -> Vec2F {
        return Vec2F { x, y };
    }

    pub fn zero() -> Vec2F {
        return Vec2F { x: 0.0, y: 0.0 };
    }

    pub fn magnitude(&self) -> f32 {
        f32::sqrt(self.x.powi(2) + self.y.powi(2))
    }

    pub fn normalize(&mut self) {
        self.x /= self.magnitude();
        self.y /= self.magnitude();
    }

    pub fn scale(&mut self, scale: f32) {
        self.x *= scale;
        self.y *= scale;
    }

    pub fn scaled(&self, scale: f32) -> Vec2F {
        Vec2F::new(self.x * scale, self.y * scale)
    }
}

#[derive(Clone, Copy)]
struct Vec3F {
    x: f32,
    y: f32,
    z: f32,
}

#[allow(dead_code)]
impl Vec3F {
    pub fn new(x: f32, y: f32, z: f32) -> Vec3F {
        return Vec3F { x, y, z };
    }

    pub fn zero() -> Vec3F {
        return Vec3F {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
    }
}

#[derive(Clone, Copy)]
struct TextureID(usize);

impl Deref for TextureID {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
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
    ) -> TextureID {
        let texture = texture_creator.load_texture(&path).unwrap();
        self.textures.push(texture);
        TextureID(self.textures.len() - 1)
    }

    pub fn get(&self, id: TextureID) -> &Texture {
        self.textures.get(*id).unwrap()
    }
}

struct TextureWithDepth {
    texture_id: TextureID,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    depth: i32,
}

impl PartialEq for TextureWithDepth {
    fn eq(&self, other: &Self) -> bool {
        self.depth == other.depth
    }
}

impl Eq for TextureWithDepth {}

impl PartialOrd for TextureWithDepth {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.depth == other.depth {
            Some(std::cmp::Ordering::Equal)
        } else if self.depth > other.depth {
            Some(std::cmp::Ordering::Less)
        } else {
            Some(std::cmp::Ordering::Greater)
        }
    }
}

impl Ord for TextureWithDepth {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.depth == other.depth {
            std::cmp::Ordering::Equal
        } else if self.depth > other.depth {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    }
}

#[derive(Resource)]
struct DepthBuffer {
    buffer: BinaryHeap<TextureWithDepth>,
}

impl DepthBuffer {
    pub fn new() -> Self {
        DepthBuffer {
            buffer: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, texture: TextureWithDepth) {
        self.buffer.push(texture);
    }

    pub fn draw_to_canvas<T: RenderTarget>(
        &mut self,
        canvas: &mut Canvas<T>,
        texture_repo: &TextureRepository,
    ) {
        while !self.buffer.is_empty() {
            let tex = self.buffer.pop().unwrap();
            canvas
                .copy(
                    texture_repo.get(tex.texture_id),
                    None,
                    Some(Rect::new(tex.x, tex.y, tex.w as u32, tex.h as u32)),
                )
                .unwrap();
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
}

#[derive(Resource)]
pub struct Ctx {
    player_textures: [TextureID; 3],
    enemy_textures: [TextureID; 2],
    bullet_textures: [TextureID; 2],
    textures: TextureRepository,
    canvas: Canvas<Window>,
    input: Input,
    // player_pos: Vec2F,
    player_speed: f32,
    enemy_speed: f32,
    bullet_speed: f32,
    bullet_lifetime: usize,
    player_fire_cooldown: usize,
    enemy_spawn_cooldown: usize,
    enemy_spawn_in: usize,

    despawn_queue: RwLock<Vec<Entity>>,

    // Debug
    frame_time_avg: u128,
    update_time_avg: u128,
    render_time_avg: u128,
    debug_draw_colliders: bool,
    debug_draw_hitboxes: bool,
}

unsafe impl Sync for Ctx {}

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
        .build()
        .map_err(|e| e.to_string())
        .unwrap();

    let texture_creator = canvas.texture_creator();

    let mut textures = TextureRepository::new();

    let mut font = ttf_context
        .load_font("assets/fonts/comic_sans.ttf", 16)
        .unwrap();
    font.set_style(sdl2::ttf::FontStyle::NORMAL);

    let player_textures = [
        textures.load_texture(
            &texture_creator,
            "assets/textures/guy_front_0.png".to_owned(),
        ),
        textures.load_texture(
            &texture_creator,
            "assets/textures/guy_front_1.png".to_owned(),
        ),
        textures.load_texture(
            &texture_creator,
            "assets/textures/guy_front_2.png".to_owned(),
        ),
    ];

    let enemy_textures = [
        textures.load_texture(&texture_creator, "assets/textures/blob_0.png".to_owned()),
        textures.load_texture(&texture_creator, "assets/textures/blob_1.png".to_owned()),
    ];

    let bullet_textures = [
        textures.load_texture(&texture_creator, "assets/textures/bullet_1.png".to_owned()),
        textures.load_texture(&texture_creator, "assets/textures/bullet_2.png".to_owned()),
    ];

    let ctx = Ctx {
        player_textures,
        enemy_textures,
        bullet_textures,
        textures,
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
        },
        // player_pos: Vec2F::new(0.0, 0.0),
        player_speed: 2.0,
        enemy_speed: 1.2,
        bullet_speed: 8.0,
        enemy_spawn_cooldown: 100,
        enemy_spawn_in: 0,
        debug_draw_colliders: false,
        debug_draw_hitboxes: false,
        bullet_lifetime: 60,
        player_fire_cooldown: 10,

        despawn_queue: RwLock::new(Vec::new()),

        frame_time_avg: 0,
        update_time_avg: 0,
        render_time_avg: 0,
    };

    world.add_resource(ctx);
    world.add_resource(DepthBuffer::new());
    let ctx = world.get_resource_mut::<Ctx>().unwrap();

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
                } => ctx.debug_draw_colliders = !ctx.debug_draw_colliders,
                Event::KeyDown {
                    keycode: Some(Keycode::F2),
                    ..
                } => ctx.debug_draw_hitboxes = !ctx.debug_draw_hitboxes,
                _ => {}
            }
        }

        if event_pump.keyboard_state().is_scancode_pressed(Scancode::W) {
            ctx.input.up = true;
        } else {
            ctx.input.up = false;
        }

        if event_pump.keyboard_state().is_scancode_pressed(Scancode::S) {
            ctx.input.down = true;
        } else {
            ctx.input.down = false;
        }

        if event_pump.keyboard_state().is_scancode_pressed(Scancode::A) {
            ctx.input.left = true;
        } else {
            ctx.input.left = false;
        }

        if event_pump.keyboard_state().is_scancode_pressed(Scancode::D) {
            ctx.input.right = true;
        } else {
            ctx.input.right = false;
        }

        if event_pump
            .keyboard_state()
            .is_scancode_pressed(Scancode::Right)
        {
            ctx.input.fire_right = true;
        } else {
            ctx.input.fire_right = false;
        }

        if event_pump
            .keyboard_state()
            .is_scancode_pressed(Scancode::Left)
        {
            ctx.input.fire_left = true;
        } else {
            ctx.input.fire_left = false;
        }

        if event_pump
            .keyboard_state()
            .is_scancode_pressed(Scancode::Up)
        {
            ctx.input.fire_up = true;
        } else {
            ctx.input.fire_up = false;
        }

        if event_pump
            .keyboard_state()
            .is_scancode_pressed(Scancode::Down)
        {
            ctx.input.fire_down = true;
        } else {
            ctx.input.fire_down = false;
        }

        if event_pump
            .keyboard_state()
            .is_scancode_pressed(Scancode::LShift)
        {
            ctx.input.shift = true;
        }

        let update_start = Instant::now();
        game::update(&world);
        let end = Instant::now().duration_since(update_start);
        ctx.update_time_avg = (ctx.update_time_avg + end.as_micros()) / 2;

        ctx.canvas.set_draw_color(Color::RGB(16, 16, 16));
        ctx.canvas.clear();

        let render_start = Instant::now();
        game::render(&world);
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
