mod common;
mod c6502;
mod ppu;
mod apu;
mod mapper;
mod nes;

extern crate sdl2;

use sdl2::pixels::PixelFormatEnum;
use sdl2::pixels::Color;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::render::Canvas;
use sdl2::surface::Surface;
use sdl2::video::Window;
use std::time::Duration;

use crate::nes::Nes;
use crate::nes::{load_ines, read_ines, Joystick};
use crate::ppu::*;

// https://wiki.nesdev.com/w/index.php/Cycle_reference_chart
const CLOCKS_PER_FRAME:u32 = 29780;

fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem.window("Rusty NES", 800, 600)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    let mut render_surface = Surface::new(RENDER_WIDTH as u32, RENDER_HEIGHT as u32, PixelFormatEnum::Index8).unwrap();
    let mut nes = create_nes();

    canvas.set_draw_color(Color::RGB(0, 255, 255));
    canvas.clear();
    canvas.present();
    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut i = 0;
    'running: loop {
        i = (i + 1) % 255;
        canvas.set_draw_color(Color::RGB(i, 64, 255 - i));
        canvas.clear();
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running
                },
                _ => {}
            }
        }
        // The rest of the game loop goes here...
        render_frame(&mut nes, &mut render_surface);
        present_frame(&mut canvas, &render_surface);

        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

fn create_nes() -> Nes {
    let rom = read_ines("roms/nestest.nes".to_string()).unwrap();
    let joystick1 = Joystick::new();
    let joystick2 = Joystick::new();
    return load_ines(rom, Box::new(joystick1), Box::new(joystick2));
}

fn render_frame(nes: &mut Nes, surface: &mut Surface) {
    let buffer = surface.without_lock_mut().unwrap();
    nes.run_frame(buffer);
}

fn present_frame(canvas: &mut Canvas<Window>, surface: &Surface) {
    let texture_creator = canvas.texture_creator();
    let texture = texture_creator.create_texture_from_surface(surface).unwrap();
    canvas.copy(&texture, None, None);
}
