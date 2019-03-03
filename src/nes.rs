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
use crate::ppu::PaletteControl;
use crate::apu::ApuPort::*;
use crate::mapper::{Mapper, Ram};
use crate::joystick::Joystick;

use crate::globals::*;

use core::ops::DerefMut;
use core::ptr::null_mut;
use core::convert::AsMut;

pub struct Nes {
    pub cpu: &'static mut C6502,
    pub apu: &'static mut Apu,
    pub ppu: &'static mut Ppu,
}

impl Nes {
    fn new(cpu: &'static mut C6502, ppu: &'static mut Ppu, apu: &'static mut Apu) -> Nes {
        return Nes {
            cpu: cpu,
            apu: apu,
            ppu: ppu,
        };
    }
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
    prg_rom: [u8; 1024*16],
    chr_rom: [u8; 1024*16],
}

struct HiddenBytes([u8; 1024*16]);

pub fn read_ines() -> Result<Ines, ()> {
    let ret = Ines {
        num_prg_chunks: 0,
        num_chr_chunks: 0,
        mirroring: false,
        has_battery_backed_ram: false,
        has_trainer: false,
        has_four_screen_vram: false,
        is_playchoice10: false,
        is_vs_unisystem: false,
        mapper: 0,
        prg_rom: [0; 1024*16],
        chr_rom: [0; 1024*16],
    };
    // // https://wiki.nesdev.com/w/index.php/INES
    // let mut file = File::open(filename)?;
    // // Header
    // let mut header:[u8;16] = [0; 16];
    // file.read_exact(&mut header)?;
    // assert!(header[0] == 0x4e);
    // assert!(header[1] == 0x45);
    // assert!(header[2] == 0x53);
    // assert!(header[3] == 0x1a);
    // let num_prg_chunks = header[4];
    // let num_chr_chunks = header[5];
    // let mut prg_rom:Vec<u8> = Vec::new();
    // for _i in 0 .. num_prg_chunks {
    //     let mut bf:Vec<u8> = vec!(0; 16384);
    //     file.read_exact(&mut bf)?;
    //     prg_rom.append(&mut bf);
    // }
    // let mut chr_rom:Vec<u8> = Vec::new();
    // for _i in 0 .. num_chr_chunks {
    //     let mut bf:Vec<u8> = vec!(0; 8192);
    //     file.read_exact(&mut bf)?;
    //     chr_rom.append(&mut bf);
    // }
    // let ret = Ines {
    //     num_prg_chunks: num_prg_chunks,
    //     num_chr_chunks: header[5],
    //     mirroring: get_bit(header[6], 0) > 0,
    //     has_battery_backed_ram: get_bit(header[6], 1) > 0,
    //     has_trainer: get_bit(header[6], 2) > 0,
    //     has_four_screen_vram: get_bit(header[6], 3) > 0,
    //     is_playchoice10: false, // TODO
    //     is_vs_unisystem: false, // TODO
    //     mapper: (header[6] >> 4) + (header[7] >> 4) << 4,
    //     prg_rom: HiddenBytes(prg_rom),
    //     chr_rom: HiddenBytes(chr_rom),
    // };
    // // eprintln!("DEBUG - INES LOADED - {:?}", ret);
    return Ok(ret);
}

