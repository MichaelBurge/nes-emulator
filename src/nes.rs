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

struct Rom {
    mapper: Mapper,
}

fn read_byte(fh: File) -> u8 {
    let bs:[u8;1] = [0];
    fh.read_exact(&bs);
    return bs[0];
}

struct Ines {
    num_prg_chunks: u8,
    num_chr_chunks: u8,
    mapper: u8,
    mirroring: bool,
    has_battery_backed_ram: bool,
    has_trainer: bool,
    has_four_screen_vram: bool,
    is_vs_unisystem: bool,
    is_playchoice10: bool,
    prg_rom: Vec<u8>;
}

fn read_ines(filename: String) -> Ines {
    // https://wiki.nesdev.com/w/index.php/INES
    let mut file = File::open(filename);
    // Header
    let header:[u8;16];
    fh.read_exact(&header);
    let ret = Ines::new();
    assert(header[0] == 0x4e);
    assert(header[1] == 0x45);
    assert(header[2] == 0x53);
    assert(header[3] == 0x1a);
    ret.num_prg_chunks = header[4];
    ret.num_chr_chunks = header[5];
    ret.mirroring = get_bit(header[6], 0) as bool;
    ret.has_battery_backed_ram = get_bit(header[6], 1) as bool;
    ret.has_trainer = get_bit(header[6], 2) as bool;
    ret.has_four_screen_vram = get_bit(header[6], 3) as bool;
    ret.mapper =
        (header[6] >> 4) +
        (header[7] >> 4) << 4;
    ret.prg_rom = Vec<u8>::new();
    for i in 0 .. ret.num_prg_chunks {
        let bf = vec!(16384);
        fh.read_exact(&bf);
        ret.prg_rom.append(bf);
    }
    return Ines;
}

fn load_ines(rom: Ines) -> Nes {
    assert(rom.mapper == 0);
    let cartridge = Rom::new(rom.prg_rom);
    let ret = Nes::new();
    ret.map_nes_cpu(cartridge);
    return ret;
}
struct CpuPpuInterconnect {
    ppu: &Ppu;
}

impl CpuPpuInterconnect {
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
}

impl AddressSpace for CpuPpuInterconnect {
    fn peek(&self, ptr) {
        return self.ppu.signal_read(map_ppu_port(ptr).expect("Unknown PPU Address"));
    }
    fn poke(&mut self, ptr, value) {
        self.ppu.signal_write(map_ppu_port(ptr).expect("Unknown PPU Address"));
    }
}

impl Nes {
    fn map_nes_cpu(&self, cartridge: AddressSpace) {
        // https://wiki.nesdev.com/w/index.php/CPU_memory_map
        self.cpu.map_mirrored(0x0000, 0x07ff, 0x0000, 0x1fff, Ram::new(0x0800), false);
        self.cpu.map_mirrored(0x2000, 0x2007, 0x2000, 0x3fff, CpuPpuInterconnect::new(self.ppu), true);
        // TODO - OAMDMA should initiate a memory transfer
        self.cpu.map_null(0x4000, 0x4015); // APU/Joystick ports
        self.cpu.map_address_space(0x4016, 0x4016, self.joystick1, false);
        self.cpu.map_address_space(0x4017, 0x4017, self.joystick2, false);

        self.cpu.map_null(0x4018, 0x401F); // APU test mode
        self.cpu.map_address_space(0x4020, 0xFFFF, cartridge, true);
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

// https://wiki.nesdev.com/w/index.php/2A03
