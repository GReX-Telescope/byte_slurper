use byte_slurper::{
    args::{convert_filter, Args},
    capture::{capture_udp, PAYLOAD_SIZE},
    exfil::{exfil_consumer, WINDOW_SIZE},
    monitoring::listen_consumer,
};
use clap::Parser;
use crossbeam_channel::{bounded, Receiver};
use psrdada::builder::DadaClientBuilder;
use rtrb::RingBuffer;

fn ctrl_channel() -> Result<Receiver<()>, ctrlc::Error> {
    let (sender, receiver) = bounded(100);
    ctrlc::set_handler(move || {
        let _ = sender.send(());
    })?;

    Ok(receiver)
}

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
    let client_builder = DadaClientBuilder::new(args.key)
        .buf_size(WINDOW_SIZE as u64 * 2) // We're going to send u16
        .num_bufs(8)
        .num_headers(8)
        .lock(true);

    // Setup the monitoring channel
    let (tcp_s, tcp_r) = bounded(1);

    // Setup the termination channel
    let ctrlc_r_exfil = ctrl_channel().unwrap();
    let ctrlc_r_listen = ctrlc_r_exfil.clone();
    let ctrlc_r_capture = ctrlc_r_exfil.clone();

    // Spawn the exfil thread
    let exfil_handle =
        std::thread::spawn(move || exfil_consumer(client_builder, consumer, tcp_s, ctrlc_r_exfil));

    // Spawn the monitoring thread
    let listen_handle =
        std::thread::spawn(move || listen_consumer(tcp_r, args.listen_port, ctrlc_r_listen));

    // Startup the main capture thread - blocks until Ctrl C
    capture_udp(cap, producer, ctrlc_r_capture);

    // On teardown, join all the threads
    exfil_handle.join().unwrap();
    listen_handle.join().unwrap();
}
