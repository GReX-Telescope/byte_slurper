use af_packet::rx::Ring;
use byte_slice_cast::*;
use byte_slurper::*;
use chrono::{Datelike, Timelike, Utc};
use crossbeam_channel::bounded;
use crossbeam_channel::Receiver;
use etherparse::SlicedPacket;
use etherparse::TransportSlice;
use psrdada::DadaDBBuilder;
use std::collections::HashMap;
use std::default::Default;
use std::thread;
use std::time::Instant;

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

// Every DOWNSAMPLE_FACTOR, send data to psrdada
// Every

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
    let port = 6000u16;
    let dada_key = 0xdead;

    // Open the memory-mapped device
    let mut ring = Ring::from_if_name(device_name).unwrap();
    // Setup multithreading
    let (sender, receiver) = bounded(1000);

    // Start producing polarizations on a thread
    thread::spawn(move || {
        let mut buf = [0u8; PAYLOAD_SIZE];
        let mut pol_x = [ComplexByte::default(); CHANNELS];
        let mut pol_y = [ComplexByte::default(); CHANNELS];
        loop {
            // Grab incoming data
            let mut block = ring.get_block();
            for framed_packet in block.get_raw_packets() {
                match SlicedPacket::from_ip(framed_packet.data) {
                    Ok(v) => {
                        if let Some(TransportSlice::Udp(udp_header)) = v.transport {
                            let n = udp_header.length();
                            let dest_port = udp_header.destination_port();
                            if n as usize != PAYLOAD_SIZE || dest_port != port {
                                continue;
                            }
                            // Build spectra from payload
                            payload_to_spectra(v.payload, &mut pol_x, &mut pol_y);
                            // Send to channel
                            sender.send((pol_x, pol_y)).unwrap();
                        } else {
                            continue;
                        }
                    }
                    Err(e) => {
                        eprintln!("Malformed ethernet packet - {}", e);
                        continue;
                    }
                }
            }
            block.mark_as_consumed();
        }
    });

    // Setup PSRDADA
    let hdu = DadaDBBuilder::new(dada_key, "byte_slurper")
        .buf_size(CHANNELS as u64 * 2) // We're going to send u16
        .num_bufs(1024)
        .num_headers(1024)
        .build(true) // Memlock
        .unwrap();

    let (_, mut writer) = hdu.split();

    // Start consumer
    stokes_to_dada(receiver, writer);
    Ok(())
}
