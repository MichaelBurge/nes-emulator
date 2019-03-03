#![allow(mutable_transmutes)]

use core::mem::transmute;

use crate::common::get_bit;
use crate::mapper::AddressSpace;

pub struct Joystick {
    sdl_controller: Option<()>,
    sdl_id: u32,
    is_software: bool,
    button_mask: u8,
    strobe_active: bool,
}

impl Joystick {
    pub fn new_software() -> Joystick {
        Joystick {
            sdl_controller: None,
            sdl_id: 0,
            is_software: true,
            button_mask: 0,
            strobe_active: false,
        }
    }
    pub fn set_buttons(&mut self, button_mask: u8) {
        if self.is_software {
            self.button_mask = button_mask;
        } else {
            panic!("Can only override the buttons on software-controlled joysticks");
        }
    }
    fn reset_from_strobe(&mut self) {
    }
}

impl AddressSpace for Joystick {
    fn peek(&self, _ptr:u16) -> u8 {
        let mut_self:&mut Self = unsafe { transmute(self) };
        mut_self.reset_from_strobe();
        let byte = self.button_mask & 1;
        mut_self.button_mask >>= 1;
        return byte;
    }
    fn poke(&mut self, _ptr:u16, v:u8) {
        self.strobe_active = get_bit(v,0)>0;
        self.reset_from_strobe();
    }
}
