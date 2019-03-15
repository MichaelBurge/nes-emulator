#[macro_use]
extern crate criterion;
extern crate nes_emulator;

use nes_emulator::nes::read_ines;
use nes_emulator::nes::load_ines;
use nes_emulator::joystick::Joystick;

use criterion::Criterion;
use std::time::Duration;

fn criterion_benchmark(c: &mut Criterion) {
    let joystick1 = Box::new(Joystick::new());
    let joystick2 = Box::new(Joystick::new());
    let ines = read_ines("roms/mario.nes".to_string()).unwrap();
    let mut nes = load_ines(ines, joystick1, joystick2);
    c.bench_function(
        "1 frame",
        move |b|
        b.iter(|| {
            for _ in 0..1 {
                nes.run_frame();
            }
        }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
