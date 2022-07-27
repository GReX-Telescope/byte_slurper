use byte_slurper::*;
use chrono::{Datelike, Timelike, Utc};
use crossbeam_channel::{bounded, Receiver};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::Packet;
use pnet::transport::{transport_channel, udp_packet_iter};
use pnet::transport::{TransportChannelType::Layer4, TransportProtocol::Ipv4};
use psrdada::DadaDBBuilder;
use std::thread;
use std::{collections::HashMap, default::Default};

const AVG_SIZE: usize = 1024;

fn gen_header(
    nchan: u32,
    bw: f32,
    freq: f32,
    npol: u32,
    nbit: u32,
    tsamp: f32,
    utc_start: &str,
) -> HashMap<String, String> {
    HashMap::from([
        ("NCHAN".to_owned(), nchan.to_string()),
        ("BW".to_owned(), bw.to_string()),
        ("FREQ".to_owned(), freq.to_string()),
        ("NPOL".to_owned(), npol.to_string()),
        ("NBIT".to_owned(), nbit.to_string()),
        ("TSAMP".to_owned(), tsamp.to_string()),
        ("UTC_START".to_owned(), utc_start.to_owned()),
    ])
}

fn stokes_to_dada(
    reciever: Receiver<([ComplexByte; 2048], [ComplexByte; 2048])>,
    writer: psrdada::WriteHalf,
) {
    let mut stokes = [0f32; CHANNELS];
    let mut stokes_accum = [0f32; CHANNELS];

    let mut sums = 0usize;
    let mut cnt = 0usize;

    for (pol_x, pol_y) in reciever {
        // Grab from channel
        stokes_i(&pol_x, &pol_y, &mut stokes);
        // Sum stokes
        vsum_mut(&stokes, &mut stokes_accum, AVG_SIZE as u32);

        // Metrics
        sums += 1;
        cnt += PAYLOAD_SIZE;

        if sums == AVG_SIZE {
            println!("Avg complete");
            // Generate the header
            let now = Utc::now();
            let timestamp = format!(
                "{}-{:02}-{:02}-{:02}:{:02}:{:02}",
                now.year(),
                now.month(),
                now.day(),
                now.hour(),
                now.minute(),
                now.second()
            );
            let header = gen_header(CHANNELS as u32, 250f32, 1405f32, 1, 16, 0.001, &timestamp);
            // Resets
            stokes_accum = [0f32; CHANNELS];
            sums = 0;
            cnt = 0;
        }
    }
}

fn main() -> std::io::Result<()> {
    // Get these from args
    let device_name = "enp129s0f0";
    let port = 60000u16;
    let dada_key = 0xbeef;

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
        .buf_size(CHANNELS as u64 * 2) // We're going to send u16
        .build(true) // Memlock
        .unwrap();

    let (_, mut writer) = hdu.split();

    // Start consumer
    stokes_to_dada(receiver, writer);
    Ok(())
}
