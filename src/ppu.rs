#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]

use crate::c6502::C6502;
use crate::common::*;
use crate::mapper::{AddressSpace, Mapper};
use crate::serialization::Savable;

use std::io::Read;
use std::io::Write;
use std::mem::transmute;

//use std::vec;

pub type SystemColor = u8; // [0,64)
type PatternColor = u8; // [0,4)
type PatternId = u8;
type PaletteId = u8; // [0, 4)
type TileIndex = u8;
type Attribute = u8; // An entry in the attribute table
type SpriteId = u8;

const COLOR_TRANSPARENT: u8 = 0;

const ADDRESS_NAMETABLE0: u16 = 0x2000;
const ADDRESS_ATTRIBUTE_TABLE0: u16 = 0x23C0;
const NAMETABLE_SIZE: u16 = 0x0400;
const ADDRESS_UNIVERSAL_BACKGROUND_COLOR: u16 = 0x3f00;
const ADDRESS_BACKGROUND_PALETTE0: u16 = 0x3f00;
const SPRITE_HEIGHT: u8 = 8;
const SPRITE_WIDTH: u8 = 8;
const SCANLINE_PRERENDER: u16 = 261;
const SCANLINE_RENDER: u16 = 0;
const SCANLINE_POSTRENDER: u16 = 240;
const SCANLINE_VBLANK: u16 = 241;
const GLOBAL_BACKGROUND_COLOR: PaletteColor = PaletteColor { color: 0 };

pub const RENDER_WIDTH: usize = 256;
pub const RENDER_HEIGHT: usize = 240;
pub const UNRENDER_SIZE: usize = RENDER_WIDTH * RENDER_HEIGHT;
pub const RENDER_SIZE: usize = UNRENDER_SIZE * 3;

pub struct Ppu {
    pub display: [u8; UNRENDER_SIZE],
    pub oam: [u8; 256],
    pub mapper: Box<dyn AddressSpace>,
    pub is_vblank_nmi: bool,
    pub is_scanline_irq: bool,

    registers: PpuRegisters,
    sprite_pattern_table: bool, // Is the sprite pattern table the 'right' one?
    background_pattern_table: bool, // Is the background pattern table the right one?
    sprite_overflow: bool,
    sprite0_hit: bool,
    sprite_size: bool,  // false=8x8, true=8x16
    frame_parity: bool, // Toggled every frame
    open_bus: u8,       // Open bus shared by all PPU registers
    ppu_master_select: bool,
    generate_vblank_nmi: bool,
    ppudata_buffer: u8,

    oam_ptr: u8,

    clocks_until_nmi: u8,
    nmi_occurred: bool,
    frame: u32,
    scanline: u16,
    cycle: u16,
    sprite_count: u8,
    tile_pattern_low_shift: u16,
    tile_pattern_high_shift: u16,
    tile_palette_shift: u16,
    // Used to store fetched tile attributes until they're stored in cycles 0 mod 8.
    tile_nametable: u8,
    tile_attribute: u8,
    tile_pattern_low: u8,
    tile_pattern_high: u8,
    sprite_patterns: [u16; 8],
    sprite_palettes: [u8; 8],
    sprite_xs: [u8; 8],
    sprite_priorities: [bool; 8],
    sprite_indices: [u8; 8],
}

impl Savable for Ppu {
    fn save(&self, fh: &mut dyn Write) {
        self.display.save(fh);
        self.oam.save(fh);
        self.mapper.save(fh);
        self.is_vblank_nmi.save(fh);
        self.is_scanline_irq.save(fh);
        self.registers.save(fh);
        self.sprite_pattern_table.save(fh);
        self.background_pattern_table.save(fh);
        self.sprite_overflow.save(fh);
        self.sprite0_hit.save(fh);
        self.sprite_size.save(fh);
        self.frame_parity.save(fh);
        self.open_bus.save(fh);
        self.ppu_master_select.save(fh);
        self.generate_vblank_nmi.save(fh);
        self.ppudata_buffer.save(fh);
        self.oam_ptr.save(fh);
        self.clocks_until_nmi.save(fh);
        self.nmi_occurred.save(fh);
        self.frame.save(fh);
        self.scanline.save(fh);
        self.cycle.save(fh);
        self.sprite_count.save(fh);
        self.tile_pattern_low_shift.save(fh);
        self.tile_pattern_high_shift.save(fh);
        self.tile_palette_shift.save(fh);
        self.tile_nametable.save(fh);
        self.tile_attribute.save(fh);
        self.tile_pattern_low.save(fh);
        self.tile_pattern_high.save(fh);
        self.sprite_patterns.save(fh);
        self.sprite_palettes.save(fh);
        self.sprite_xs.save(fh);
        self.sprite_priorities.save(fh);
        self.sprite_indices.save(fh);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        self.display.load(fh);
        self.oam.load(fh);
        self.mapper.load(fh);
        self.is_vblank_nmi.load(fh);
        self.is_scanline_irq.load(fh);
        self.registers.load(fh);
        self.sprite_pattern_table.load(fh);
        self.background_pattern_table.load(fh);
        self.sprite_overflow.load(fh);
        self.sprite0_hit.load(fh);
        self.sprite_size.load(fh);
        self.frame_parity.load(fh);
        self.open_bus.load(fh);
        self.ppu_master_select.load(fh);
        self.generate_vblank_nmi.load(fh);
        self.ppudata_buffer.load(fh);
        self.oam_ptr.load(fh);
        self.clocks_until_nmi.load(fh);
        self.nmi_occurred.load(fh);
        self.frame.load(fh);
        self.scanline.load(fh);
        self.cycle.load(fh);
        self.sprite_count.load(fh);
        self.tile_pattern_low_shift.load(fh);
        self.tile_pattern_high_shift.load(fh);
        self.tile_palette_shift.load(fh);
        self.tile_nametable.load(fh);
        self.tile_attribute.load(fh);
        self.tile_pattern_low.load(fh);
        self.tile_pattern_high.load(fh);
        self.sprite_patterns.load(fh);
        self.sprite_palettes.load(fh);
        self.sprite_xs.load(fh);
        self.sprite_priorities.load(fh);
        self.sprite_indices.load(fh);
    }
}

