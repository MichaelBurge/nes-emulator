#![allow(unused_imports)]

use crate::common::*;
use crate::mapper::{AddressSpace, Mapper};
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
const ADDRESS_BACKGROUND_PALETTE0:u16 = 0x3f01;
const ADDRESS_SPRITE_PALETTE0:u16 = 0x3f11;
const SPRITE_HEIGHT:u8 = 8;
const SPRITE_WIDTH:u8 = 8;
pub const RENDER_WIDTH:usize = 256;
pub const RENDER_HEIGHT:usize = 240;
pub const RENDER_SIZE:usize = RENDER_WIDTH * RENDER_HEIGHT * 3;

pub struct Ppu {
    pub display: [u8; RENDER_SIZE],
    pub oam: [u8; 256],
    pub mapper: Box<dyn AddressSpace>,
    pub is_vblank_nmi: bool,
    pub is_scanline_irq: bool,

    base_nametable: u8,
    vram_ptr: u16, // PPUADDR
    vram_ptr_increment: u8,
    sprite_pattern_table: bool, // Is the sprite pattern table the 'right' one?
    background_pattern_table: bool, // Is the background pattern table the right one?
    sprite_overflow: bool,
    sprite0_hit: bool,
    sprite_size: bool,
    frame_parity: bool, // Toggled every frame
    vblank: bool,
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

    // Lift state in the render loop to PPU-level registers to allow running a render clock-by-clock.
    /* clocks_to_pause causes the PPU to do nothing for X clocks. This should be unnecessary in a
       clock-accurate emulation. However, this emulator may choose to "batch" the actions of many clocks
       all at once, and fill the remaining clocks with do-nothing operations. */
    clocks_to_pause: u16,
    scanline: u16,
    next_tile_reg: u8,
    next_tile_reg2: u8,
    next_pixel_reg: u8,
    next_pixel_reg2: u8,
    next_tile_attributes_reg: u8,
    next_tile_attributes_reg2: u8,
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
        CpuPpuInterconnect { ppu: ppu }
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
            Some(PPUADDR) => ppu.write_address(value),
            Some(PPUDATA) => ppu.write_data(value),
            port => panic!("Unimplemented PPU Port {:?}", port),
        }
    }
}
#[derive(Copy, Clone)]
enum PaletteType { Sprite, Background }

const SCANLINE_PRERENDER:u16 = 261;
const SCANLINE_RENDER:u16 = 0;
const SCANLINE_POSTRENDER:u16 = 240;
const SCANLINE_VBLANK:u16 = 241;

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
            let mut buf = [0;256];
            self.render_scanline(scanline as u8, &mut buf);
            let ptr_base = 256 * (scanline as usize);
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

            base_nametable: 0,
            vram_ptr: 0,
            vram_ptr_increment: 1,
            sprite_pattern_table: false,
            background_pattern_table: false,
            sprite_overflow: false,
            sprite0_hit: false,
            sprite_size: false,
            frame_parity: false,
            vblank: false,
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

            clocks_to_pause: 0,
            scanline: 0,
            next_tile_reg: 0,
            next_tile_reg2: 0,
            next_pixel_reg: 0,
            next_pixel_reg2: 0,
            next_tile_attributes_reg: 0,
            next_tile_attributes_reg2: 0,
        }
    }

    pub fn render(&self, buf: &mut [u8]) {
        buf.copy_from_slice(&self.display[0..RENDER_SIZE]);
    }

    pub fn render_scanline_prerender(&mut self) {
        if self.is_rendering_enabled() {
            self.clocks_to_pause = ternary(self.frame_parity, 341, 340)-1;
            // TODO - "During pixels 280 through 304 of this scanline, the vertical scroll bits are reloaded if rendering is enabled. "
            // TODO - Make memory accesses that a regular scanline would
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

    // https://wiki.nesdev.com/w/index.php/PPU_sprite_priority
    fn render_scanline(&mut self, y: u8, buf: &mut[u8]) {
        let ptr_base = 256 * (y as usize);
        let global_background_color = self.lookup_global_background_color();
        // Each PPU clock cycle produces one pixel. The HBlank period is used to perform memory accesses.
        let sprites = self.fetch_scanline_sprites(y);
        for x in 0..255 {

            let (background_tile, background_palette) = self.lookup_tile(x,y);
            let background_color = self.render_pattern_subpixel(background_tile, PaletteType::Background, background_palette, x % 8, y % 8);

            let (is_sprite_front, sprite_color) =
                match self.find_matching_sprite(x, &sprites) {
                    None => (false, None),
                    Some(sprite) => {
                        // TODO: Horizontal/vertical flipping
                        let xsub = (x.wrapping_sub(sprite.x)) % 8;
                        // TODO: In 8x16 mode, use the next tile if ysub belongs in the lower half of the sprite.
                        let ysub = (x.wrapping_sub(sprite.y)) % 8;
                        let sprite_color = self.render_pattern_subpixel(sprite.tile_index, PaletteType::Sprite, sprite.palette, xsub, ysub);
                        (sprite.is_front, sprite_color)
                    },
                };

            if is_sprite_front && sprite_color.is_some() {
                self.write_system_pixel(x, y, sprite_color.unwrap());
            } else if background_color.is_some() {
                self.write_system_pixel(x, y, background_color.unwrap());
                buf[x as usize] = background_color.unwrap();
            } else if sprite_color.is_some() {
                self.write_system_pixel(x, y, sprite_color.unwrap());
            } else {
                self.write_system_pixel(x, y, global_background_color);
            }
        }
    }

    fn render_pattern_subpixel(&self, tile_index: u8, palette_type: PaletteType, palette: u8, xsub: u8, ysub: u8) -> Option<SystemColor> {
        let color = self.lookup_pattern_color(palette_type, tile_index, xsub, ysub);
        if color == COLOR_TRANSPARENT { return None; }
        let system_color = self.lookup_palette(palette, palette_type, color);
        return Some(system_color);
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
        self.generate_vblank_nmi = get_bit(v,7)>0;
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
            ((self.vblank as u8) << 7)
            ;
        self.set_vblank(false);
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
            self.vram_ptr |= (v as u16) << 0;
        } else {
            self.vram_ptr  = (v as u16) << 8;
        }
        eprintln!("DEBUG - PPU WRITE ADDRESS - {} {} {:x}", v, self.address_latch_set, self.vram_ptr);
        self.address_latch_set = true;
    }
    pub fn read_data(&mut self) -> u8 {
        let ptr = self.vram_ptr;
        return self.peek(ptr);
    }
    pub fn write_data(&mut self, v:u8) {
        let ptr = self.vram_ptr;
        eprintln!("DEBUG - PPU WRITE DATA - {}", v);
        self.poke(ptr, v);
        let increment = self.vram_ptr_increment;
        self.vram_ptr = self.vram_ptr.wrapping_add(increment as u16);
    }
    pub fn is_rendering_complete(&self) -> bool {
        return self.scanline >= 240;
    }
    pub fn is_rendering_enabled(&self) -> bool {
        return self.show_background || self.show_sprites;
    }
    fn lookup_system_pixel(&self, i: SystemColor) -> RgbColor {
        return SYSTEM_PALETTE[i as usize];
    }
    fn write_system_pixel(&mut self, x: u8, y: u8, c: SystemColor) {
        // c = 22;
        let (r,g,b) = self.lookup_system_pixel(c);
        let xz = x as usize;
        let yz = y as usize;
        let i1 = xz+(256*yz)+0;
        let i2 = xz+(256*yz)+1;
        let i3 = xz+(256*yz)+2;
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
