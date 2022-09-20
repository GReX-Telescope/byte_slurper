//! This module contains all the capture logic

use tracing::warn;

use crate::complex::Complex;

// FPGA UDP "Word" size (8 bytes as per CASPER docs)
const WORD_SIZE: usize = 8;
// 8192 bytes for 1 chunk of 2048 channels
pub const PAYLOAD_SIZE: usize = 8192;
// UDP Header size (spec-defined)
const UDP_HEADER_SIZE: usize = 42;

pub type PayloadBytes = [u8; PAYLOAD_SIZE];

pub fn capture_udp(
    mut cap: pcap::Capture<pcap::Active>,
    mut producer: rtrb::Producer<PayloadBytes>,
) -> ! {
    loop {
        let mut payload = [0u8; PAYLOAD_SIZE];
        let packet = if let Ok(pak) = cap.next() {
            pak
        } else {
            // Keep truckin, we don't care!
            warn!("libpcap error");
            continue;
        };
        let data = &packet.data[UDP_HEADER_SIZE..];
        // Skip bad packets (we should probably count how often this happens)
        if data.len() != PAYLOAD_SIZE {
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

/// Unpacks a raw UDP payload into the two polarizations
pub fn unpack(payload: &[u8], pol_a: &mut Vec<Complex<i8>>, pol_b: &mut Vec<Complex<i8>>) {
    assert_eq!(
        pol_a.len(),
        payload.len() / WORD_SIZE * 2,
        "Polarization A container must equal the length of the payload / word size * 2"
    );
    assert_eq!(
        pol_b.len(),
        payload.len() / WORD_SIZE * 2,
        "Polarization B container must equal the length of the payload / word size * 2"
    );
    for (i, word) in payload.chunks_exact(WORD_SIZE).enumerate() {
        // Each word contains two frequencies for each polarization
        // [A1 B1 A2 B2]
        // Where each channel is [Re Im] as FixedI8<7>
        pol_a[2 * i] = Complex::new(word[0] as i8, word[1] as i8);
        pol_a[2 * i + 1] = Complex::new(word[4] as i8, word[5] as i8);
        pol_b[2 * i] = Complex::new(word[2] as i8, word[3] as i8);
        pol_b[2 * i + 1] = Complex::new(word[6] as i8, word[7] as i8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unpack() {
        let payload: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let mut pol_a = vec![Complex { re: 0, im: 0 }; 2];
        let mut pol_b = vec![Complex { re: 0, im: 0 }; 2];
        unpack(&payload, &mut pol_a, &mut pol_b);
        assert_eq!(pol_a[0], Complex { re: 1i8, im: 2i8 });
        assert_eq!(pol_b[0], Complex { re: 3i8, im: 4i8 });
        assert_eq!(pol_a[1], Complex { re: 5i8, im: 6i8 });
        assert_eq!(pol_b[1], Complex { re: 7i8, im: 8i8 });
    }
}
