#![allow(unused_imports)]
#![allow(non_camel_case_types)]
#![allow(dead_code)] // TODO
#![allow(unused_variables)]

use crate::common::{get_bit, ternary, Clocked};
use crate::mapper::AddressSpace;
use crate::serialization::Savable;

use std::collections::VecDeque;
use std::io::Read;
use std::io::Write;

// https://wiki.nesdev.com/w/index.php/2A03
pub enum ApuPort {
    SQ1_VOL,
    SQ1_SWEEP,
    SQ1_LO,
    SQ1_HI,
    SQ2_VOL,
    SQ2_SWEEP,
    SQ2_LO,
    SQ2_HI,
    TRI_LINEAR,
    TRI_LO,
    TRI_HI,
    NOISE_VOL,
    NOISE_LO,
    NOISE_HI,
    DMC_FREQ,
    DMC_RAW,
    DMC_START,
    DMC_LEN,
    SND_CHN,
    FRAME_COUNTER,
}

use ApuPort::*;

const ENABLE_PULSE1: bool = true;
const ENABLE_PULSE2: bool = true;
const ENABLE_TRIANGLE: bool = true;
const ENABLE_NOISE: bool = true;
const ENABLE_DMC: bool = false;

const SAMPLES_PER_FRAME: f64 = 735.0; // 44100 Hz audio / 60 FPS
const CLOCKS_PER_FRAME: f64 = 29780.0;

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
        0x4015 => Some(SND_CHN),
        0x4017 => Some(FRAME_COUNTER),
        _ => None,
    }
}

// ttps://wiki.nesdev.com/w/index.php/APU_Frame_Counter
struct FrameCounter {
    step: u16,
    interrupt_inhibit: bool,
    mode: bool, // false=4-step, true=5-step
}

impl FrameCounter {
    pub fn new() -> FrameCounter {
        FrameCounter {
            step: 0,
            interrupt_inhibit: false,
            mode: false,
        }
    }
    pub fn is_quarter_frame_edge(&self) -> bool {
        match (self.mode, self.step) {
            (_, 3728) => true,
            (_, 7456) => true,
            (_, 11185) => true,
            (false, 14914) => true,
            (true, 18640) => true,
            _ => false,
        }
    }
    pub fn is_half_frame_edge(&self) -> bool {
        match (self.mode, self.step) {
            (_, 7456) => true,
            (false, 14914) => true,
            (true, 18640) => true,
            _ => false,
        }
    }
    pub fn is_frame_edge(&self) -> bool {
        match (self.mode, self.step) {
            (false, 14914) => true,
            _ => false,
        }
    }
    pub fn write_control(&mut self, value: u8) {
        self.mode = get_bit(value, 7) > 0;
        self.interrupt_inhibit = get_bit(value, 6) > 0;
    }
}

impl Clocked for FrameCounter {
    fn clock(&mut self) {
        self.step += 1;
        let cap = ternary(self.mode, 18641, 14915);
        // self.step %= cap;
        if self.step >= cap {
            self.step -= cap;
        }
    }
}

pub struct Apu {
    pub samples: Vec<f32>,
    pub is_recording: bool,
    cycle: u64,
    sample_rate: f64,
    sample_timer: f64,
    frame_counter: FrameCounter,
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    noise: Noise,
    dmc: Dmc,
}

impl Savable for Apu {
    fn save(&self, fh: &mut dyn Write) {
        // TODO
    }
    fn load(&mut self, fh: &mut dyn Read) {
        // TODO
    }
}

