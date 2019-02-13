struct ppu {
    data_bus: u8,
}

enum PpuPort {
    PPUCTRL, PPUMASK, PPUSTATUS,
    OAMADDR, OAMDATA, PPUSCROLL,
    PPUADDR, PPUDATA, OAMDMA,
}
