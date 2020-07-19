#![allow(unused_must_use)] // TODO
#![allow(dead_code)]

use std::default::Default;
use std::fs::File;
use std::io::Read;
use std::io::Write;

pub trait Savable {
    fn save(&self, fh: &mut dyn Write);
    fn load(&mut self, fh: &mut dyn Read);
}

impl Savable for bool {
    fn save(&self, fh: &mut dyn Write) {
        let bytes = [*self as u8];
        fh.write_all(&bytes);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        let mut bytes = [0];
        fh.read_exact(&mut bytes);
        *self = bytes[0] > 0;
    }
}

impl Savable for u8 {
    fn save(&self, fh: &mut dyn Write) {
        let bytes = [*self as u8];
        fh.write_all(&bytes);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        let mut bytes = [0];
        fh.read_exact(&mut bytes);
        *self = bytes[0];
    }
}

impl Savable for u16 {
    fn save(&self, fh: &mut dyn Write) {
        let bytes = [(*self & 0xff) as u8, ((*self >> 8) & 0xff) as u8];
        fh.write_all(&bytes);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        let mut bytes = [0; 2];
        fh.read_exact(&mut bytes);
        *self = 0;
        *self |= bytes[0] as u16;
        *self |= (bytes[1] as u16) << 8;
    }
}

impl Savable for u32 {
    fn save(&self, fh: &mut dyn Write) {
        let bytes = [
            ((*self >> 0) & 0xff) as u8,
            ((*self >> 8) & 0xff) as u8,
            ((*self >> 16) & 0xff) as u8,
            ((*self >> 24) & 0xff) as u8,
        ];
        fh.write_all(&bytes);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        let mut bytes = [0u8; 4];
        fh.read_exact(&mut bytes);
        *self = 0;
        *self |= (bytes[0] as u32) << 0;
        *self |= (bytes[1] as u32) << 8;
        *self |= (bytes[2] as u32) << 16;
        *self |= (bytes[3] as u32) << 24;
    }
}

impl Savable for u64 {
    fn save(&self, fh: &mut dyn Write) {
        let bytes = [
            ((*self >> 0) & 0xff) as u8,
            ((*self >> 8) & 0xff) as u8,
            ((*self >> 16) & 0xff) as u8,
            ((*self >> 24) & 0xff) as u8,
            ((*self >> 32) & 0xff) as u8,
            ((*self >> 40) & 0xff) as u8,
            ((*self >> 48) & 0xff) as u8,
            ((*self >> 56) & 0xff) as u8,
        ];
        fh.write_all(&bytes);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        let mut bytes = [0u8; 8];
        fh.read_exact(&mut bytes);
        *self = 0;
        *self |= (bytes[0] as u64) << 0;
        *self |= (bytes[1] as u64) << 8;
        *self |= (bytes[2] as u64) << 16;
        *self |= (bytes[3] as u64) << 24;
        *self |= (bytes[4] as u64) << 32;
        *self |= (bytes[5] as u64) << 40;
        *self |= (bytes[6] as u64) << 48;
        *self |= (bytes[7] as u64) << 56;
    }
}

impl Savable for usize {
    fn save(&self, fh: &mut dyn Write) {
        (*self as u64).save(fh);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        let mut x: u64 = *self as u64;
        x.load(fh);
        *self = x as usize;
    }
}

impl<T: Savable> Savable for [T] {
    fn save(&self, fh: &mut dyn Write) {
        let len: usize = self.len();
        len.save(fh);
        for i in self.iter() {
            i.save(fh);
        }
    }
    fn load(&mut self, fh: &mut dyn Read) {
        let mut len = 0usize;
        len.load(fh);
        for i in 0..len {
            self[i].load(fh);
        }
    }
}

impl<T: Savable + Default> Savable for Vec<T> {
    fn save(&self, fh: &mut dyn Write) {
        let len: usize = self.len();
        len.save(fh);
        for i in self.iter() {
            i.save(fh);
        }
    }
    fn load(&mut self, fh: &mut dyn Read) {
        let mut len = 0usize;
        len.load(fh);
        self.truncate(0);
        self.reserve(len);
        for _ in 0..len {
            let mut x: T = Default::default();
            x.load(fh);
            self.push(x);
        }
    }
}

impl Savable for String {
    fn save(&self, fh: &mut dyn Write) {
        let len = self.len() as u32;
        len.save(fh);
        for byte in self.bytes() {
            byte.save(fh);
        }
    }
    fn load(&mut self, fh: &mut dyn Read) {
        let len = read_value::<u32>(fh) as usize;
        let mut bytes = vec![0; len];
        for i in 0..len {
            bytes[i] = read_value::<u8>(fh);
        }
        *self = String::from_utf8(bytes).expect("Invalid utf8");
    }
}

pub fn read_value<T: Default + Savable>(fh: &mut dyn Read) -> T {
    let mut t: T = Default::default();
    t.load(fh);
    t
}

use std::io::Seek;
use std::io::SeekFrom;

pub fn file_position(fh: &mut File) -> u64 {
    fh.seek(SeekFrom::Current(0)).unwrap()
}
