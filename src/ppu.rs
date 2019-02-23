#![allow(unused_imports)]

use crate::common::*;
use crate::mapper::{AddressSpace, Mapper};
use crate::c6502::C6502;

use std::mem::transmute;

//use std::vec;

pub type SystemColor = u8; // [0,64)
type PatternColor = u8; // [0,4)
type PatternId = u8;
type PaletteId = u8; // [0, 4)
type TileIndex = u8;

const COLOR_TRANSPARENT:u8 = 0;

const ADDRESS_NAMETABLE0:u16 = 0x2000;
const ADDRESS_ATTRIBUTE_TABLE0:u16 = 0x23C0;
const NAMETABLE_SIZE:u16 = 0x0400;
const ADDRESS_UNIVERSAL_BACKGROUND_COLOR:u16 = 0x3f00;
const ADDRESS_BACKGROUND_PALETTE0:u16 = 0x3f00;
const SPRITE_HEIGHT:u8 = 8;
const SPRITE_WIDTH:u8 = 8;
const SCANLINE_PRERENDER:u16 = 261;
const SCANLINE_RENDER:u16 = 0;
const SCANLINE_POSTRENDER:u16 = 240;
const SCANLINE_VBLANK:u16 = 241;

pub const RENDER_WIDTH:usize = 256;
pub const RENDER_HEIGHT:usize = 240;
pub const RENDER_SIZE:usize = RENDER_WIDTH * RENDER_HEIGHT * 3;

pub struct Ppu {
    pub display: [u8; RENDER_SIZE],
    pub oam: [u8; 256],
    pub mapper: Box<dyn AddressSpace>,
    pub is_vblank_nmi: bool,
    pub is_scanline_irq: bool,

    registers:PpuRegisters,
    sprite_pattern_table: bool, // Is the sprite pattern table the 'right' one?
    background_pattern_table: bool, // Is the background pattern table the right one?
    sprite_overflow: bool,
    sprite0_hit: bool,
    sprite_size: bool,
    frame_parity: bool, // Toggled every frame
    vblank: bool,
    open_bus: u8, // Open bus shared by all PPU registers
    ppu_master_select: bool,
    generate_vblank_nmi: bool,
    ppudata_buffer:u8,

    oam_ptr: u8,

    // Lift state in the render loop to PPU-level registers to allow running a render clock-by-clock.
    /* clocks_to_pause causes the PPU to do nothing for X clocks. This should be unnecessary in a
       clock-accurate emulation. However, this emulator may choose to "batch" the actions of many clocks
       all at once, and fill the remaining clocks with do-nothing operations. */
    clocks_to_pause: u16,
    scanline: u16,
    tile_idx_shift: u16,
    tile_attribute_shift: u16,
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
    t: u16, // t is the address of the top-left onscreen tile
    x: u8, // Fine x scroll
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
    pub fn prerender_copy(&mut self) {
        let mask:u16 = 0b111101111100000;
        self.v &= !mask;
        self.v |= self.t & mask;
    }
    pub fn post_scanline_copy(&mut self) {
        let mask:u16 = 0b10000011111;
        self.v &= !mask;
        self.v |= self.t & mask;
    }
    pub fn handle_scanline_x(&mut self, x:u16) {
        if x == 256 {
            self.y_increment();
        }
        if x == 257 {
            self.reset_horizontal_position();
        }
        // if x == 328 {
        //     self.shift_new_tile();
        // }
        // if x == 336 {
        //     self.shift_new_tile();
        // }
    }
    fn reset_horizontal_position(&mut self) {
        let mask:u16 = 0b10000011111;
        self.v &= !mask;
        self.v |= self.t & mask;
    }

