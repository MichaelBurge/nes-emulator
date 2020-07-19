#![allow(dead_code)]
#![allow(unused_mut)]

mod apu;
mod c6502;
mod common;
mod joystick;
mod mapper;
mod nes;
mod ppu;
mod serialization;

extern crate sdl2;

use sdl2::audio::{AudioCallback, AudioQueue, AudioSpecDesired};
use sdl2::controller::GameController;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::Canvas;
use sdl2::render::Texture;
use sdl2::render::TextureAccess;
use sdl2::render::TextureCreator;
use sdl2::video::Window;
use sdl2::video::WindowContext;
use sdl2::AudioSubsystem;
use sdl2::EventPump;
use sdl2::GameControllerSubsystem;
use sdl2::VideoSubsystem;
use std::fs::File;
use std::io::ErrorKind;
use std::os::raw::c_int;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use core::ptr::null_mut;

use crate::apu::Apu;
use crate::joystick::Joystick;
use crate::mapper::AddressSpace;
use crate::nes::Nes;
use crate::nes::Tas;
use crate::nes::{load_ines, read_ines};
use crate::ppu::*;
use crate::serialization::Savable;

extern "C" {
    fn emscripten_set_main_loop(m: extern "C" fn(), fps: c_int, infinite: c_int);
}

// https://wiki.nesdev.com/w/index.php/Cycle_reference_chart
const CLOCKS_PER_FRAME: u32 = 29780;
const APU_FREQUENCY: i32 = 240;
const AUDIO_FREQUENCY: usize = 44100;
const SAMPLES_PER_FRAME: usize = 1024;
const SCALE: usize = 4;
const RECORDING: bool = true;
const ROM_BEGIN_SAVESTATE: &'static str = "initial.state";
const DEFAULT_SAVESTATE: &'static str = "save.state";
const DEFAULT_RECORDING: &'static str = "save.video";

struct GlobalState {
    sdl_context: *mut sdl2::Sdl,
    joystick1: *mut Joystick,
    joystick2: *mut Joystick,
    video_subsystem: *mut VideoSubsystem,
    audio_subsystem: *mut AudioSubsystem,
    controller_subsystem: *mut GameControllerSubsystem,
    canvas: *mut Canvas<Window>,
    event_pump: *mut EventPump,
    nes: *mut Nes,
    audio_device: *mut AudioQueue<f32>,
    texture: *mut Texture<'static>,
    sdl_controller1: *mut sdl2::controller::GameController,
    sdl_controller2: *mut sdl2::controller::GameController,
    tas: *mut Tas,
    tas_frame: usize,
    turbo_mode: bool,
}

