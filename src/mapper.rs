#![allow(unused_imports)]
#![allow(dead_code)]

use std::borrow::BorrowMut;
use std::cell::UnsafeCell;
use std::io::Read;
use std::io::Write;

use crate::common::ternary;
use crate::serialization::Savable;

pub trait AddressSpace: Savable {
    // Minimal definition
    fn peek(&self, ptr: u16) -> u8;
    fn poke(&mut self, ptr: u16, v: u8);

    // Helper methods
    fn peek16(&self, ptr: u16) -> u16 {
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
    bs: Vec<u8>,
}

impl Ram {
    pub fn new(size: usize) -> Ram {
        Ram { bs: vec![0; size] }
    }
}

impl Savable for Ram {
    fn save(&self, fh: &mut dyn Write) {
        self.bs.save(fh);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        self.bs.load(fh);
    }
}

impl AddressSpace for Ram {
    fn peek(&self, ptr: u16) -> u8 {
        return self.bs[ptr as usize];
    }
    fn poke(&mut self, ptr: u16, v: u8) {
        self.bs[ptr as usize] = v;
    }
}

impl<T: AddressSpace> AddressSpace for *mut T {
    fn peek(&self, ptr: u16) -> u8 {
        let t: &T = unsafe { &**self as &T };
        return t.peek(ptr);
    }
    fn poke(&mut self, ptr: u16, x: u8) {
        let t: &mut T = unsafe { &mut **self as &mut T };
        t.poke(ptr, x);
    }
}

impl<T: AddressSpace> Savable for *mut T {
    // The pointer should still be valid, since this Trait updates objects in-place.
    fn save(&self, _: &mut dyn Write) {}
    fn load(&mut self, _: &mut dyn Read) {}
}

pub struct Rom {
    bs: Vec<u8>,
}

impl Rom {
    pub fn new(bs: Vec<u8>) -> Rom {
        Rom { bs: bs }
    }
}

impl Savable for Rom {
    fn save(&self, fh: &mut dyn Write) {
        self.bs.save(fh);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        self.bs.load(fh);
    }
}

impl AddressSpace for Rom {
    fn peek(&self, ptr: u16) -> u8 {
        let value = self.bs[ptr as usize];
        //eprintln!("DEBUG - ROM-ACCESS - ({:?}, {:?})", ptr, value);
        return value;
    }
    fn poke(&mut self, ptr: u16, value: u8) {
        panic!(
            "Rom - Attempted to write read-only memory at {} - value={}",
            ptr, value
        );
    }
}

pub struct MirroredAddressSpace {
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
            panic!(
                "map_address: Out of mapped range ({:?} not in range [{:?}, {:?}]",
                ptr, self.extended_begin, self.extended_end
            );
        }
        let width = self.base_end - self.base_begin + 1;
        let relptr = ptr - self.extended_begin;
        if relptr >= width {
            return (ptr - self.extended_begin) % width + self.base_begin;
        } else {
            return relptr + self.base_begin;
        }
    }
}

impl Savable for MirroredAddressSpace {
    fn save(&self, fh: &mut dyn Write) {
        self.base.save(fh);
        self.base_begin.save(fh);
        self.base_end.save(fh);
        self.extended_begin.save(fh);
        self.extended_end.save(fh);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        self.base.load(fh);
        self.base_begin.load(fh);
        self.base_end.load(fh);
        self.extended_begin.load(fh);
        self.extended_end.load(fh);
    }
}

impl AddressSpace for MirroredAddressSpace {
    fn peek(&self, ptr: u16) -> u8 {
        return self.base.peek(self.map_address(ptr));
    }
    fn poke(&mut self, ptr: u16, value: u8) {
        let space_ptr = self.map_address(ptr);
        self.base.poke(space_ptr, value);
    }
}

pub struct NullAddressSpace {}
impl NullAddressSpace {
    pub fn new() -> NullAddressSpace {
        NullAddressSpace {}
    }
}

impl Savable for NullAddressSpace {
    fn save(&self, _fh: &mut dyn Write) {}
    fn load(&mut self, _fh: &mut dyn Read) {}
}

impl AddressSpace for NullAddressSpace {
    fn peek(&self, ptr: u16) -> u8 {
        eprintln!("DEBUG - READ FROM NULL MAP {:x}", ptr);
        return 0;
    }
    fn poke(&mut self, ptr: u16, value: u8) {
        eprintln!("DEBUG - WRITE TO NULL MAP {:x} {:x}", ptr, value);
    }
}

type UsesOriginalAddress = bool;
struct Mapping(u16, u16, Box<dyn AddressSpace>, UsesOriginalAddress);
impl Mapping {
    fn map_ptr(&self, ptr: u16) -> Option<u16> {
        let Mapping(range_begin, range_end, _, use_original_address) = *self;
        if ptr >= range_begin && ptr <= range_end {
            let space_ptr = ternary(use_original_address, ptr, ptr - range_begin);
            return Some(space_ptr);
        }
        return None;
    }
}

impl Savable for Mapping {
    fn save(&self, fh: &mut dyn Write) {
        self.0.save(fh);
        self.1.save(fh);
        self.2.save(fh);
        self.3.save(fh);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        self.0.load(fh);
        self.1.load(fh);
        self.2.load(fh);
        self.3.load(fh);
    }
}

pub struct Mapper {
    mappings: Vec<Mapping>,
}

impl Savable for Mapper {
    fn save(&self, fh: &mut dyn Write) {
        self.mappings.as_slice().save(fh);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        self.mappings.as_mut_slice().load(fh);
    }
}

