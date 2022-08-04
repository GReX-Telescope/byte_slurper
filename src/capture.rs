//! This module contains all the capture logic

use byte_slurper::PAYLOAD_SIZE;
use tracing::warn;

type PayloadBytes = [u8; PAYLOAD_SIZE];

pub fn capture_udp(
    mut cap: pcap::Capture<pcap::Active>,
    mut producer: rtrb::Producer<PayloadBytes>,
) -> ! {
    loop {
        let mut payload = [0u8; PAYLOAD_SIZE];
        let packet;
        if let Ok(pak) = cap.next() {
            packet = pak;
        } else {
            // Keep truckin, we don't care!
            warn!("libpcap error");
            continue;
        }
        let data = &packet.data[42..];
        // Skip bad packets (we should probably count how often this happens)
        if data.len() != PAYLOAD_SIZE {
            warn!("Got a payload of a size we didn't expect, throwing out");
            continue;
        }
        // Memcpy payload to payload
        payload.copy_from_slice(data);
        // Send to ringbuffer
        producer
            .push(payload)
            .expect("ring buffer full, try increasing capacity");
    }
}

pub fn consume_and_drop(mut consumer: rtrb::Consumer<PayloadBytes>) -> ! {
    loop {
        if consumer.pop().is_ok() {
        } else {
            // Spin until there's data
            // We could yield, but thats a 15ms penalty because linux
            continue;
        }
    }
}