struct PpuRegisters {
    // Register is only 15 bits in hardware.
    /*
    yyy NN YYYYY XXXXX
    y = fine y scroll
    N = nametable select
    Y = coarse Y scroll
    X = coarse X scroll
     */
    v: u16,
    t: u16,  // t is the address of the top-left onscreen tile
    x: u8,   // Fine x scroll
    w: bool, // First-or-second write toggle(PPUSCROLL and PPUADDR)

    vram_increment: bool, // false=increment by 1; true = increment by 32
    is_greyscale: bool,
    background_enabled: bool,
    sprites_enabled: bool,
    emphasize_red: bool,
    emphasize_green: bool,
    emphasize_blue: bool,
    show_leftmost_background: bool,
    show_leftmost_sprite: bool,
}

impl Savable for PpuRegisters {
    fn save(&self, fh: &mut dyn Write) {
        self.v.save(fh);
        self.t.save(fh);
        self.x.save(fh);
        self.w.save(fh);

        self.vram_increment.save(fh);
        self.is_greyscale.save(fh);
        self.background_enabled.save(fh);
        self.sprites_enabled.save(fh);
        self.emphasize_red.save(fh);
        self.emphasize_green.save(fh);
        self.emphasize_blue.save(fh);
        self.show_leftmost_background.save(fh);
        self.show_leftmost_sprite.save(fh);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        self.v.load(fh);
        self.t.load(fh);
        self.x.load(fh);
        self.w.load(fh);

        self.vram_increment.load(fh);
        self.is_greyscale.load(fh);
        self.background_enabled.load(fh);
        self.sprites_enabled.load(fh);
        self.emphasize_red.load(fh);
        self.emphasize_green.load(fh);
        self.emphasize_blue.load(fh);
        self.show_leftmost_background.load(fh);
        self.show_leftmost_sprite.load(fh);
    }
}

impl PpuRegisters {
    // https://wiki.nesdev.com/w/index.php/PPU_scrolling
    pub fn new() -> PpuRegisters {
        PpuRegisters {
            v: 0,
            t: 0,
            x: 0,
            w: false,

            vram_increment: false,
            is_greyscale: false,
            background_enabled: false,
            sprites_enabled: false,
            emphasize_red: false,
            emphasize_green: false,
            emphasize_blue: false,
            show_leftmost_background: false,
            show_leftmost_sprite: false,
        }
    }
    pub fn copy_y(&mut self) {
        let mask: u16 = 0b111101111100000;
        self.v &= !mask;
        self.v |= self.t & mask;
    }
    pub fn copy_x(&mut self) {
        let mask: u16 = 0b10000011111;
        self.v &= !mask;
        self.v |= self.t & mask;
    }
    fn reset_horizontal_position(&mut self) {
        let mask: u16 = 0b10000011111;
        self.v &= !mask;
        self.v |= self.t & mask;
    }

    pub fn increment_x(&mut self) {
        let v = self.v;
        if (self.v & 0x001F) == 31 {
            self.v &= !0x001F;
            self.v ^= 0x0400;
        } else {
            self.v = (self.v + 1) & 0x7FFF;
        }
        //eprintln!("DEBUG - COARSE-X {:x} {:x}", v, self.v);
    }
    pub fn scanline(&self) -> u16 {
        let coarse_y_mask: u16 = 0b1111100000;
        let fine_y_mask: u16 = 0b111000000000000;
        let y = (self.v & coarse_y_mask) >> 2 | (self.v & fine_y_mask) >> 12;
        return y;
    }
    pub fn increment_y(&mut self) {
        let mut v = self.v;
        if (v & 0x7000) != 0x7000 {
            self.v += 0x1000;
        } else {
            v &= !0x7000;
            let mut y: u16 = (v & 0x03e0) >> 5;
            if y == 29 {
                y = 0;
                v ^= 0x0800;
            } else if y == 31 {
                y = 0;
            } else {
                y += 1;
            }
            self.v = (v & !0x03E0) | (y << 5);
        }
    }
    pub fn write_control(&mut self, px: u8) {
        let x = px as u16;
        let mask: u16 = 0b110000000000;
        self.t &= !mask;
        self.t |= (x & 0x3) << 10;
        self.vram_increment = get_bit(px, 2) > 0;
        // eprintln!("DEBUG - PPU CONTROL WRITE {:x} {}", px, self.vram_increment);
    }

