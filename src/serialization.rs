use std::fs::File;
use std::io::Read;
use std::io::Write;

pub trait Savable {
    fn save(&self, fh: &mut File);
    fn load(&mut self, fh: &mut File);
}

impl Savable for bool {
    fn save(&self, fh: &mut File) {
        let bytes = [*self as u8];
        fh.write_all(&bytes);
    }
    fn load(&mut self, fh: &mut File) {
        let mut bytes = [0];
        fh.read_exact(&mut bytes);
        *self = bytes[0]>0;
    }
}

impl Savable for u8 {
    fn save(&self, fh: &mut File) {
        let bytes = [*self as u8];
        fh.write_all(&bytes);
    }
    fn load(&mut self, fh: &mut File) {
        let mut bytes = [0];
        fh.read_exact(&mut bytes);
        *self = bytes[0];
    }
}

impl Savable for u16 {
    fn save(&self, fh: &mut File) {
        let bytes = [(*self & 0xff) as u8, ((*self >> 8) & 0xff) as u8];
        fh.write_all(&bytes);
    }
    fn load(&mut self, fh:&mut File) {
        let mut bytes = [0; 2];
        fh.read_exact(&mut bytes);
        *self = 0;
        *self |= bytes[0] as u16;
        *self |= (bytes[1] as u16) << 8;
    }
}

impl Savable for u32 {
    fn save(&self, fh: &mut File) {
        let bytes = [
            ((*self >> 0 ) & 0xff) as u8,
            ((*self >> 8 ) & 0xff) as u8,
            ((*self >> 16) & 0xff) as u8,
            ((*self >> 24) & 0xff) as u8,
        ];
        fh.write_all(&bytes);
    }
    fn load(&mut self, fh: &mut File) {
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
    fn save(&self, fh: &mut File) {
        let bytes = [
            ((*self >> 0 ) & 0xff) as u8,
            ((*self >> 8 ) & 0xff) as u8,
            ((*self >> 16) & 0xff) as u8,
            ((*self >> 24) & 0xff) as u8,
            ((*self >> 32) & 0xff) as u8,
            ((*self >> 40) & 0xff) as u8,
            ((*self >> 48) & 0xff) as u8,
            ((*self >> 56) & 0xff) as u8,
        ];
        fh.write_all(&bytes);
    }
    fn load(&mut self, fh: &mut File) {
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
    fn save(&self, fh: &mut File) {
        (*self as u64).save(fh);
    }
    fn load(&mut self, fh: &mut File) {
        let mut x:u64 = *self as u64;
        x.load(fh);
        *self = x as usize;
    }
}

impl<T: Savable> Savable for [T] {
    fn save(&self, fh: &mut File) {
        let len:usize = self.len();
        len.save(fh);
        for i in self.iter() {
            i.save(fh);
        }
    }
    fn load(&mut self, fh: &mut File) {
        let mut len = 0usize;
        len.load(fh);
        for i in 0..len {
            self[i].load(fh);
        }
    }
}

use std::io::SeekFrom;
use std::io::Seek;

pub fn file_position(fh: &mut File) -> u64 {
    fh.seek(SeekFrom::Current(0)).unwrap()
}