impl Clocked for Apu {
    fn clock(&mut self) {
        if self.cycle % 2 == 0 {
            self.frame_counter.clock();
            self.pulse1.clock();
            self.pulse2.clock();
            self.noise.clock();
            self.dmc.clock();
        }
        self.triangle.clock();
        if self.frame_counter.is_half_frame_edge() {
            self.pulse1.clock_half_frame();
            self.pulse2.clock_half_frame();
            self.triangle.clock_half_frame();
            self.noise.clock_half_frame();
            self.dmc.clock_half_frame();
        }
        if self.frame_counter.is_quarter_frame_edge() {
            self.pulse1.clock_quarter_frame();
            self.pulse2.clock_quarter_frame();
            self.triangle.clock_quarter_frame();
            self.noise.clock_quarter_frame();
            self.dmc.clock_half_frame();
        }
        if self.sample_timer <= 0.0 && self.is_recording {
            let sample = self.sample();
            //let sample = 0.1;
            self.samples.push(sample);
            self.sample_timer += self.sample_rate;
        }
        self.sample_timer -= 1.0;
        self.cycle += 1;
    }
}

impl Apu {
    pub fn new() -> Apu {
        Apu {
            cycle: 0,
            is_recording: true,
            samples: Vec::new(),
            sample_rate: CLOCKS_PER_FRAME / SAMPLES_PER_FRAME,
            sample_timer: 0.0,
            frame_counter: FrameCounter::new(),
            pulse1: Pulse::new(false),
            pulse2: Pulse::new(true),
            triangle: Triangle::new(),
            noise: Noise::new(),
            dmc: Dmc::new(),
        }
    }
    pub fn reset(&mut self) {
        // TODO - Implement reset
        // self.pulse1.reset();
        // self.pulse2.reset();
        // self.triangle.reset();
        // self.noise.reset();
        // self.dmc.reset();
    }
    pub fn sample(&self) -> f32 {
        // https://wiki.nesdev.com/w/index.php/APU_Mixer
        let pulse1 = ternary(ENABLE_PULSE1, self.pulse1.sample(), 0.0);
        let pulse2 = ternary(ENABLE_PULSE2, self.pulse2.sample(), 0.0);
        let triangle = ternary(ENABLE_TRIANGLE, self.triangle.sample(), 0.0);
        let noise = ternary(ENABLE_NOISE, self.noise.sample(), 0.0);
        let dmc = ternary(ENABLE_DMC, self.dmc.sample(), 0.0);

        let pulse_out = 0.00752 * (pulse1 + pulse2);
        //let tnd_out = triangle / 15.0 ; // 0.00851 * triangle + 0.00494 * noise + 0.00335 * dmc;
        let tnd_out = 0.00851 * triangle + 0.00494 * noise + 0.00335 * dmc;
        let output = pulse_out + tnd_out;
        //return 0.5;
        return output as f32;
    }
    fn read_status(&self) -> u8 {
        // TODO - Clear the frame interrupt flag
        return (self.pulse1.is_enabled() as u8) << 0
            | (self.pulse2.is_enabled() as u8) << 1
            | (self.triangle.is_enabled() as u8) << 2
            | (self.noise.is_enabled() as u8) << 3
            | (self.dmc.is_enabled() as u8) << 4;
    }
    fn write_status(&mut self, v: u8) {
        let enable_pulse1 = v & 0b00001;
        let enable_pulse2 = v & 0b00010;
        let enable_triangle = v & 0b00100;
        let enable_noise = v & 0b01000;
        let enable_dmc = v & 0b10000;
        self.pulse1.set_enabled(enable_pulse1 > 0);
        self.pulse2.set_enabled(enable_pulse2 > 0);
        self.triangle.set_enabled(enable_triangle > 0);
        self.noise.set_enabled(enable_noise > 0);
        self.dmc.set_enabled(enable_dmc > 0);
    }
}