    pub fn write_mask(&mut self, v: u8) {
        // eprintln!("DEBUG - PPU MASK WRITE {:x}", v);
        self.is_greyscale = get_bit(v, 0) > 0;
        self.show_leftmost_background = get_bit(v, 1) > 0;
        self.show_leftmost_sprite = get_bit(v, 2) > 0;
        self.background_enabled = get_bit(v, 3) > 0;
        self.sprites_enabled = get_bit(v, 4) > 0;
        self.emphasize_red = get_bit(v, 5) > 0;
        self.emphasize_green = get_bit(v, 6) > 0;
        self.emphasize_blue = get_bit(v, 7) > 0;
    }

    pub fn read_status(&mut self) {
        self.w = false;
    }
    pub fn write_scroll(&mut self, px: u8) {
        let x = px as u16;
        if !self.w {
            // First write
            self.t &= !0b11111;
            self.t |= x >> 3;
            self.x = px & 0x7;
        } else {
            self.t &= !0b111001111100000;
            self.t |= ((x >> 3) & 0x1F) << 5; // FED
            self.t |= (x & 0x3) << 12; // CBA
        }
        self.w = !self.w;
    }

    pub fn write_address(&mut self, x: u8) {
        if !self.w {
            self.t &= 0x00FF;
            self.t |= ((x & 0b00111111) as u16) << 8;
        } else {
            self.t &= 0xFF00;
            self.t |= x as u16;
            self.v = self.t;
        }
        self.w = !self.w;
        // eprintln!("DEBUG - PPU WRITE ADDRESS - {:x} {}", self.v, self.w);
    }
    fn is_rendering(&self) -> bool {
        let scanline = self.scanline();
        return self.is_rendering_enabled()
            && ((scanline == SCANLINE_PRERENDER) || scanline < SCANLINE_POSTRENDER);
    }
    pub fn vram_ptr(&self) -> u16 {
        return self.v;
    }
    pub fn is_sprites_enabled(&self) -> bool {
        return self.sprites_enabled;
    }
    pub fn is_background_enabled(&self) -> bool {
        return self.background_enabled;
    }
    pub fn is_rendering_enabled(&self) -> bool {
        return self.sprites_enabled || self.background_enabled;
    }

    fn advance_vram_ptr(&mut self) {
        // TODO - VRAM ptr is supposed to increment in a weird way during rendering.
        if self.is_rendering() && false {
            self.increment_x();
            self.increment_y();
        } else {
            let increment = ternary(self.vram_increment, 32, 1);
            // eprintln!("DEBUG - VRAM INCREMENT - {} {}", self.vram_increment, increment);
            self.v = self.v.wrapping_add(increment) & 0x3FFF;
        }
    }

    pub fn fine_x(&self) -> u8 {
        return self.x;
    }

    pub fn fine_y(&self) -> u8 {
        return ((self.v >> 12) & 0x7) as u8;
    }

    pub fn tile_x(&self) -> u8 {
        return (self.v & 0b11111) as u8;
    }

    pub fn tile_y(&self) -> u8 {
        return ((self.v & 0b1111100000) >> 5) as u8;
    }

    pub fn tile_address(&self) -> u16 {
        return ADDRESS_NAMETABLE0 | (self.v & 0x0FFF);
    }
    pub fn attribute_address(&self) -> u16 {
        let v = self.v;
        return ADDRESS_ATTRIBUTE_TABLE0 | (v & 0x0c00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x07);
    }
}

#[derive(Copy, Clone, Debug)]
pub enum PpuPort {
    PPUCTRL,
    PPUMASK,
    PPUSTATUS,
    OAMADDR,
    OAMDATA,
    PPUSCROLL,
    PPUADDR,
    PPUDATA,
    OAMDMA,
}

impl AddressSpace for Ppu {
    fn peek(&self, ptr: u16) -> u8 {
        return self.mapper.peek(ptr);
    }
    fn poke(&mut self, ptr: u16, v: u8) {
        self.mapper.poke(ptr, v);
    }
}

#[derive(Copy, Clone)]
struct Sprite {
    index: u8,
    x: u8,
    y: u8,
    tile_index: u8,
    palette: u8,
    is_front: bool, // priority
    flip_horizontal: bool,
    flip_vertical: bool,
}

/* a=ABCDEFGH, b=12345678, combine_bitplanes(a,b) = A1B2C3D4E5F6G7H8 */
fn combine_bitplanes(mut a: u8, mut b: u8) -> u16 {
    let mut out = 0u16;
    for i in 0..8 {
        out |= (((a & 1) << 1 | (b & 1)) as u16) << (i * 2);
        a >>= 1;
        b >>= 1;
    }
    return out;
}

fn reverse_bits(mut a: u8) -> u8 {
    let mut out = 0u8;
    for i in 0..8 {
        out <<= 1;
        out |= a & 1;
        a >>= 1;
    }
    return out;
}

#[derive(Copy, Clone)]
pub struct CpuPpuInterconnect {
    ppu: *mut Ppu,
    cpu: *mut C6502,
}