static mut GLOBAL_STATE: Option<GlobalState> = None;
static mut TEXTURE_CREATOR: Option<TextureCreator<WindowContext>> = None;
fn main() {
    let mut sdl_context = Box::new(sdl2::init().unwrap());
    let mut video_subsystem = Box::new(sdl_context.video().unwrap());
    let mut controller_subsystem = Box::new(sdl_context.game_controller().unwrap());
    let mut audio_subsystem = Box::new(sdl_context.audio().unwrap());
    let mut joystick1 = Box::new(Joystick::new());
    let mut joystick2 = Box::new(Joystick::new());
    let joystick1_ptr = (&mut *joystick1) as *mut Joystick;
    let joystick2_ptr = (&mut *joystick2) as *mut Joystick;
    let window = video_subsystem
        .window(
            "NES emulator",
            (RENDER_WIDTH * SCALE) as u32,
            (RENDER_HEIGHT * SCALE) as u32,
        )
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = Box::new(window.into_canvas().build().unwrap());
    let texture_creator = canvas.texture_creator();
    let mut texture = {
        let mut tex = texture_creator
            .create_texture(
                PixelFormatEnum::RGB24,
                TextureAccess::Streaming,
                RENDER_WIDTH as u32,
                RENDER_HEIGHT as u32,
            )
            .unwrap();
        unsafe { Box::new(std::mem::transmute(tex)) }
    };
    let mut nes = Box::new(create_nes(joystick1, joystick2));
    match File::open(ROM_BEGIN_SAVESTATE) {
        Ok(mut fh) => nes.load(&mut fh),
        Err(ref e) if e.kind() == ErrorKind::NotFound => {
            if let Ok(mut fh) = File::create(ROM_BEGIN_SAVESTATE) {
                nes.save(&mut fh);
            }
        }
        Err(e) => eprintln!("DEBUG - Unhandled file error - {:?}", e),
    }
    let desired_spec = AudioSpecDesired {
        freq: Some(AUDIO_FREQUENCY as i32),
        channels: Some(1),
        //samples: Some(8820),
        samples: Some(SAMPLES_PER_FRAME as u16),
    };
    let mut tas = Box::new(Tas::new());
    // let audio_device = audio_subsystem.open_playback(None, &desired_spec, |spec| {
    //     ApuSampler {
    //         apu: NonNull::from(&mut nes.apu),
    //         volume: 1.0,
    //         resample_step:0,
    //         sample: 0.0,
    //         last_sample: 0.0,
    //         last_time: Instant::now(),
    //     }
    // }).unwrap();
    canvas.set_draw_color(Color::RGB(0, 255, 255));
    canvas.clear();
    canvas.present();

    let mut audio_device = Box::new(audio_subsystem.open_queue(None, &desired_spec).unwrap());
    audio_device.resume();
    let mut event_pump = Box::new(sdl_context.event_pump().unwrap());
    unsafe {
        GLOBAL_STATE = Some(GlobalState {
            sdl_context: &mut *sdl_context,
            joystick1: joystick1_ptr,
            joystick2: joystick2_ptr,
            video_subsystem: &mut *video_subsystem,
            audio_subsystem: &mut *audio_subsystem,
            controller_subsystem: &mut *controller_subsystem,
            canvas: &mut *canvas,
            event_pump: &mut *event_pump,
            nes: &mut *nes,
            audio_device: &mut *audio_device,
            texture: &mut *texture,
            sdl_controller1: null_mut(),
            sdl_controller2: null_mut(),
            tas: &mut *tas,
            tas_frame: 0,
            turbo_mode: false,
        });
    }

    if cfg!(target_os = "emscripten") {
        // void emscripten_set_main_loop(em_callback_func func, int fps, int simulate_infinite_loop);
        unsafe { emscripten_set_main_loop(main_loop, 60, 1) };
        loop {}
    } else {
        let mut every_second = Instant::now();
        let mut num_frames = 0;
        loop {
            let now = Instant::now();
            main_loop();
            let after = Instant::now();
            num_frames += 1;
            if after - every_second >= Duration::from_millis(1000) {
                eprintln!(
                    "DEBUG - FPS - {} {:?} {:?}",
                    num_frames,
                    after - every_second,
                    after - now
                );
                num_frames = 0;
                every_second = after;
            }
            //SDL_Delay(time_to_next_frame());
        }
    }
    //std::unreachable!();
}

