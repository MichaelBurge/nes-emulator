use std::vec;

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

struct Ram {
    bs: Vec<u8>;
}


impl AddressSpace for Ram {
    fn peek(&self, ptr) {
        return self.bs[ptr];
    }
    fn poke(&mut self, ptr, v) {
        self.bs[ptr] = v;
    }
}

struct Rom {
    bs: Vec<u8>;
}

impl AddressSpace for Rom {
    fn peek(&self, ptr) {
        return self.bs[ptr];
    }
    fn poke(&self, ptr, value) {
        println("Rom - Attempted to write read-only memory");
    }
}

struct MirroredAddressSpace {
    base: AddressSpace;
    base_begin: u16;
    base_end: u16;
    extended_low: u16;
    extended_high: u16;
}

impl MirroredAddressSpace {
    // If a memory range has been mirrored to another, map a pointer to the "base range" or fail if it lies outside.
    fn map_address(&self, ptr: u16) -> u16 {
        if ptr < self.extended_low | ptr > self.extended_high {
            panic!("map_address: Out of mapped ranged");
        }
        let width = (self.base_high - self.base_low);
        return ((ptr - self.extended_low) % width + self.base_low);
    }
}

impl AddressSpace for MirroredAddressSpace {
    fn peek(&self, ptr) {
        return self.base.peek(self.map_address(ptr));
    }
    fn poke(&mut self, ptr, value) {
        self.base.poke(self.map_address(ptr), value);
    }
}

struct RegisterAddressSpace {
    register: &mut u8;
}

impl AddressSpace for RegisterAddressSpace {
    fn peek(&self, ptr) {
        if ptr != 0 { panic("Register must be mapped to address 0"); }
        return *self.register;
    }
    fn poke(&self, ptr, value) {
        if ptr != 0 { panic("Register must be mapped to address 0"); }
        *self.register = value;
    }
}

type NumBytes = u16;

struct Mapper {
    next_ptr: u16;
    mappings: Vec<(NumBytes, Box<AddressSpace>)>;
}

impl Mapper {
    fn lookup_address_space(ptr: u16) -> (Box<AddressSpace>, u16) {
        for (size, space) in mappings {
            if ptr < size {
                return (space, ptr);
            } else {
                ptr -= space;
            }
        }
        panic!("lookup_address_space - Unmapped pointer");
    }
    fn map_address_space(size: u16, space: Box<AddressSpace>) {
        self.next_ptr += size;
        self.mappings.push(size, space);
    }

    fn map_ram(size: u16) {
        self.map_address_space(size, Box::new(Ram(size)));
    }
    fn map_rom(bytes: &[u8]) {
        self.map_address_space(bytes.length, Box::new(Rom(vec!(bytes))));
    }
    fn map_mirrored(size: u16, extended_size: u16, space: &AddressSpace) {
        let space = MirroredAddressSpace::new(
            space,
            0, size,
            self.next_ptr, self.next_ptr + extended_size
        );
        self.map_address_space(extended_size, Box::new(space));
    }
    fn map_register(register: &mut u8) {
        self.map_address_space(1, Box::new(RegisterAddressSpace::new(register)));
    }
}

impl AddressSpace for Mapper {
    fn peek(&self, ptr) {
        let (space, space_ptr) = self.lookup_address_space(ptr);
        return space.peek(space_ptr);
    }
    fn poke(&self, ptr, value) {
        let (space, space_ptr) = self.lookup_address_space(ptr);
        space.poke(space_ptr, value);
    }
}