impl Savable for CpuPpuInterconnect {
    fn save(&self, fh: &mut dyn Write) {
        self.ppu.save(fh);
        self.cpu.save(fh);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        self.ppu.load(fh);
        self.cpu.load(fh);
    }
}

impl CpuPpuInterconnect {
    pub fn new(ppu: &mut Ppu, cpu: &mut C6502) -> CpuPpuInterconnect {
        CpuPpuInterconnect { ppu: ppu, cpu: cpu }
    }
}

use PpuPort::*;

pub fn map_ppu_port(ptr: u16) -> Option<PpuPort> {
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
        _ => None,
    }
}

impl AddressSpace for CpuPpuInterconnect {
    fn peek(&self, ptr: u16) -> u8 {
        let ppu: &mut Ppu = unsafe { &mut *self.ppu };
        match map_ppu_port(ptr) {
            Some(PPUCTRL) => ppu.open_bus,
            Some(PPUMASK) => ppu.open_bus,
            Some(PPUSTATUS) => ppu.read_status(),
            Some(OAMADDR) => ppu.open_bus,
            Some(OAMDATA) => ppu.read_oam_data(),
            Some(PPUSCROLL) => ppu.open_bus,
            Some(PPUADDR) => ppu.open_bus,
            Some(PPUDATA) => ppu.read_data(),
            Some(OAMDMA) => ppu.open_bus,
            port => panic!("INVALID PPU PORT READ {:?} {:x}", port, ptr),
        }
    }
    fn poke(&mut self, ptr: u16, value: u8) {
        let ppu: &mut Ppu = unsafe { &mut *self.ppu };
        ppu.open_bus = value;
        match map_ppu_port(ptr) {
            Some(PPUCTRL) => ppu.write_control(value),
            Some(PPUMASK) => ppu.write_mask(value),
            Some(PPUSTATUS) => {}
            Some(OAMADDR) => ppu.write_oam_address(value),
            Some(OAMDATA) => ppu.write_oam_data(value),
            Some(PPUSCROLL) => ppu.write_scroll(value),
            Some(PPUADDR) => ppu.write_address(value),
            Some(PPUDATA) => ppu.write_data(value),
            Some(OAMDMA) => {
                let cpu = unsafe { &*self.cpu };
                let ptr_base = (value as u16) << 8;
                for i in 0..=255 {
                    let addr = ptr_base + i;
                    let v = cpu.peek(addr);
                    ppu.oam[ppu.oam_ptr as usize] = v;
                    ppu.oam_ptr = ppu.oam_ptr.wrapping_add(1);
                }
            }
            port => panic!("INVALID PPU PORT WRITE {:?} {:x} {:x}", port, ptr, value),
        }
    }
}

pub struct PaletteControl {
    memory: [u8; 32],
}

impl PaletteControl {
    pub fn new() -> PaletteControl {
        PaletteControl { memory: [0; 32] }
    }
    fn map_ptr(&self, ptr: u16) -> usize {
        let remapped = match ptr {
            0x3f10 => 0x3f00,
            0x3f14 => 0x3f04,
            0x3f18 => 0x3f08,
            0x3f1c => 0x3f0c,
            _ => ptr,
        };
        return (remapped - 0x3f00) as usize;
    }
}

impl Savable for PaletteControl {
    fn save(&self, fh: &mut dyn Write) {
        self.memory.save(fh);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        self.memory.load(fh);
    }
}

impl AddressSpace for PaletteControl {
    fn peek(&self, ptr: u16) -> u8 {
        let true_ptr = self.map_ptr(ptr);
        return self.memory[true_ptr];
    }
    fn poke(&mut self, ptr: u16, v: u8) {
        let true_ptr = self.map_ptr(ptr);
        self.memory[true_ptr] = v;
    }
}

#[derive(Copy, Clone)]
enum PaletteType {
    Sprite,
    Background,
}

impl Clocked for Ppu {
    // High-level pattern here taken from https://github.com/fogleman/nes/blob/master/nes/ppu.go
    fn clock(&mut self) {
        // https://wiki.nesdev.com/w/index.php/PPU_rendering
        self.tick_counters();

        let is_visible_line = self.scanline < 240;
        let is_visible_cycle = self.cycle >= 1 && self.cycle <= 256;
        let is_fetch_line = self.scanline == SCANLINE_PRERENDER || is_visible_line;
        let is_prefetch_cycle = self.cycle >= 321 && self.cycle <= 336;
        let is_fetch_cycle = is_prefetch_cycle || is_visible_cycle;

        //eprintln!("DEBUG - CYCLE - {} {} {} {} {} {}", self.cycle, self.scanline, self.frame, self.is_rendering_enabled(), is_visible_line, is_visible_cycle);
        // Background logic
        if self.is_rendering_enabled() {
            if is_visible_line && is_visible_cycle {
                self.render_pixel();
            }
            if is_fetch_line && is_fetch_cycle {
                self.shift_registers();
                match self.cycle % 8 {
                    0 => self.shift_new_tile(),
                    1 => self.fetch_bg_tile(),
                    3 => self.fetch_bg_attribute(),
                    5 => self.fetch_bg_pattern_low(),
                    7 => self.fetch_bg_pattern_high(),
                    _ => {}
                }
            }
            if self.scanline == SCANLINE_PRERENDER && self.cycle >= 280 && self.cycle <= 304 {
                self.registers.copy_y();
            }
            if is_fetch_line {
                if is_fetch_cycle && self.cycle % 8 == 0 {
                    self.registers.increment_x();
                }
                if self.cycle == 256 {
                    self.registers.increment_y();
                }
                if self.cycle == 257 {
                    self.registers.copy_x();
                }
            }
        }

        // Sprite logic
        if self.is_rendering_enabled() {
            if self.cycle == 257 {
                if is_visible_line {
                    self.evaluate_sprites();
                } else {
                    self.sprite_count = 0;
                }
            }
        }
        // Vblank
        if self.scanline == 241 && self.cycle == 1 {
            // eprintln!("DEBUG - VBLANK HIT - {}", self.generate_vblank_nmi);
            self.set_vblank(true);
        }
        if self.scanline == SCANLINE_PRERENDER && self.cycle == 1 {
            self.set_vblank(false);
            self.sprite0_hit = false;
            self.sprite_overflow = false;
        }
    }
}

