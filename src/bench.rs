mod apu;
mod c6502;
mod common;
mod joystick;
mod mapper;
mod nes;
mod ppu;
mod serialization;

use crate::joystick::Joystick;
use crate::nes::{load_ines, read_ines};

fn main() {
    let joystick1 = Box::new(Joystick::new());
    let joystick2 = Box::new(Joystick::new());
    let ines = read_ines("roms/mario.nes".to_string()).unwrap();
    let mut nes = load_ines(ines, joystick1, joystick2);
    for _ in 0..10_000 {
        nes.run_frame();
    }
}
