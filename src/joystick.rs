#![allow(mutable_transmutes)]

use std::mem::transmute;
use sdl2::GameControllerSubsystem;

use crate::common::get_bit;
use crate::mapper::AddressSpace;

pub struct Joystick {
    sdl_controller: sdl2::controller::GameController,
    button_mask: u8,
    strobe_active: bool,
}

impl Joystick {
    pub fn new(subsystem:&GameControllerSubsystem, id:u8) -> Joystick {
        Joystick {
            sdl_controller: subsystem.open(0).unwrap(),
            button_mask: 0,
            strobe_active: false,
        }
    }
    fn get_button_bit(&self, button_id:u8) -> u8 {
        // Button order: A,B, Select,Start,Up,Down,Left,Right
        let button = match button_id {
            0 => sdl2::controller::Button::A,
            1 => sdl2::controller::Button::B,
            2 => sdl2::controller::Button::Back,
            3 => sdl2::controller::Button::Start,
            4 => sdl2::controller::Button::DPadUp,
            5 => sdl2::controller::Button::DPadDown,
            6 => sdl2::controller::Button::DPadLeft,
            7 => sdl2::controller::Button::DPadRight,
            _ => panic!("Unknown button"),
        };
        return self.sdl_controller.button(button) as u8;
    }
    fn refresh_button_mask(&mut self) {
        self.button_mask = 0;
        self.button_mask |= self.get_button_bit(0) << 0;
        self.button_mask |= self.get_button_bit(1) << 1;
        self.button_mask |= self.get_button_bit(2) << 2;
        self.button_mask |= self.get_button_bit(3) << 3;
        self.button_mask |= self.get_button_bit(4) << 4;
        self.button_mask |= self.get_button_bit(5) << 5;
        self.button_mask |= self.get_button_bit(6) << 6;
        self.button_mask |= self.get_button_bit(7) << 7;
    }
    fn reset_from_strobe(&mut self) {
        if self.strobe_active {
            self.refresh_button_mask();
        }
    }
}

impl AddressSpace for Joystick {
    fn peek(&self, _ptr:u16) -> u8 {
        let mut_self:&mut Self = unsafe { transmute(self) };
        mut_self.reset_from_strobe();
        /*
        eprintln!("DEBUG - CONTROLLER - A:{} B:{} Select:{} Start:{} Up:{} Down:{} Left:{} Right:{}",
                  self.sdl_controller.button(sdl2::controller::Button::A),
                  self.sdl_controller.button(sdl2::controller::Button::B),
                  self.sdl_controller.button(sdl2::controller::Button::Back),
                  self.sdl_controller.button(sdl2::controller::Button::Start),
                  self.sdl_controller.button(sdl2::controller::Button::DPadUp),
                  self.sdl_controller.button(sdl2::controller::Button::DPadDown),
                  self.sdl_controller.button(sdl2::controller::Button::DPadLeft),
                  self.sdl_controller.button(sdl2::controller::Button::DPadRight),
        );
         */
        let byte = self.button_mask & 1;
        // eprintln!("DEBUG - CONTROLLER {} {}", self.button_mask, byte);
        mut_self.button_mask >>= 1;
        return byte;
    }
    fn poke(&mut self, _ptr:u16, v:u8) {
        self.strobe_active = get_bit(v,0)>0;
        self.reset_from_strobe();
    }
}