#[derive(Debug)]
struct PaletteColor {
    color: u8,
}

impl PaletteColor {
    pub fn new_from_parts(palette: u8, color: u8) -> PaletteColor {
        PaletteColor {
            color: (palette * 4 + color),
        }
    }
    pub fn address(&self) -> u16 {
        // https://wiki.nesdev.com/w/index.php/PPU_palettes
        let base_address = ADDRESS_BACKGROUND_PALETTE0;
        let palette_size = 4;
        let address: u16 =
            base_address + palette_size * (self.palette() as u16) + (self.palette_color() as u16);
        address
    }
    pub fn palette(&self) -> u8 {
        return self.color / 4;
    }
    pub fn palette_color(&self) -> u8 {
        return self.color % 4;
    }
    pub fn is_transparent(&self) -> bool {
        return self.palette_color() == 0;
    }
    pub fn is_opaque(&self) -> bool {
        return !self.is_transparent();
    }
}

// https://wiki.nesdev.com/w/index.php/PPU_rendering
impl Ppu {
    pub fn new() -> Ppu {
        let mapper = Mapper::new();
        Ppu {
            display: [0; UNRENDER_SIZE],
            oam: [0; 256],
            mapper: Box::new(mapper),
            is_vblank_nmi: false,
            is_scanline_irq: false,

            registers: PpuRegisters::new(),
            sprite_pattern_table: false,
            background_pattern_table: false,
            sprite_overflow: false,
            sprite0_hit: false,
            sprite_size: false,
            frame_parity: false,
            open_bus: 0,
            ppu_master_select: false,
            generate_vblank_nmi: false,
            ppudata_buffer: 0,

            oam_ptr: 0,

            clocks_until_nmi: 0,
            nmi_occurred: false,
            frame: 0,
            scanline: 0,
            cycle: 0,
            sprite_count: 0,
            tile_pattern_low_shift: 0,
            tile_pattern_high_shift: 0,
            tile_palette_shift: 0,
            tile_nametable: 0,
            tile_attribute: 0,
            tile_pattern_low: 0,
            tile_pattern_high: 0,
            sprite_patterns: [0; 8],
            sprite_palettes: [0; 8],
            sprite_xs: [0; 8],
            sprite_priorities: [false; 8],
            sprite_indices: [0; 8],
        }
    }

    pub fn current_frame(&self) -> u32 {
        return self.frame;
    }

    pub fn render(&self) -> [u8; RENDER_SIZE] {
        let mut ret = [0; RENDER_SIZE];
        for i in 0..UNRENDER_SIZE {
            let c = self.display[i];
            let (r, g, b) = self.lookup_system_pixel(c);
            ret[i * 3 + 0] = r;
            ret[i * 3 + 1] = g;
            ret[i * 3 + 2] = b;
        }
        return ret;
    }

    // pub fn render(&self, buf: &mut [u8]) {
    //     buf.copy_from_slice(&self.display[0..RENDER_SIZE]);
    // }

    fn shift_new_tile(&mut self) {
        let background_tile = self.tile_nametable;
        let attribute = self.tile_attribute;
        let idx_x = self.registers.tile_x();
        let idx_y = self.registers.tile_y();
        let palette = self.split_attribute_entry(attribute, idx_x, idx_y) as u16;
        let pattern_low = self.tile_pattern_low;
        let pattern_high = self.tile_pattern_high;
        self.tile_pattern_low_shift |= pattern_low as u16;
        self.tile_pattern_high_shift |= pattern_high as u16;
        self.tile_palette_shift <<= 2;
        self.tile_palette_shift |= palette;
    }

    fn fetch_bg_tile(&mut self) {
        let tile_address = self.registers.tile_address();
        self.tile_nametable = self.peek(tile_address);
    }
    fn fetch_bg_attribute(&mut self) {
        let address = self.registers.attribute_address();
        self.tile_attribute = self.peek(address);
    }
    fn fetch_bg_pattern_low(&mut self) {
        let (ptr_low, _) = self.locate_pattern_row(
            PaletteType::Background,
            self.tile_nametable,
            self.registers.fine_y(),
        );
        self.tile_pattern_low = self.peek(ptr_low);
    }
    fn fetch_bg_pattern_high(&mut self) {
        let (_, ptr_high) = self.locate_pattern_row(
            PaletteType::Background,
            self.tile_nametable,
            self.registers.fine_y(),
        );
        self.tile_pattern_high = self.peek(ptr_high);
    }

