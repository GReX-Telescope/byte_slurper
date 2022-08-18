//! In this module, we implement all the monitoring logic for the packet capture.
//! This includes getting drop data from libpcap as well as various runtime stats.
//! Additionally, we'll hold on to a chunk of average spectra so it can be queried
//! from some TCP listener.

use std::{io::Write, net::TcpListener};

use byte_slice_cast::AsByteSlice;
use crossbeam_channel::Receiver;
use tracing::info;

use crate::exfil::CHANNELS;

// At incoming samples at 8us, if we're averaging over there by 4, this is about 62.5ms
const TCP_CLIENT_AVG: usize = 2048;

pub fn listen_consumer(rx: Receiver<[u16; CHANNELS]>, port: u16) {
    let mut avg = [0f32; CHANNELS];
    // Setup listeners
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
    let mut avg_cnt = 0usize;
    loop {
        info!("Listen thread waiting for new client");
        // Wait for new connections
        let mut socket = if let Ok((sock, _)) = listener.accept() {
            sock
        } else {
            continue;
        };
        info!("New listen client - starting monitoring");
        loop {
            // Grab next stokes sample and add to avg
            rx.recv()
                .unwrap()
                .into_iter()
                .enumerate()
                .for_each(|(i, v)| avg[i] += (v as f32 / u16::MAX as f32) / TCP_CLIENT_AVG as f32);
            avg_cnt += 1;
            if avg_cnt == TCP_CLIENT_AVG {
                avg_cnt = 0;
                match socket.write_all(avg.as_byte_slice()) {
                    Ok(_) => (),
                    Err(_) => break,
                };
                avg = [0f32; CHANNELS];
            }
        }
    }
}
