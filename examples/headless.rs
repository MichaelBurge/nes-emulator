#![cfg(unix)]

use clap::Clap;
use core::ptr::null_mut;
use log::{debug, info, trace};
use nes_emulator::{
    common::Clocked,
    headless_protocol::{
        Command::{self, *},
        ReadWrite, RenderStyle, StdInOut,
    },
    joystick::Joystick,
    mapper::AddressSpace,
    nes::{load_ines, read_ines, Nes},
    serialization::{read_value, Savable},
};
use std::{
    fs::File,
    io::{BufWriter, Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::{
        io::FromRawFd,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
};

#[derive(Clap)]
struct Opts {
    #[clap(long = "host")]
    host: Option<String>,
    #[clap(long = "socket")]
    socket: Option<PathBuf>,
}

fn main() {
    env_logger::init();
    let opts = Opts::parse();
    let command_loop = |mut headless: Headless| loop {
        let command = read_value::<Option<Command>>(&mut headless.fh);
        match command {
            None => break,
            Some(command) => {
                headless.dispatch_command(command);
                headless.emit_sync_byte();
            }
        }
    };
    match opts.host {
        None => {}
        Some(ref host) => {
            let listener = TcpListener::bind(host.clone())
                .expect(&*format!("Unable to connect to host at {:?}", host));
            info!("Listening on {}", host);
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        debug!(
                            "read_timeout={:?} stream={:?}",
                            stream.read_timeout(),
                            stream
                        );
                        let headless = Headless::new(stream);
                        command_loop(headless);
                    }
                    Err(err) => {
                        panic!("Error: {:?}", err);
                    }
                }
            }
        }
    };
    match opts.socket {
        None => {}
        Some(ref socket) => {
            let path = Path::new(socket);
            if path.exists() {
                std::fs::remove_file(&path).expect("Unable to clear existing socket");
            }
            let listener = UnixListener::bind(path)
                .expect(&*format!("Unable to connect to socket at {:?}", path));
            info!("Listening on {:?}", path);
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        debug!(
                            "read_timeout={:?} stream={:?}",
                            stream.read_timeout(),
                            stream
                        );
                        let headless = Headless::new(stream);
                        command_loop(headless);
                    }
                    Err(err) => {
                        panic!("Error: {:?}", err);
                    }
                }
            }
        }
    };
    info!("Defaulting to stdin/stdout for IO");
    // Standard stdout() object is line-buffered
    let stdin = unsafe { File::from_raw_fd(0) };
    let stdout = unsafe { File::from_raw_fd(1) };
    let headless = Headless::new(StdInOut(stdin, stdout));
    command_loop(headless);
}

struct Headless {
    joystick1: *mut Joystick,
    joystick2: *mut Joystick,
    nes: Option<Box<Nes>>,
    fh: Box<dyn ReadWrite>,
    is_synchronized: bool,
    num_commands: u64,
    is_rendering: bool,
}

impl Headless {
    pub fn new<RW: ReadWrite + 'static>(fh: RW) -> Self {
        let nes = None;
        Self {
            joystick1: null_mut(),
            joystick2: null_mut(),
            nes: nes,
            fh: Box::new(fh),
            is_synchronized: true,
            num_commands: 0,
            is_rendering: true,
        }
    }
    fn dispatch_command(&mut self, command: Command) {
        debug!("Received command: {:?}", command);
        match command {
            LoadRom(_, filename) => {
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
            StepFrame => {
                if self.is_rendering {
                    self.nes.as_mut().unwrap().run_frame();
                } else {
                    self.nes.as_mut().unwrap().run_frame_headless();
                }
            }
            RenderFrame(render_style) => {
                let bytes: Vec<u8> = match render_style {
                    RenderStyle::Plain => self.nes.as_ref().unwrap().ppu.display.to_vec(),
                    RenderStyle::Rgb => self.nes.as_ref().unwrap().ppu.render().to_vec(),
                };
                self.fh
                    .write(&bytes)
                    .expect(&*format!("Unable to write bytes for {:?}", render_style));
            }
            SetInputs(controller_id, button_mask) => {
                assert_eq!(
                    controller_id, 0,
                    "Unsupported controller_id {}",
                    controller_id
                );
                unsafe { (*self.joystick1).set_buttons(button_mask) };
            }
            SaveState(filename) => {
                let mut file = File::create(filename).unwrap();
                self.nes.as_ref().unwrap().save(&mut file);
            }
            LoadState(filename) => {
                let mut file = File::open(filename).unwrap();
                self.nes.as_mut().unwrap().load(&mut file);
            }
            GetInfo => panic!("Unimplemented"),
            Step => self.nes.as_mut().unwrap().clock(),
            SaveTas => panic!("Unimplemented"),
            Peek(address) => {
                let result = self.nes.as_ref().unwrap().cpu.peek(address);
                trace!("peek({})={}", address, result);
                result.save(&mut self.fh);
            }
            Poke(address, value) => self.nes.as_mut().unwrap().cpu.poke(address, value),
            SetRendering(is_rendering) => self.is_rendering = is_rendering,
        }
    }

    fn emit_sync_byte(&mut self) {
        self.num_commands += 1;
        if self.is_synchronized {
            let x = (self.num_commands % 256) as u8;
            x.save(&mut self.fh);
        }
    }
}
