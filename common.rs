trait AddressSpace {
    // Minimal definition
    fn peek(&self, ptr: u16) -> u8;
    fn poke(&mut self, ptr: u16, v: u8);

    // Helper methods
    fn peek16(&self, ptr:u16) -> u16 {
        return
            self.peek(ptr) +
            self.peek(wrapped_add(ptr, 1)) << 8;
    }
    fn peek_offset(&self, ptr: u16, os: i16) -> u8 {
        self.peek(wrapped_add(ptr, os as u16));
    }
    fn peek_offset16(&self, ptr: u16, os: i16) -> u16 {
        self.peek16(wrapped_add(ptr, os as u16));
    }
}

struct FlatMemory {
    bs: &[u8],
}


impl AddressSpace for FlatMemory {
    fn peek(&self, ptr) {
        return self.bs[ptr];
    }
    fn poke(&mut self, ptr, v) {
        self.bs[ptr] = v;
    }
}



// If a memory range has been mirrored to another, map a pointer to the "base range" or fail if it lies outside.
fn mirrored_lea(ptr: u16, (base_low, base_high): (u16, u16), (extended_low, extended_high): (u16, u16)) -> Option<u16> {
    if ptr < extended_low | ptr > extended_high {
        return None;
    }
    let width = (base_high - base_low);
    return Some((ptr - extended_low) % width + base_low);
}

fn get_bit(x: u8, i: u8) {
    return (x >> i) & 1;
}
