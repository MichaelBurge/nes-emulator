mod common;
mod mapper;

use common::*;
use mapper::{AddressSpace};

//use std::vec;

type SystemColor = u8; // [0,64)
type PatternColor = u8; // [0,4)
type PatternId = u8;
type PaletteId = u8; // [0, 4)
type TileIndex = u8;

const COLOR_INVALID:u8 = 255;
const COLOR_TRANSPARENT:u8 = 0;

const ADDRESS_NAMETABLE0:u16 = 0x2000;
const ADDRESS_ATTRIBUTE_TABLE0:u16 = 0x23C0;
const NAMETABLE_SIZE:u16 = 0x0400;
const ADDRESS_UNIVERSAL_BACKGROUND_COLOR:u16 = 0x3f00;
const ADDRESS_BACKGROUND_PALETTE0:u16 = 0x3f01;
const ADDRESS_SPRITE_PALETTE0:u16 = 0x3f11;
const SPRITE_HEIGHT:u8 = 8;
const SPRITE_WIDTH:u8 = 8;
const RENDER_WIDTH:usize = 256;
const RENDER_HEIGHT:usize = 240;
const RENDER_SIZE:usize = RENDER_WIDTH * RENDER_HEIGHT;


struct Ppu {
    data_bus: u8,
    ram: [u8; 2048 ],
    oam: [u8; 256],
    // Registers
    frame_parity: bool, // Toggled every frame
    control: u8, // PPUCTRL register
    mapper: AddressSpace,
}

enum PpuPort {
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

#[derive(Copy, Clone)]
enum PaletteType { Sprite, Background }

// https://wiki.nesdev.com/w/index.php/PPU_rendering
impl Ppu {
    fn render(&self, buffer: &mut [u8; RENDER_SIZE]) {
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
        let address:u16 = base_address + palette_size * (palette as u16);
        return self.peek(address);
    }

    fn lookup_pattern_color(&self, palette_type: PaletteType, tile_index: u8, xsub: u8, ysub: u8) -> PatternColor {
        // https://wiki.nesdev.com/w/index.php/PPU_pattern_tables
        let ptr_pattern_table_base = 0x0000;
        let size_pattern_table = 0x1000;
        let size_tile = 16;
        let is_pattern_table_right = match palette_type {
            PaletteType::Sprite => self.is_sprite_pattern_table_right(),
            PaletteType::Background => self.is_background_pattern_table_right(),
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

    fn is_sprite_pattern_table_right(&self) -> bool {
        return get_bit(self.control, 3) > 0;
    }
    fn is_background_pattern_table_right(&self) -> bool {
        return get_bit(self.control, 4) > 0;
    }
}
