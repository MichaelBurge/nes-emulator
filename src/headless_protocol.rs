use crate::serialization::Savable;
use log::{debug, error, trace};
use std::io::{BufWriter, Read, Write};

#[cfg(unix)]
use std::{
    ffi::OsStr,
    fs::File,
    net::{TcpStream, ToSocketAddrs},
    os::unix::net::UnixStream,
    path::Path,
};

#[derive(Debug, Clone)]
pub enum Command {
    LoadRom(bool, String),
    StepFrame,
    RenderFrame(RenderStyle),
    SetInputs(u8, u8),
    SaveState(String),
    LoadState(String),
    GetInfo,
    Step,
    SaveTas,
    Peek(u16),
    Poke(u16, u8),
    SetRendering(bool),
}

impl Default for Command {
    fn default() -> Self {
        StepFrame
    }
}

use Command::*;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum RenderStyle {
    Plain = 0,
    Rgb = 1,
}

impl Savable for Option<Command> {
    fn save(&self, fh: &mut dyn Write) {
        trace!("Sending command: {:?}", self);
        let mut fh = &mut BufWriter::new(fh);
        match self.clone().expect("Empty command received") {
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
            SetInputs(controller_id, inputs) => {
                write_byte(fh, 4);
                write_byte(fh, controller_id);
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
        };
        fh.flush().expect("Unable to flush buffer");
    }
    fn load(&mut self, fh: &mut dyn Read) {
        let command = read_byte(fh);
        *self = Some(match command {
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
            4 => SetInputs(read_value::<u8>(fh), read_value::<u8>(fh)),
            5 => SaveState(read_value::<String>(fh)),
            6 => LoadState(read_value::<String>(fh)),
            7 => GetInfo,
            8 => Step,
            9 => SaveTas,
            10 => Peek(read_value::<u16>(fh)),
            11 => Poke(read_value::<u16>(fh), read_value::<u8>(fh)),
            12 => SetRendering(read_value::<bool>(fh)),
            x => {
                error!("Received command {}. Probably a sync error", x);
                *self = None;
                return;
            }
        })
    }
}
impl Savable for Command {
    fn save(&self, fh: &mut dyn Write) {
        Some(self.clone()).save(fh)
    }
    fn load(&mut self, fh: &mut dyn Read) {
        *self = read_value::<Option<Command>>(fh).expect("Unable to read command");
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
    r.read_exact(&mut bytes)
        .expect(&*format!("Unable to read {} bytes", num_bytes));
    bytes
}

fn read_value<T: Savable + Default>(r: &mut dyn Read) -> T {
    let mut t = T::default();
    t.load(r);
    t
}

pub struct SocketHeadlessClient(Box<dyn ReadWrite>);
impl SocketHeadlessClient {
    pub fn new<T: ReadWrite + 'static>(t: T) -> Self {
        SocketHeadlessClient(Box::new(t))
    }
    pub fn load_rom(&mut self, save_tas: bool, filename: String) {
        LoadRom(save_tas, filename).save(&mut self.0);
        self.sync();
    }
    pub fn step_frame(&mut self) {
        StepFrame.save(&mut self.0);
        self.sync();
    }
    pub fn render_frame(&mut self, render_style: RenderStyle) -> Vec<u8> {
        RenderFrame(render_style).save(&mut self.0);
        let bytes = match render_style as u8 {
            0 => read_bytes(&mut self.0, crate::ppu::UNRENDER_SIZE),
            1 => read_bytes(&mut self.0, crate::ppu::RENDER_SIZE),
            x => panic!("Unknown render style {:?}", x),
        };
        self.sync();
        bytes
    }
    pub fn set_inputs(&mut self, controller_id: u8, inputs: u8) {
        SetInputs(controller_id, inputs).save(&mut self.0);
        self.sync();
    }
    pub fn save_state(&mut self, filename: String) {
        SaveState(filename).save(&mut self.0);
        self.sync();
    }
    pub fn load_state(&mut self, filename: String) {
        LoadState(filename).save(&mut self.0);
        self.sync();
    }
    pub fn get_info(&mut self) {
        GetInfo.save(&mut self.0);
        self.sync();
    }
    pub fn step(&mut self) {
        Step.save(&mut self.0);
        self.sync();
    }
    pub fn save_tas(&mut self) {
        SaveTas.save(&mut self.0);
        self.sync();
    }
    pub fn peek(&mut self, address: u16) -> u8 {
        Peek(address).save(&mut self.0);
        let x = read_value::<u8>(&mut self.0);
        self.sync();
        trace!("Peek({})={}", address, x);
        x
    }
    pub fn poke(&mut self, address: u16, value: u8) {
        Poke(address, value).save(&mut self.0);
        self.sync();
    }
    pub fn set_rendering(&mut self, is_rendering: bool) {
        SetRendering(is_rendering).save(&mut self.0);
        self.sync();
    }
    fn sync(&mut self) {
        let byte = read_value::<u8>(&mut self.0);
        trace!("sync={}", byte);
    }
}

#[allow(dead_code)]
pub fn connect_tcp(host: &str) -> SocketHeadlessClient {
    let stream = TcpStream::connect(host).expect(&*format!(
        "Unable to connect to {:?}",
        host.to_socket_addrs()
    ));
    SocketHeadlessClient::new(stream)
}

pub fn connect_socket<P: AsRef<Path>>(p: P) -> SocketHeadlessClient {
    let stream = UnixStream::connect(p.as_ref()).expect(&*format!(
        "Unable to connect to domain socket at {:?}",
        p.as_ref()
    ));
    SocketHeadlessClient::new(stream)
}

pub trait ReadWrite: Read + Write + Send + Sync {}
pub struct StdInOut(pub File, pub File);
impl Read for StdInOut {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}
impl Write for StdInOut {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.1.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.1.flush()
    }
}

impl ReadWrite for StdInOut {}
impl ReadWrite for TcpStream {}
impl ReadWrite for UnixStream {}
