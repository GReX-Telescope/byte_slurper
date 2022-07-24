use byte_slice_cast::*;
use byte_slurper::*;
use std::default::Default;
use std::io::Write;
use std::net::TcpListener;
use std::net::UdpSocket;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Instant;

const AVG_SIZE: usize = 10000;

fn stokes_consumer(reciever: Receiver<([ComplexByte; 2048], [ComplexByte; 2048])>) {
    let stokes_stream = TcpListener::bind("0.0.0.0:4242").unwrap();
    let (mut stokes_socket, _) = stokes_stream.accept().unwrap();

    let mut stokes = [0f32; CHANNELS];
    let mut stokes_accum = [0f32; CHANNELS];

    let mut sums = 0usize;
    let mut cnt = 0usize;

    let mut last_reported = Instant::now();

    for (pol_x, pol_y) in reciever {
        // Grab from channel
        stokes_i(&pol_x, &pol_y, &mut stokes);
        // Sum stokes
        vsum_mut(&stokes, &mut stokes_accum, AVG_SIZE as u32);

        // Metrics
        sums += 1;
        cnt += PAYLOAD_SIZE;

        if sums == AVG_SIZE {
            let rate = (cnt as f32) / last_reported.elapsed().as_secs_f32() / 1.25e8;
            println!("TX Cycle! Rate - {} Gb/s", rate);
            stokes_socket
                .write_all(stokes_accum.as_byte_slice())
                .unwrap();
            // Resets
            stokes_accum = [0f32; CHANNELS];
            sums = 0;
            cnt = 0;
            last_reported = Instant::now();
        }
    }
}

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("192.168.5.1:60000")?;

    // Setup multithreading
    let (sender, receiver) = mpsc::channel();

    // Start producing polarizations on a thread
    thread::spawn(move || {
        let mut buf = [0u8; PAYLOAD_SIZE];
        let mut pol_x = [ComplexByte::default(); CHANNELS];
        let mut pol_y = [ComplexByte::default(); CHANNELS];
        loop {
            // Grab incoming data
            let n = socket.recv(&mut buf).unwrap();
            // Skip bad packets
            if n != PAYLOAD_SIZE {
                continue;
            }
            // Unpack
            payload_to_spectra(&buf, &mut pol_x, &mut pol_y);
            // Send to channel
            sender.send((pol_x, pol_y)).unwrap();
        }
    });

    // Start consumer
    stokes_consumer(receiver);
    Ok(())
}
