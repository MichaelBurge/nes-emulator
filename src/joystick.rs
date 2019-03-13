#![allow(mutable_transmutes)]

use std::mem::transmute;
use sdl2::GameControllerSubsystem;
use sdl2::event::Event;
use std::io::Read;
use std::io::Write;

use crate::common::get_bit;
use crate::mapper::AddressSpace;
use crate::serialization::Savable;

pub struct Joystick {
    sdl_controller: Option<sdl2::controller::GameController>,
    sdl_id: u32,
    is_software: bool,
    pub button_mask: u8,
    strobe_active: bool,
}

impl Savable for Joystick {
    fn save(&self, fh: &mut Write) {
        self.button_mask.save(fh);
        self.strobe_active.save(fh);
    }
    fn load(&mut self, fh: &mut Read) {
        self.button_mask.load(fh);
        self.strobe_active.load(fh);
    }
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
    pub fn new(subsystem:&GameControllerSubsystem, id:u32) -> Joystick {
        Joystick {
            sdl_controller: subsystem.open(0).ok(),
            sdl_id: id,
            is_software: false,
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
    pub fn process_event(&mut self, subsystem:&GameControllerSubsystem, event:&Event) {
        match event {
            Event::ControllerDeviceAdded { which: id, .. } => {
                eprintln!("DEBUG - CONTROLLER ADDED - {}", id);
                if *id == self.sdl_id {
                    self.sdl_controller = Some(subsystem.open(*id).unwrap());
                    eprintln!("DEBUG - CONTROLLER ACQUIRED - {}", self.sdl_controller.is_some());
                }
            }
            _ => {},
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
        match &self.sdl_controller {
            None => 0,
            Some(controller) => controller.button(button) as u8,
        }
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
        if self.strobe_active && ! self.is_software {
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
        mut_self.button_mask >>= 1;
        return byte;
    }
    fn poke(&mut self, _ptr:u16, v:u8) {
        self.strobe_active = get_bit(v,0)>0;
        self.reset_from_strobe();
    }
}
