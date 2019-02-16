pub trait Clocked {
    fn clock(&mut self);
}

pub fn get_bit(x: u8, i: u8) -> u8 {
    return (x >> i) & 1;
}

pub fn run_clocks(x: &mut Clocked, num_clocks: u32) {
    for _i in 0 .. num_clocks {
        x.clock();
    }
}
