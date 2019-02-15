use "common.rs";

use std::vec;

type SystemColor = u8; // [0,64)
type PatternColor = u8; // [0,4)
type PaletteId = u8; // [0,4)

const COLOR_INVALID = 255;
const COLOR_TRANSPARENT = 0;

const ADDRESS_UNIVERSAL_BACKGROUND_COLOR = 0x3f00;
const ADDRESS_BACKGROUND_PALETTE0 = 0x3f01;
const ADDRESS_SPRITE_PALETTE0 = 0x3f11;
const SPRITE_HEIGHT = 8;
const SPRITE_WIDTH = 8;

struct Ppu {
    data_bus: u8,
    ram: [u8; 2048 ],
    oam: [u8; 256],
    mapper: AddressSpace,
    // Registers
    frame_parity: bool, // Toggled every frame
    control: u8, // PPUCTRL register
}

enum PpuPort {
    PPUCTRL, PPUMASK, PPUSTATUS,
    OAMADDR, OAMDATA, PPUSCROLL,
    PPUADDR, PPUDATA, OAMDMA,
}

impl AddressSpace for Ppu {
    fn peek(&self, ptr) { return self.mapper.peek(ptr); }
    fn poke(&self, ptr, v) { self.mapper.poke(ptr, v); }
}

struct Sprite {
    x: u8,
    y: u8,
    tile_index: u8,
    palette: u8,
    is_front: bool, // priority
    flip_horizontal: bool,
    flip_vertical: bool,
}

enum PaletteType { PALETTE_SPRITE, PALETTE_BACKGROUND };

// https://wiki.nesdev.com/w/index.php/PPU_rendering
const RENDER_WIDTH = 256;
const RENDER_HEIGHT = 240;
const RENDER_SIZE = RENDER_WIDTH * RENDER_HEIGHT;
impl Ppu {
    fn render(&self, buffer: &mut [u8; RENDER_SIZE]) {
        for y in range 0..239 {
            let ptr_base = 256 * y;
            self.render_scanline(y, buffer[ptr_base .. ptr_base+256]);
        }
    }

    // https://wiki.nesdev.com/w/index.php/PPU_sprite_priority
    fn render_scanline(&self, y: u8, b: &[u8; 256]) {
        let global_background_color = self.lookup_global_background_color();
        // Each PPU clock cycle produces one pixel. The HBlank period is used to perform memory accesses.
        let sprites = self.fetch_scanline_sprites(y);
        for x in 0..255 {

            let background_tile = self.lookup_nametable(x,y);
            let background_palette = self.lookup_attribute_table(x,y);
            let background_color = self.render_pattern_subpixel(background_tile, PALETTE_BACKGROUND, background_palette, x % 8, y % 8);

            let (is_sprint_front, sprite_color) =
                match self.find_matching_sprite(&sprites, x) {
                    None => (false, COLOR_TRANSPARENT),
                    Some(sprite) => {
                        // TODO: Horizontal/vertical flipping
                        let xsub = (wrapping_sub(x, sprite.x)) % 8;
                        // TODO: In 8x16 mode, use the next tile if ysub belongs in the lower half of the sprite.
                        let ysub = (wrapping_sub(y, sprite.y)) % 8;
                        let sprite_color = self.render_pattern_subpixel(sprite.tile_index, PALETTE_SPRITE, sprite.palette, xsub, ysub);
                        return (sprite.front, sprite_color);
                    },
                };

            let is_sprite_opaque = (sprite_color != COLOR_TRANSPARENT);
            let is_background_opaque = (background_color != COLOR_TRANSPARENT);
            if is_sprite_front and is_sprite_opaque {
                b[x] = sprite_color;
            } else if is_background_opaque {
                b[x] = background_color;
            } else if is_sprite_opaque {
                b[x] = sprite_color;
            } else {
                b[x] = global_background_color;
            }
        }
    }

    fn render_pattern_subpixel(&self, tile_index: u8, palette_type: u8, palette: u8, xsub: u8, ysub: u8) -> SystemColor {
        let color = self.lookup_pattern_color(palette_type, tile_index, xsub, ysub);
        let system_color = self.lookup_palette(palette, palette_type, color);
        return system_color;
    }

    fn lookup_system_pixel(&self, i: SystemColor) -> RgbColor {
        return self.system_palette[i];
    }

    fn lookup_palette(&self, palette: u8, palette_type: PaletteType, color: u8) -> SystemColor {
        // https://wiki.nesdev.com/w/index.php/PPU_palettes
        let base_address =
            match palette_type {
                PALETTE_SPRITE => ADDRESS_SPRITE_PALETTE0,
                PALETTE_BACKGROUND => ADDRESS_BACKGROUND_PALETTE0,
            };
        let palette_size = 4;
        let address = base_address + palette_size * palette;
        return self.peek(address);
    }

    fn lookup_pattern_color(&self, palette_type: PaletteType, tile_index: u8, xsub: u8, ysub: u8) -> PatternColor {
        // https://wiki.nesdev.com/w/index.php/PPU_pattern_tables
        let ptr_pattern_table_base = 0x0000;
        let size_pattern_table = 0x1000;
        let size_tile = 16;
        let is_pattern_table_right = match palette_type {
            PALETTE_SPRITE => self.is_sprite_pattern_table_right(),
            PALETTE_BACKGROUND => self.is_background_pattern_table_right(),
        };
        let ptr_tile:u8 =
            ptr_pattern_table_base +
            size_pattern_table * (is_pattern_table_right as u16) +
            size_tile * tile_index;
        let ptr_tile_row0 = ptr_tile + ysub;
        let ptr_tile_row1 = ptr_tile_row0 + 8; // The bits of the color id are stored in separate bit planes.
        let tile_row0 = self.peek(ptr_tile_row0);
        let tile_row1 = self.peek(ptr_tile_row1);
        let color =
            tile_row0 << 0 +
            tile_row1 << 1;
        return color;
    }

    fn fetch_scanline_sprites(&self, y: u8) -> Vec<Sprite> {
        let mut vec = Vec::new();
        for i in 0..64 {
            let sprite = self.lookup_sprite(i);
            if y >= sprite.y && y < (sprite.y + SPRITE_HEIGHT) {
                vec.push(sprite);
            }
            if vec.size() >= 8 {
                return vec;
            }
        }
        return vec;
    }

    fn find_matching_sprite(&self, x: u8, sprites: &Vec<Sprite>) -> Option<Sprite> {
        for sprite in sprites {
            if x >= sprite.x && x < (sprite.x + SPRITE_WIDTH) {
                return Some(sprite);
            }
        }
        return None;
    }

    fn lookup_global_background_color(&self) -> SystemColor {
        self.peek(ADDRESS_UNIVERSAL_BACKGROUND_COLOR);
    }

    fn is_sprite_pattern_table_right(&self) -> bool {
        return get_bit(self.control, 3);
    }
    fn is_background_pattern_table_right(&self) -> bool {
        return get_bit(self.control, 4);
    }
}