impl AddressSpace for Apu {
    fn peek(&self, ptr: u16) -> u8 {
        match map_apu_port(ptr) {
            Some(SND_CHN) => self.read_status(),
            _ => {
                //eprintln!("DEBUG - APU READ - {:x}", ptr);
                return 0;
            }
        }
    }
    fn poke(&mut self, ptr: u16, v: u8) {
        // eprintln!("DEBUG - APU WRITE - {:x} {:x}", ptr, v);
        match map_apu_port(ptr) {
            Some(SQ1_VOL) => self.pulse1.set_volume(v),
            Some(SQ1_SWEEP) => self.pulse1.set_sweep(v),
            Some(SQ1_LO) => self.pulse1.set_timer_low(v),
            Some(SQ1_HI) => self.pulse1.set_timer_high(v),
            Some(SQ2_VOL) => self.pulse2.set_volume(v),
            Some(SQ2_SWEEP) => self.pulse2.set_sweep(v),
            Some(SQ2_LO) => self.pulse2.set_timer_low(v),
            Some(SQ2_HI) => self.pulse2.set_timer_high(v),
            Some(TRI_LINEAR) => self.triangle.write_linear_counter(v),
            Some(TRI_LO) => self.triangle.write_timer_low(v),
            Some(TRI_HI) => self.triangle.write_timer_high(v),
            Some(NOISE_VOL) => self.noise.set_volume(v),
            Some(NOISE_LO) => self.noise.set_period(v),
            Some(NOISE_HI) => self.noise.set_length(v),
            Some(DMC_FREQ) => { /* TODO */ }
            Some(DMC_RAW) => { /* TODO */ }
            Some(DMC_START) => { /* TODO */ }
            Some(DMC_LEN) => { /* TODO */ }
            Some(SND_CHN) => self.write_status(v),
            Some(FRAME_COUNTER) => self.frame_counter.write_control(v),
            None => panic!("Unexpected APU port {:x} {:x}", ptr, v),
        }
    }
}

struct LengthCounter {
    enabled: bool,
    halt: bool,
    counter: u8, // 5-bit value
}

impl Clocked for LengthCounter {
    fn clock(&mut self) {
        // eprintln!("DEBUG - LENGTH COUNTER CLOCKED {} {} {}", self.enabled, self.halt, self.counter);
        if self.counter == 0 || self.halt {
        } else {
            self.counter -= 1;
        }
    }
}

const LENGTH_COUNTER_LOOKUP_TABLE: [u8; 32] = [
    /*00-0F*/ 10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, /*10-1F*/ 12,
    16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30,
];

