//! This module contains all the capture logic

// The packet capture thread will do one thing, as fast as possible:
// Capture packets from the NIC, and that's it. We're going to take
// those bytes and pass them through an rtrb ring buffer to be processed
// in another thread

use std::mem;

use byte_slurper::PAYLOAD_SIZE;

type PayloadBytes = [u8; PAYLOAD_SIZE];

pub fn capture_udp(
    mut cap: pcap::Capture<pcap::Active>,
    producers: &mut [rtrb::Producer<PayloadBytes>],
) -> ! {
    let mut idx = 0usize;
    let num_producers = producers.len();
    loop {
        let mut payload = [0u8; PAYLOAD_SIZE];
        let packet;
        if let Ok(pak) = cap.next() {
            packet = pak;
        } else {
            // Keep truckin, we don't care!
            continue;
        }
        let data = &packet.data[42..];
        // Skip bad packets (we should probably count how often this happens)
        if data.len() != PAYLOAD_SIZE {
            continue;
        }
        // Memcpy payload to payload
        payload.copy_from_slice(data);
        // Round robin send the data
        producers[idx]
            .push(payload)
            .expect("ring buffer full, try increasing capacity");
        // Increment our rr idx
        // We want N to be a power of 2 so the mod is fast
        idx = (idx + 1) % num_producers;
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
