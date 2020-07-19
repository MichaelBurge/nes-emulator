#![allow(dead_code)]

use core::cell::Cell;
use std::io::Read;
use std::io::Write;

use crate::common::get_bit;
use crate::mapper::AddressSpace;
use crate::serialization::Savable;

#[derive(Debug)]
pub struct Joystick {
    buttons: u8,
    buttons_register: Cell<u8>,
    strobe_active: bool,
}

impl Savable for Joystick {
    fn save(&self, fh: &mut dyn Write) {
        self.buttons_register.get().save(fh);
        self.strobe_active.save(fh);
    }
    fn load(&mut self, fh: &mut dyn Read) {
        let mut buttons_register = 0;
        buttons_register.load(fh);
        self.buttons_register.set(buttons_register);
        self.strobe_active.load(fh);
    }
}

impl Joystick {
    pub fn new() -> Joystick {
        Joystick {
            buttons: 0,
            buttons_register: Cell::new(0),
            strobe_active: false,
        }
    }
    pub fn set_buttons(&mut self, button_mask: u8) {
        self.buttons = button_mask;
    }
    fn get_next_button(&self) -> u8 {
        let byte = self.buttons_register.get() & 1;
        let new_buttons_register = self.buttons_register.get() >> 1;
        self.buttons_register.set(new_buttons_register);
        return byte;
    }
    fn reset_from_strobe(&self) {
        if self.strobe_active {
            let buttons = self.buttons;
            self.buttons_register.set(buttons);
        }
    }
}

impl AddressSpace for Joystick {
    fn peek(&self, _ptr: u16) -> u8 {
        self.reset_from_strobe();
        return self.get_next_button();
    }
    fn poke(&mut self, _ptr: u16, v: u8) {
        self.strobe_active = get_bit(v, 0) > 0;
        self.reset_from_strobe();
    }
}
