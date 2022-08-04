use byte_slurper::PAYLOAD_SIZE;
use capture::consume_and_drop;
use rtrb::RingBuffer;

use crate::capture::capture_udp;

mod args;
mod capture;
// mod exfil;
// mod monitoring;

fn main() -> ! {
    // Get these from args
    let port = 60000u16;
    let dada_key = 0xb0ba;
    let device_name = "enp129s0f0";
    let rr_size = 1usize;
    let rb_capacity = 256usize;

    // Grab the pcap device that matches this interface
    let device = pcap::Device::list()
        .expect("Error listing devices from Pcap")
        .into_iter()
        .find(|d| d.name == device_name)
        .unwrap_or_else(|| panic!("Device named {} not found", device_name));

    // Create the "capture"
    let mut cap = pcap::Capture::from_device(device)
        .unwrap()
        .timeout(1000000000)
        .buffer_size(2 * PAYLOAD_SIZE as i32)
        .open()
        .unwrap();

    // Add the port filter
    cap.filter(&format!("dst port {}", port), true)
        .expect("Error creating port filter");

    // Create rtrb pairs
    let (mut producers, consumers) = {
        let mut ps = vec![];
        let mut cs = vec![];
        (0..rr_size).for_each(|_| {
            let (p, c) = RingBuffer::new(rb_capacity);
            ps.push(p);
            cs.push(c);
        });
        (ps, cs)
    };

    // For each consumer, make a thread that does the thing
    for consumer in consumers {
        std::thread::spawn(move || consume_and_drop(consumer));
    }

    // Startup the main capture thread
    capture_udp(cap, &mut producers);
}