extern "C" fn main_loop() {
    let now = Instant::now();
    let st = unsafe { GLOBAL_STATE.as_mut().unwrap() };
    // let mut sdl_context = unsafe { &mut *st.sdl_context };
    let joystick1: &mut Joystick = unsafe { &mut *st.joystick1 };
    let joystick2: &mut Joystick = unsafe { &mut *st.joystick2 };
    let mut nes = unsafe { &mut *st.nes };
    let mut event_pump = unsafe { &mut *st.event_pump };
    let mut audio_device = unsafe { &mut *st.audio_device };
    let mut canvas = unsafe { &mut *st.canvas };
    let mut texture = unsafe { &mut *st.texture };
    let mut controller_subsystem = unsafe { &mut *st.controller_subsystem };
    let mut tas = unsafe { &mut *st.tas };
    // eprintln!("DEBUG - POINTERS - ({:p}, {:?}) ({:p}, {:?}) {:p} {:p} {:p} {:p} {:p}",
    //           joystick1,
    //           joystick1,
    //           joystick2,
    //           joystick2,
    //           nes,
    //           event_pump,
    //           audio_device,
    //           canvas,
    //           texture,
    //           );

    for event in event_pump.poll_iter() {
        match event {
            // Exit game
            Event::Quit { .. }
            | Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => {
                std::process::exit(0);
            }
            // Break CPU debugger
            Event::KeyDown {
                keycode: Some(Keycode::Pause),
                ..
            } => {
                nes.break_debugger();
            }
            // Save state
            Event::KeyDown {
                keycode: Some(Keycode::F5),
                ..
            } => {
                let mut file = File::create(DEFAULT_SAVESTATE).unwrap();
                nes.save(&mut file);
                let mut tas_file = File::create(DEFAULT_RECORDING).unwrap();
                tas.save(&mut tas_file);
            }
            // Load State
            Event::KeyDown {
                keycode: Some(Keycode::F6),
                ..
            } => {
                let mut file = File::open(DEFAULT_SAVESTATE).unwrap();
                nes.load(&mut file);
                let mut tas_file = File::open(DEFAULT_RECORDING).unwrap();
                tas.load(&mut tas_file);
            }
            // Play recording from initial state
            Event::KeyDown {
                keycode: Some(Keycode::F7),
                ..
            } => {
                let mut tas_fh = File::open(DEFAULT_RECORDING).unwrap();
                tas.load(&mut tas_fh);
                let mut ss_fh = File::open(ROM_BEGIN_SAVESTATE).unwrap();
                nes.load(&mut ss_fh);
                st.tas_frame = 0;
                nes.cpu.poke(0x075a, 3);
            }
            // Begin recording at current point
            Event::KeyDown {
                keycode: Some(Keycode::F8),
                ..
            } => {
                let mut ss_fh = File::create(ROM_BEGIN_SAVESTATE).unwrap();
                nes.save(&mut ss_fh);
                *tas = Tas::new();
                st.tas_frame = 0;
            }
            // Attach controller
            Event::ControllerDeviceAdded { which: id, .. } => {
                eprintln!("DEBUG - CONTROLLER ADDED - {}", id);
                match id {
                    0 => {
                        st.sdl_controller1 =
                            Box::leak(Box::new(controller_subsystem.open(id).unwrap()))
                    }
                    1 => {
                        st.sdl_controller2 =
                            Box::leak(Box::new(controller_subsystem.open(id).unwrap()))
                    }
                    _ => eprintln!("DEBUG - UNEXPECTED CONTROLLER ID {}", id),
                }
            }
            // Toggle Turbo Mode
            Event::KeyDown {
                keycode: Some(Keycode::Tab),
                ..
            } => {
                st.turbo_mode = !st.turbo_mode;
            }
            _ => {}
        }
    }

    let frame = st.tas_frame;
    let j1_bmask = tas.get_inputs(frame).unwrap_or_else(|| {
        let buttons = get_button_mask(st.sdl_controller1);
        if RECORDING {
            tas.record_frame(frame, buttons);
        }
        buttons
    });
    st.tas_frame += 1;
    let j2_bmask = get_button_mask(st.sdl_controller2);
    joystick1.set_buttons(j1_bmask);
    joystick2.set_buttons(j2_bmask);
    nes.run_frame();
    present_frame(&mut canvas, &mut texture, &nes.ppu.render());
    enqueue_frame_audio(&audio_device, &mut nes.apu.samples);

    canvas.present();

    let after = Instant::now();
    let target_millis = Duration::from_millis(1000 / 60);
    let sleep_millis = target_millis.checked_sub(after - now);
    match sleep_millis {
        None => {} // Took too long last frame
        Some(sleep_millis) => {
            //eprintln!("DEBUG - SLEEP - {:?}", sleep_millis);
            if !st.turbo_mode {
                ::std::thread::sleep(sleep_millis);
            }
        }
    }
}

struct ApuSampler {
    apu: NonNull<Box<Apu>>,
    volume: f32,
    resample_step: u32,
    sample: f32,
    last_sample: f32,
    last_time: Instant,
}

unsafe impl std::marker::Send for ApuSampler {}

