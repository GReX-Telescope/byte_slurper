use byte_slurper::{
    args::{convert_filter, Args},
    capture::{capture_udp, PAYLOAD_SIZE},
    exfil::{dada_consumer, filterbank_consumer},
    monitoring::listen_consumer,
    CaptureConfig,
};
use casperfpga::transport::{tapcp::Tapcp, Transport};
use clap::Parser;
use crossbeam_channel::bounded;
use hifitime::Epoch;
use rtrb::RingBuffer;
use tracing::info;

fn main() {
    // Parse args
    let args = Args::parse();

    // Build the cap config from the args
    let cc = CaptureConfig {
        channels: args.channels,
        samples: args.samples,
        avgs: args.avgs,
        cadence: args.cadence,
    };

    // Setup logging
    tracing_subscriber::fmt()
        .with_max_level(convert_filter(args.verbose.log_level_filter()))
        .init();

    // Print some useful information
    info!("Starting packet capture!\nDownsample factor: {}\nDownsampled sample time: {}us\nChannels: {}\nDADA chunk size: {}\nDADA chunk time: {}s", cc.avgs, cc.tsamp()*1e6, cc.channels, cc.samples, cc.twindow());

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

    // Setup the monitoring channel
    let (tcp_s, tcp_r) = bounded(1);

    // Signal the FPGA to start on the next rising PPS edge
    let mut transport = Tapcp::connect(args.fpga_addr).expect("UDP Connection to the FPGA failed");
    assert!(
        transport.is_running().unwrap(),
        "SNAP board is not programmed/running"
    );
    // FIXME replace 32 bit word with bool
    transport.write("master_rst", 0, &1u32).unwrap();
    transport.write("master_rst", 0, &0u32).unwrap();
    // FIXME, actually time this
    let payload_start = Epoch::now().unwrap();
    transport.write("pps_trig", 0, &1u32).unwrap();
    transport.write("pps_trig", 0, &0u32).unwrap();

    // Spawn the exfil thread
    if let Some(key) = args.key {
        std::thread::spawn(move || dada_consumer(key, consumer, tcp_s, &cc, payload_start));
    } else {
        std::thread::spawn(move || filterbank_consumer(consumer, tcp_s, &cc, payload_start));
    }

    // Spawn the monitoring thread
    std::thread::spawn(move || listen_consumer(tcp_r, args.listen_port, &cc));

    // Startup the main capture thread
    capture_udp(cap, producer);
}