impl Mapper {
    pub fn new() -> Mapper {
        Mapper {
            mappings: Vec::new(),
        }
    }
    fn print_mappings(&self) {
        for Mapping(range_begin, range_end, _, use_original_address) in self.mappings.iter() {
            eprintln!(
                "[{:x}, {:x}] - {:?}",
                range_begin, range_end, use_original_address
            );
        }
    }

    fn lookup_address_space(&self, ptr: u16) -> (usize, u16) {
        for space_idx in 0..self.mappings.len() {
            if let Some(space_ptr) = self.mappings[space_idx].map_ptr(ptr) {
                return (space_idx, space_ptr);
            }
        }
        eprintln!("lookup_address_space - Unmapped pointer {:?}.", ptr);
        eprintln!("Mappings:");
        self.print_mappings();
        panic!();
    }
    pub fn map_address_space(
        &mut self,
        begin: u16,
        end: u16,
        space: Box<dyn AddressSpace>,
        use_original: bool,
    ) {
        self.mappings.push(Mapping(begin, end, space, use_original));
    }

    pub fn map_ram(&mut self, begin: u16, end: u16) {
        let size = end - begin;
        let space: Ram = Ram {
            bs: vec![0; size as usize],
        };
        self.map_address_space(begin, end, Box::new(space), false);
    }
    pub fn map_rom(&mut self, begin: u16, end: u16, bytes: &[u8]) {
        let space: Rom = Rom { bs: bytes.to_vec() };
        self.map_address_space(begin, end, Box::new(space), false);
    }
    pub fn map_null(&mut self, begin: u16, end: u16) {
        let space: NullAddressSpace = NullAddressSpace {};
        self.map_address_space(begin, end, Box::new(space), true);
    }
    pub fn map_mirrored(
        &mut self,
        begin: u16,
        end: u16,
        extended_begin: u16,
        extended_end: u16,
        space: Box<dyn AddressSpace>,
        use_original: bool,
    ) {
        let base_begin = if use_original { begin } else { 0 };
        let base_end = base_begin + (end - begin);
        let space: MirroredAddressSpace = MirroredAddressSpace {
            base: space,
            base_begin: base_begin,
            base_end,
            extended_begin,
            extended_end,
        };
        self.map_address_space(extended_begin, extended_end, Box::new(space), true);
    }
}

impl AddressSpace for Mapper {
    fn peek(&self, ptr: u16) -> u8 {
        let (space_idx, space_ptr) = self.lookup_address_space(ptr);
        let Mapping(_, _, space, _) = &self.mappings[space_idx];
        //eprintln!("DEBUG - MEMORY-ACCESS - ({:?}, {:?})", space_idx, space_ptr);
        let value = space.peek(space_ptr);
        //eprintln!("DEBUG - MEMORY-ACCESS-RESULT - ({:x})", value);
        return value;
    }
    fn poke(&mut self, ptr: u16, value: u8) {
        let (space_idx, space_ptr) = self.lookup_address_space(ptr);
        let &mut Mapping(_, _, ref mut space, _) = self.mappings.get_mut(space_idx).unwrap();
        space.poke(space_ptr, value);
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum AccessType {
    Read,
    Write,
}

pub type LoggedAddressSpaceRecord = (usize, AccessType, u16, u8);

pub struct LoggedAddressSpace {
    pub space: Box<dyn AddressSpace>,
    pub log: UnsafeCell<Vec<LoggedAddressSpaceRecord>>,
}

impl Savable for LoggedAddressSpace {
    fn save(&self, _fh: &mut dyn Write) {
        panic!("save() unimplemented");
    }
    fn load(&mut self, _fh: &mut dyn Read) {
        panic!("load() unimplemented");
    }
}

impl LoggedAddressSpace {
    pub fn new(space: Box<dyn AddressSpace>) -> LoggedAddressSpace {
        LoggedAddressSpace {
            space: space,
            log: UnsafeCell::new(vec![]),
        }
    }
    pub fn get_log(&self) -> &mut Vec<LoggedAddressSpaceRecord> {
        return unsafe { &mut *self.log.get() };
    }
    pub fn copy_log(&self) -> Vec<LoggedAddressSpaceRecord> {
        return self.get_log().clone();
    }
}

impl AddressSpace for LoggedAddressSpace {
    fn peek(&self, ptr: u16) -> u8 {
        let v = self.space.peek(ptr);
        let log = self.get_log();
        let record = (log.len(), AccessType::Read, ptr, v);
        log.push(record);
        return v;
    }
    fn poke(&mut self, ptr: u16, v: u8) {
        let log = self.get_log();
        let record = (log.len(), AccessType::Write, ptr, v);
        log.push(record);
        self.space.poke(ptr, v);
    }
}

mod tests {
    use super::AddressSpace;
    use super::Mapper;
    use super::Rom;

    #[test]
    fn test_rom() {
        let mut mapper = Mapper::new();
        let bs = [1, 2, 3];
        mapper.map_rom(0x1000, 0x1002, &bs);
        assert_eq!(mapper.peek(0x1001), 2);
    }
    #[test]
    fn test_mirrored() {
        let mut mapper = Mapper::new();
        let bs = vec![1, 2, 3];
        let rom: Rom = Rom::new(bs);
        mapper.map_mirrored(0x1000, 0x1002, 0x5000, 0x6000, Box::new(rom), false);
        assert_eq!(mapper.peek(0x5002), 3);
        assert_eq!(mapper.peek(0x5005), 3);
    }
}