// https://wiki.nesdev.com/w/index.php/APU_Length_Counter
impl LengthCounter {
    pub fn new() -> LengthCounter {
        LengthCounter {
            enabled: true,
            halt: false,
            counter: 0,
        }
    }
    pub fn is_silenced(&self) -> bool {
        return self.counter == 0 || !self.enabled;
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn set_enabled(&mut self, x: bool) {
        if !x {
            self.counter = 0;
        }
        self.enabled = x;
    }
    pub fn set_halt(&mut self, x: bool) {
        self.halt = x;
    }
    pub fn set_load(&mut self, x: u8) {
        if self.enabled {
            self.counter = LENGTH_COUNTER_LOOKUP_TABLE[x as usize & 0x1F];
        }
    }
}

struct LinearCounter {
    counter: u8,
    reload_value: u8,
    reload: bool,
    enabled: bool,
}

// https://wiki.nesdev.com/w/index.php/APU_Triangle
impl LinearCounter {
    pub fn new() -> LinearCounter {
        LinearCounter {
            counter: 0,
            reload_value: 0,
            reload: false,
            enabled: false,
        }
    }
    pub fn set_reload_value(&mut self, value: u8) {
        self.reload_value = value;
    }
    pub fn set_reload_flag(&mut self, value: bool) {
        self.reload = value;
    }
    pub fn is_silenced(&self) -> bool {
        return self.counter == 0;
    }
    pub fn set_enabled(&mut self, value: bool) {
        self.enabled = value;
    }
}

impl Clocked for LinearCounter {
    fn clock(&mut self) {
        if self.reload {
            self.counter = self.reload_value;
        } else {
            if self.counter != 0 {
                self.counter -= 1;
                if self.counter == 0 {
                    //eprintln!("DEBUG - LINEAR COUNTER SET TO 0 - {}", self.reload_value);
                }
            }
        }
        if self.enabled {
            self.reload = false;
        }
    }
}

struct Triangle {
    timer_period: u16,
    timer: u16,
    sequencer_step: u8,
    length_counter: LengthCounter,
    linear_counter: LinearCounter,
}

impl Clocked for Triangle {
    fn clock(&mut self) {
        if self.timer == 0 {
            //eprintln!("DEBUG - TRIANGLE STEP {} {}", self.sequencer_step, self.timer_period);
            self.timer = self.timer_period;
            if !self.is_silenced() {
                self.sequencer_step += 1;
                self.sequencer_step &= 0x1F;
            }
        } else {
            self.timer -= 1;
        }
    }
}

impl Triangle {
    pub fn new() -> Triangle {
        Triangle {
            timer_period: 0,
            timer: 0,
            sequencer_step: 0,
            length_counter: LengthCounter::new(),
            linear_counter: LinearCounter::new(),
        }
    }
    fn is_silenced(&self) -> bool {
        self.length_counter.is_silenced() || self.linear_counter.is_silenced()
    }
    pub fn is_enabled(&self) -> bool {
        self.length_counter.is_enabled()
    }
    pub fn set_enabled(&mut self, x: bool) {
        self.length_counter.set_enabled(x);
        self.linear_counter.set_enabled(x);
    }
    pub fn write_enabled(&mut self, x: bool) {
        self.length_counter.set_enabled(x);
    }
    pub fn write_linear_counter(&mut self, x: u8) {
        //eprintln!("DEBUG - WRITE LINEAR COUNTER {}", x);
        let flag = (x & 0x80) > 0;
        self.length_counter.set_halt(flag);
        self.linear_counter.set_reload_flag(flag);
        self.linear_counter.set_reload_value(x & 0x7f);
    }
    pub fn write_timer_low(&mut self, x: u8) {
        self.timer_period &= 0xFF00;
        self.timer_period |= x as u16;
    }
    pub fn write_timer_high(&mut self, x: u8) {
        //eprintln!("DEBUG - WRITE TIMER HIGH {}", x);
        self.timer_period &= 0x00FF;
        self.timer_period |= (x as u16 & 0x7) << 8;
        self.length_counter.set_load(x >> 3);
        self.linear_counter.set_reload_flag(true);
    }
    pub fn sample(&self) -> f64 {
        let step = self.sequencer_step;
        return if step < 16 { 15 - step } else { step - 16 } as f64;
    }
    pub fn clock_half_frame(&mut self) {
        self.length_counter.clock();
    }
    pub fn clock_quarter_frame(&mut self) {
        self.linear_counter.clock();
    }
}

// https://wiki.nesdev.com/w/index.php/APU_Sweep
struct Sweep {
    negation: bool, // false=one's complement; true = two's complement
    enabled: bool,
    divider_period: u8,
    divider: u8,
    negate: bool,
    shift_count: u8,
    reload: bool,
    period: u16,
    timer: u16, // A copy of the timer on the unit
}

impl Clocked for Sweep {
    // Clocked by Frame Counter, half-frame
    fn clock(&mut self) {
        // eprintln!("DEBUG - SWEEP - silenced:{} nt:{} e:{} dp:{} d:{} n:{} sc:{} r:{} p:{} t:{}",
        //           self.is_muted(),
        //           self.negation,
        //           self.enabled,
        //           self.divider_period,
        //           self.divider,
        //           self.negate,
        //           self.shift_count,
        //           self.reload,
        //           self.period,
        //           self.timer);

        if self.divider == 0 && self.enabled && !self.is_muted() && self.shift_count != 0 {
            self.period = self.target_period();
        }
        if self.divider == 0 || self.reload {
            self.divider = self.divider_period;
            self.reload = false;
        } else {
            self.divider -= 1;
        }
    }
}

impl Sweep {
    pub fn new(negation: bool) -> Sweep {
        Sweep {
            negation: negation,
            enabled: false,
            divider_period: 0,
            divider: 0,
            negate: false,
            shift_count: 0,
            reload: false,
            period: 0,
            timer: 0,
        }
    }
    pub fn set_period(&mut self, period: u16) {
        self.period = period;
    }
    pub fn is_muted(&self) -> bool {
        return self.period < 8 || self.target_period() > 0x7ff;
    }
    pub fn write_control(&mut self, x: u8) {
        self.shift_count = x & 0x7;
        self.negate = get_bit(x, 3) > 0;
        self.divider_period = (x >> 4) & 0x7;
        self.enabled = get_bit(x, 7) > 0;
    }
    pub fn period(&self) -> u16 {
        return self.period;
    }
    fn target_period(&self) -> u16 {
        let time = self.timer;
        let mut change = time >> self.shift_count;
        if self.negate {
            change = self.negate(change);
        }
        let period = self.period;
        return period.wrapping_add(change);
    }