    pub fn coarse_x_increment(&mut self) {
        let v = self.v;
        if (self.v & 0x001F) == 31 {
            self.v &= !0x001F;
            self.v ^= 0x0400;
        } else {
            self.v = (self.v+1)& 0x7FFF;
        }
        //eprintln!("DEBUG - COARSE-X {:x} {:x}", v, self.v);
        }
    pub fn scanline(&self) -> u16 {
        let coarse_y_mask:u16 =      0b1111100000;
        let fine_y_mask:u16   = 0b111000000000000;
        let y = (self.v & coarse_y_mask) >> 2 |
                (self.v & fine_y_mask) >> 12;
        return y;
    }
    pub fn y_increment(&mut self) {
        let mut v = self.v;
        if (v & 0x7000) != 0x7000 {
            self.v += 0x1000;
        } else {
            v &= !0x7000;
            let mut y:u16 = (v & 0x03e0) >> 5;
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
    pub fn write_control(&mut self, px:u8) {
        let x = px as u16;
        let mask:u16 = 0b110000000000;
        self.t &= !mask;
        self.t |= (x & 0x3) << 10;
        self.vram_increment = get_bit(px, 2) > 0;
        eprintln!("DEBUG - PPU CONTROL WRITE {:x} {}", px, self.vram_increment);
    }

    pub fn write_mask(&mut self, v:u8) {
        self.is_greyscale = get_bit(v,0)>0;
        self.show_leftmost_background = get_bit(v,1)>0;
        self.show_leftmost_sprite = get_bit(v,2)>0;
        self.background_enabled = get_bit(v,3)>0;
        self.sprites_enabled = get_bit(v,4)>0;
        self.emphasize_red = get_bit(v,5)>0;
        self.emphasize_green = get_bit(v,6)>0;
        self.emphasize_blue = get_bit(v,7)>0;
    }

    pub fn read_status(&mut self) {
        self.w = false;
    }
    pub fn write_scroll(&mut self, px:u8) {
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

    pub fn write_address(&mut self, x:u8) {
        if !self.w {
            self.t &= 0x00FF;
            self.t |= ((x & 0b00111111) as u16) << 8;
        } else {
            self.t &= 0xFF00;
            self.t |= x as u16;
            self.v = self.t;
        }
        self.w = !self.w;
        eprintln!("DEBUG - PPU WRITE ADDRESS - {:x} {}", self.v, self.w);
    }
    fn is_rendering(&self) -> bool {
        let scanline = self.scanline();
        return self.is_rendering_enabled() &&
            ((scanline == SCANLINE_PRERENDER) ||
             scanline < SCANLINE_POSTRENDER
             );
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
            self.coarse_x_increment();
            self.y_increment();
        } else {
            let increment = ternary(self.vram_increment, 32, 1);
            eprintln!("DEBUG - VRAM INCREMENT - {} {}", self.vram_increment, increment);
            self.v = self.v.wrapping_add(increment) & 0x3FFF;
        }
    }

    pub fn fine_x(&self) -> u8 {
        return self.x;
    }

    pub fn fine_y(&self) -> u8 {
        return ((self.v >> 12) & 0x3) as u8;
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
        return ADDRESS_ATTRIBUTE_TABLE0 |
        (v & 0x0c00) |
        ((v >> 4) & 0x38) |
        ((v >> 2) & 0x07)
            ;
    }
}

#[derive(Copy, Clone, Debug)]
pub enum PpuPort {
    PPUCTRL, PPUMASK, PPUSTATUS,
    OAMADDR, OAMDATA, PPUSCROLL,
    PPUADDR, PPUDATA, OAMDMA,
}

impl AddressSpace for Ppu {
    fn peek(&self, ptr:u16) -> u8 { return self.mapper.peek(ptr); }
    fn poke(&mut self, ptr:u16, v:u8) { self.mapper.poke(ptr, v); }
}

#[derive(Copy, Clone)]
struct Sprite {
    sprite_index:u8,
    x: u8,
    y: u8,
    tile_index: u8,
    palette: u8,
    is_front: bool, // priority
    flip_horizontal: bool,
    flip_vertical: bool,
}

#[derive(Copy, Clone)]
pub struct CpuPpuInterconnect {
    ppu: *mut Ppu,
    cpu: *mut C6502,
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
        _      => None
    }
}

impl AddressSpace for CpuPpuInterconnect {
    fn peek(&self, ptr:u16) -> u8 {
        let ppu:&mut Ppu = unsafe { &mut *self.ppu };
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
    fn poke(&mut self, ptr:u16, value:u8) {
        let ppu:&mut Ppu = unsafe { &mut *self.ppu };
        ppu.open_bus = value;
        match map_ppu_port(ptr) {
            Some(PPUCTRL) => ppu.write_control(value),
            Some(PPUMASK) => ppu.write_mask(value),
            Some(PPUSTATUS) => {},
            Some(OAMADDR) => ppu.write_oam_address(value),
            Some(OAMDATA) => ppu.write_oam_data(value),
            Some(PPUSCROLL) => ppu.write_scroll(value),
            Some(PPUADDR) => ppu.write_address(value),
            Some(PPUDATA) => ppu.write_data(value),
            Some(OAMDMA) => {
                let cpu = unsafe { &mut *self.cpu };
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
    memory: [u8;32],
}

impl PaletteControl {
    pub fn new() -> PaletteControl {
        PaletteControl { memory: [0; 32 ] }
    }
    fn map_ptr(&self, ptr:u16) -> usize {
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
enum PaletteType { Sprite, Background }

impl Clocked for Ppu {
    fn clock(&mut self) {
        // https://wiki.nesdev.com/w/index.php/PPU_rendering
        if self.clocks_to_pause > 0 {
            self.clocks_to_pause -= 1;
            return;
        }
        let scanline = self.scanline;
        if scanline == 261 {
            self.render_scanline_prerender();
        } else if scanline < SCANLINE_POSTRENDER {
            self.render_scanline(scanline as u8);
            self.clocks_to_pause = 341;
        } else if scanline == SCANLINE_POSTRENDER {
            self.render_scanline_postrender();
            self.clocks_to_pause = 341;
        } else {
            self.render_scanline_vblank();
            self.clocks_to_pause = 341;
        }
        self.scanline += 1;
        if self.scanline > SCANLINE_PRERENDER {
            self.debug_print_attribute_table(0);
            self.scanline = 0;
        }
    }
}

// https://wiki.nesdev.com/w/index.php/PPU_rendering
impl Ppu {
    pub fn new() -> Ppu {
        let mapper = Mapper::new();
        Ppu {
            display: [0; RENDER_SIZE],
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
            vblank: false,
            open_bus: 0,
            ppu_master_select: false,
            generate_vblank_nmi: false,
            ppudata_buffer: 0,

            oam_ptr: 0,

            clocks_to_pause: 0,
            scanline: 0,
            tile_idx_shift: 0,
            tile_attribute_shift: 0,
        }
    }

    pub fn render(&self, buf: &mut [u8]) {
        buf.copy_from_slice(&self.display[0..RENDER_SIZE]);
    }

    pub fn render_scanline_prerender(&mut self) {
        if self.is_rendering_enabled() {
            self.registers.prerender_copy();
            self.clocks_to_pause = ternary(self.frame_parity, 341, 340)-1;
            self.sprite0_hit = false;
            self.shift_new_tile();
            self.shift_new_tile();
            //self.registers.y_increment(); // x=256 action
            //self.registers.reset_horizontal_position(); // x=257 action

        } else {
            self.clocks_to_pause = 341;
        }
    }

    pub fn render_scanline_vblank(&mut self) {
        if self.scanline == 241 {
            eprintln!("DEBUG - VBLANK HIT - {}", self.generate_vblank_nmi);
            self.set_vblank(true);
            if self.generate_vblank_nmi {
                self.is_vblank_nmi = true;
            }
        }
    }

    pub fn render_scanline_postrender(&mut self) {
    }

    fn shift_new_tile(&mut self) {
        let tile_address = self.registers.tile_address();
        let background_tile = self.peek(tile_address);
        let palette = self.lookup_attribute_table();
        self.tile_idx_shift >>= 8;
        self.tile_idx_shift |= (background_tile as u16) << 8;
        self.tile_attribute_shift >>= 8;
        self.tile_attribute_shift |= (palette as u16) << 8;
        self.registers.coarse_x_increment();
    }

    fn fetch_tile_from_shift(&self, x:u16, y:u16) -> (TileIndex, PaletteId, u8, u8) {
        let fine_x = self.registers.fine_x() as u16;
        let use_next_tile = (x%8 + fine_x) > 7;
        let tile_id = (self.tile_idx_shift >> ternary(use_next_tile,8,0))&0xff;
        let palette = (self.tile_attribute_shift >> ternary(use_next_tile,8,0))&0x3;
        let xsub = (x + fine_x as u16) % 8;
        return (tile_id as u8, palette as u8, xsub as u8, y as u8 % 8);
    }

    // https://wiki.nesdev.com/w/index.php/PPU_sprite_priority
    fn render_scanline(&mut self, scanline: u8) {
        if ! self.is_rendering_enabled() {
            return;
        }
        let global_background_color = self.lookup_global_background_color();
        let y = self.scanline as u16;
        // Each PPU clock cycle produces one pixel. The HBlank period is used to perform memory accesses.
        let sprites = self.fetch_scanline_sprites(y);
        for x in 0u16..=263 {
            self.registers.handle_scanline_x(x);
            if (x % 8) == 0 {
                self.shift_new_tile();
            }

            let (background_tile, background_palette, tile_xsub, tile_ysub) = self.fetch_tile_from_shift(x, y);
            let background_color = self.render_pattern_subpixel(background_tile, PaletteType::Background, background_palette, tile_xsub, tile_ysub);
            let (is_sprite_front, sprite_color) =
                match self.find_matching_sprite(x, &sprites) {
                    None => (false, None),
                    Some(sprite) => {
                        let xsub = (x.wrapping_sub(sprite.x as u16)) % 8;
                        let xsub = ternary(sprite.flip_horizontal, 7 - xsub, xsub);
                        // TODO: In 8x16 mode, use the next tile if ysub belongs in the lower half of the sprite.
                        let ysub = (y.wrapping_sub(sprite.y as u16)) % 8;
                        let ysub = ternary(sprite.flip_vertical, 7 - ysub, ysub);
                        let sprite_color = self.render_pattern_subpixel(sprite.tile_index, PaletteType::Sprite, sprite.palette, xsub as u8, ysub as u8);
                        // Sprite 0 test
                        if sprite.sprite_index == 0 &&
                            sprite_color.is_some() &&
                            background_color.is_some() {
                                self.sprite0_hit = true;
                            }

                        (sprite.is_front, sprite_color)
                    },
                };

        //let y = (y.wrapping_sub(self.registers.fine_y() as u16));
            if is_sprite_front && sprite_color.is_some() {
                // eprintln!("DEBUG - SPRITEF - {}", sprite_color.unwrap());
                self.write_system_pixel(x, y, sprite_color.unwrap());
            } else if background_color.is_some() {
                // eprintln!("DEBUG - BACKGROUND - {}", background_color.unwrap());
                self.write_system_pixel(x, y, background_color.unwrap());
            } else if sprite_color.is_some() {
                // eprintln!("DEBUG - SPRITEB - {}", sprite_color.unwrap());
                self.write_system_pixel(x, y, sprite_color.unwrap());
            } else {
                self.write_system_pixel(x, y, global_background_color);
            }
        }
        //self.registers.handle_scanline_x(256);
        //self.registers.handle_scanline_x(257);
        self.shift_new_tile();
        self.shift_new_tile();
        // self.registers.handle_scanline_x(328);
        // self.registers.handle_scanline_x(336);
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

    fn render_pattern_subpixel(&self, tile_index: u8, palette_type: PaletteType, palette: u8, xsub: u8, ysub: u8) -> Option<SystemColor> {
        let color = self.lookup_pattern_color(palette_type, tile_index, xsub, ysub);
        if color == COLOR_TRANSPARENT { return None; }
        let system_color = self.lookup_palette(palette, color);
        return Some(system_color);
    }

    fn lookup_palette(&self, palette: u8, color: u8) -> SystemColor {
        // https://wiki.nesdev.com/w/index.php/PPU_palettes
        let base_address = ADDRESS_BACKGROUND_PALETTE0;
        let palette_size = 4;
        let address:u16 = base_address + palette_size * (palette as u16) + (color as u16);
        return self.peek(address) & 0x3f;
    }

    fn lookup_pattern_color(&self, palette_type: PaletteType, tile_index: u8, xsub: u8, ysub: u8) -> PatternColor {
        // https://wiki.nesdev.com/w/index.php/PPU_pattern_tables
        let ptr_pattern_table_base = 0x0000;
        let size_pattern_table = 0x1000;
        let size_tile = 16;
        let is_pattern_table_right = match palette_type {
            PaletteType::Sprite => self.sprite_pattern_table,
            PaletteType::Background => self.background_pattern_table,
        };
        let ptr_tile:u16 =
            ptr_pattern_table_base +
            size_pattern_table * (is_pattern_table_right as u16) +
            (size_tile * tile_index as u16);
        let ptr_tile_row0 = ptr_tile + (ysub as u16);
        let ptr_tile_row1 = ptr_tile_row0 + 8; // The bits of the color id are stored in separate bit planes.
        let tile_row0 = self.peek(ptr_tile_row0);
        let tile_row1 = self.peek(ptr_tile_row1);
        let color =
            get_bit(tile_row0, 7 - xsub) << 0 |
            get_bit(tile_row1, 7 - xsub) << 1;
        return color;
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
        let attribute = self.oam[i*4 + 2];
        return Sprite {
            sprite_index: i as u8,
            y: self.oam[i*4 + 0],
            tile_index: self.oam[i*4 + 1],
            palette: (attribute & 3) + 4,
            is_front: get_bit(attribute, 5) == 0,
            flip_horizontal: get_bit(attribute, 6) > 0,
            flip_vertical: get_bit(attribute, 7) > 0,
            x: self.oam[i*4 + 3],
        };
    }

    fn lookup_global_background_color(&self) -> SystemColor {
        return self.peek(ADDRESS_UNIVERSAL_BACKGROUND_COLOR);
    }

    fn lookup_tile(&self) -> (PatternId, PaletteId) {
        let tile_addr = self.registers.tile_address();
        let tile = self.peek(tile_addr);
        let palette = self.lookup_attribute_table();

        return (tile, palette);
    }

    fn lookup_attribute_table(&self) -> PaletteId {
        let ptr = self.registers.attribute_address();
        let entry = self.peek(ptr);
        let idx_x = self.registers.tile_x();
        let idx_y = self.registers.tile_y();
        let (left, top) = ((idx_x % 4) < 2, (idx_y % 4) < 2);
        let palette_id = match (left, top) {
            (true,  true)  => (entry >> 0) & 0x3,
            (false, true)  => (entry >> 2) & 0x3,
            (true,  false) => (entry >> 4) & 0x3,
            (false, false) => (entry >> 6) & 0x3,
        };
        //eprintln!("DEBUG - ATTRIBUTE ENTRY - {:x} {}", entry, palette_id);
        return palette_id;
    }

    fn debug_print_attribute_table(&self, nametable:u8) {
        // for idx_x in 0..=32 {
        //     for idx_y in 0..=30 {
        //         let palette_id = self.lookup_attribute_table(nametable, idx_x, idx_y);
        //         //eprintln!("DEBUG - PALETTE ID - {} {} {}", idx_x, idx_y, palette_id);
        //     }
        // }
    }

    pub fn write_control(&mut self, v:u8) {
        self.registers.write_control(v);
        self.sprite_pattern_table = get_bit(v, 3)>0;
        self.background_pattern_table = get_bit(v,4)>0;
        self.sprite_size = get_bit(v,5)>0;
        self.ppu_master_select = get_bit(v,6)>0;
        self.generate_vblank_nmi = get_bit(v,7)>0;
    }

    pub fn write_mask(&mut self, v:u8) {
        self.registers.write_mask(v);
    }

    pub fn read_status(&mut self) -> u8 {
        let ret =
            (self.open_bus & 0b00011111) |
            ((self.sprite_overflow as u8) << 5) |
            ((self.sprite0_hit as u8) << 6) |
            ((self.vblank as u8) << 7)
            ;
        self.set_vblank(false);
        self.registers.read_status();
        return ret;
    }
    pub fn write_oam_address(&mut self, v:u8) {
        self.oam_ptr = v;
    }
    pub fn read_oam_data(&mut self) -> u8 {
        let ptr:u8 = self.oam_ptr;
        return self.oam[ptr as usize];
    }
    pub fn write_oam_data(&mut self, v:u8) {
        let ptr:u8 = self.oam_ptr;
        self.oam[ptr as usize] = v;
        self.oam_ptr = self.oam_ptr.wrapping_add(1);
    }
    pub fn write_scroll(&mut self, v:u8) {
        self.registers.write_scroll(v);
    }
    pub fn write_address(&mut self, v:u8) {
        self.registers.write_address(v);
    }
    pub fn read_data(&mut self) -> u8 {
        let ptr = self.registers.vram_ptr();
        let val = self.peek(ptr);
        if ptr < 0x3f00 {
            let old_val = self.ppudata_buffer;
            self.ppudata_buffer = val;
            old_val
        } else {
            val
        }
    }
    pub fn write_data(&mut self, v:u8) {
        let ptr = self.registers.vram_ptr();
        self.registers.advance_vram_ptr();
        eprintln!("DEBUG - PPU WRITE DATA - {:x} {:x} {:x}", ptr, v, self.registers.vram_ptr());
        self.poke(ptr, v);
    }
    fn lookup_system_pixel(&self, i: SystemColor) -> RgbColor {
        return SYSTEM_PALETTE[i as usize];
    }
    fn write_system_pixel(&mut self, x: u16, y: u16, c: SystemColor) {
        if x >= 256 || y >= 240 {
            return;
        }
        let (r,g,b) = self.lookup_system_pixel(c);
        let xz = x as usize;
        let yz = y as usize;
        let i1 = 3*(xz+(256*yz))+0;
        let i2 = 3*(xz+(256*yz))+1;
        let i3 = 3*(xz+(256*yz))+2;
        // eprintln!("DEBUG - ({} {}) ({} {}) ({} {})", i1, r, i2, g, i3, b);
        self.display[i1] = r;
        self.display[i2] = g;
        self.display[i3] = b;
    }
    fn set_vblank(&mut self, new_vblank: bool) {
        let vblank = self.vblank;
        if vblank != new_vblank {
            eprintln!("DEBUG - VBLANK CHANGED FROM {:?} TO {:?}", vblank, new_vblank);
        }
        self.vblank = new_vblank;
    }
}

type RgbColor = (u8, u8, u8);
type SystemPalette = [RgbColor; 64];

// The NES can refer to 64 separate colors. This table has RGB values for each.
pub const SYSTEM_PALETTE:SystemPalette =
    [
        // 0x
        (124, 124, 124), // x0
        (0,   0,   252), // x1
        (0,   0,   188), // x2
        (68,  40,  188), // x3
        (148, 0,   132), // x4
        (168, 0,   32),  // x5
        (168, 16,  0),   // x6
        (136, 20,  0),   // x7
        (80,  48,  0),   // x8
        (0,   120, 0),   // x9
        (0,   104, 0),   // xA
        (0,   88,  0),   // xB
        (0,   64,  88),  // xC
        (0,   0,   0),   // xD
        (0,   0,   0),   // xE
        (0,   0,   0),   // xF
        // 1x
        (188, 188, 188), // x0
        (0,   120, 248), // x1
        (0,   88,  248), // x2
        (104, 68,  252), // x3
        (216, 0,   204), // x4
        (228, 0,   88),  // x5
        (248, 56,  0),   // x6
        (228, 92,  16),  // x7
        (172, 124, 0),   // x8
        (0,   184, 0),   // x9
        (0,   168, 0),   // xA
        (0,   168, 68),  // xB
        (0,   136, 136), // xC
        (0,   0,   0),   // xD
        (0,   0,   0),   // xE
        (0,   0,   0),   // xF
        // 2x
        (248, 248, 248), // x0
        (60,  188, 252), // x1
        (104, 136, 252), // x2
        (152, 120, 248), // x3
        (248, 120, 248), // x4
        (248, 88,  152), // x5
        (248, 120, 88),  // x6
        (252, 160, 68),  // x7
        (248, 184, 0),   // x8
        (184, 248, 24),  // x9
        (88,  216, 84),  // xA
        (88,  248, 152), // xB
        (0,   232, 216), // xC
        (120, 120, 120), // xD
        (0,   0,   0),   // xE
        (0,   0,   0),   // xF
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
        (0,   252, 252), // xC
        (216, 216, 216), // xD
        (0,   0,   0),   // xE
        (0,   0,   0),   // xF
    ];
