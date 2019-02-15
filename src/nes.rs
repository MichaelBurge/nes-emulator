use "common.rs";
use "c6502.rs";
use "ppu.rs";

struct Nes {
    cpu: C2a03,
    apu: Rp2a02,
    ram: [u8; 2048],
    ppu: Ppu,
    j1: Joystick,
    j2: Joystick,
    cartridge: [u8;15000],
}

enum Mapper {
}

struct Rom {
    mapper: Mapper,
}

fn read_ines(filename: String) -> Rom {
}

impl Nes {
    fn load_rom(r: &Rom) {

    }
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

fn map_apu_port(ptr: u16) -> Option<ApuPort> {
    match ptr {
        0x4000 => Some(SQ1_VOL),
        0x4001 => Some(SQ1_SWEEP),
        0x4002 => Some(SQ1_LO),
        0x4003 => Some(SQ1_HI),
        0x4004 => Some(SQ2_VOL),
        0x4005 => Some(SQ2_SWEEP),
        0x4006 => Some(SQ2_LO),
        0x4007 => Some(SQ2_HI),
        0x4008 => Some(TRI_LINEAR),
        0x4009 => Some(APU_DUMMY1),
        0x400A => Some(TRI_LO),
        0x400B => Some(TRI_HI),
        0x400C => Some(NOISE_VOL),
        0x400D => Some(APU_DUMMY2),
        0x400E => Some(NOISE_LO),
        0x400F => Some(NOISE_HI),
        0x4010 => Some(DMC_FREQ),
        0x4011 => Some(DMC_RAW),
        0x4012 => Some(DMC_START),
        0x4013 => Some(DMC_LEN),
        0x4014 => Some(SND_CHN),
        _      => None,
    }
}

impl Clocked for Nes {
    fn clock(&mut self) {
        self.cpu.clock();
        for i in 1..3 { self.ppu.clock(); }
    }
}

// https://wiki.nesdev.com/w/index.php/CPU_memory_map
impl AddressSpace for Nes {
    fn peek(&self, ptr) -> u8 {
        if let Some(base_ptr) = mirrored_lea(ptr, 0x0000, 0x07ff, 0x0000, 0x1fff) {
            return self.ram[base_ptr];
        }
        if let Some(base_ptr) = mirrored_lea(ptr, 0x2000, 0x2007, 0x2000, 0x3fff) {
            return self.ppu.signal_read(map_ppu_port(base_ptr).expect("Unknown PPU Port"));
        }
        if let apu_port = map_apu_port(ptr) {
            return self.apu.signal_read(apu_port);
        }
        if ptr == 0x4016 { return self.joystick1.signal_read(); }
        if ptr == 0x4017 { return self.joystick2.signal_read(); }
        if ptr >= 0x4018 && ptr <= 0x401F { return 0; /* UNUSED REGISTERS */ }
        return cartridge[ptr - 0x4020];
    }
    fn poke(&mut self, ptr, v) {
        if let Some(base_ptr) = mirrored_lea(ptr, 0x0000, 0x07ff, 0x0000, 0x1fff) {
            self.ram[base_ptr] = v;
            return;
        }
        if let Some(base_ptr) = mirrored_lea(ptr, 0x2000, 0x2007, 0x2000, 0x3fff) {
            let port = map_ppu_port(base_ptr).expect("Unknown PPU Port");
            return self.ppu.signal_write(port, v);
        }
        // TODO: OAMDMA should initiate a memory transfer
        if let apu_port = map_apu_port(ptr) {
            return self.apu.signal_write(apu_port);
        }
        if ptr == 0x4016 { return self.joystick1.signal_write(v); }
        if ptr == 0x4017 { return self.joystick2.signal_write(v); }
        if ptr >= 0x4018 && ptr <= 0x401F { return; /* UNUSED REGISTERS */ }
        return; // Cartridge is read-only
    }
}

// https://wiki.nesdev.com/w/index.php/2A03
