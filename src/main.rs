extern crate sdl2;

use sdl2::pixels::PixelFormatEnum;
use sdl2::pixels::Color;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::render::Canvas;
use sdl2::surface::Surface;
use sdl2::video::Window;
use std::time::Duration;

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
    let mut render_surface = Surface::new(RENDER_WIDTH, RENDER_HEIGHT, PixelFormatEnum::Index8).unwrap();
    let nes = create_nes();

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
        render_frame(nes, render_surface);
        present_frame(canvas, &render_surface);

        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

fn create_nes() -> Nes {
    let rom = read_ines("roms/nestest.nes");
    return load_ines(rom);
}

fn render_frame(nes: Nes, surface: Surface) {
    run_clocks(nes, 29780);
    let Some(buffer) = surface.without_lock_mut();
    nes.ppu.render(buffer);
}

fn present_frame(canvas: Canvas<Window>, surface: &Surface) {
    let texture_creator = canvas.texture_creator();
    let texture = texture_creator.create_texture_from_surface(surface).unwrap();
    canvas.copy(&texture, None, None);
}