pub fn load_ines(rom: Ines, joystick1: *mut AddressSpace, joystick2: *mut AddressSpace) -> Nes {
    if rom.mapper != 0 {
        panic!("Only mapper 0 supported. Found {}", rom.mapper);
    }
    let cpu_cartridge_mapper:&'static mut Mapper = {
        let cartridge = unsafe {
            THE_CPU_CARTRIDGE_BYTES = rom.prg_rom;
            THE_CPU_CARTRIDGE_ROM = Some(Rom::new(&mut THE_CPU_CARTRIDGE_BYTES));
            THE_CPU_CARTRIDGE_ROM.as_mut().unwrap() as *mut Rom
        };
        let mut mapper = Mapper::new();
        match rom.num_prg_chunks {
            1 => { mapper.map_mirrored(0x0000, 0x3FFF, 0x8000, 0xFFFF, cartridge, true) },
            2 => { mapper.map_mirrored(0x0000, 0x7FFF, 0x8000, 0xFFFF, cartridge, true) },
            _ => panic!("load_ines - Unexpected number of PRG chunks"),
        };
        unsafe {
            THE_CPU_CARTRIDGE_MAPPER = Some(mapper);
            THE_CPU_CARTRIDGE_MAPPER.as_mut().unwrap()
        }
    };
    unsafe {
        THE_APU = Some(Apu::new());
        THE_CPU_MAPPER = Some(Mapper::new());
        THE_CPU = Some(C6502::new(THE_CPU_MAPPER.as_mut().unwrap()));
        Nes::map_nes_cpu(THE_CPU_MAPPER.as_mut().unwrap(),
                         joystick1,
                         joystick2,
                         cpu_cartridge_mapper,
                         THE_APU.as_mut().unwrap(),
                         &mut THE_PPU_RAM,
        );

        THE_PPU_MAPPER = Some(Mapper::new());
        THE_PPU = Some(Ppu::new(THE_PPU_MAPPER.as_mut().unwrap()));
        Nes::map_nes_ppu(THE_PPU_MAPPER.as_mut().unwrap(), &mut THE_PPU_CARTRIDGE_ROM);
        let ret = Nes::new(THE_CPU.as_mut().unwrap(),
                           THE_PPU.as_mut().unwrap(),
                           THE_APU.as_mut().unwrap(),
        );
        ret.cpu.initialize();
        return ret;
    }
}

impl Nes {
    pub fn run_frame(&mut self) {
        run_clocks(self, 29780);
    }
    pub fn break_debugger(&mut self) {
        self.cpu.break_debugger();
    }
    pub fn map_nes_cpu(mapper:&mut Mapper, joystick1: *mut AddressSpace, _joystick2: *mut AddressSpace, cartridge: *mut AddressSpace, apu: *mut Apu, cpu_ram: *mut Ram) {
        // https://wiki.nesdev.com/w/index.php/CPU_memory_map
        mapper.map_mirrored(0x0000, 0x07ff, 0x0000, 0x1fff, cpu_ram, false);
        mapper.map_mirrored(0x2000, 0x2007, 0x2000, 0x3fff, unsafe { &mut THE_CPU_PPU_INTERCONNECT }, true);
        mapper.map_address_space(0x4000, 0x4013, apu, true);
        mapper.map_address_space(0x4014, 0x4014, unsafe { &mut THE_CPU_PPU_INTERCONNECT }, true);
        mapper.map_address_space(0x4015, 0x4015, apu, true);
        mapper.map_address_space(0x4016, 0x4016, joystick1, false);
        mapper.map_address_space(0x4017, 0x4017, apu, true); // TODO - 0x4017 is also mapped to joystick2

        mapper.map_null(0x4018, 0x401F); // APU test mode
        mapper.map_address_space(0x4020, 0xFFFF, cartridge, true);
    }
    pub fn map_nes_ppu(mapper:&mut Mapper, cartridge_ppu: *mut AddressSpace) {
        // https://wiki.nesdev.com/w/index.php/PPU_memory_map
        // Pattern table
        mapper.map_address_space(0x0000, 0x1FFF, cartridge_ppu, true);
        // Nametables
        unsafe {
            mapper.map_mirrored(0x2000, 0x27FF, 0x2000, 0x3EFF, &mut THE_PPU_RAM, false);
            mapper.map_mirrored(0x3f00, 0x3f1f, 0x3f00, 0x3fff, &mut THE_PPU_PALETTE_CONTROL, true);
        }
    }
}

impl Clocked for Nes {
    fn clock(&mut self) {
        self.cpu.clock();
        for _i in 1..3 { self.ppu.clock(); }
        if self.ppu.is_vblank_nmi {
            //eprintln!("DEBUG - VBLANK-NMI DETECTED");
            self.cpu.nmi();
            self.ppu.is_vblank_nmi = false;
        } else if self.ppu.is_scanline_irq {
            self.cpu.irq();
            self.ppu.is_scanline_irq = false;
        }
        self.apu.clock(); // Clocked by SDL callback.
    }
}
