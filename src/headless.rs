#![allow(unused_must_use)]
#![cfg(unix)]

mod common;
mod c6502;
mod ppu;
mod apu;
mod mapper;
mod nes;
mod joystick;
mod serialization;

use core::ptr::null_mut;
use std::io::Read;
use std::io::Write;
use std::fs::File;
use std::fmt::Debug;
use std::os::unix::io::FromRawFd;

use crate::joystick::Joystick;
use crate::mapper::AddressSpace;
use crate::nes::Nes;
use crate::nes::load_ines;
use crate::nes::read_ines;
use crate::serialization::Savable;
use crate::serialization::read_value;
use crate::common::Clocked;

fn main() {
    // Standard stdout() object is line-buffered
    let stdin = unsafe { File::from_raw_fd(0) };
    let stdout = unsafe { File::from_raw_fd(1) };
    let mut headless = Headless::new(
        Box::new(stdin),
        Box::new(stdout),
    );
    loop { headless.dispatch_command() }
}

struct Headless {
    joystick1: *mut Joystick,
    joystick2: *mut Joystick,
    nes: Option<Box<Nes>>,
    in_fh: Box<Read>,
    out_fh: Box<Write>,
    is_synchronized: bool,
    num_commands: u64,
}

impl Headless {
    pub fn new(in_fh: Box<Read>, out_fh: Box<Write>) -> Headless {
        let mut nes = None;
        Headless {
            joystick1: null_mut(),
            joystick2: null_mut(),
            nes: nes,
            in_fh: in_fh,
            out_fh: out_fh,
            is_synchronized: true,
            num_commands: 0,
        }
    }
    fn dispatch_command(&mut self) {
        let b = self.read_byte();
        match b {
            0  => panic!("'Abort with error' received. Check for synchronization issues."),
            1  => self.command_load_rom(),
            2  => self.command_step_frame(),
            3  => self.command_render_frame(),
            4  => self.command_set_inputs(),
            5  => self.command_save_state(),
            6  => self.command_load_state(),
            7  => self.command_get_info(),
            8  => self.command_step(),
            9  => self.command_save_tas(),
            10  => self.command_peek(),
            11 => self.command_poke(),
            _ => panic!("Unknown command {}", b),
        }
        self.num_commands += 1;
        if self.is_synchronized {
            let x = (self.num_commands % 256) as u8;
            x.save(&mut self.out_fh);
        }
    }

    fn command_load_rom(&mut self) {
        let mut record_tas = self.read_byte();
        let filename = self.read_length_string();
        let mut joystick1 = Box::new(Joystick::new());
        let mut joystick2 = Box::new(Joystick::new());
        self.joystick1 = &mut *joystick1;
        self.joystick2 = &mut *joystick2;
        match read_ines(filename.clone()) {
            Ok(ines) => {
                let mut nes = load_ines(ines, joystick1, joystick2);
                nes.apu.is_recording = false; // TODO - Expose some way to retrieve recorded sound
                self.nes = Some(Box::new(nes));
            }
            x@Err{..} => panic!("Error loading rom file {:?} - {:?}", filename, x),
        }
    }
    fn command_step_frame(&mut self) {
        self.nes.as_mut().unwrap().run_frame();
    }
    fn command_render_frame(&mut self) {
        let render_style = self.read_byte();
        let bytes:Vec<u8> = match render_style {
            0 => self.nes.as_ref().unwrap().ppu.display.to_vec(),
            1 => self.nes.as_ref().unwrap().ppu.render().to_vec(),
            _ => panic!("Unknown render style {}", render_style),
        };
        self.out_fh.write(&bytes);
    }
    fn command_set_inputs(&mut self) {
        self.read_byte();
        let button_mask = self.read_byte();
        unsafe { (*self.joystick1).set_buttons(button_mask) };
    }
    fn command_save_state(&mut self) {
        let filename = self.read_length_string();
        let mut file = File::create(filename).unwrap();
        self.nes.as_ref().unwrap().save(&mut file);
    }
    fn command_load_state(&mut self) {
        let filename = self.read_length_string();
        let mut file = File::open(filename).unwrap();
        self.nes.as_mut().unwrap().load(&mut file);
    }
    fn command_get_info(&mut self) {
        panic!("Unimplemented");
    }
    fn command_step(&mut self) {
        self.nes.as_mut().unwrap().clock();
    }
    fn command_save_tas(&mut self) {
        panic!("Unimplemented");
    }
    fn command_peek(&mut self) {
        let ptr = self.read_value::<u16>();
        self.nes.as_ref().unwrap().cpu.peek(ptr);
    }
    fn command_poke(&mut self) {
        let ptr = self.read_value::<u16>();
        let v = self.read_byte();
        self.nes.as_mut().unwrap().cpu.poke(ptr, v)
    }

    fn read_value<T:Default + Savable + Debug>(&mut self) -> T {
        let x = read_value::<T>(&mut self.in_fh);
        x
    }
    fn read_byte(&mut self) -> u8 {
        self.read_value::<u8>()
    }
    fn read_length_string(&mut self) -> String {
        let len:usize = self.read_value::<u32>() as usize;
        let mut data:Vec<u8> = vec!(0; len);
        for i in 0..len {
            data[i] = self.read_byte();
        }
        return String::from_utf8(data).unwrap();
    }
}
