use byte_slurper::{
    capture::unpack,
    complex::ComplexByte,
    exfil::{add_stokes_avg, stokes_i},
    CaptureConfig,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::prelude::*;

fn benchmark(c: &mut Criterion) {
    let mut rng = rand::thread_rng();

    let mut dummy_payload = [0u8; 8192];
    rng.fill(&mut dummy_payload[..]);

    let cc = CaptureConfig {
        channels: 2048,
        samples: 65536,
        avgs: 4,
        cadence: 8.192e-6,
    };

    // Containers
    let mut pol_a = vec![ComplexByte::default(); cc.channels];
    let mut pol_b = vec![ComplexByte::default(); cc.channels];
    let mut avg = vec![0f32; cc.channels];

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

    c.bench_function("average", |b| {
        b.iter(|| {
            add_stokes_avg(
                black_box(&mut avg),
                black_box(&pol_a),
                black_box(&pol_b),
                black_box(&cc),
            )
        })
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
