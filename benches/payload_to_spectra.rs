use byte_slurper::payload_to_spectra;
use byte_slurper::ComplexByte;
use byte_slurper::CHANNELS;
use byte_slurper::PAYLOAD_SIZE;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::prelude::*;

fn criterion_benchmark(c: &mut Criterion) {
    let mut rng = rand::thread_rng();
    let mut dummy_payload = [0u8; PAYLOAD_SIZE];
    let mut pol_a = [ComplexByte::default(); CHANNELS];
    let mut pol_b = [ComplexByte::default(); CHANNELS];
    rng.fill(&mut dummy_payload[..]);

    c.bench_function("payload_to_spectra", |b| {
        b.iter(|| {
            payload_to_spectra(
                black_box(&dummy_payload),
                black_box(&mut pol_a),
                black_box(&mut pol_b),
            )
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
