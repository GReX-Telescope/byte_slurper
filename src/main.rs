use args::convert_filter;
use byte_slurper::PAYLOAD_SIZE;
use capture::consume_and_drop;
use clap::Parser;
use rtrb::RingBuffer;

use crate::{args::Args, capture::capture_udp};

mod args;
mod capture;
// mod exfil;
// mod monitoring;

fn main() -> ! {
    // Parse args
    let args = Args::parse();

    // Setup logging
    tracing_subscriber::fmt()
        .with_max_level(convert_filter(args.verbose.log_level_filter()))
        .init();

    // Grab the pcap device that matches this interface
    let device = pcap::Device::list()
        .expect("Error listing devices from Pcap")
        .into_iter()
        .find(|d| d.name == args.device_name)
        .unwrap_or_else(|| panic!("Device named {} not found", args.device_name));

    // Create the "capture"
    let mut cap = pcap::Capture::from_device(device)
        .unwrap()
        .timeout(1000000000)
        .buffer_size(2 * PAYLOAD_SIZE as i32)
        .open()
        .unwrap();

    // Add the port filter
    cap.filter(&format!("dst port {}", args.port), true)
        .expect("Error creating port filter");

    // Create rtrb pairs
    let (producer, consumer) = RingBuffer::new(args.capacity);

    // For each consumer, make a thread that does the thing
    std::thread::spawn(move || consume_and_drop(consumer));

    // Startup the main capture thread
    capture_udp(cap, producer);
}