    fn fetch_tile_color_from_shift(&mut self) -> PaletteColor {
        let fine_x = self.registers.fine_x() as u16;
        let low = (((self.tile_pattern_low_shift << fine_x) & 0x8000) > 0) as u8;
        let high = (((self.tile_pattern_high_shift << fine_x) & 0x8000) > 0) as u8;
        let color = low | (high << 1);
        let x = self.cycle - 1;
        let palette = (self.tile_palette_shift >> ternary((x % 8 + fine_x) > 7, 0, 2)) & 0x3;
        return PaletteColor::new_from_parts(palette as u8, color as u8);
    }

    fn shift_registers(&mut self) {
        self.tile_pattern_low_shift <<= 1;
        self.tile_pattern_high_shift <<= 1;
    }

    fn tick_counters(&mut self) {
        if self.clocks_until_nmi > 0 {
            self.clocks_until_nmi -= 1;
            if self.clocks_until_nmi == 0 && self.generate_vblank_nmi && self.nmi_occurred {
                self.is_vblank_nmi = true;
            }
        }
        if self.is_rendering_enabled() {
            if self.frame_parity && self.scanline == 261 && self.cycle == 339 {
                self.cycle = 0;
                self.scanline = 0;
                self.frame += 1;
                self.frame_parity = !self.frame_parity;
                return;
            }
        }
        self.cycle += 1;
        if self.cycle > 340 {
            self.cycle = 0;
            self.scanline += 1;
            if self.scanline > 261 {
                self.scanline = 0;
                self.frame += 1;
                self.frame_parity = !self.frame_parity;
            }
        }
    }

    fn render_pixel(&mut self) {
        let x = self.cycle - 1;
        let y = self.scanline;
        let bg_color = self.background_pixel();
        let (i, sprite_color) = self.sprite_pixel();
        // Sprite 0 test
        if self.sprite_indices[i as usize] == 00 && sprite_color.is_opaque() && bg_color.is_opaque()
        {
            self.sprite0_hit = true;
        }
        // Determine display color
        let color = if self.sprite_priorities[i as usize] && sprite_color.is_opaque() {
            sprite_color
        } else if bg_color.is_opaque() {
            bg_color
        } else if sprite_color.is_opaque() {
            sprite_color
        } else {
            GLOBAL_BACKGROUND_COLOR
        };
        // eprintln!("DEBUG - COLOR - {:?}", color);
        let system_color = self.peek(color.address());
        self.write_system_pixel(x, y, system_color);
    }

    fn background_pixel(&mut self) -> PaletteColor {
        if !self.is_background_enabled() {
            return GLOBAL_BACKGROUND_COLOR;
        }
        return self.fetch_tile_color_from_shift();
    }
    fn sprite_pixel(&mut self) -> (u8, PaletteColor) {
        if !self.is_sprites_enabled() {
            return (0, GLOBAL_BACKGROUND_COLOR);
        }
        let x = self.cycle - 1;
        for i in 0..self.sprite_count as usize {
            let spritex = self.sprite_xs[i];
            let xsub = x as i16 - spritex as i16;
            if xsub < 0 || xsub > 7 {
                continue;
            }
            let palette = self.sprite_palettes[i];
            let color = (self.sprite_patterns[i] >> ((7 - xsub) * 2)) & 0x3;
            let palette_color = PaletteColor::new_from_parts(palette, color as u8);
            if palette_color.is_transparent() {
                continue;
            }
            return (i as u8, palette_color);
        }
        return (0, GLOBAL_BACKGROUND_COLOR);
    }
    fn is_sprites_enabled(&self) -> bool {
        return self.registers.is_sprites_enabled();
    }
    fn is_background_enabled(&self) -> bool {
        return self.registers.is_background_enabled();
    }

    fn is_rendering_enabled(&self) -> bool {
        return self.registers.is_rendering_enabled();
    }

    fn evaluate_sprites(&mut self) {
        let height = ternary(self.sprite_size, 16, 8);

        let mut count = 0;
        for i in 0..64 {
            let sprite = self.lookup_sprite(i);
            let pattern = self.fetch_sprite_pattern(&sprite, self.scanline);
            match pattern {
                None => continue,
                Some(pattern) => {
                    if count < 8 {
                        self.sprite_patterns[count] = pattern;
                        self.sprite_xs[count] = sprite.x;
                        self.sprite_priorities[count] = sprite.is_front;
                        self.sprite_indices[count] = i as u8;
                        self.sprite_palettes[count] = sprite.palette;
                    }
                    count += 1;
                }
            };
        }

        if count > 8 {
            count = 8;
            self.sprite_overflow = true;
        }
        self.sprite_count = count as u8;
    }

