#![allow(unused_imports)]

use crate::common::*;
use crate::mapper::{AddressSpace, Mapper};

//use std::vec;

type SystemColor = u8; // [0,64)
type PatternColor = u8; // [0,4)
type PatternId = u8;
type PaletteId = u8; // [0, 4)
type TileIndex = u8;

const COLOR_TRANSPARENT:u8 = 0;

const ADDRESS_NAMETABLE0:u16 = 0x2000;
const ADDRESS_ATTRIBUTE_TABLE0:u16 = 0x23C0;
const NAMETABLE_SIZE:u16 = 0x0400;
const ADDRESS_UNIVERSAL_BACKGROUND_COLOR:u16 = 0x3f00;
const ADDRESS_BACKGROUND_PALETTE0:u16 = 0x3f01;
const ADDRESS_SPRITE_PALETTE0:u16 = 0x3f11;
const SPRITE_HEIGHT:u8 = 8;
const SPRITE_WIDTH:u8 = 8;
pub const RENDER_WIDTH:usize = 256;
pub const RENDER_HEIGHT:usize = 240;

pub struct Ppu {
    pub oam: [u8; 256],
    pub mapper: Box<dyn AddressSpace>,

    base_nametable: u8,
    vram_ptr: u16, // PPUADDR
    vram_ptr_increment: u8,
    sprite_pattern_table: bool, // Is the sprite pattern table the 'right' one?
    background_pattern_table: bool, // Is the background pattern table the right one?
    sprite_overflow: bool,
    sprite0_hit: bool,
    sprite_size: bool,
    frame_parity: bool, // Toggled every frame
    vblank_started: bool,
    open_bus: u8, // Open bus shared by all PPU registers
    scroll_x: u8, // PPUSCROLL
    scroll_y: u8,
    address_latch_set: bool,
    ppu_master_select: bool,
    generate_vblank_nmi: bool,

    is_greyscale: bool,
    show_leftmost_background: bool,
    show_leftmost_sprite: bool,
    show_background: bool,
    show_sprites: bool,
    emphasize_red: bool,
    emphasize_green: bool,
    emphasize_blue: bool,

    oam_ptr: u8,
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
    x: u8,
    y: u8,
    tile_index: u8,
    palette: u8,
    is_front: bool, // priority
    flip_horizontal: bool,
    flip_vertical: bool,
}

pub struct CpuPpuInterconnect {
    ppu: *mut Ppu,
}

impl CpuPpuInterconnect {
    pub fn new(ppu: &mut Ppu) -> CpuPpuInterconnect {
        CpuPpuInterconnect { ppu: ppu as &mut Ppu }
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
            port => panic!("Unimplemented PPU Port {:?}", port),
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
            Some(PPUDATA) => ppu.write_data(value),
            port => panic!("Unimplemented PPU Port {:?}", port),
        }
    }
}
#[derive(Copy, Clone)]
enum PaletteType { Sprite, Background }

// https://wiki.nesdev.com/w/index.php/PPU_rendering
impl Ppu {
    pub fn new() -> Ppu {
        let mapper = Mapper::new();
        Ppu {
            oam: [0; 256],
            mapper: Box::new(mapper),

            base_nametable: 0,
            vram_ptr: 0,
            vram_ptr_increment: 1,
            sprite_pattern_table: false,
            background_pattern_table: false,
            sprite_overflow: false,
            sprite0_hit: false,
            sprite_size: false,
            frame_parity: false,
            vblank_started: false,
            open_bus: 0,
            scroll_x: 0,
            scroll_y: 0,
            address_latch_set: false,
            ppu_master_select: false,
            generate_vblank_nmi: false,

            is_greyscale: false,
            show_leftmost_background: false,
            show_leftmost_sprite: false,
            show_background: false,
            show_sprites: false,
            emphasize_red: false,
            emphasize_green: false,
            emphasize_blue: false,

            oam_ptr: 0,
        }
    }
    pub fn render(&self, buffer: &mut [u8]) {
        for y in 0..239 {
            let ptr_base:usize = 256 * (y as usize);
            self.render_scanline(y, &mut buffer[ptr_base .. ptr_base+256]);
        }
    }

