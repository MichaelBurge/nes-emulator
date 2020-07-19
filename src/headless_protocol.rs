use crate::serialization::Savable;
use std::io::{Read, Write};

#[cfg(unix)]
use std::{
    ffi::OsStr,
    fs::File,
    os::unix::{
        io::{AsRawFd, FromRawFd},
        net::UnixStream,
    },
};

#[derive(Debug, Clone)]
pub enum Command {
    LoadRom(bool, String),
    StepFrame,
    RenderFrame(RenderStyle),
    SetInputs(u8),
    SaveState(String),
    LoadState(String),
    GetInfo,
    Step,
    SaveTas,
    Peek(u16),
    Poke(u16, u8),
    SetRendering(bool),
}

use Command::*;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum RenderStyle {
    Plain = 0,
    Rgb = 1,
}

pub fn read_command_response<F: Read>(fh: &mut F, c: &Command) -> Vec<u8> {
    match c {
        LoadRom(_, _) => vec![],
        StepFrame => vec![],
        RenderFrame(render_style) => match *render_style as u8 {
            0 => read_bytes(fh, crate::ppu::UNRENDER_SIZE),
            1 => read_bytes(fh, crate::ppu::RENDER_SIZE),
            x => panic!("Unknown render style {:?}", x),
        },
        SetInputs(_) => vec![],
        SaveState(_) => vec![],
        LoadState(_) => vec![],
        GetInfo => vec![],
        Step => vec![],
        SaveTas => vec![],
        Peek(_) => read_bytes(fh, 1),
        Poke(_, _) => vec![],
        SetRendering(_) => vec![],
    }
}

impl Savable for Command {
    fn save(&self, fh: &mut dyn Write) {
        match self.clone() {
            LoadRom(record_tas, filename) => {
                write_byte(fh, 1);
                write_byte(fh, if record_tas { 1 } else { 0 });
                write_value::<String>(fh, filename);
            }
            StepFrame => {
                write_byte(fh, 2);
            }
            RenderFrame(render_style) => {
                write_byte(fh, 3);
                write_byte(fh, render_style as u8);
            }
            SetInputs(inputs) => {
                write_byte(fh, 4);
                write_byte(fh, inputs);
            }
            SaveState(filename) => {
                write_byte(fh, 5);
                write_value::<String>(fh, filename);
            }
            LoadState(filename) => {
                write_byte(fh, 6);
                write_value::<String>(fh, filename);
            }
            GetInfo => {
                write_byte(fh, 7);
            }
            Step => {
                write_byte(fh, 8);
            }
            SaveTas => {
                write_byte(fh, 9);
            }
            Peek(address) => {
                write_byte(fh, 10);
                write_value(fh, address);
            }
            Poke(address, value) => {
                write_byte(fh, 11);
                write_value(fh, address);
                write_byte(fh, value);
            }
            SetRendering(is_rendering) => {
                write_byte(fh, 12);
                write_value(fh, is_rendering);
            }
        }
    }
    fn load(&mut self, fh: &mut dyn Read) {
        let command = read_byte(fh);
        *self = match command {
            1 => {
                let record_tas = read_value::<bool>(fh);
                let filename = read_value::<String>(fh);
                LoadRom(record_tas, filename)
            }
            2 => StepFrame,
            3 => {
                let style_byte = read_value::<u8>(fh);
                let render_style = unsafe { std::mem::transmute::<u8, RenderStyle>(style_byte) };
                RenderFrame(render_style)
            }
            4 => SetInputs(read_value::<u8>(fh)),
            5 => SaveState(read_value::<String>(fh)),
            6 => LoadState(read_value::<String>(fh)),
            7 => GetInfo,
            8 => Step,
            9 => SaveTas,
            10 => Peek(read_value::<u16>(fh)),
            11 => Poke(read_value::<u16>(fh), read_value::<u8>(fh)),
            12 => SetRendering(read_value::<bool>(fh)),
            x => panic!("Received command {}. Probably a sync error", x),
        };
    }
}

fn write_byte(w: &mut dyn Write, byte: u8) {
    byte.save(w);
}
fn write_value<T: Savable>(w: &mut dyn Write, t: T) {
    t.save(w);
}
fn read_byte(r: &mut dyn Read) -> u8 {
    let mut x: u8 = 0;
    x.load(r);
    x
}
fn read_bytes(r: &mut dyn Read, num_bytes: usize) -> Vec<u8> {
    let mut bytes = vec![0; num_bytes];
    bytes.load(r);
    bytes
}

fn read_value<T: Savable + Default>(r: &mut dyn Read) -> T {
    let mut t = T::default();
    t.load(r);
    t
}

#[cfg(unix)]
#[allow(dead_code)]
pub fn connect_socket<P: AsRef<OsStr>>(filename: P) -> UnixStream {
    let stream = UnixStream::connect(filename.as_ref()).expect(&*format!(
        "Unable to connect to unix domain socket at {:?}",
        filename.as_ref()
    ));
    stream
}

pub type SocketHeadlessClient = UnixStream;
