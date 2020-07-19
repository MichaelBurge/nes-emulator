use crate::serialization::Savable;
use std::io::{Read, Write};

#[cfg(unix)]
use std::{
    fs::File,
    os::unix::{
        io::{AsRawFd, FromRawFd},
        net::UnixStream,
    },
    path::Path,
};

const COMMAND_LOAD_ROM: u8 = 1;
const COMMAND_STEP_FRAME: u8 = 2;
const COMMAND_RENDER_FRAME: u8 = 3;
const COMMAND_SET_INPUTS: u8 = 4;
const COMMAND_SAVE_STATE: u8 = 5;
const COMMAND_LOAD_STATE: u8 = 6;
const COMMAND_GET_INFO: u8 = 7;
const COMMAND_STEP: u8 = 8;
const COMMAND_SAVE_TAS: u8 = 9;
const COMMAND_PEEK: u8 = 10;
const COMMAND_POKE: u8 = 11;
const COMMAND_SET_RENDERING: u8 = 12;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum RenderStyle {
    Plain = 0,
    Rgb = 1,
}

pub trait THeadlessClient {
    fn load_rom(&mut self, record_tas: bool, filename: &String);
    fn step_frame(&mut self);
    fn render_frame(&mut self, render_style: RenderStyle) -> Vec<u8>;
    fn set_inputs(&mut self, inputs: u8);
    fn save_state(&mut self, filename: &String);
    fn load_state(&mut self, filename: &String);
    fn get_info(&mut self);
    fn step(&mut self);
    fn save_tas(&mut self);
    fn peek(&mut self, address: u16) -> u8;
    fn poke(&mut self, address: u16, value: u8);
    fn set_rendering(&mut self, is_rendering: bool);
}

pub struct HeadlessClient<R: Read, W: Write> {
    r: R,
    w: W,
}
impl<R: Read, W: Write> THeadlessClient for HeadlessClient<R, W> {
    fn load_rom(&mut self, record_tas: bool, filename: &String) {
        self.write_byte(COMMAND_LOAD_ROM);
        self.write_byte(if record_tas { 1 } else { 0 });
        self.write_string(filename);
    }
    fn step_frame(&mut self) {
        self.write_byte(COMMAND_STEP_FRAME);
    }
    fn render_frame(&mut self, render_style: RenderStyle) -> Vec<u8> {
        self.write_byte(COMMAND_RENDER_FRAME);
        self.write_byte(render_style as u8);
        match render_style as u8 {
            0 => self.read_bytes(crate::ppu::UNRENDER_SIZE),
            1 => self.read_bytes(crate::ppu::RENDER_SIZE),
            x => panic!("Unknown render style {:?}", x),
        }
    }
    fn set_inputs(&mut self, inputs: u8) {
        self.write_byte(COMMAND_SET_INPUTS);
        self.write_byte(inputs);
    }
    fn save_state(&mut self, filename: &String) {
        self.write_byte(COMMAND_SAVE_STATE);
        self.write_string(filename);
    }
    fn load_state(&mut self, filename: &String) {
        self.write_byte(COMMAND_LOAD_STATE);
        self.write_string(filename);
    }
    fn get_info(&mut self) {
        self.write_byte(COMMAND_GET_INFO);
    }
    fn step(&mut self) {
        self.write_byte(COMMAND_STEP);
    }
    fn save_tas(&mut self) {
        self.write_byte(COMMAND_SAVE_TAS);
    }
    fn peek(&mut self, address: u16) -> u8 {
        self.write_byte(COMMAND_PEEK);
        self.write_value(address);
        self.read_byte()
    }
    fn poke(&mut self, address: u16, value: u8) {
        self.write_byte(COMMAND_POKE);
        self.write_value(address);
        self.write_byte(value);
    }
    fn set_rendering(&mut self, is_rendering: bool) {
        self.write_byte(COMMAND_SET_RENDERING);
        self.write_value(is_rendering);
    }
}

impl<R: Read, W: Write> HeadlessClient<R, W> {
    fn write_byte(&mut self, byte: u8) {
        byte.save(&mut self.w);
    }
    fn write_value<T: Savable>(&mut self, t: T) {
        t.save(&mut self.w);
    }
    fn write_string(&mut self, x: &String) {
        self.write_value::<u32>(x.len() as u32);
        for byte in x.as_bytes() {
            self.write_byte(*byte);
        }
    }
    fn read_byte(&mut self) -> u8 {
        let mut x: u8 = 0;
        x.load(&mut self.r);
        x
    }
    fn read_bytes(&mut self, num_bytes: usize) -> Vec<u8> {
        let mut bytes = vec![0; num_bytes];
        bytes.load(&mut self.r);
        bytes
    }
    #[cfg(unix)]
    #[allow(dead_code)]
    pub fn connect_socket<P: AsRef<Path>>(filename: P) -> SocketHeadlessClient {
        let stream = UnixStream::connect(filename.as_ref()).expect(&*format!(
            "Unable to connect to unix domain socket at {:?}",
            filename.as_ref()
        ));
        let fd = stream.as_raw_fd();
        let fd_out = unsafe { File::from_raw_fd(fd) };
        HeadlessClient {
            r: stream,
            w: fd_out,
        }
    }
}

pub type SocketHeadlessClient = HeadlessClient<UnixStream, File>;