    // https://wiki.nesdev.com/w/index.php/PPU_sprite_priority
    fn render_scanline(&self, y: u8, b: &mut [u8]) {
        let global_background_color = self.lookup_global_background_color();
        // Each PPU clock cycle produces one pixel. The HBlank period is used to perform memory accesses.
        let sprites = self.fetch_scanline_sprites(y);
        for x in 0..255 {

            let (background_tile, background_palette) = self.lookup_tile(x,y);
            let background_color = self.render_pattern_subpixel(background_tile, PaletteType::Background, background_palette, x % 8, y % 8);

            let (is_sprite_front, sprite_color) =
                match self.find_matching_sprite(x, &sprites) {
                    None => (false, COLOR_TRANSPARENT),
                    Some(sprite) => {
                        // TODO: Horizontal/vertical flipping
                        let xsub = (x.wrapping_sub(sprite.x)) % 8;
                        // TODO: In 8x16 mode, use the next tile if ysub belongs in the lower half of the sprite.
                        let ysub = (x.wrapping_sub(sprite.y)) % 8;
                        let sprite_color = self.render_pattern_subpixel(sprite.tile_index, PaletteType::Sprite, sprite.palette, xsub, ysub);
                        (sprite.is_front, sprite_color)
                    },
                };

            let is_sprite_opaque = sprite_color != COLOR_TRANSPARENT;
            let is_background_opaque = background_color != COLOR_TRANSPARENT;
            if is_sprite_front && is_sprite_opaque {
                b[x as usize] = sprite_color;
            } else if is_background_opaque {
                b[x as usize] = background_color;
            } else if is_sprite_opaque {
                b[x as usize] = sprite_color;
            } else {
                b[x as usize] = global_background_color;
            }
        }
    }

    fn render_pattern_subpixel(&self, tile_index: u8, palette_type: PaletteType, palette: u8, xsub: u8, ysub: u8) -> SystemColor {
        let color = self.lookup_pattern_color(palette_type, tile_index, xsub, ysub);
        let system_color = self.lookup_palette(palette, palette_type, color);
        return system_color;
    }

    fn lookup_palette(&self, palette: u8, palette_type: PaletteType, color: u8) -> SystemColor {
        // https://wiki.nesdev.com/w/index.php/PPU_palettes
        let base_address =
            match palette_type {
                PaletteType::Sprite => ADDRESS_SPRITE_PALETTE0,
                PaletteType::Background => ADDRESS_BACKGROUND_PALETTE0,
            };
        let palette_size = 4;
        let address:u16 = base_address + palette_size * (palette as u16) + ((color - 1) as u16);
        return self.peek(address);
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
            get_bit(tile_row0, xsub) << 0 +
            get_bit(tile_row1, xsub) << 1;
        return color;
    }

    fn fetch_scanline_sprites(&self, y: u8) -> Vec<Sprite> {
        let mut vec = Vec::new();
        for i in 0..64 {
            let sprite = self.lookup_sprite(i);
            if y >= sprite.y && y < (sprite.y + SPRITE_HEIGHT) {
                vec.push(sprite);
            }
            if vec.len() >= 8 {
                return vec;
            }
        }
        return vec;
    }

    fn find_matching_sprite(&self, x: u8, sprites: &Vec<Sprite>) -> Option<Sprite> {
        for sprite in sprites {
            if x >= sprite.x && x < (sprite.x + SPRITE_WIDTH) {
                return Some(sprite.clone());
            }
        }
        return None;
    }

    fn lookup_sprite(&self, i: usize) -> Sprite {
        let attribute = self.oam[i*4 + 2];
        return Sprite {
            y: self.oam[i*4 + 0],
            tile_index: self.oam[i*4 + 1],
            palette: get_bit(attribute, 0) + get_bit(attribute, 1) << 1,
            is_front: get_bit(attribute, 5) == 0,
            flip_horizontal: get_bit(attribute, 6) > 0,
            flip_vertical: get_bit(attribute, 7) > 0,
            x: self.oam[i*4 + 3],
        };
    }

