#![allow(dead_code)]
#![allow(mutable_transmutes)]

use std::mem::transmute;
use std::io::Read;
use std::io::Write;
// use core::cell::UnsafeCell;

use crate::common::get_bit;
use crate::mapper::AddressSpace;
use crate::serialization::Savable;

#[derive(Debug)]
pub struct Joystick {
    buttons: u8,
    buttons_register: u8,
    strobe_active: bool,
}

impl Savable for Joystick {
    fn save(&self, fh: &mut Write) {
        self.buttons_register.save(fh);
        self.strobe_active.save(fh);
    }
    fn load(&mut self, fh: &mut Read) {
        self.buttons_register.load(fh);
        self.strobe_active.load(fh);
    }
}

impl Joystick {
    pub fn new() -> Joystick {
        Joystick {
            buttons: 0,
            buttons_register: 0,
            strobe_active: false,
        }
    }
    pub fn set_buttons(&mut self, button_mask: u8) {
        self.buttons = button_mask;
    }
    fn get_next_button(&self) -> u8 {
        let mut_self:&mut Self = unsafe { transmute(self) };
        let byte = self.buttons_register & 1;
        mut_self.buttons_register >>= 1;
        return byte;
    }
    fn reset_from_strobe(&self) {
        let mut_self:&mut Self = unsafe { transmute(self) };
        if self.strobe_active {
            mut_self.buttons_register = self.buttons;
        }
    }
}

impl AddressSpace for Joystick {
    fn peek(&self, _ptr:u16) -> u8 {
        self.reset_from_strobe();
        return self.get_next_button();
    }
    fn poke(&mut self, _ptr:u16, v:u8) {
        self.strobe_active = get_bit(v,0)>0;
        self.reset_from_strobe();
    }
}
