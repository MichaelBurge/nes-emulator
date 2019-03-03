use core::ptr::null_mut;

use crate::c6502::C6502;
use crate::mapper::Mapper;
use crate::mapper::Rom;
use crate::mapper::Ram;
use crate::ppu::PaletteControl;
use crate::ppu::Ppu;
use crate::apu::Apu;
use crate::ppu::CpuPpuInterconnect;

pub static mut THE_CPU_RAM:[u8;0x800] = [0; 0x800];
pub static mut THE_CPU_CARTRIDGE_BYTES:[u8; 1024*16] = [0; 1024*16];
pub static mut THE_CPU_CARTRIDGE_ROM:Option<Rom> = None;
pub static mut THE_CPU_CARTRIDGE_MAPPER:Option<Mapper> = None;
pub static mut THE_CPU_MAPPER:Option<Mapper> = None;
pub static mut THE_CPU:Option<C6502> = None;

pub static mut THE_PPU_PALETTE_CONTROL:PaletteControl = PaletteControl { memory: [0; 32 ] };
pub static mut THE_PPU_CARTRIDGE_BYTES:[u8; 1024*16] = [0; 1024*16];
pub static mut THE_PPU_CARTRIDGE_ROM:Rom = Rom { bs: unsafe { &mut THE_PPU_CARTRIDGE_BYTES } };
pub static mut THE_PPU_RAM_BYTES:[u8;0x800] = [0; 0x800];
pub static mut THE_PPU_RAM:Ram = Ram { bs: unsafe { &mut THE_PPU_RAM_BYTES } };
pub static mut THE_PPU_MAPPER:Option<Mapper> = None;
pub static mut THE_PPU:Option<Ppu> = None;

pub static mut THE_CPU_PPU_INTERCONNECT:CpuPpuInterconnect = CpuPpuInterconnect { cpu: null_mut(), ppu: null_mut() };

pub static mut THE_APU:Option<Apu> = None;