    fn lookup_global_background_color(&self) -> SystemColor {
        return self.peek(ADDRESS_UNIVERSAL_BACKGROUND_COLOR);
    }

    fn lookup_tile(&self, x:u8, y:u8) -> (PatternId, PaletteId) {
        let nametable = 0; // TODO - Implement scrolling
        let idx_x = x/32;
        let idx_y = y/32;

        let tile = self.lookup_nametable(nametable, idx_x, idx_y);
        let palette = self.lookup_attribute_table(nametable, idx_x, idx_y);

        return (tile, palette);
    }

    fn lookup_nametable(&self, nametable:u8, idx_x:u8, idx_y:u8) -> PatternId {
        let idx = idx_y * 32 + idx_x;
        let ptr =
            ADDRESS_NAMETABLE0 +
            NAMETABLE_SIZE*(nametable as u16) +
            (idx as u16);
        return self.peek(ptr);
    }

    fn lookup_attribute_table(&self, nametable:u8, x:u8, y:u8) -> PaletteId {
        let idx_x = x/32;
        let idx_y = x/32;
        let idx = idx_y * 32 + idx_x;
        let ptr =
            ADDRESS_ATTRIBUTE_TABLE0 +
            NAMETABLE_SIZE*(nametable as u16) +
            (idx as u16);
        let entry = self.peek(ptr);
        let quadrant = (x % 32) / 16 + 2*(y % 32) / 16;
        let palette_id =
            get_bit(entry, quadrant*2) << 0 +
            get_bit(entry, quadrant*2+1) << 1;
        return palette_id;
    }

    pub fn write_control(&mut self, v:u8) {
        self.base_nametable = v & 0b11;
        self.vram_ptr_increment = ternary(get_bit(v, 2) > 0, 32, 1);
        self.sprite_pattern_table = get_bit(v, 3)>0;
        self.background_pattern_table = get_bit(v,4)>0;
        self.sprite_size = get_bit(v,5)>0;
        self.ppu_master_select = get_bit(v,6)>0;
        self.generate_vblank_nmi = get_bit(v,6)>0;
    }
    pub fn write_mask(&mut self, v:u8) {
        self.is_greyscale = get_bit(v,0)>0;
        self.show_leftmost_background = get_bit(v,1)>0;
        self.show_leftmost_sprite = get_bit(v,2)>0;
        self.show_background = get_bit(v,3)>0;
        self.show_sprites = get_bit(v,4)>0;
        self.emphasize_red = get_bit(v,5)>0;
        self.emphasize_green = get_bit(v,6)>0;
        self.emphasize_blue = get_bit(v,7)>0;
    }

    pub fn read_status(&mut self) -> u8 {
        let ret =
            (self.open_bus & 0b00011111) |
            ((self.sprite_overflow as u8) << 5) |
            ((self.sprite0_hit as u8) << 6) |
            ((self.vblank_started as u8) << 7)
            ;
        self.vblank_started = false;
        self.address_latch_set = false;
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
        if self.address_latch_set {
            self.scroll_y = v;
        } else {
            self.scroll_x = v;
        }
        self.address_latch_set = true;
    }
    pub fn write_address(&mut self, v:u8) {
        if self.address_latch_set {
            self.vram_ptr &= 0xff00;
            self.vram_ptr |= (v as u16) << 8;
        } else {
            self.vram_ptr &= 0x00ff;
            self.vram_ptr |= (v as u16) << 0;
        }
        self.address_latch_set = true;
    }
    pub fn read_data(&mut self) -> u8 {
        let ptr = self.vram_ptr;
        return self.peek(ptr);
    }
    pub fn write_data(&mut self, v:u8) {
        let ptr = self.vram_ptr;
        self.poke(ptr, v);
        let increment = self.vram_ptr_increment;
        self.vram_ptr = self.vram_ptr.wrapping_add(increment as u16);
    }
}
