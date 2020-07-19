#[macro_use]
extern crate criterion;
extern crate nes_emulator;

use nes_emulator::joystick::Joystick;
use nes_emulator::nes::load_ines;
use nes_emulator::nes::read_ines;

use criterion::{criterion_main, Criterion, Throughput};
use std::time::Duration;

fn criterion_benchmark(c: &mut Criterion) {
    let joystick1 = Box::new(Joystick::new());
    let joystick2 = Box::new(Joystick::new());
    let ines = read_ines("roms/mario.nes".to_string()).unwrap();
    let mut nes = load_ines(ines, joystick1, joystick2);
    let mut group = c.benchmark_group("Mario");
    for &size in &[100, 1_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.measurement_time(Duration::from_secs(30));
        group.sample_size(10);
        group.bench_function(format!("frames {}", size), |b| {
            b.iter(|| {
                for _ in 0..size {
                    nes.run_frame()
                }
            })
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
