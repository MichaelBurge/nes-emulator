#![allow(unused_imports)]
#![allow(non_camel_case_types)]

pub enum ApuPort {
    SQ1_VOL, SQ1_SWEEP, SQ1_LO, SQ1_HI,
    SQ2_VOL, SQ2_SWEEP, SQ2_LO, SQ2_HI,
    TRI_LINEAR, TRI_LO, TRI_HI,
    NOISE_VOL, NOISE_LO, NOISE_HI,
    DMC_FREQ, DMC_RAW, DMC_START, DMC_LEN,
    SND_CHN,
}

use ApuPort::*;

pub fn map_apu_port(ptr: u16) -> Option<ApuPort> {
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
        //0x4009 => Some(APU_DUMMY1),
        0x400A => Some(TRI_LO),
        0x400B => Some(TRI_HI),
        0x400C => Some(NOISE_VOL),
        //0x400D => Some(APU_DUMMY2),
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

pub struct Apu {

}

impl Apu {
    pub fn new() -> Apu {
        Apu { }
    }
    // fn output(&mut self) -> f64 {
    //     // https://wiki.nesdev.com/w/index.php/APU_Mixer
    //     let pulse1 = self.pulse1() as f64;
    //     let pulse2 = self.pulse2() as f64;
    //     let triangle = self.triangle() as f64;
    //     let noise = self.noise() as f64;
    //     let dmc = self.dmc() as f64;

    //     let pulse_out = 95.88 / ((8128 / (pulse1 + pulse2)) + 100);
    //     let tnd_out = 159.79 / (1 / ((triangle / 8227) + (noise / 12241) + (dmc / 22638)) + 100);
    //     return pulse_out + tnd_out;
    // }
    // fn pulse1(&mut self) -> u8 { /* TODO */ }
    // fn pulse2(&mut self) -> u8 { /* TODO */ }
    // // https://github.com/bfirsh/jsnes/blob/master/src/papu.js#L12
    // fn triangle(&mut self) -> u8 { /* TODO */ }
    // fn noise(&mut self) -> u8 { /* TODO */ }
    // fn dmc(&mut self) -> u8 { /* TODO */ }
}
