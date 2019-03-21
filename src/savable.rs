use std::io::{Read, Write};
use byteorder::{ReadBytesExt, WriteBytesExt, LittleEndian};

pub trait NewSavable {
    fn save(&self, writer: &mut Write);
    fn load(reader: &mut Read) -> Self;
}

impl NewSavable for bool {
    fn save(&self, writer: &mut Write) {
        writer.write_u8(*self as u8)
            .expect("Could not save bool");
    }
    fn load(reader: &mut Read) -> bool {
        let byte = reader.read_u8()
            .expect("Could not load bool");
        byte > 0
    }
}

impl NewSavable for u8 {
    fn save(&self, writer: &mut Write) {
        writer.write_u8(*self)
            .expect("Could not save u8");
    }
    fn load(reader: &mut Read) -> u8 {
        reader.read_u8()
            .expect("Could not load u8")
    }
}

impl NewSavable for u16 {
    fn save(&self, writer: &mut Write) {
        writer.write_u16::<LittleEndian>(*self)
            .expect("Could not save u16");
    }
    fn load(reader: &mut Read) -> u16 {
        reader.read_u16::<LittleEndian>()
            .expect("Could not load u16")
    }
}

impl NewSavable for u32 {
    fn save(&self, writer: &mut Write) {
        writer.write_u32::<LittleEndian>(*self)
            .expect("Could not save u32");
    }
    fn load(reader: &mut Read) -> u32 {
        reader.read_u32::<LittleEndian>()
            .expect("Could not load u32")
    }
}

impl NewSavable for u64 {
    fn save(&self, writer: &mut Write) {
        writer.write_u64::<LittleEndian>(*self)
            .expect("Could not save u64");
    }
    fn load(reader: &mut Read) -> u64 {
        reader.read_u64::<LittleEndian>()
            .expect("Could not load u64")
    }
}

// Treat usize as u64
impl NewSavable for usize {
    fn save(&self, writer: &mut Write) {
        writer.write_u64::<LittleEndian>(*self as u64)
            .expect("Could not save usize");
    }
    fn load(reader: &mut Read) -> usize {
        reader.read_u64::<LittleEndian>()
            .expect("Could not load usize") as usize
    }
}

impl<T: NewSavable> NewSavable for Vec<T> {
    fn save(&self, writer: &mut Write) {
        self.len().save(writer);
        self.iter().for_each(|item| item.save(writer));
    }
    fn load(reader: &mut Read) -> Vec<T> {
        let capacity : usize = load(reader);
        let mut result = Self::with_capacity(capacity);
        (0..capacity).for_each(|_count| {
            let item = load(reader);
            result.push(item);
        });
        result
    }
}

pub fn load<T: NewSavable>(reader: &mut Read) -> T {
    T::load(reader)
}

pub fn save<T: NewSavable>(writer: &mut Write, item: T) {
    item.save(writer)
}
