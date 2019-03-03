#![allow(unused_imports)]

use core::cell::UnsafeCell;
use core::ptr::null_mut;

pub trait AddressSpace {
    // Minimal definition
    fn peek(&self, ptr: u16) -> u8;
    fn poke(&mut self, ptr: u16, v: u8);

    // Helper methods
    fn peek16(&self, ptr:u16) -> u16 {
        let low = self.peek(ptr);
        let high = self.peek(ptr.wrapping_add(1));
        let result = (low as u16) + ((high as u16) << 8);
        //eprintln!("DEBUG - PEEK16 - {:x} {:x} {:x} {:x} {:x} {:x}", high, low, result, high as u16, low as u16, (high as u16) << 8);
        return result;
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

pub struct Ram {
    pub bs: &'static mut [u8]
}


impl Ram {
    pub fn new(bs: &'static mut [u8]) -> Ram {
        Ram { bs: bs }
    }
}

impl AddressSpace for Ram {
    fn peek(&self, ptr:u16) -> u8 {
        return self.bs[ptr as usize];
    }
    fn poke(&mut self, ptr:u16, v:u8) {
        self.bs[ptr as usize] = v;
    }
}

impl<T:AddressSpace> AddressSpace for *mut T {
    fn peek(&self, ptr:u16) -> u8 {
        let t:&T = unsafe { &**self as &T};
        return t.peek(ptr);
    }
    fn poke(&mut self, ptr:u16, x:u8) {
        let t:&mut T = unsafe { &mut **self as &mut T };
        t.poke(ptr, x);
    }
}

pub struct Rom {
    pub bs: &'static [u8]
}

impl Rom {
    pub fn new(bs: &'static [u8]) -> Rom{
        Rom { bs: bs }
    }
}

impl AddressSpace for Rom {
    fn peek(&self, ptr:u16) -> u8 {
        let value = self.bs[ptr as usize];
        //eprintln!("DEBUG - ROM-ACCESS - ({:?}, {:?})", ptr, value);
        return value;
    }
    fn poke(&mut self, ptr:u16, value:u8) {
        panic!("Rom - Attempted to write read-only memory at {} - value={}", ptr, value);
    }
}

pub struct MirroredAddressSpace {
    base: *mut AddressSpace,
    base_begin: u16,
    base_end: u16,
    extended_begin: u16,
    extended_end: u16,
}

impl MirroredAddressSpace {
    // If a memory range has been mirrored to another, map a pointer to the "base range" or fail if it lies outside.
    fn map_address(&self, ptr: u16) -> u16 {
        if ptr < self.extended_begin || ptr > self.extended_end {
            panic!("map_address: Out of mapped range ({:?} not in range [{:?}, {:?}]", ptr, self.extended_begin, self.extended_end);
        }
        let width = self.base_end - self.base_begin + 1;
        return (ptr - self.extended_begin) % width + self.base_begin;
    }
}


impl AddressSpace for MirroredAddressSpace {
    fn peek(&self, ptr:u16) -> u8 {
        return unsafe { (*self.base).peek(self.map_address(ptr)) };
    }
    fn poke(&mut self, ptr:u16, value:u8) {
        let space_ptr = self.map_address(ptr);
        unsafe { (*self.base).poke(space_ptr, value) };
    }
}

pub struct NullAddressSpace { }
impl NullAddressSpace {
    pub fn new() -> NullAddressSpace {
        NullAddressSpace { }
    }
}


impl AddressSpace for NullAddressSpace {
    fn peek(&self, ptr:u16) -> u8{
        panic!("DEBUG - READ FROM NULL MAP {:x}", ptr);
    }
    fn poke(&mut self, ptr:u16, value:u8) {
        panic!("DEBUG - WRITE TO NULL MAP {:x} {:x}", ptr, value);
    }
}

type UsesOriginalAddress = bool;
#[derive(Copy, Clone)]
struct Mapping(u16, u16, *mut AddressSpace, UsesOriginalAddress);

#[derive(Copy, Clone)]
pub struct Mapper {
    next_mapping_id: u8,
    mappings: [Mapping;16],
}

const THE_NULL_ADDRESS_SPACE:NullAddressSpace = NullAddressSpace { };

impl Mapper {
    #[rustc_promotable]
    pub fn new() -> Mapper {
        let mapper:*mut AddressSpace = null_mut::<Mapper>();
        Mapper {
            next_mapping_id: 0,
            mappings: [Mapping(0,0,mapper, false); 16],
        }
    }
    fn push(&mut self, mapping: Mapping) {
        let id = self.next_mapping_id;
        self.mappings[id as usize] = mapping;
        self.next_mapping_id += 1;
    }

    fn lookup_address_space(&self, ptr: u16) -> (usize, u16) {
        for space_idx in 0..self.next_mapping_id {
            let Mapping(range_begin, range_end, _, use_original_address) = self.mappings[space_idx as usize];
            if ptr >= range_begin && ptr <= range_end {
                let space_ptr =
                    if use_original_address { ptr }
                else { (ptr - range_begin) };
                return (space_idx as usize, space_ptr);
            }
        }
        panic!("Unknown mapping");
    }
    pub fn map_address_space(&mut self, begin: u16, end: u16, space: *mut AddressSpace, use_original: bool) {
        self.push(Mapping(begin, end, space, use_original));
    }
    pub fn map_null(&mut self, begin: u16, end: u16) {
        self.map_address_space(begin, end, &mut THE_NULL_ADDRESS_SPACE, true);
    }
    pub fn map_mirrored(&mut self, begin: u16, end: u16, extended_begin: u16, extended_end: u16, space: *mut AddressSpace, use_original: bool) {
        let base_begin =
            if use_original { begin } else { 0 };
        let base_end = base_begin + (end - begin);
        let mut space:MirroredAddressSpace = MirroredAddressSpace {
                base: space,
                base_begin: base_begin, base_end,
                extended_begin, extended_end,
        };
        // TODO - Stack-allocated variable
        self.map_address_space(extended_begin, extended_end, &mut space, true);
    }
}

impl AddressSpace for Mapper {
    fn peek(&self, ptr:u16) -> u8{
        let (space_idx, space_ptr) = self.lookup_address_space(ptr);
        let Mapping(_, _, space, _) = &self.mappings[space_idx];
        //eprintln!("DEBUG - MEMORY-ACCESS - ({:?}, {:?})", space_idx, space_ptr);
        let value = unsafe { (**space).peek(space_ptr) };
        //eprintln!("DEBUG - MEMORY-ACCESS-RESULT - ({:x})", value);
        return value;
    }
    fn poke(&mut self, ptr:u16, value:u8) {
        let (space_idx, space_ptr) = self.lookup_address_space(ptr);
        let Mapping(_,_, ref mut space,_) = self.mappings[space_idx];
        unsafe { (**space).poke(space_ptr, value) };
    }
}
