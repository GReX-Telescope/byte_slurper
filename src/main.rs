use byte_slice_cast::AsByteSlice;
use byte_slurper::*;
use chrono::Utc;
use crossbeam_channel::{unbounded, Receiver, Sender};
use psrdada::DadaDBBuilder;
use std::{
    default::Default,
    sync::{Arc, Mutex},
    thread,
};

fn stokes_to_dada(
    avg_mutex: Arc<Mutex<[i16; CHANNELS]>>,
    mut writer: psrdada::WriteHalf,
    sig_rx: Receiver<Signal>,
) {
    // Allocate window on the heap to avoid a stack overflow
    let mut window = vec![0i16; WINDOW_SIZE].into_boxed_slice();
    let mut stokes_cnt = 0usize;
    let mut first_sample_time = Utc::now();

    loop {
        match sig_rx.recv().unwrap() {
            Signal::Stop => {
                break;
            }
            Signal::NewAvg => {
                // Get a lock of the avg shared memory
                let avg = *avg_mutex.lock().unwrap();
                // Push the incoming average to the right place in the output
                window[(stokes_cnt * CHANNELS)..((stokes_cnt + 1) * CHANNELS)]
                    .copy_from_slice(&avg);
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
    }
}

fn udp_to_avg(
    udp: pcap::Device,
    port: u16,
    avg_mutex: Arc<Mutex<[i16; CHANNELS]>>,
    sig_tx: Sender<Signal>,
) {
    // Locals
    let mut pol_x = [ComplexByte::default(); CHANNELS];
    let mut pol_y = [ComplexByte::default(); CHANNELS];
    // State to hold the averaging window
    let mut avg_window = [0i16; AVG_WINDOW_SIZE];
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
        let avg_slice = &mut avg_window[(avg_cnt * CHANNELS)..((avg_cnt + 1) * CHANNELS)];
        gen_stokes_i(&pol_x, &pol_y, avg_slice);
        avg_cnt += 1;
        if avg_cnt == AVG_SIZE {
            // Reset the counter
            avg_cnt = 0;
            // Generate average
            let mut avg = *avg_mutex.lock().unwrap();
            avg_from_window(&avg_window, &mut avg, CHANNELS);
            // Signal the consumer that there's new data
            sig_tx.send(Signal::NewAvg).unwrap();
        }
    }
}

fn main() -> std::io::Result<()> {
    // Get these from args
    let port = 60000u16;
    let dada_key = 0xbeef;
    let device_name = "enp129s0f0";

    // Grab the pcap device that matches this interface
    let device = pcap::Device::list()
        .expect("Error listing devices from Pcap")
        .into_iter()
        .filter(|d| d.name == device_name)
        .next()
        .unwrap_or_else(|| panic!("Device named {} not found", device_name));

    // Setup multithreading
    // We'll use a mutex to hold the average that we'll pass to the dada consumer
    let avg_mutex = Arc::new(Mutex::new([0i16; CHANNELS]));
    // And then use a channel for state messaging
    let (sig_tx, sig_rx) = unbounded();

    // Make a clone we'll move to the thread
    let avg_cloned = avg_mutex.clone();

    // Start producing polarizations on a thread
    thread::spawn(move || udp_to_avg(device, port, avg_cloned, sig_tx));

    // Setup PSRDADA
    let hdu = DadaDBBuilder::new(dada_key, "byte_slurper")
        .buf_size(WINDOW_SIZE as u64 * 2) // We're going to send u16
        .build(true) // Memlock
        .unwrap();

    let (_, writer) = hdu.split();

    // Start consumer
    stokes_to_dada(avg_mutex, writer, sig_rx);
    Ok(())
}
