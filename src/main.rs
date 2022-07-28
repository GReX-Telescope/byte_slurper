use byte_slice_cast::AsByteSlice;
use byte_slurper::*;
use chrono::Utc;
use crossbeam_channel::{bounded, Receiver, Sender};
use pnet::{
    packet::{ip::IpNextHeaderProtocols, Packet},
    transport::{
        transport_channel, udp_packet_iter, TransportChannelType::Layer4, TransportProtocol::Ipv4,
        TransportReceiver,
    },
};
use psrdada::DadaDBBuilder;
use std::{default::Default, thread, time::Instant};

fn stokes_to_dada(receiver: Receiver<[i16; CHANNELS]>, mut writer: psrdada::WriteHalf) {
    // Allocate window on the heap to avoid a stack overflow
    let mut window = vec![0i16; WINDOW_SIZE].into_boxed_slice();
    let mut stokes_cnt = 0usize;
    let mut first_sample_time = Utc::now();

    let mut last_avg = Instant::now();
    for stokes in receiver {
        println!("Avg time - {}", last_avg.elapsed().as_secs_f32());
        last_avg = Instant::now();
        // Push the incoming average to the right place in the output
        window[(stokes_cnt * CHANNELS)..((stokes_cnt + 1) * CHANNELS)].clone_from_slice(&stokes);
        // If this was the first one, update the start time
        if stokes_cnt == 0 {
            first_sample_time = Utc::now();
        }
        // Increment the stokes counter
        stokes_cnt += 1;
        // If we've filled the window, generate the header and send the whole thing
        if stokes_cnt == NSAMP {
            println!("New window");
            // Reset the stokes counter
            stokes_cnt = 0;
            // Most of these should be constants or set by args
            let header = gen_header(
                CHANNELS as u32,
                250f32,
                1405f32,
                1,
                16,
                TSAMP * 1e6,
                &heimdall_timestamp(first_sample_time),
            );
            writer.push_header(&header).unwrap();
            writer.push(window.as_byte_slice()).unwrap();
        }
    }
}

fn udp_to_avg(mut udp_rx: TransportReceiver, port: u16, sender: Sender<[i16; CHANNELS]>) {
    // Locals
    let mut pol_x = [ComplexByte::default(); CHANNELS];
    let mut pol_y = [ComplexByte::default(); CHANNELS];
    // State to hold the averaging window
    let mut avg_window = [0i16; AVG_WINDOW_SIZE];
    let mut avg = [0i16; CHANNELS];
    let mut avg_cnt = 0usize;
    // Capture packets
    let mut iter = udp_packet_iter(&mut udp_rx);

    let mut last_avg = Instant::now();
    loop {
        match iter.next() {
            Ok((packet, _)) => {
                println!("Udp time - {}", last_avg.elapsed().as_secs_f32());
                last_avg = Instant::now();

                // Skip invalid packets
                if packet.get_destination() != port {
                    continue;
                }
                if packet.get_length() as usize != PAYLOAD_SIZE {
                    continue;
                }
                // Unpack
                payload_to_spectra(packet.packet(), &mut pol_x, &mut pol_y);
                // --- Average
                // Generate stokes and push to averaging window
                let avg_slice = &mut avg_window[(avg_cnt * CHANNELS)..((avg_cnt + 1) * CHANNELS)];
                gen_stokes_i(&pol_x, &pol_y, avg_slice);
                avg_cnt += 1;
                if avg_cnt == AVG_SIZE {
                    // Reset the counter
                    avg_cnt = 0;
                    // Generate average
                    avg_from_window(&avg_window, &mut avg, CHANNELS);
                    // Send to channel
                    //sender.send(avg).unwrap();
                }
            }
            Err(e) => {
                eprintln!("Packet next error - {}", e);
            }
        }
    }
}

fn main() -> std::io::Result<()> {
    // Get these from args
    let port = 60000u16;
    let dada_key = 0xbeef;

    // We're going to be looking at Layer 4, IpV4, UDP data
    let (_, udp_rx) = transport_channel(PAYLOAD_SIZE * 2, Layer4(Ipv4(IpNextHeaderProtocols::Udp)))
        .expect("Error creating transport channel");

    // Setup multithreading
    let (stokes_sender, stokes_receiver) = bounded(1000000);

    // Start producing polarizations on a thread
    thread::spawn(move || udp_to_avg(udp_rx, port, stokes_sender));

    // Setup PSRDADA
    let hdu = DadaDBBuilder::new(dada_key, "byte_slurper")
        .buf_size(WINDOW_SIZE as u64 * 2) // We're going to send u16
        .build(true) // Memlock
        .unwrap();

    let (_, writer) = hdu.split();

    // Start consumer
    stokes_to_dada(stokes_receiver, writer);
    Ok(())
}
