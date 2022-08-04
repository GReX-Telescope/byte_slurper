#![feature(int_log)]

use chrono::{DateTime, Datelike, Timelike, Utc};

// Don't change these (set by the framing of the ethernet stuff)
pub const WORD_SIZE: usize = 8;
pub const CHANNELS: usize = 2048;
pub const PAYLOAD_SIZE: usize = 8192;

// How many UDP samples do we average
// This needs to be a power of 2 so we can average easily with a bit shift
// At tsamp of 8.192 us, 8 gives us 1 stoke per 65.536us
pub const AVG_SIZE: usize = 8;
// How many of the averaged time slices do we put in the window we're sending to heimdall
// At stoke time of 65.536, this is a little more than a second
pub const NSAMP: usize = 16384;

// ----- Calculated constants
// How big is the psrdada window (elements, not bytes)
pub const WINDOW_SIZE: usize = CHANNELS * NSAMP;
// How big is the averaging window (elements, not bytes)
pub const AVG_WINDOW_SIZE: usize = AVG_SIZE * CHANNELS;
// Sample time after averaging
pub const TSAMP: f32 = 8.192e-6 * AVG_SIZE as f32;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Complex<T> {
    re: T,
    im: T,
}

pub type ComplexByte = Complex<i8>;

fn square_byte(byte: i8) -> u16 {
    byte.unsigned_abs() as u16 * byte.unsigned_abs() as u16
}

// If we need to, these can be unchecked-add
fn norm_sq(cb: ComplexByte) -> u16 {
    square_byte(cb.re) + square_byte(cb.im)
}

pub fn stokes_i(pol_x: ComplexByte, pol_y: ComplexByte) -> u16 {
    norm_sq(pol_x) + norm_sq(pol_y)
}

fn raw_to_fpga(byte: u8) -> i8 {
    byte as i8
}

pub fn payload_to_spectra(
    payload: &[u8],
    pol_a: &mut [ComplexByte; CHANNELS],
    pol_b: &mut [ComplexByte; CHANNELS],
) {
    for (i, word) in payload.chunks_exact(WORD_SIZE).enumerate() {
        // Each word contains two frequencies for each polarization
        // [A1 B1 A2 B2]
        // Where each channel is [Re Im] as FixedI8<7>
        let a1 = ComplexByte {
            re: raw_to_fpga(word[7]),
            im: raw_to_fpga(word[6]),
        };
        let b1 = ComplexByte {
            re: raw_to_fpga(word[5]),
            im: raw_to_fpga(word[4]),
        };
        let a2 = ComplexByte {
            re: raw_to_fpga(word[3]),
            im: raw_to_fpga(word[2]),
        };
        let b2 = ComplexByte {
            re: raw_to_fpga(word[1]),
            im: raw_to_fpga(word[0]),
        };
        // Update spectra
        pol_a[2 * i] = a1;
        pol_a[2 * i + 1] = a2;
        pol_b[2 * i] = b1;
        pol_b[2 * i + 1] = b2;
    }
}

/// Average from a fixed window with `N` channels
pub fn avg_from_window<const N: usize>(input: &[u16], output: &mut [u16]) {
    let chunks = input.len() / N;
    let shift = AVG_SIZE.log2();
    input
        .chunks_exact(chunks)
        .into_iter()
        .map(|chunk| chunk.iter().fold(0u32, |x, y| x + *y as u32))
        .map(|x| (x >> shift) as u16)
        .enumerate()
        .for_each(|(i, v)| output[i] = v);
}

pub enum Signal {
    NewAvg,
    Stop,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stokes() {
        let pol_x = Complex { re: -1i8, im: -1i8 };
        let pol_y = Complex { re: -1i8, im: -1i8 };
        assert_eq!(4u16, stokes_i(pol_x, pol_y))
    }
}
