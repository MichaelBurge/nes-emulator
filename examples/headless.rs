#![allow(unused_must_use)]
#![cfg(unix)]

use clap::Clap;
use core::ptr::null_mut;
use std::{
    fmt::Debug,
    fs::File,
    io::{Read, Write},
    os::unix::{
        io::{AsRawFd, FromRawFd},
        net::UnixListener,
    },
    path::{Path, PathBuf},
};

use nes_emulator::{
    common::Clocked,
    joystick::Joystick,
    mapper::AddressSpace,
    nes::{load_ines, read_ines, Nes},
    serialization::{read_value, Savable},
};

#[derive(Clap)]
struct Opts {
    socket: Option<PathBuf>,
}

fn main() {
    let opts = Opts::parse();
    match opts.socket {
        None => {
            // Standard stdout() object is line-buffered
            let stdin = unsafe { File::from_raw_fd(0) };
            let stdout = unsafe { File::from_raw_fd(1) };
            let mut headless = Headless::new(Box::new(stdin), Box::new(stdout));
            loop {
                headless.dispatch_command()
            }
        }
        Some(ref path) => {
            let path = Path::new(path);
            if path.exists() {
                std::fs::remove_file(&path).expect(&*format!(
                    "Unable to clean up old domain socket at {:?}",
                    path
                ));
            }
            let listener = UnixListener::bind(path).expect(&*format!(
                "Unable to connect to domain socket at {:?}",
                path
            ));
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let fd = stream.as_raw_fd();
                        // Duplicate the socket for both read and write.
                        let stdin = unsafe { File::from_raw_fd(fd) };
                        let stdout = unsafe { File::from_raw_fd(fd) };
                        let mut headless = Headless::new(Box::new(stdin), Box::new(stdout));
                        loop {
                            headless.dispatch_command()
                        }
                    }
                    Err(err) => {
                        panic!("Error: {:?}", err);
                    }
                }
            }
        }
    };
}

struct Headless {
    joystick1: *mut Joystick,
    joystick2: *mut Joystick,
    nes: Option<Box<Nes>>,
    in_fh: Box<dyn Read>,
    out_fh: Box<dyn Write>,
    is_synchronized: bool,
    num_commands: u64,
    is_rendering: bool,
}

impl Headless {
    pub fn new(in_fh: Box<dyn Read>, out_fh: Box<dyn Write>) -> Headless {
        let nes = None;
        Headless {
            joystick1: null_mut(),
            joystick2: null_mut(),
            nes: nes,
            in_fh: in_fh,
            out_fh: out_fh,
            is_synchronized: true,
            num_commands: 0,
            is_rendering: true,
        }
    }
    fn dispatch_command(&mut self) {
        let b = self.read_byte();
        match b {
            0 => panic!("'Abort with error' received. Check for synchronization issues."),
            1 => self.command_load_rom(),
            2 => self.command_step_frame(),
            3 => self.command_render_frame(),
            4 => self.command_set_inputs(),
            5 => self.command_save_state(),
            6 => self.command_load_state(),
            7 => self.command_get_info(),
            8 => self.command_step(),
            9 => self.command_save_tas(),
            10 => self.command_peek(),
            11 => self.command_poke(),
            12 => self.command_set_rendering(),
            _ => panic!("Unknown command {}", b),
        }
        self.num_commands += 1;
        if self.is_synchronized {
            let x = (self.num_commands % 256) as u8;
            x.save(&mut self.out_fh);
        }
    }

    fn command_load_rom(&mut self) {
        let _record_tas = self.read_byte();
        let filename = self.read_value::<String>();
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
            x @ Err { .. } => panic!("Error loading rom file {:?} - {:?}", filename, x),
        }
    }
    fn command_step_frame(&mut self) {
        if self.is_rendering {
            self.nes.as_mut().unwrap().run_frame();
        } else {
            self.nes.as_mut().unwrap().run_frame_headless();
        }
    }
    fn command_render_frame(&mut self) {
        let render_style = self.read_byte();
        let bytes: Vec<u8> = match render_style {
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
        let filename = self.read_value::<String>();
        let mut file = File::create(filename).unwrap();
        self.nes.as_ref().unwrap().save(&mut file);
    }
    fn command_load_state(&mut self) {
        let filename = self.read_value::<String>();
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
        let result = self.nes.as_ref().unwrap().cpu.peek(ptr);
        let mut out_fh = &mut self.out_fh;
        result.save(&mut out_fh);
    }
    fn command_poke(&mut self) {
        let ptr = self.read_value::<u16>();
        let v = self.read_byte();
        self.nes.as_mut().unwrap().cpu.poke(ptr, v)
    }
    fn command_set_rendering(&mut self) {
        let is_rendering = self.read_byte();
        self.is_rendering = is_rendering > 0;
    }

    fn read_value<T: Default + Savable + Debug>(&mut self) -> T {
        let x = read_value::<T>(&mut self.in_fh);
        x
    }
    fn read_byte(&mut self) -> u8 {
        self.read_value::<u8>()
    }
}
