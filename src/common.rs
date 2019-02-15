trait Clocked {
    fn clock(&mut self);
}

fn get_bit(x: u8, i: u8) {
    return (x >> i) & 1;
}

fn run_clocks(x: Clocked, num_clocks: u32) {
    for i in 0 .. num_clocks {
        x.clock();
    }
}
