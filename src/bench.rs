#![no_std]
#![feature(rustc_attrs)]
#![feature(lang_items)]

mod common;
mod c6502;
mod ppu;
mod apu;
mod mapper;
mod nes;
mod joystick;
mod globals;

use core::panic::PanicInfo;

use crate::joystick::Joystick;
use crate::nes::{read_ines,load_ines};

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop { }
}
#[lang = "eh_personality"]
pub extern fn rust_eh_personality() {}

#[lang = "start"]
pub extern fn start(_argc: isize, _argv: *const *const u8) -> isize {
    0
}

fn main() {
    let mut joystick1 = Joystick::new_software();
    let mut joystick2 = Joystick::new_software();
    let ines = read_ines().unwrap();
    let mut nes = load_ines(ines, &mut joystick1, &mut joystick2);
    for _ in 0..10_000 {
        nes.run_frame();
    }
}