    fn fetch_sprite_pattern(&self, sprite: &Sprite, row: u16) -> Option<u16> {
        let is_size_16 = self.sprite_size;
        let row = row as i16 - sprite.y as i16;
        let height = ternary(is_size_16, 16, 8);
        if row < 0 || row >= height {
            return None;
        }
        let row = ternary(sprite.flip_vertical, height - 1 - row, row);
        let tile = sprite.tile_index; //TODO -- ternary(row >= 8, sprite.tile_index+1, sprite.tile_index);
        let row = ternary(row >= 8, row - 8, row);
        let (tile_row0, tile_row1) = self.fetch_pattern_row(PaletteType::Sprite, tile, row as u8);
        let (tile_row0, tile_row1) = ternary(
            sprite.flip_horizontal,
            (reverse_bits(tile_row0), reverse_bits(tile_row1)),
            (tile_row0, tile_row1),
        );
        return Some(combine_bitplanes(tile_row1, tile_row0));
    }

    fn locate_pattern_row(
        &self,
        palette_type: PaletteType,
        tile_index: u8,
        ysub: u8,
    ) -> (u16, u16) {
        // https://wiki.nesdev.com/w/index.php/PPU_pattern_tables
        let ptr_pattern_table_base = 0x0000;
        let size_pattern_table = 0x1000;
        let size_tile = 16;
        let is_pattern_table_right = match palette_type {
            PaletteType::Sprite => self.sprite_pattern_table,
            PaletteType::Background => self.background_pattern_table,
        };
        let ptr_tile: u16 = ptr_pattern_table_base
            + size_pattern_table * (is_pattern_table_right as u16)
            + (size_tile * tile_index as u16);
        let ptr_tile_row0 = ptr_tile + (ysub as u16);
        let ptr_tile_row1 = ptr_tile_row0 + 8; // The bits of the color id are stored in separate bit planes.
        (ptr_tile_row0, ptr_tile_row1)
    }
    fn fetch_pattern_row(&self, palette_type: PaletteType, tile_index: u8, ysub: u8) -> (u8, u8) {
        let (ptr_tile_row0, ptr_tile_row1) =
            self.locate_pattern_row(palette_type, tile_index, ysub);
        let tile_row0 = self.peek(ptr_tile_row0);
        let tile_row1 = self.peek(ptr_tile_row1);
        return (tile_row0, tile_row1);
    }

    fn fetch_scanline_sprites(&mut self, y: u16) -> Vec<Sprite> {
        let mut vec = Vec::new();
        for i in 0..64 {
            let sprite = self.lookup_sprite(i);
            if y >= sprite.y as u16 && y < (sprite.y as u16 + SPRITE_HEIGHT as u16) {
                vec.push(sprite);
            }
            if vec.len() >= 8 {
                self.sprite_overflow = true;
                return vec;
            }
        }
        return vec;
    }

    fn find_matching_sprite(&self, x: u16, sprites: &Vec<Sprite>) -> Option<Sprite> {
        for sprite in sprites {
            if x >= (sprite.x as u16) && x < (sprite.x as u16 + SPRITE_WIDTH as u16) {
                return Some(sprite.clone());
            }
        }
        return None;
    }

    fn lookup_sprite(&self, i: usize) -> Sprite {
        let attribute = self.oam[i * 4 + 2];
        return Sprite {
            index: i as u8,
            y: self.oam[i * 4 + 0],
            tile_index: self.oam[i * 4 + 1],
            palette: (attribute & 3) + 4,
            is_front: get_bit(attribute, 5) == 0,
            flip_horizontal: get_bit(attribute, 6) > 0,
            flip_vertical: get_bit(attribute, 7) > 0,
            x: self.oam[i * 4 + 3],
        };
    }

    fn lookup_global_background_color(&self) -> SystemColor {
        return self.peek(ADDRESS_UNIVERSAL_BACKGROUND_COLOR);
    }

    fn split_attribute_entry(&self, entry: Attribute, idx_x: u8, idx_y: u8) -> PaletteId {
        let (left, top) = ((idx_x % 4) < 2, (idx_y % 4) < 2);
        let palette_id = match (left, top) {
            (true, true) => (entry >> 0) & 0x3,
            (false, true) => (entry >> 2) & 0x3,
            (true, false) => (entry >> 4) & 0x3,
            (false, false) => (entry >> 6) & 0x3,
        };
        //eprintln!("DEBUG - ATTRIBUTE ENTRY - {:x} {}", entry, palette_id);
        return palette_id;
    }

    pub fn write_control(&mut self, v: u8) {
        self.registers.write_control(v);
        self.sprite_pattern_table = get_bit(v, 3) > 0;
        self.background_pattern_table = get_bit(v, 4) > 0;
        self.sprite_size = get_bit(v, 5) > 0;
        self.ppu_master_select = get_bit(v, 6) > 0;
        self.generate_vblank_nmi = get_bit(v, 7) > 0;
    }

    pub fn write_mask(&mut self, v: u8) {
        self.registers.write_mask(v);
    }

