#![allow(unused_imports)]
#![allow(non_camel_case_types)]

use crate::common::*;
use crate::mapper::*;
use crate::c6502::C6502;
use crate::apu::Apu;
use crate::ppu::Ppu;
use crate::ppu::PpuPort;
use crate::ppu::PpuPort::*;
use crate::apu::ApuPort::*;
use crate::mapper::{Mapper, Ram};

use std::fs::File;
use std::io::Read;
use std::io;

pub struct Nes {
    cpu: C6502,
    apu: Apu,
    ppu: Ppu,
}

impl Nes {
    fn new() -> Nes {
        return Nes {
            cpu: C6502::new(),
            apu: Apu::new(),
            ppu: Ppu::new(),
        };
    }
}

pub struct Joystick {
}

impl Joystick {
    pub fn new() -> Joystick {
        Joystick {
        }
    }
}

impl AddressSpace for Joystick {
    fn peek(&self, _ptr:u16) -> u8 {
        return 0; // TODO - Implement joystick
    }
    fn poke(&mut self, _ptr:u16, _v:u8) { }
}

pub struct Ines {
    num_prg_chunks: u8,
    num_chr_chunks: u8,
    mapper: u8,
    mirroring: bool,
    has_battery_backed_ram: bool,
    has_trainer: bool,
    has_four_screen_vram: bool,
    is_vs_unisystem: bool,
    is_playchoice10: bool,
    prg_rom: Vec<u8>,
}

pub fn read_ines(filename: String) -> Result<Ines, io::Error> {
    // https://wiki.nesdev.com/w/index.php/INES
    let mut file = File::open(filename)?;
    // Header
    let mut header:[u8;16] = [0; 16];
    file.read_exact(&mut header)?;
    assert!(header[0] == 0x4e);
    assert!(header[1] == 0x45);
    assert!(header[2] == 0x53);
    assert!(header[3] == 0x1a);
    let num_prg_chunks = header[4];
    let mut prg_rom:Vec<u8> = Vec::new();
    for _i in 0 .. num_prg_chunks {
        let mut bf:Vec<u8> = vec!(0; 16384);
        file.read_exact(&mut bf)?;
        prg_rom.append(&mut bf);
    }
    return Ok(Ines {
        num_prg_chunks: num_prg_chunks,
        num_chr_chunks: header[5],
        mirroring: get_bit(header[6], 0) > 0,
        has_battery_backed_ram: get_bit(header[6], 1) > 0,
        has_trainer: get_bit(header[6], 2) > 0,
        has_four_screen_vram: get_bit(header[6], 3) > 0,
        is_playchoice10: false, // TODO
        is_vs_unisystem: false, // TODO
        mapper: (header[6] >> 4) + (header[7] >> 4) << 4,
        prg_rom: prg_rom,
    });
}

pub fn load_ines(rom: Ines, joystick1: Box<Joystick>, joystick2: Box<Joystick>) -> Nes {
    assert!(rom.mapper == 0);
    let cartridge = Rom::new(rom.prg_rom);
    let mut ret = Nes::new();
    ret.map_nes_cpu(joystick1, joystick2, Box::new(cartridge));
    return ret;
}
struct CpuPpuInterconnect {
    ppu: *mut Ppu,
}

fn map_ppu_port(ptr: u16) -> Option<PpuPort> {
    match ptr {
        0x2000 => Some(PPUCTRL),
        0x2001 => Some(PPUMASK),
        0x2002 => Some(PPUSTATUS),
        0x2003 => Some(OAMADDR),
        0x2004 => Some(OAMDATA),
        0x2005 => Some(PPUSCROLL),
        0x2006 => Some(PPUADDR),
        0x2007 => Some(PPUDATA),
        0x4014 => Some(OAMDMA),
        _      => None
    }
}

impl AddressSpace for CpuPpuInterconnect {
    fn peek(&self, ptr:u16) -> u8 {
        unsafe {
            match map_ppu_port(ptr) {
                Some(PPUCTRL) => (*self.ppu).control,
                port => panic!("Unimplemented PPU Port {:?}", port),
            }
        }
    }
    fn poke(&mut self, ptr:u16, value:u8) {
        unsafe {
            match map_ppu_port(ptr) {
                Some(PPUCTRL) => (*self.ppu).control = value,
                port => panic!("Unimplemented PPU Port {:?}", port),
            }
        }
    }
}

impl Nes {
    pub fn run_frame(&mut self, buffer: &mut [u8]) {
        run_clocks(self, 29780);
        self.ppu.render(buffer);
    }
    fn map_nes_cpu(&mut self, joystick1: Box<Joystick>, joystick2: Box<Joystick>, cartridge: Box<AddressSpace>) {
        let mut mapper:Mapper = Mapper::new();
        let cpu_ram:Ram = Ram::new(0x800);
        let cpu_ppu:CpuPpuInterconnect = CpuPpuInterconnect { ppu: &mut self.ppu as *mut Ppu };
        // https://wiki.nesdev.com/w/index.php/CPU_memory_map
        mapper.map_mirrored(0x0000, 0x07ff, 0x0000, 0x1fff, Box::new(cpu_ram), false);
        mapper.map_mirrored(0x2000, 0x2007, 0x2000, 0x3fff, Box::new(cpu_ppu), true);
        // TODO - OAMDMA should initiate a memory transfer
        mapper.map_null(0x4000, 0x4015); // APU/Joystick ports
        mapper.map_address_space(0x4016, 0x4016, joystick1, false);
        mapper.map_address_space(0x4017, 0x4017, joystick2, false);

        mapper.map_null(0x4018, 0x401F); // APU test mode
        mapper.map_address_space(0x4020, 0xFFFF, cartridge, true);
    }
}

impl Clocked for Nes {
    fn clock(&mut self) {
        self.cpu.clock();
        //for i in 1..3 { self.ppu.clock(); }
    }
}

// https://wiki.nesdev.com/w/index.php/2A03
