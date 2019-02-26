mod common;
mod c6502;
mod ppu;
mod apu;
mod mapper;
mod nes;
mod joystick;

extern crate sdl2;

use sdl2::audio::{AudioCallback,AudioSpecDesired,AudioQueue};
use sdl2::pixels::PixelFormatEnum;
use sdl2::pixels::Color;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::render::Canvas;
use sdl2::render::Texture;
use sdl2::render::TextureAccess;
use sdl2::video::Window;
use std::ptr::NonNull;
use std::time::{Duration,Instant};

use crate::joystick::Joystick;
use crate::mapper::AddressSpace;
use crate::nes::Nes;
use crate::nes::{load_ines, read_ines};
use crate::ppu::*;
use crate::apu::Apu;

// https://wiki.nesdev.com/w/index.php/Cycle_reference_chart
const CLOCKS_PER_FRAME:u32 = 29780;
const APU_FREQUENCY:i32 = 240;
const AUDIO_FREQUENCY:usize = 44100;
const SAMPLES_PER_FRAME:usize = 2032;
const SCALE:usize = 4;

fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let controller_subsystem = sdl_context.game_controller().unwrap();
    let audio_subsystem = sdl_context.audio().unwrap();

    let joystick1 = Joystick::new(&controller_subsystem, 0);
    let joystick2 = Joystick::new(&controller_subsystem, 1);
    let window = video_subsystem.window("NES emulator", (RENDER_WIDTH*SCALE) as u32, (RENDER_HEIGHT*SCALE) as u32)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator.create_texture(
        PixelFormatEnum::RGB24,
        TextureAccess::Streaming,
        RENDER_WIDTH as u32,
        RENDER_HEIGHT as u32
    ).unwrap();
    let mut nes = create_nes(Box::new(joystick1), Box::new(joystick2));

    let desired_spec = AudioSpecDesired {
        freq: Some(AUDIO_FREQUENCY as i32),
        channels: Some(1),
        //samples: Some(8820),
        samples: Some(SAMPLES_PER_FRAME as u16),
    };

    let audio_device = audio_subsystem.open_queue(None, &desired_spec).unwrap();
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
    audio_device.resume();
    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut i = 0;
    'running: loop {
        i = (i + 1) % 255;
        canvas.set_draw_color(Color::RGB(i, 64, 255 - i));
        canvas.clear();
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running
                },
                Event::KeyDown { keycode: Some(Keycode::Pause), .. } => {
                    nes.break_debugger();
                },
                _ => {}
            }
        }
        // The rest of the game loop goes here...
        //audio_device.pause();
        nes.run_frame();
        // eprintln!("DEBUG - NUM SAMPLES {} {}", nes.apu.samples.len(), audio_device.size());
        present_frame(&mut canvas, &mut texture, &nes.ppu.display);
        //audio_device.resume();
        //eprintln!("DEBUG - SAMPLE SIZE - {}", nes.apu.samples.len());
        enqueue_frame_audio(&audio_device, &mut nes.apu.samples);

        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

struct ApuSampler {
    apu:NonNull<Box<Apu>>,
    volume:f32,
    resample_step:u32,
    sample:f32,
    last_sample:f32,
    last_time:Instant,
}

unsafe impl std::marker::Send for ApuSampler { }

const SAMPLES_PER_SECOND:u32 = 1789920;
const CLOCK_FREQUENCY:u32 = 30000;
const SAMPLES_PER_CLOCK:u32 = SAMPLES_PER_SECOND / CLOCK_FREQUENCY;

impl ApuSampler {
    fn resample(last_sample:&mut f32, samples: &[f32], resamples:&mut [f32]) {
        let num_samples = samples.len();
        let num_resamples = resamples.len();
        let new_time = Instant::now();

        let ratio = num_samples as f32 / num_resamples as f32;
        let mut t = 0.0f32;
        let mut sample_idx = 0;
        let mut num_skip = 0;
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
    // fn resample(last_sample:&mut f32, samples: &[f32], resamples:&mut [f32]) {
    //     let num_samples = samples.len();
    //     let num_resamples = resamples.len();
    //     eprintln!("DEBUG - SAMPLES - {} {}", num_samples, num_resamples);
    //     let ratio = num_samples as f32 / num_resamples as f32;
    //     let mut count = 0.0f32;
    //     let mut sample_idx = 0;
    //     for i in resamples.iter_mut() {
    //         if num_samples == 0 {
    //             *i = *last_sample;
    //         } else {
    //             let sample = samples[sample_idx];
    //             *last_sample = sample;
    //             *i = sample;
    //             if count >= 1.0 {
    //                 sample_idx += count as usize;
    //                 count %= 1.0;
    //             }
    //             count += ratio;
    //         }
    //     }
    // }
}

impl AudioCallback for ApuSampler {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        let apu:&mut Apu = unsafe { self.apu.as_mut() };
        let new_time = Instant::now();
        // eprintln!("SAMPLES {} {} {:?}", out.len(), apu.samples.len(), (new_time - self.last_time));
        let samples_slice = apu.samples.as_slice();
        // eprintln!("DEBUG - SAMPLES - {} {} {:?}", samples_slice.len(), out.len(), new_time - self.last_time);
        ApuSampler::resample(&mut self.last_sample, samples_slice, out);
        apu.samples.clear();
        // if apu.samples.len() > 1000000 {
        //     eprintln!("TOO MANY SAMPLES {}", apu.samples.len());
        // }
        // for x in out.iter_mut() {
        //     if self.resample_step == 0 {
        //         self.sample = (apu.samples.pop_front().unwrap_or(0.0))*self.volume;
        //         if self.sample != self.last_sample {
        //             eprintln!("SAMPLE {} {}", self.last_sample, self.sample);
        //             self.last_sample = self.sample;
        //         }
        //         self.resample_step = SAMPLES_PER_CLOCK;
        //     } else {
        //         self.resample_step -= 1;
        //     }
        //     *x = self.sample;
        // }
        self.last_time = new_time;
    }
}

fn create_nes(joystick1:Box<AddressSpace>, joystick2:Box<AddressSpace>) -> Nes {
    //let filename = "roms/donkey_kong.nes";
    let filename = "roms/mario.nes";
    let rom = read_ines(filename.to_string()).unwrap();
    return load_ines(rom, joystick1, joystick2);
}

fn present_frame(canvas: &mut Canvas<Window>, texture: &mut Texture, ppu_pixels: &[u8]) {
    texture.update(None, ppu_pixels, RENDER_WIDTH*3).unwrap();
    canvas.clear();
    canvas.copy(&texture, None, None).unwrap();
}

fn enqueue_frame_audio(audio:&AudioQueue<f32>, samples:&mut Vec<f32>) {
    let mut xs = samples.as_slice();
    if audio.size() as usize <= 4*SAMPLES_PER_FRAME {
        audio.queue(&xs);
    }
    samples.clear();
}

struct SquareWave {
    phase_inc: f32,
    phase: f32,
    volume: f32
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
