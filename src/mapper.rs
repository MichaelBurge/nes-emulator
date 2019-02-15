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

struct NullAddressSpace { };
impl AddressSpace for NullAddressSpace {
    fn peek(&self, ptr) { return 0; }
    fn poke(&mut self, ptr, value) { }
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
type UsesOriginalAddress = bool;
struct Mapper {
    mappings: Vec<(u16, u16, Box<AddressSpace>, UsesOriginalAddress)>;
}

impl Mapper {
    fn lookup_address_space(ptr: u16) -> (Box<AddressSpace>, u16) {
        for (range_begin, range_end, space, use_original_address) in mappings {
            if ptr >= range_begin && ptr <= range_end {
                return (space, use_original_address ? ptr : (ptr - range_begin));
            }
        }
        panic!("lookup_address_space - Unmapped pointer");
    }
    fn map_address_space(begin: u16, end: u16, space: Box<AddressSpace>, use_original: bool) {
        self.mappings.push(begin, end, space, use_original);
    }

    fn map_ram(begin: u16, end: u16) {
        let size = end - begin;
        self.map_address_space(begin, end, Box::new(Ram(size)), false);
    }
    fn map_rom(begin: u16, end: u16, bytes: &[u8]) {
        self.map_address_space(begin, end, Box::new(Rom(vec!(bytes))), false);
    }
    fn map_null(begin: u16, end: u16) {
        self.map_address_space(begin, end, Box::new(NullAddressSpace::New()), false);
    }
    fn map_mirrored(begin: u16, end: u16, extended_begin: u16, extended_end: u16, space: &AddressSpace, use_original: bool) {
        let base_begin =
            if use_original { begin } else { 0 };
        let space = MirroredAddressSpace::new(
            space,
            base_begin, size,
            extended_begin, extended_end,
        );
        self.map_address_space(extended_size, Box::new(space), use_original);
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
