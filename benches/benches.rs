use byte_slurper::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use crossbeam_channel::{unbounded, Receiver, Sender};
use rand::prelude::*;

fn clone_from_boxed(to: &mut Box<[i16]>, from: &[i16]) {
    to[0..2048].clone_from_slice(from);
}

fn rx_tx_chan(tx: &Sender<Signal>, rx: &Receiver<Signal>) {
    tx.send(Signal::NewAvg).unwrap();
    rx.recv().unwrap();
}

fn benchmark(c: &mut Criterion) {
    let mut rng = rand::thread_rng();

    let mut dummy_payload = [0u8; PAYLOAD_SIZE];
    rng.fill(&mut dummy_payload[..]);

    // Containers
    let mut pol_a = [ComplexByte::default(); CHANNELS];
    let mut pol_b = [ComplexByte::default(); CHANNELS];
    let mut spectra = [0i16; CHANNELS];

    let avging_window = [0i16; AVG_WINDOW_SIZE];
    let mut window = vec![0i16; WINDOW_SIZE].into_boxed_slice();

    let (sender, receiver) = unbounded();

    c.bench_function("payload_to_spectra", |b| {
        b.iter(|| {
            payload_to_spectra(
                black_box(&dummy_payload),
                black_box(&mut pol_a),
                black_box(&mut pol_b),
            )
        })
    });

    c.bench_function("avg_from_window", |b| {
        b.iter(|| {
            avg_from_window(
                black_box(&avging_window),
                black_box(&mut spectra),
                black_box(CHANNELS),
            )
        })
    });

    c.bench_function("stokes", |b| {
        b.iter(|| {
            gen_stokes_i(
                black_box(&pol_a),
                black_box(&pol_b),
                black_box(&mut spectra),
            )
        })
    });

    c.bench_function("Clone from heap slice", |b| {
        b.iter(|| clone_from_boxed(black_box(&mut window), &spectra))
    });

    c.bench_function("Channel signaling", |b| {
        b.iter(|| rx_tx_chan(black_box(&sender), black_box(&receiver)))
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);