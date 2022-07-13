use byte_slurper::*;
use std::net::UdpSocket;
use std::time::Instant;

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("192.168.5.1:60000")?;
    let mut buf = [0u8; PAYLOAD_SIZE];
    let mut cnt = 0usize;

    let mut last_reported = Instant::now();
    let program_start = Instant::now();

    let mut pol_a = [ComplexByte::default(); CHANNELS];
    let mut pol_b = [ComplexByte::default(); CHANNELS];

    loop {
        // Grab incoming data
        socket.recv(&mut buf)?;
        payload_to_spectra(&buf, &mut pol_a, &mut pol_b);
        // let _spectra = total_power_spectra(&pol_a, &pol_b);
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
