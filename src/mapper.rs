pub trait AddressSpace {
    // Minimal definition
    fn peek(&self, ptr: u16) -> u8;
    fn poke(&mut self, ptr: u16, v: u8);

    // Helper methods
    fn peek16(&self, ptr:u16) -> u16 {
        return
            (
            self.peek(ptr) +
            self.peek(ptr.wrapping_add(1)) << 8
            )
            as u16;
    }
    fn peek_offset(&self, ptr: u16, os: i16) -> u8 {
        return self.peek(ptr.wrapping_add(os as u16));
    }
    fn peek_offset16(&self, ptr: u16, os: i16) -> u16 {
        return self.peek16(ptr.wrapping_add(os as u16));
    }
    fn poke_offset(&mut self, ptr: u16, os: i16, v: u8) {
        self.poke(ptr.wrapping_add(os as u16), v);
    }
}

struct Ram {
    bs: Vec<u8>,
}


impl AddressSpace for Ram {
    fn peek(&self, ptr:u16) -> u8 {
        return self.bs[ptr as usize];
    }
    fn poke(&mut self, ptr:u16, v:u8) {
        self.bs[ptr as usize] = v;
    }
}

struct Rom {
    bs: Vec<u8>,
}

impl AddressSpace for Rom {
    fn peek(&self, ptr:u16) -> u8 {
        return self.bs[ptr as usize];
    }
    fn poke(&mut self, ptr:u16, value:u8) {
        println!("Rom - Attempted to write read-only memory");
    }
}

struct MirroredAddressSpace {
    base: Box<dyn AddressSpace>,
    base_begin: u16,
    base_end: u16,
    extended_begin: u16,
    extended_end: u16,
}

impl MirroredAddressSpace {
    // If a memory range has been mirrored to another, map a pointer to the "base range" or fail if it lies outside.
    fn map_address(&self, ptr: u16) -> u16 {
        if ptr < self.extended_begin || ptr > self.extended_end {
            panic!("map_address: Out of mapped ranged");
        }
        let width = self.base_end - self.base_begin;
        return (ptr - self.extended_begin) % width + self.base_begin;
    }
}

impl AddressSpace for MirroredAddressSpace {
    fn peek(&self, ptr:u16) -> u8 {
        return self.base.peek(self.map_address(ptr));
    }
    fn poke(&mut self, ptr:u16, value:u8) {
        let space_ptr = self.map_address(ptr);
        self.base.poke(space_ptr, value);
    }
}

struct NullAddressSpace { }
impl AddressSpace for NullAddressSpace {
    fn peek(&self, ptr:u16) -> u8{ return 0; }
    fn poke(&mut self, ptr:u16, value:u8) { }
}

type NumBytes = u16;
type UsesOriginalAddress = bool;
type Mapping = (u16, u16, Box<dyn AddressSpace>, UsesOriginalAddress);
struct Mapper {
    mappings: Vec<Mapping>,
}

impl Mapper {
    fn lookup_address_space(&self, ptr: u16) -> (usize, u16) {
        for ((range_begin, range_end, space, use_original_address),
             space_idx) in
            self.mappings.iter()
                .zip(0..self.mappings.len()) {
            if ptr >= *range_begin && ptr <= *range_end {
                let space_ptr =
                    if *use_original_address { ptr }
                    else { (ptr - *range_begin) };
                return (space_idx, space_ptr);
            }
        }
        panic!("lookup_address_space - Unmapped pointer");
    }
    fn map_address_space(&mut self, begin: u16, end: u16, space: Box<dyn AddressSpace>, use_original: bool) {
        self.mappings.push((begin, end, space, use_original));
    }

    fn map_ram(&mut self, begin: u16, end: u16) {
        let size = end - begin;
        let space:Ram = Ram{ bs: vec![0; size as usize] };
        self.map_address_space(begin, end, Box::new(space), false);
    }
    fn map_rom(&mut self, begin: u16, end: u16, bytes: &[u8]) {
        let space:Rom = Rom{ bs: bytes.to_vec() };
        self.map_address_space(begin, end, Box::new(space), false);
    }
    fn map_null(&mut self, begin: u16, end: u16) {
        let space:NullAddressSpace = NullAddressSpace {};
        self.map_address_space(begin, end, Box::new(space), false);
    }
    fn map_mirrored(&mut self, begin: u16, end: u16, extended_begin: u16, extended_end: u16, space: Box<dyn AddressSpace>, use_original: bool) {
        let base_begin =
            if use_original { begin } else { 0 };
        let base_end = base_begin + (end - begin);
            let space:MirroredAddressSpace = MirroredAddressSpace {
                base: space,
                base_begin: base_begin, base_end,
            extended_begin, extended_end,
        };
        self.map_address_space(extended_begin, extended_end, Box::new(space), use_original);
    }
}

impl AddressSpace for Mapper {
    fn peek(&self, ptr:u16) -> u8{
        let (space_idx, space_ptr) = self.lookup_address_space(ptr);
        let (_, _, space, _) = &self.mappings[space_idx];
        return space.peek(space_ptr);
    }
    fn poke(&mut self, ptr:u16, value:u8) {
        let (space_idx, space_ptr) = self.lookup_address_space(ptr);
        let &mut (_,_, ref mut space,_) = self.mappings.get_mut(space_idx).unwrap();
        space.poke(space_ptr, value);
    }
}
