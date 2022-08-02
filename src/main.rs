use std::{
    collections::HashMap,
    default::Default,
    io::Write,
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

use byte_slice_cast::AsByteSlice;
use byte_slurper::*;
use chrono::Utc;
use crossbeam_channel::{unbounded, Receiver, Sender};
use lending_iterator::LendingIterator;
use psrdada::{builder::DadaClientBuilder, client::DadaClient};

fn stokes_to_dada(
    avg_mutex: Arc<Mutex<Vec<u16>>>,
    sig_rx: Receiver<Signal>,
    mut stream: TcpStream,
) {
    let mut stokes_cnt = 0usize;
    // Setup our timer for the samples
    let mut first_sample_time = Utc::now();
    // Most of these should be constants or set by args
    let mut header = HashMap::from([
        ("NCHAN".to_owned(), CHANNELS.to_string()),
        ("BW".to_owned(), "250".to_owned()),
        ("FREQ".to_owned(), "1405".to_owned()),
        ("NPOL".to_owned(), "1".to_owned()),
        ("NBIT".to_owned(), "16".to_owned()),
        ("TSAMP".to_owned(), (TSAMP * 1e6).to_string()),
        (
            "UTC_START".to_owned(),
            heimdall_timestamp(&first_sample_time),
        ),
    ]);
    // let (mut hc, mut dc) = client.split();
    // let mut data_writer = dc.writer();

    loop {
        // Grab the block
        // let mut block = data_writer.next().unwrap();
        loop {
            match sig_rx.recv().unwrap() {
                Signal::Stop => {
                    break;
                }
                Signal::NewAvg => {
                    // Get a lock of the avg shared memory
                    let avg = &*avg_mutex.lock().unwrap();
                    // Push the incoming average to the right place in the output
                    // block.write_all(avg.as_byte_slice()).unwrap();
                    // If this was the first one, update the start time
                    if stokes_cnt == 0 {
                        first_sample_time = Utc::now();
                    }
                    // Increment the stokes counter
                    stokes_cnt += 1;
                    // If we've filled the window, generate the header and send the whole thing
                    if stokes_cnt == NSAMP {
                        println!("New window");
                        // Send to TCP viewer
                        stream.write_all(avg.as_byte_slice()).unwrap();
                        // Reset the stokes counter
                        stokes_cnt = 0;
                        // update header time
                        // header
                        //     .entry("UTC_START".to_owned())
                        //     .or_insert_with(|| heimdall_timestamp(&first_sample_time));
                        // // Safety: All these header keys and values are valid
                        // unsafe { hc.push_header(&header).unwrap() };
                        // // Commit data and update
                        // block.commit();
                        // Break to finish the write
                        break;
                    }
                }
            }
        }
    }
}

fn udp_to_avg(
    udp: pcap::Device,
    port: u16,
    avg_mutex: Arc<Mutex<Vec<u16>>>,
    sig_tx: Sender<Signal>,
) {
    // Locals
    let mut pol_x = [ComplexByte::default(); CHANNELS];
    let mut pol_y = [ComplexByte::default(); CHANNELS];
    // State to hold the averaging window
    let mut avg_window = [0u16; AVG_WINDOW_SIZE];
    let mut avg_cnt = 0usize;
    // Capture packets
    let mut cap = pcap::Capture::from_device(udp)
        .unwrap()
        .timeout(1000000000)
        .buffer_size(2 * PAYLOAD_SIZE as i32)
        .open()
        .unwrap();
    // Add a port filterer for what we expect
    cap.filter(&format!("dst port {}", port), true).unwrap();
    while let Ok(packet) = cap.next() {
        // Trim off the header
        let payload = &packet.data[42..];
        // Skip invalid packets
        if payload.len() != PAYLOAD_SIZE {
            continue;
        }
        // Unpack
        payload_to_spectra(payload, &mut pol_x, &mut pol_y);
        // Generate stokes and push to averaging window
        for i in 0..CHANNELS {
            avg_window[i * AVG_SIZE] = stokes_i(pol_x[i], pol_y[i]);
        }
        avg_cnt += 1;
        if avg_cnt == AVG_SIZE {
            // Reset the counter
            avg_cnt = 0;
            // Generate average
            let avg = &mut *avg_mutex.lock().unwrap();
            avg_from_window::<AVG_SIZE>(&avg_window, avg);
            println!("{:?}", avg);
            // Signal the consumer that there's new data
            sig_tx.send(Signal::NewAvg).unwrap();
        }
    }
}

fn main() -> std::io::Result<()> {
    // Get these from args
    let port = 60000u16;
    let dada_key = 0xb0ba;
    let device_name = "enp129s0f0";

    // Grab the pcap device that matches this interface
    let device = pcap::Device::list()
        .expect("Error listing devices from Pcap")
        .into_iter()
        .find(|d| d.name == device_name)
        .unwrap_or_else(|| panic!("Device named {} not found", device_name));

    // Setup multithreading
    // We'll use a mutex to hold the average that we'll pass to the dada consumer
    let avg_mutex = Arc::new(Mutex::new(vec![0u16; CHANNELS]));
    // And then use a channel for state messaging
    let (sig_tx, sig_rx) = unbounded();

    // Make a clone we'll move to the thread
    let avg_cloned = avg_mutex.clone();

    // Start producing polarizations on a thread
    thread::spawn(move || udp_to_avg(device, port, avg_cloned, sig_tx));

    // Setup listener socket
    let stokes_stream = TcpListener::bind("0.0.0.0:4242").unwrap();
    println!("Waiting for listen connection");
    let (stokes_socket, _) = stokes_stream.accept()?;

    // Setup PSRDADA
    // let client = DadaClientBuilder::new(dada_key)
    //     .buf_size(WINDOW_SIZE as u64 * 2) // We're going to send u16
    //     .num_bufs(8)
    //     .num_headers(8)
    //     .build()
    //     .unwrap();

    // Start consumer
    stokes_to_dada(avg_mutex, sig_rx, stokes_socket);
    Ok(())
}
