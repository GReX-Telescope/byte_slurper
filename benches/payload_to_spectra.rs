use byte_slurper::{payload_to_spectra, stokes_i, ComplexByte, CHANNELS, PAYLOAD_SIZE};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::prelude::*;

fn criterion_benchmark(c: &mut Criterion) {
    let mut rng = rand::thread_rng();

    let mut dummy_payload = [0u8; PAYLOAD_SIZE];
    rng.fill(&mut dummy_payload[..]);

    // Containers
    let mut pol_a = [ComplexByte::default(); CHANNELS];
    let mut pol_b = [ComplexByte::default(); CHANNELS];
    let mut spectra = [0f32; CHANNELS];

    c.bench_function("payload_to_spectra", |b| {
        b.iter(|| {
            payload_to_spectra(
                black_box(&dummy_payload),
                black_box(&mut pol_a),
                black_box(&mut pol_b),
            )
        })
    });

    c.bench_function("stokes", |b| {
        b.iter(|| {
            stokes_i(
                black_box(&pol_a),
                black_box(&pol_b),
                black_box(&mut spectra),
            )
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
