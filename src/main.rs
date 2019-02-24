mod common;
mod c6502;
mod ppu;
mod apu;
mod mapper;
mod nes;
mod joystick;

extern crate sdl2;

use sdl2::pixels::PixelFormatEnum;
use sdl2::pixels::Color;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::render::Canvas;
use sdl2::render::Texture;
use sdl2::render::TextureAccess;
use sdl2::video::Window;
use std::time::Duration;

use crate::joystick::Joystick;
use crate::mapper::AddressSpace;
use crate::nes::Nes;
use crate::nes::{load_ines, read_ines};
use crate::ppu::*;

// https://wiki.nesdev.com/w/index.php/Cycle_reference_chart
const CLOCKS_PER_FRAME:u32 = 29780;
const SCALE:usize = 4;

fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let controller_subsystem = sdl_context.game_controller().unwrap();
    let joystick1 = Joystick::new(&controller_subsystem, 0);
    let joystick2 = Joystick::new(&controller_subsystem, 1);
    let window = video_subsystem.window("NES emulator", (RENDER_WIDTH*SCALE) as u32, (RENDER_HEIGHT*SCALE) as u32)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator.create_texture(
        PixelFormatEnum::RGB24,
        TextureAccess::Streaming,
        RENDER_WIDTH as u32,
        RENDER_HEIGHT as u32
    ).unwrap();
    let mut nes = create_nes(Box::new(joystick1), Box::new(joystick2));

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
                Event::KeyDown { keycode: Some(Keycode::Pause), .. } => {
                    nes.break_debugger();
                },
                _ => {}
            }
        }
        // The rest of the game loop goes here...
        nes.run_frame();
        present_frame(&mut canvas, &mut texture, &nes.ppu.display);

        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

fn create_nes(joystick1:Box<AddressSpace>, joystick2:Box<AddressSpace>) -> Nes {
    //let filename = "roms/donkey_kong.nes";
    let filename = "roms/mario.nes";
    let rom = read_ines(filename.to_string()).unwrap();
    return load_ines(rom, joystick1, joystick2);
}

fn present_frame(canvas: &mut Canvas<Window>, texture: &mut Texture, ppu_pixels: &[u8]) {
    texture.update(None, ppu_pixels, RENDER_WIDTH*3).unwrap();
    canvas.clear();
    canvas.copy(&texture, None, None).unwrap();
    canvas.present();
}
