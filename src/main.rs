use byte_slice_cast::AsByteSlice;
use byte_slurper::*;
use chrono::Utc;
use crossbeam_channel::{bounded, Receiver};
use pnet::{
    packet::{ip::IpNextHeaderProtocols, Packet},
    transport::{
        transport_channel, udp_packet_iter, TransportChannelType::Layer4, TransportProtocol::Ipv4,
    },
};
use psrdada::DadaDBBuilder;
use std::{default::Default, thread};

fn stokes_to_dada(
    reciever: Receiver<([ComplexByte; CHANNELS], [ComplexByte; CHANNELS])>,
    mut writer: psrdada::WriteHalf,
) {
    let mut avg_window = [0i16; AVG_WINDOW_SIZE];
    let mut window = [0i16; WINDOW_SIZE];

    let mut avg_cnt = 0usize;
    let mut stokes_cnt = 0usize;

    let mut first_sample_time = Utc::now();

    for (pol_x, pol_y) in reciever {
        let avg_slice = &mut avg_window[(avg_cnt * CHANNELS)..((avg_cnt + 1) * CHANNELS)];
        // Grab from channel and push stokes to average
        gen_stokes_i(&pol_x, &pol_y, avg_slice);
        avg_cnt += 1;

        if avg_cnt == (AVG_SIZE - 1) {
            // Average the averaging window, push to output window
            let stokes_slice = &mut window[(stokes_cnt * CHANNELS)..((stokes_cnt + 1) * CHANNELS)];
            avg_from_window(&avg_window, stokes_slice, CHANNELS);
            // Reset the counter
            avg_cnt = 0;

            // If this is the first sample in the output window, mark the time
            if stokes_cnt == 0 {
                first_sample_time = Utc::now();
            }

            // If we've filled the window
            // generate the header and send the whole thing
            if stokes_cnt == (WINDOW_SIZE - 1) {
                // Reset the counter
                stokes_cnt = 0;
                // Most of these should be constants or set by args
                let header = gen_header(
                    CHANNELS as u32,
                    250f32,
                    1405f32,
                    1,
                    16,
                    TSAMP,
                    &heimdall_timestamp(first_sample_time),
                );
                println!("Sending data to heimdall via PSRDADA");
                writer.push_header(&header).unwrap();
                writer.push(window.as_byte_slice()).unwrap();
            }
        }
    }
}

fn main() -> std::io::Result<()> {
    // Get these from args
    let port = 60000u16;
    let dada_key = 0xbeef;

    // Deal with shutdowns properly
    ctrlc::set_handler(move || {
        println!("Bringing down!");
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    // We're going to be looking at Layer 4, IpV4, UDP data
    let (_, mut udp_rx) =
        transport_channel(PAYLOAD_SIZE * 2, Layer4(Ipv4(IpNextHeaderProtocols::Udp)))
            .expect("Error creating transport channel");

    // Setup multithreading
    let (sender, receiver) = bounded(1000);

    // Start producing polarizations on a thread
    thread::spawn(move || {
        let mut pol_x = [ComplexByte::default(); CHANNELS];
        let mut pol_y = [ComplexByte::default(); CHANNELS];
        let mut iter = udp_packet_iter(&mut udp_rx);
        loop {
            match iter.next() {
                Ok((packet, _)) => {
                    // Skip invalid packets
                    if packet.get_destination() != port {
                        continue;
                    }
                    if packet.get_length() as usize != PAYLOAD_SIZE {
                        continue;
                    }
                    // Unpack
                    payload_to_spectra(packet.packet(), &mut pol_x, &mut pol_y);
                    // Send to PSRDADA
                    sender.send((pol_x, pol_y)).unwrap();
                }
                Err(e) => {
                    eprintln!("Packet next error - {}", e);
                }
            }
        }
    });

    // Setup PSRDADA
    let hdu = DadaDBBuilder::new(dada_key, "byte_slurper")
        .buf_size(WINDOW_SIZE as u64 * 2) // We're going to send u16
        .build(true) // Memlock
        .unwrap();

    let (_, writer) = hdu.split();

    // Start consumer
    stokes_to_dada(receiver, writer);
    Ok(())
}
