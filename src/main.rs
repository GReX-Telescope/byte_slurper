use std::net::UdpSocket;
use std::time::Instant;

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("192.168.5.1:60000")?;
    let mut buf = [0u8; 8192];
    let mut cnt = 0u64;

    let mut last_reported = Instant::now();
    let program_start = Instant::now();
    loop {
        socket.recv(&mut buf)?;
        cnt += 8192;
        if last_reported.elapsed().as_secs_f32() >= 1.0 {
            last_reported = Instant::now();
            println!(
                "Rate - {}",
                (cnt as f64) / program_start.elapsed().as_secs_f64()
            );
        }
    }
}
