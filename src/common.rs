pub trait Clocked {
    fn clock(&mut self);
}

pub fn get_bit(x: u8, i: u8) -> u8 {
    return (x >> i) & 1;
}

pub fn run_clocks(x: &mut dyn Clocked, num_clocks: u32) {
    for _i in 0..num_clocks {
        x.clock();
    }
}

pub fn ternary<T>(cond: bool, on_true: T, on_false: T) -> T {
    if cond {
        on_true
    } else {
        on_false
    }
}
