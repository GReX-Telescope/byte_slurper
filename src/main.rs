use byte_slice_cast::*;
use byte_slurper::*;
use std::default::Default;
use std::io::Write;
use std::net::TcpListener;
use std::net::UdpSocket;

const AVG_SIZE: usize = 5000;

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("192.168.5.1:60000")?;
    let stokes_stream = TcpListener::bind("0.0.0.0:4242")?;
    let (mut stokes_socket, _) = stokes_stream.accept()?;
    let mut buf = [0u8; PAYLOAD_SIZE];

    let mut pol_x = [ComplexByte::default(); CHANNELS];
    let mut pol_y = [ComplexByte::default(); CHANNELS];

    let mut stokes = [0f32; CHANNELS];
    let mut stokes_accum = [0f32; CHANNELS];

    let mut sums = 0usize;
    println!("Here we go! (Mario voice)");
    loop {
        // Grab incoming data
        let n = socket.recv(&mut buf)?;
        if n != PAYLOAD_SIZE {
            continue;
        }
        payload_to_spectra(&buf, &mut pol_x, &mut pol_y);
        stokes_i(&pol_x, &pol_y, &mut stokes);
        // Sum stokes
        vsum_mut(&stokes, &mut stokes_accum, AVG_SIZE as u32); // Packets per s

        // Metrics
        sums += 1;

        if sums == AVG_SIZE {
            println!("Sending payload");
            stokes_socket.write_all(stokes_accum.as_byte_slice())?;
            stokes_accum = [0f32; CHANNELS];
            sums = 0;
        }
    }
}
