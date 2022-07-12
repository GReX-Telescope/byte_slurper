use std::net::UdpSocket;
use std::time::Instant;

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("192.168.5.1:60000")?;
    let mut buf = [0u8; 8192];
    let mut work_time = 0f64;
    let mut idle_time = 0f64;

    let mut last_reported = Instant::now();
    let mut b = Instant::now();
    loop {
        let a = Instant::now();
        work_time += a.duration_since(b).as_secs_f64();
        socket.recv(&mut buf)?;
        b = Instant::now();
        idle_time += b.duration_since(a).as_secs_f64();
        if last_reported.elapsed().as_secs_f32() >= 1.0 {
            last_reported = Instant::now();
            println!("Work Ratio - {}", work_time / (work_time + idle_time));
        }
    }
}