const SAMPLES_PER_SECOND: u32 = 1789920;
const CLOCK_FREQUENCY: u32 = 30000;
const SAMPLES_PER_CLOCK: u32 = SAMPLES_PER_SECOND / CLOCK_FREQUENCY;

impl ApuSampler {
    fn resample(last_sample: &mut f32, samples: &[f32], resamples: &mut [f32]) {
        let num_samples = samples.len();
        let num_resamples = resamples.len();

        let ratio = num_samples as f32 / num_resamples as f32;
        let mut t = 0.0f32;
        let mut sample_idx = 0;
        for i in resamples.iter_mut() {
            *i = match samples.get(sample_idx) {
                None => *last_sample,
                Some(sample) => {
                    *last_sample = *sample;
                    t * *sample + (1.0 - t) * *last_sample
                }
            };
            if t >= 1.0 {
                sample_idx += t as usize;
                t %= 1.0;
            }
            t += ratio;
        }
    }
}

impl AudioCallback for ApuSampler {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        let apu: &mut Apu = unsafe { self.apu.as_mut() };
        let new_time = Instant::now();
        // eprintln!("SAMPLES {} {} {:?}", out.len(), apu.samples.len(), (new_time - self.last_time));
        let samples_slice = apu.samples.as_slice();
        // eprintln!("DEBUG - SAMPLES - {} {} {:?}", samples_slice.len(), out.len(), new_time - self.last_time);
        ApuSampler::resample(&mut self.last_sample, samples_slice, out);
        apu.samples.clear();

        self.last_time = new_time;
    }
}

fn create_nes(joystick1: Box<dyn AddressSpace>, joystick2: Box<dyn AddressSpace>) -> Nes {
    //let filename = "roms/donkey_kong.nes";
    let filename = "roms/mario.nes";
    match read_ines(filename.to_string()) {
        e @ Err { .. } => panic!("Unable to load ROM {} {:?}", filename, e),
        Ok(rom) => load_ines(rom, joystick1, joystick2),
    }
}

fn present_frame(canvas: &mut Canvas<Window>, texture: &mut Texture, ppu_pixels: &[u8]) {
    texture.update(None, ppu_pixels, RENDER_WIDTH * 3).unwrap();
    canvas.clear();
    canvas.copy(&texture, None, None).unwrap();
    canvas.present();
}

fn enqueue_frame_audio(audio: &AudioQueue<f32>, samples: &mut Vec<f32>) {
    let xs = samples.as_slice();
    let bytes_per_sample: u32 = 8;
    if audio.size() as usize <= 2 * (bytes_per_sample as usize) * SAMPLES_PER_FRAME {
        audio.queue(&xs);
    } else {
        eprintln!(
            "DEBUG - SAMPLE OVERFLOW - {}",
            audio.size() / bytes_per_sample
        );
    }
    samples.clear();
}

struct SquareWave {
    phase_inc: f32,
    phase: f32,
    volume: f32,
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        // Generate a square wave
        for x in out.iter_mut() {
            *x = if self.phase <= 0.5 {
                self.volume
            } else {
                -self.volume
            };
            self.phase = (self.phase + self.phase_inc) % 1.0;
        }
    }
}

fn get_button_bit(controller: *mut GameController, button_id: u8) -> u8 {
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
    unsafe {
        match controller.as_ref() {
            None => {
                // eprintln!("DEBUG - ZERO");
                0
            }
            Some(controller) => {
                //eprintln!("DEBUG - NOT ZERO");
                controller.button(button) as u8
            }
        }
    }
}

fn get_button_mask(controller: *mut GameController) -> u8 {
    let mut button_mask = 0;
    button_mask |= get_button_bit(controller, 0) << 0;
    button_mask |= get_button_bit(controller, 1) << 1;
    button_mask |= get_button_bit(controller, 2) << 2;
    button_mask |= get_button_bit(controller, 3) << 3;
    button_mask |= get_button_bit(controller, 4) << 4;
    button_mask |= get_button_bit(controller, 5) << 5;
    button_mask |= get_button_bit(controller, 6) << 6;
    button_mask |= get_button_bit(controller, 7) << 7;
    return button_mask;
}
