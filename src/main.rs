use std::net::UdpSocket;
use std::time::Instant;

const PAYLOAD_SIZE: usize = 8192;
const WORD_SIZE: usize = 8;

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("192.168.5.1:60000")?;
    let mut buf = [0u8; PAYLOAD_SIZE];
    let mut cnt = 0usize;

    let mut last_reported = Instant::now();
    let program_start = Instant::now();

    let mut pol_a_time_series = [0u8; PAYLOAD_SIZE * 2];
    let mut pol_b_time_series = [0u8; PAYLOAD_SIZE * 2];

    loop {
        // Grab incoming data
        socket.recv(&mut buf)?;
        // Extract time series
        for (i, word) in buf.chunks_exact(WORD_SIZE).enumerate() {
            // Top of word is all zeros (for now)
            // Bottom of word is a(t1), a(t0), b(t1), b(t0)
            pol_b_time_series[2 * i] = word[0];
            pol_b_time_series[2 * i + 1] = word[1];
            pol_a_time_series[2 * i] = word[2];
            pol_a_time_series[2 * i + 1] = word[3];
        }

        cnt += PAYLOAD_SIZE;
        if last_reported.elapsed().as_secs_f32() >= 1.0 {
            last_reported = Instant::now();
            println!(
                "Rate - {} Gb/s",
                (cnt as f64) / program_start.elapsed().as_secs_f64() / 1.25e8
            );
        }
    }
}
