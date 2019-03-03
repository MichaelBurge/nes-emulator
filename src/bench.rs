mod common;
mod c6502;
mod ppu;
mod apu;
mod mapper;
mod nes;
mod joystick;
mod serialization;

use crate::joystick::Joystick;
use crate::nes::{read_ines,load_ines};


fn main() {
    let joystick1 = Box::new(Joystick::new_software());
    let joystick2 = Box::new(Joystick::new_software());
    let ines = read_ines("roms/mario.nes".to_string()).unwrap();
    let mut nes = load_ines(ines, joystick1, joystick2);
    for _ in 0..10_000 {
        nes.run_frame();
    }
}