    fn negate(&self, value: u16) -> u16 {
        if self.negation {
            (-(value as i16)) as u16
        } else {
            (-(value as i16) - 1) as u16
        }
    }
}

struct Envelope {
    looping: bool,
    constant: bool,
    period: u8,
    divider: u8,
    volume: u8,
    start: bool,
}

impl Clocked for Envelope {
    // Clocked by quarter-frame
    fn clock(&mut self) {
        // eprintln!("DEBUG - ENVELOPE - {} {} {} {} {} {} {}",
        //           self.looping, self.constant, self.period, self.divider, self.volume, self.start, self.sample());
        if self.start {
            self.start = false;
            self.volume = 15;
            self.divider = self.period;
        } else if self.divider > 0 {
            self.divider -= 1;
        } else {
            if self.volume > 0 {
                self.volume -= 1;
            } else if self.looping {
                self.volume = 15;
            }
            self.divider = self.period;
        }
    }
}

impl Envelope {
    pub fn new() -> Envelope {
        Envelope {
            looping: false,
            constant: false,
            period: 0,
            divider: 0,
            volume: 0,
            start: false,
        }
    }
    pub fn set_control(&mut self, v: u8) {
        self.period = v & 0xf;
        self.volume = v & 0xf;
        self.constant = get_bit(v, 4) > 0;
        self.looping = get_bit(v, 5) > 0;
    }

    pub fn sample(&self) -> f64 {
        if self.constant {
            self.period as f64
        } else {
            self.volume as f64
        }
    }
    pub fn reset(&mut self) {
        self.start = true;
    }
}

struct Pulse {
    pub sweep: Sweep,
    timer_period: u16,
    timer: u16,
    duty_cycle: u8,
    sequencer_step: u8,
    envelope: Envelope,
    length_counter: LengthCounter,
}

const PULSE_SEQUENCER_DUTY_TABLE: [u8; 4] = [0b01000000, 0b01100000, 0b01111000, 0b10011111];

impl Pulse {
    pub fn new(negation: bool) -> Pulse {
        Pulse {
            sweep: Sweep::new(negation),
            timer_period: 0,
            timer: 0,
            duty_cycle: 0,
            sequencer_step: 0,
            envelope: Envelope::new(),
            length_counter: LengthCounter::new(),
        }
    }
    pub fn set_volume(&mut self, v: u8) {
        self.envelope.set_control(v);
        self.length_counter.set_halt(get_bit(v, 5) > 0);
        self.duty_cycle = v >> 6;
    }
    pub fn set_sweep(&mut self, v: u8) {
        self.sweep.write_control(v);
    }
    pub fn set_timer_low(&mut self, v: u8) {
        self.timer_period &= 0xFF00;
        self.timer_period |= v as u16;
        let period = self.timer_period;
        self.sweep.set_period(period);
    }
    pub fn set_timer_high(&mut self, v: u8) {
        //eprintln!("DEBUG - PULSE TIMER HIGH {} {} {}", 0x
        self.timer_period &= 0x00FF;
        self.timer_period |= ((v & 0x7) as u16) << 8;
        self.length_counter.set_load(v >> 3);
        self.sequencer_step = 0;
        self.envelope.reset();
        let period = self.timer_period;
        self.sweep.set_period(period);
    }
    fn sequencer(&self) -> f64 {
        let duty = PULSE_SEQUENCER_DUTY_TABLE[self.duty_cycle as usize];
        let x = (duty >> (7 - self.sequencer_step % 8)) & 1;
        return x as f64;
    }
    fn sample(&self) -> f64 {
        // eprintln!("DEBUG - PULSE - {} {} {} {}",
        //           self.sweep.is_muted(),
        //           self.length_counter.is_silenced(),
        //           self.sequencer(),
        //           self.envelope.sample());
        if !self.is_muted() {
            return self.sequencer() * self.envelope.sample();
        } else {
            return 0.0;
        }
    }
    fn is_muted(&self) -> bool {
        self.sweep.is_muted() || self.length_counter.is_silenced()
    }
    fn is_enabled(&self) -> bool {
        self.length_counter.is_enabled()
    }
    fn set_enabled(&mut self, v: bool) {
        self.length_counter.set_enabled(v);
    }
    fn clock_quarter_frame(&mut self) {
        self.envelope.clock();
    }
    fn clock_half_frame(&mut self) {
        self.timer_period = self.sweep.period();
        self.sweep.set_period(self.timer_period);
        self.sweep.clock();
        self.length_counter.clock();
    }
}

impl Clocked for Pulse {
    // Clocked every APU cycle(2 CPU cycles)
    fn clock(&mut self) {
        if self.timer == 0 {
            self.sequencer_step = self.sequencer_step.wrapping_sub(1) & 0x7;
            self.timer = self.timer_period;
        } else {
            self.timer -= 1;
        }
    }
}

const NOISE_PERIOD_LOOKUP_TABLE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

struct Noise {
    envelope: Envelope,
    length_counter: LengthCounter,
    mode: bool,
    period: u16,
    feedback: u16,
    timer: u16,
}

impl Clocked for Noise {
    // Clocked every APU cycle(= 2 CPU cycles)
    fn clock(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            let feedback = self.feedback;
            self.feedback = self.next_feedback(feedback);
        } else {
            self.timer -= 1;
        }
    }
}

