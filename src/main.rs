extern crate sdl2;

mod components;
mod game;
mod math;

use std::{
    collections::BinaryHeap,
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
    render::{Canvas, RenderTarget, Texture, TextureCreator},
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
        let texture = texture_creator.load_texture(&path).unwrap();
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

// TODO dunno what to call this
struct DrawCmd {
    texture_id: TextureId,
    pos: Vec3<i32>,
    w: u32,
    h: u32,
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
        if self.pos.z == other.pos.z {
            Some(std::cmp::Ordering::Equal)
        } else if self.pos.z > other.pos.z {
            Some(std::cmp::Ordering::Less)
        } else {
            Some(std::cmp::Ordering::Greater)
        }
    }
}

impl Ord for DrawCmd {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.pos.z == other.pos.z {
            std::cmp::Ordering::Equal
        } else if self.pos.z > other.pos.z {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
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

    pub fn draw_to_canvas<T: RenderTarget>(
        &mut self,
        canvas: &mut Canvas<T>,
        texture_repo: &TextureRepository,
    ) {
        while let Some(draw_cmd) = self.buffer.pop() {
            canvas
                .copy_ex(
                    texture_repo.get(draw_cmd.texture_id),
                    None,
                    Some(Rect::new(
                        draw_cmd.pos.x,
                        draw_cmd.pos.y,
                        draw_cmd.w as u32,
                        draw_cmd.h as u32,
                    )),
                    0.0,
                    None,
                    draw_cmd.flip_horizontal,
                    false,
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
    textures: TextureRepository,
    lightmap: Lightmap,
    despawn_queue: RwLock<Vec<Entity>>,
    player_textures: [TextureId; 4],
    enemy_textures: [TextureId; 2],
    bullet_textures: [TextureId; 2],
    floor_texture: TextureId,
    wall_texture: TextureId,
    torch_textures: [TextureId; 3],
    lever_texture: TextureId,
    spawner_texture: TextureId,
    exclamation_mark_texture: TextureId,
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

    let mut textures = TextureRepository::new();

    let mut font = ttf_context
        .load_font("assets/fonts/comic_sans.ttf", 16)
        .unwrap();
    font.set_style(sdl2::ttf::FontStyle::NORMAL);

    let player_textures = [
        textures.load_texture(&texture_creator, "assets/textures/player_0.png".to_owned()),
        textures.load_texture(&texture_creator, "assets/textures/player_1.png".to_owned()),
        textures.load_texture(&texture_creator, "assets/textures/player_2.png".to_owned()),
        textures.load_texture(&texture_creator, "assets/textures/player_3.png".to_owned()),
    ];

    let enemy_textures = [
        textures.load_texture(&texture_creator, "assets/textures/blob_0.png".to_owned()),
        textures.load_texture(&texture_creator, "assets/textures/blob_1.png".to_owned()),
    ];

    let bullet_textures = [
        textures.load_texture(&texture_creator, "assets/textures/bullet_1.png".to_owned()),
        textures.load_texture(&texture_creator, "assets/textures/bullet_2.png".to_owned()),
    ];

    let floor_texture =
        textures.load_texture(&texture_creator, "assets/textures/floor.png".to_owned());
    let wall_texture =
        textures.load_texture(&texture_creator, "assets/textures/wall.png".to_owned());

    let torch_textures = [
        textures.load_texture(&texture_creator, "assets/textures/torch_0.png".to_owned()),
        textures.load_texture(&texture_creator, "assets/textures/torch_1.png".to_owned()),
        textures.load_texture(&texture_creator, "assets/textures/torch_2.png".to_owned()),
    ];

    let lever_texture =
        textures.load_texture(&texture_creator, "assets/textures/lever.png".to_owned());
    let spawner_texture =
        textures.load_texture(&texture_creator, "assets/textures/spawner.png".to_owned());
    let exclamation_mark_texture = textures.load_texture(
        &texture_creator,
        "assets/textures/exclamation_mark.png".to_owned(),
    );

    let ctx = Ctx {
        despawn_queue: RwLock::new(Vec::new()),
        lightmap: Lightmap::new(&canvas, 800, 800),
        player_textures,
        enemy_textures,
        bullet_textures,
        floor_texture,
        wall_texture,
        torch_textures,
        lever_texture,
        spawner_texture,
        exclamation_mark_texture,
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
            interact: false,
        },
        player_speed: 2.0,
        enemy_speed: 1.2,
        bullet_speed: 4.0,
        debug_draw_nav_colliders: false,
        debug_draw_hitboxes: false,
        bullet_lifetime: 60,
        player_fire_cooldown: 20,
        frame_time_avg: 0,
        update_time_avg: 0,
        render_time_avg: 0,

        spawner_entity: None,
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
                } => ctx.debug_draw_nav_colliders = !ctx.debug_draw_nav_colliders,
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
        } else {
            ctx.input.shift = false;
        }

        if event_pump.keyboard_state().is_scancode_pressed(Scancode::E) {
            ctx.input.interact = true;
        } else {
            ctx.input.interact = false;
        }

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
                    let mut color = light.color.clone();
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
