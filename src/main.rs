use byte_slurper::*;
use std::default::Default;
use std::net::UdpSocket;
use std::time::Instant;
use std::{io, process};

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("192.168.5.1:60000")?;
    let mut buf = [0u8; PAYLOAD_SIZE];
    let mut cnt = 0usize;

    let mut last_reported = Instant::now();
    let program_start = Instant::now();

    let mut pol_x = [ComplexByte::default(); CHANNELS];
    let mut pol_y = [ComplexByte::default(); CHANNELS];

    let mut stokes = [0f32; CHANNELS];
    let mut stokes_accum = [0f32; CHANNELS];

    loop {
        // Grab incoming data
        let n = socket.recv(&mut buf)?;
        if n != PAYLOAD_SIZE {
            continue;
        }
        payload_to_spectra(&buf, &mut pol_x, &mut pol_y);
        stokes_i(&pol_x, &pol_y, &mut stokes);
        // Sum stokes
        vsum_mut(&stokes, &mut stokes_accum, 122070); // Packets per s

        // Metrics
        cnt += PAYLOAD_SIZE;
        if last_reported.elapsed().as_secs_f32() >= 1.0 {
            // Print perf
            last_reported = Instant::now();
            println!(
                "Rate - {} Gb/s\t",
                (cnt as f64) / program_start.elapsed().as_secs_f64() / 1.25e8,
            );
            let mean = stokes_accum.iter().sum::<f32>() / CHANNELS as f32;
            println!("Mean - {}", mean);
            let mut wtr = csv::Writer::from_writer(io::stdout());
            wtr.write_record(stokes_accum.map(|e| format!("{:.2}", e)))?;
            wtr.flush()?;
            process::exit(0);
            stokes_accum = [0f32; CHANNELS];
        }
    }
}