    pub fn read_status(&mut self) -> u8 {
        let ret = (self.open_bus & 0b00011111)
            | ((self.sprite_overflow as u8) << 5)
            | ((self.sprite0_hit as u8) << 6)
            | ((self.nmi_occurred as u8) << 7);
        self.set_vblank(false);
        self.registers.read_status();
        return ret;
    }
    pub fn write_oam_address(&mut self, v: u8) {
        self.oam_ptr = v;
    }
    pub fn read_oam_data(&mut self) -> u8 {
        let ptr: u8 = self.oam_ptr;
        return self.oam[ptr as usize];
    }
    pub fn write_oam_data(&mut self, v: u8) {
        let ptr: u8 = self.oam_ptr;
        self.oam[ptr as usize] = v;
        self.oam_ptr = self.oam_ptr.wrapping_add(1);
    }
    pub fn write_scroll(&mut self, v: u8) {
        self.registers.write_scroll(v);
    }
    pub fn write_address(&mut self, v: u8) {
        self.registers.write_address(v);
    }
    pub fn read_data(&mut self) -> u8 {
        let ptr = self.registers.vram_ptr();
        self.registers.advance_vram_ptr();
        let val = self.peek(ptr);
        if ptr < 0x3f00 {
            let old_val = self.ppudata_buffer;
            self.ppudata_buffer = val;
            old_val
        } else {
            val
        }
    }
    pub fn write_data(&mut self, v: u8) {
        let ptr = self.registers.vram_ptr();
        self.registers.advance_vram_ptr();
        // eprintln!("DEBUG - PPU WRITE DATA - {:x} {:x} {:x}", ptr, v, self.registers.vram_ptr());
        self.poke(ptr, v);
    }
    fn lookup_system_pixel(&self, i: SystemColor) -> RgbColor {
        return SYSTEM_PALETTE[i as usize];
    }
    fn write_system_pixel(&mut self, x: u16, y: u16, c: SystemColor) {
        if x >= 256 || y >= 240 {
            return;
        }
        let i = (x + 256 * y) as usize;
        self.display[i] = c;
    }
    fn set_vblank(&mut self, new_vblank: bool) {
        let vblank = self.nmi_occurred;
        if vblank != new_vblank {
            //eprintln!("DEBUG - VBLANK CHANGED FROM {:?} TO {:?}", vblank, new_vblank);
        }
        self.nmi_occurred = new_vblank;
        self.clocks_until_nmi = 15;
    }
}

type RgbColor = (u8, u8, u8);
type SystemPalette = [RgbColor; 64];

// The NES can refer to 64 separate colors. This table has RGB values for each.
pub const SYSTEM_PALETTE: SystemPalette = [
    // 0x
    (124, 124, 124), // x0
    (0, 0, 252),     // x1
    (0, 0, 188),     // x2
    (68, 40, 188),   // x3
    (148, 0, 132),   // x4
    (168, 0, 32),    // x5
    (168, 16, 0),    // x6
    (136, 20, 0),    // x7
    (80, 48, 0),     // x8
    (0, 120, 0),     // x9
    (0, 104, 0),     // xA
    (0, 88, 0),      // xB
    (0, 64, 88),     // xC
    (0, 0, 0),       // xD
    (0, 0, 0),       // xE
    (0, 0, 0),       // xF
    // 1x
    (188, 188, 188), // x0
    (0, 120, 248),   // x1
    (0, 88, 248),    // x2
    (104, 68, 252),  // x3
    (216, 0, 204),   // x4
    (228, 0, 88),    // x5
    (248, 56, 0),    // x6
    (228, 92, 16),   // x7
    (172, 124, 0),   // x8
    (0, 184, 0),     // x9
    (0, 168, 0),     // xA
    (0, 168, 68),    // xB
    (0, 136, 136),   // xC
    (0, 0, 0),       // xD
    (0, 0, 0),       // xE
    (0, 0, 0),       // xF
    // 2x
    (248, 248, 248), // x0
    (60, 188, 252),  // x1
    (104, 136, 252), // x2
    (152, 120, 248), // x3
    (248, 120, 248), // x4
    (248, 88, 152),  // x5
    (248, 120, 88),  // x6
    (252, 160, 68),  // x7
    (248, 184, 0),   // x8
    (184, 248, 24),  // x9
    (88, 216, 84),   // xA
    (88, 248, 152),  // xB
    (0, 232, 216),   // xC
    (120, 120, 120), // xD
    (0, 0, 0),       // xE
    (0, 0, 0),       // xF
    // 3x
    (252, 252, 252), // x0
    (164, 228, 252), // x1
    (184, 184, 248), // x2
    (216, 184, 248), // x3
    (248, 184, 248), // x4
    (248, 164, 192), // x5
    (240, 208, 176), // x6
    (252, 224, 168), // x7
    (248, 216, 120), // x8
    (216, 248, 120), // x9
    (184, 248, 184), // xA
    (184, 248, 216), // xB
    (0, 252, 252),   // xC
    (216, 216, 216), // xD
    (0, 0, 0),       // xE
    (0, 0, 0),       // xF
];

mod tests {
    use super::*;

    #[test]
    fn test_combine_bitplanes() {
        let a = 0b10011110;
        let b = 0b01101100;
        let o = 0b1001011011111000;
        assert_eq!(combine_bitplanes(a, b), o);
    }
    #[test]
    fn test_reverse_bits() {
        let a = 0b10011110;
        let o = 0b01111001;
        assert_eq!(reverse_bits(a), o);
    }
}