impl Noise {
    pub fn new() -> Noise {
        Noise {
            envelope: Envelope::new(),
            length_counter: LengthCounter::new(),
            mode: false,
            period: 0,
            feedback: 1,
            timer: 0,
        }
    }
    fn sample(&self) -> f64 {
        if get_bit(self.feedback as u8, 0) > 0 && !self.length_counter.is_silenced() {
            self.envelope.sample()
        } else {
            0.0
        }
    }
    fn is_enabled(&self) -> bool {
        self.length_counter.is_enabled()
    }
    pub fn set_volume(&mut self, v: u8) {
        self.length_counter.set_halt(get_bit(v, 5) > 0);
        self.envelope.set_control(v);
    }
    pub fn set_enabled(&mut self, v: bool) {
        self.length_counter.set_enabled(v);
    }
    pub fn set_period(&mut self, v: u8) {
        self.mode = (v & 0x8) > 0;
        self.period = NOISE_PERIOD_LOOKUP_TABLE[(v & 0xf) as usize];
    }
    pub fn set_length(&mut self, v: u8) {
        self.length_counter.set_load(v >> 3);
        self.envelope.reset();
    }
    fn next_feedback(&self, mut feedback: u16) -> u16 {
        let new_bit = (get_bit(feedback as u8, 0) > 0)
            ^ (get_bit(feedback as u8, ternary(self.mode, 6, 1)) > 0);
        feedback >>= 1;
        feedback |= ternary(new_bit, 1 << 14, 0);
        return feedback;
    }
    pub fn clock_quarter_frame(&mut self) {}
    pub fn clock_half_frame(&mut self) {
        self.length_counter.clock();
    }
}

struct Dmc {}

impl Dmc {
    pub fn new() -> Dmc {
        Dmc {}
    }
    pub fn sample(&self) -> f64 {
        return 0.0;
    }
    pub fn is_enabled(&self) -> bool {
        // TODO
        false
    }
    pub fn set_enabled(&mut self, v: bool) {
        // TODO
    }
    pub fn clock_half_frame(&mut self) {
        // TODO
    }
    pub fn clock_quarter_frame(&mut self) {
        // TODO
    }
}

impl Clocked for Dmc {
    fn clock(&mut self) {
        // TODO
    }
}
