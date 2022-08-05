use byte_slurper::{
    capture::unpack,
    complex::ComplexByte,
    exfil::{
        avg_from_window, push_to_avg_window, stokes_i, AVG_SIZE_POW, AVG_WINDOW_SIZE, CHANNELS,
    },
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::prelude::*;

fn benchmark(c: &mut Criterion) {
    let mut rng = rand::thread_rng();

    let mut dummy_payload = [0u8; 8192];
    rng.fill(&mut dummy_payload[..]);

    // Containers
    let mut pol_a = [ComplexByte::default(); CHANNELS];
    let mut pol_b = [ComplexByte::default(); CHANNELS];
    let mut window = [0u16; AVG_WINDOW_SIZE];
    let mut avg = [0u16; CHANNELS];

    c.bench_function("payload unpacking", |b| {
        b.iter(|| {
            unpack(
                black_box(&dummy_payload),
                black_box(&mut pol_a),
                black_box(&mut pol_b),
            )
        })
    });

    c.bench_function("stokes", |b| {
        b.iter(|| stokes_i(black_box(pol_a[0]), black_box(pol_b[0])))
    });

    c.bench_function("fill avg window", |b| {
        b.iter(|| {
            push_to_avg_window(
                black_box(&mut window),
                black_box(&pol_a),
                black_box(&pol_b),
                black_box(1),
            )
        })
    });

    c.bench_function("avg from window", |b| {
        b.iter(|| {
            avg_from_window(
                black_box(&window),
                black_box(AVG_SIZE_POW),
                black_box(&mut avg),
            )
        })
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
