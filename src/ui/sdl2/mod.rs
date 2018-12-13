use std::cell::Cell;

// Re-export the public interface defined in sub-modules
use sdl2::Sdl;
pub use ui::sdl2::audio::Audio;
pub use ui::sdl2::controller::Controller;
pub use ui::sdl2::display::Display;

mod audio;
mod controller;
mod display;

pub struct Context {
    pub sdl2: Sdl,
    controller: controller::Controller,
}

impl Context {
    pub fn new() -> Context {
        let sdl2 = ::sdl2::init().unwrap();

        let contr = controller::Controller::new(&sdl2);

        Context {
            sdl2: sdl2,
            controller: contr,
        }
    }

    pub fn new_display(&self, upscale: u8, fullscreen: bool) -> display::Display {
        display::Display::new(&self.sdl2, upscale, fullscreen)
    }

    pub fn buttons(&self) -> &Cell<::ui::Buttons> {
        self.controller.buttons()
    }

    pub fn update_buttons(&self) -> ::ui::Event {
        self.controller.update(&self.sdl2)
    }
}
