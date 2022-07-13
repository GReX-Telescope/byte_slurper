use std::net::UdpSocket;
use std::time::Instant;

use num_complex::Complex;

const PAYLOAD_SIZE: usize = 8192;
const WORD_SIZE: usize = 8;
const CHANNELS: usize = 2048;

type ComplexByte = Complex<u8>;

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("192.168.5.1:60000")?;
    let mut buf = [0u8; PAYLOAD_SIZE];
    let mut cnt = 0usize;

    let mut last_reported = Instant::now();
    let program_start = Instant::now();

    let mut pol_a_spectra = [ComplexByte::default(); CHANNELS];
    let mut pol_b_spectra = [ComplexByte::default(); CHANNELS];

    let mut total_power_spectra = [0f32; CHANNELS];

    loop {
        // Grab incoming data
        socket.recv(&mut buf)?;

        // Extract time series
        for (i, word) in buf.chunks_exact(WORD_SIZE).enumerate() {
            // Each word contains two frequencies for each polarization
            // [A1 B1 A2 B2]
            // Where each channel is [Re Im]
            let a1 = ComplexByte {
                re: word[7],
                im: word[6],
            };
            let a2 = ComplexByte {
                re: word[5],
                im: word[4],
            };
            let b1 = ComplexByte {
                re: word[3],
                im: word[2],
            };
            let b2 = ComplexByte {
                re: word[1],
                im: word[0],
            };
            // Update spectra
            pol_a_spectra[2 * i] = a1;
            pol_a_spectra[2 * i + 1] = a2;
            pol_b_spectra[2 * i] = b1;
            pol_b_spectra[2 * i + 1] = b2;
        }

        // Help out the compiler
        assert!(pol_a_spectra.len() == CHANNELS);
        assert!(pol_b_spectra.len() == CHANNELS);
        assert!(total_power_spectra.len() == CHANNELS);
        // Convert to power and add to spectra
        for i in 0..CHANNELS {
            let pol_a_float = Complex::new(
                pol_a_spectra[i].re as f32 / 255_f32,
                pol_a_spectra[i].im as f32 / 255_f32,
            );
            let pol_b_float = Complex::new(
                pol_b_spectra[i].re as f32 / 255_f32,
                pol_b_spectra[i].im as f32 / 255_f32,
            );
            total_power_spectra[i] = pol_a_float.norm() + pol_b_float.norm();
        }

        // Metrics
        cnt += PAYLOAD_SIZE;
        if last_reported.elapsed().as_secs_f32() >= 1.0 {
            // Print perf
            last_reported = Instant::now();
            println!(
                "Rate - {} Gb/s",
                (cnt as f64) / program_start.elapsed().as_secs_f64() / 1.25e8
            );
            // let mut wtr = csv::Writer::from_writer(io::stdout());
            // wtr.write_record(spectra.map(|e| e.to_string()))?;
            // wtr.flush()?;
            // Bail
            //process::exit(0);
        }
    }
}
