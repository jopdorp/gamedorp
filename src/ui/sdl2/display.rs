use sdl2::video::{Window, WindowBuilder};
use sdl2::render::Canvas;
use sdl2::pixels::Color as SDL_Color;
use sdl2::rect::{Point, Rect};
use sdl2::Sdl;
use sdl2::VideoSubsystem;

use gpu::Color;

pub struct Display {
    renderer: Canvas<Window>,
    /// Upscaling factor, log2.
    upscale:  u8,
}

impl Display {
    pub fn new(sdl2: &Sdl, upscale: u8) -> Display {
        let up = 1 << (upscale as u32);

        let xres = 160 * up;
        let yres = 144 * up;

        let video_subsystem = sdl2.video().unwrap();

        let window = video_subsystem.window("Example", xres, yres).build().unwrap();
        let canvas = window.into_canvas()
            .target_texture()
            .present_vsync()
            .build().unwrap();

        Display { renderer: canvas, upscale: upscale }
    }
}

impl ::ui::Display for Display {
    fn clear(&mut self) {
        let _ = &self.renderer.set_draw_color(SDL_Color::RGB(0xff, 0x00, 0x00));
        let _ = &self.renderer.clear();
    }

    fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        let color = match color {
            Color::Black     => SDL_Color::RGB(0x00, 0x00, 0x00),
            Color::DarkGrey  => SDL_Color::RGB(0x55, 0x55, 0x55),
            Color::LightGrey => SDL_Color::RGB(0xab, 0xab, 0xab),
            Color::White     => SDL_Color::RGB(0xff, 0xff, 0xff),
        };

        self.renderer.set_draw_color(color);

        let drawer = &mut self.renderer;
        if self.upscale == 0 {
            let _ = drawer.draw_point(Point::new(x as i32, y as i32));
        } else {
            let up: u32 = 1 << (self.upscale as u32);

            // Translate coordinates
            let x = x as i32 * up as i32;
            let y = y as i32 * up as i32;

            let _ = drawer.fill_rect(Rect::new(x, y, up, up));
        }
    }

    fn flip(&mut self) {
        self.renderer.present();
        self.clear();
    }
}
