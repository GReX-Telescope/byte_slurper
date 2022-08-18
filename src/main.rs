use byte_slurper::{
    args::{convert_filter, Args},
    capture::{capture_udp, PAYLOAD_SIZE},
    exfil::{dada_consumer, filterbank_consumer, WINDOW_SIZE},
    monitoring::listen_consumer,
};
use clap::Parser;
use crossbeam_channel::bounded;
use psrdada::builder::DadaClientBuilder;
use rtrb::RingBuffer;

fn main() {
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

    // Setup PSRDADA
    let client_builder = args.key.map(|key| {
        DadaClientBuilder::new(key)
        .buf_size(WINDOW_SIZE as u64 * 2) // We're going to send u16
        .num_bufs(16)
        .num_headers(16)
        .lock(true)
    });

    // Panic on ctrl
    ctrlc::set_handler(move || {
        panic!();
    })
    .expect("Error setting Ctrl-C handler");

    // Setup the monitoring channel
    let (tcp_s, tcp_r) = bounded(1);

    // Spawn the exfil thread
    if let Some(cb) = client_builder {
        std::thread::spawn(move || dada_consumer(cb, consumer, tcp_s));
    } else {
        std::thread::spawn(move || filterbank_consumer(consumer, tcp_s));
    }
    // Spawn the monitoring thread
    std::thread::spawn(move || listen_consumer(tcp_r, args.listen_port));

    // Startup the main capture thread
    capture_udp(cap, producer);
}
