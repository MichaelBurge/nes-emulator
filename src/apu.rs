struct Apu {

}

impl Apu {
    fn output(&mut self) -> f64 {
        // https://wiki.nesdev.com/w/index.php/APU_Mixer
        let pulse1 = self.pulse1() as f64;
        let pulse2 = self.pulse2() as f64;
        let triangle = self.triangle() as f64;
        let noise = self.noise() as f64;
        let dmc = self.dmc() as f64;

        let pulse_out = 95.88 / ((8128 / (pulse1 + pulse2)) + 100);
        let tnd_out = 159.79 / (1 / ((triangle / 8227) + (noise / 12241) + (dmc / 22638)) + 100);
        return pulse_out + tnd_out;
    }
    fn pulse1(&mut self) -> u8 { /* TODO */ }
    fn pulse2(&mut self) -> u8 { /* TODO */ }
    // https://github.com/bfirsh/jsnes/blob/master/src/papu.js#L12
    fn triangle(&mut self) -> u8 { /* TODO */ }
    fn noise(&mut self) -> u8 { /* TODO */ }
    fn dmc(&mut self) -> u8 { /* TODO */ }
}
