#![allow(unused_imports)]
#![allow(non_camel_case_types)]

use crate::common::*;
use crate::mapper::*;
use crate::c6502::C6502;
use crate::apu::Apu;
use crate::ppu::Ppu;
use crate::ppu::CpuPpuInterconnect;
use crate::ppu::PpuPort;
use crate::ppu::PpuPort::*;
use crate::apu::ApuPort::*;
use crate::mapper::{Mapper, Ram};

use std::fs::File;
use std::io::Read;
use std::io;
use std::fmt;
use std::ops::DerefMut;

pub struct Nes {
    cpu: C6502,
    apu: Apu,
    pub ppu: Box<Ppu>,
}

impl Nes {
    fn new(cpu_mapper: Box<AddressSpace>) -> Nes {
        return Nes {
            cpu: C6502::new(cpu_mapper),
            apu: Apu::new(),
            ppu: Box::new(Ppu::new()),
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

struct HiddenBytes(Vec<u8>);

impl fmt::Debug for HiddenBytes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let HiddenBytes(vec) = self;
        write!(f, "[<<vector of length {}>>]", vec.len())
    }
}

#[derive(Debug)]
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
    prg_rom: HiddenBytes,
    chr_rom: HiddenBytes,
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
    let num_chr_chunks = header[5];
    let mut prg_rom:Vec<u8> = Vec::new();
    for _i in 0 .. num_prg_chunks {
        let mut bf:Vec<u8> = vec!(0; 16384);
        file.read_exact(&mut bf)?;
        prg_rom.append(&mut bf);
    }
    let mut chr_rom:Vec<u8> = Vec::new();
    for _i in 0 .. num_chr_chunks {
        let mut bf:Vec<u8> = vec!(0; 8192);
        file.read_exact(&mut bf)?;
        chr_rom.append(&mut bf);
    }
    let ret = Ines {
        num_prg_chunks: num_prg_chunks,
        num_chr_chunks: header[5],
        mirroring: get_bit(header[6], 0) > 0,
        has_battery_backed_ram: get_bit(header[6], 1) > 0,
        has_trainer: get_bit(header[6], 2) > 0,
        has_four_screen_vram: get_bit(header[6], 3) > 0,
        is_playchoice10: false, // TODO
        is_vs_unisystem: false, // TODO
        mapper: (header[6] >> 4) + (header[7] >> 4) << 4,
        prg_rom: HiddenBytes(prg_rom),
        chr_rom: HiddenBytes(chr_rom),
    };
    // eprintln!("DEBUG - INES LOADED - {:?}", ret);
    return Ok(ret);
}

pub fn load_ines(rom: Ines, joystick1: Box<Joystick>, joystick2: Box<Joystick>) -> Nes {
    if rom.mapper != 0 {
        panic!("Only mapper 0 supported. Found {}", rom.mapper);
    }
    let cpu_mapper:Mapper = {
        let HiddenBytes(bytes) = rom.prg_rom;
        let cartridge = Rom::new(bytes);
        let mut mapper = Mapper::new();
        match rom.num_prg_chunks {
            1 => { mapper.map_mirrored(0x0000, 0x3FFF, 0x8000, 0xFFFF, Box::new(cartridge), true) },
            2 => { mapper.map_mirrored(0x0000, 0x7FFF, 0x8000, 0xFFFF, Box::new(cartridge), true) },
            _ => panic!("load_ines - Unexpected number of PRG chunks"),
        };
        mapper
    };
    let ppu_mapper:Rom = {
        let HiddenBytes(bytes) = rom.chr_rom;
        let cartridge_ppu = Rom::new(bytes);
        cartridge_ppu
    };
    let mut ret = Nes::new(Box::new(NullAddressSpace::new()));
    ret.map_nes_cpu(joystick1, joystick2, Box::new(cpu_mapper));
    ret.map_nes_ppu(Box::new(ppu_mapper));
    return ret;
}

impl Nes {
    pub fn run_frame(&mut self) {
        run_clocks(self, 29780);
    }
    fn map_nes_cpu(&mut self, joystick1: Box<Joystick>, joystick2: Box<Joystick>, cartridge: Box<AddressSpace>) {
        let mut mapper:Mapper = Mapper::new();
        let cpu_ram:Ram = Ram::new(0x800);
        let cpu_ppu:CpuPpuInterconnect = CpuPpuInterconnect::new(self.ppu.deref_mut());
        // https://wiki.nesdev.com/w/index.php/CPU_memory_map
        mapper.map_mirrored(0x0000, 0x07ff, 0x0000, 0x1fff, Box::new(cpu_ram), false);
        mapper.map_mirrored(0x2000, 0x2007, 0x2000, 0x3fff, Box::new(cpu_ppu), true);
        // TODO - OAMDMA should initiate a memory transfer
        mapper.map_null(0x4000, 0x4015); // APU/Joystick ports
        mapper.map_address_space(0x4016, 0x4016, joystick1, false);
        mapper.map_address_space(0x4017, 0x4017, joystick2, false);

        mapper.map_null(0x4018, 0x401F); // APU test mode
        mapper.map_address_space(0x4020, 0xFFFF, cartridge, true);
        self.cpu.mapper = Box::new(mapper);
        self.cpu.initialize();
    }
    fn map_nes_ppu(&mut self, cartridge_ppu: Box<AddressSpace>) {
        // https://wiki.nesdev.com/w/index.php/PPU_memory_map
        let mut mapper:Mapper = Mapper::new();
        let ppu_ram:Ram = Ram::new(0x800);
        let palette_ram:Ram = Ram::new(0x20);
        // Pattern table
        mapper.map_address_space(0x0000, 0x1FFF, cartridge_ppu, true);
        // Nametables
        mapper.map_mirrored(0x2000, 0x27FF, 0x2000, 0x3EFF, Box::new(ppu_ram), false);
        mapper.map_mirrored(0x3f00, 0x3f1f, 0x3f00, 0x3fff, Box::new(palette_ram), false);

        self.ppu.mapper = Box::new(mapper);
    }
}

impl Clocked for Nes {
    fn clock(&mut self) {
        self.cpu.clock();
        for _i in 1..3 { self.ppu.clock(); }
        if self.ppu.is_vblank_nmi {
            eprintln!("DEBUG - VBLANK-NMI DETECTED");
            self.cpu.nmi();
            self.ppu.is_vblank_nmi = false;
        } else if self.ppu.is_scanline_irq {
            self.cpu.irq();
            self.ppu.is_scanline_irq = false;
        }
    }
}

// https://wiki.nesdev.com/w/index.php/2A03
