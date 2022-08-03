use chrono::{DateTime, Datelike, Timelike, Utc};
use fixed::{
    types::extra::{U14, U7},
    FixedI16, FixedI8,
};

pub const PAYLOAD_SIZE: usize = 8192;
pub const WORD_SIZE: usize = 8;
pub const CHANNELS: usize = 2048;

pub const AVG_SIZE: usize = 512; // At tsamp of 8.192 us, this gives us 1 stoke per 65.536us
pub const NSAMP: usize = 16; // At stoke time of 65.536, this is a little more than a second
pub const WINDOW_SIZE: usize = CHANNELS * NSAMP;
// A buffer for the running average
pub const AVG_WINDOW_SIZE: usize = AVG_SIZE * CHANNELS;
// We can figure out sample time
pub const TSAMP: f32 = 8.192e-6 * AVG_SIZE as f32;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Complex<T> {
    re: T,
    im: T,
}

pub type FpgaByte = i8;
pub type FixedWord = u16;
pub type ComplexByte = Complex<FpgaByte>;

fn square_byte(byte: FpgaByte) -> FixedWord {
    byte.unsigned_abs() as u16 * byte.unsigned_abs() as u16
}

// If we need to, these can be unchecked-add
fn norm_sq(cb: ComplexByte) -> FixedWord {
    square_byte(cb.re) + square_byte(cb.im)
}

// We're done multiplying, so we can come back to u16 land
pub fn stokes_i(pol_x: ComplexByte, pol_y: ComplexByte) -> u16 {
    norm_sq(pol_x) + norm_sq(pol_y)
}

fn raw_to_fpga(byte: u8) -> FpgaByte {
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

pub fn avg_from_window<const N: usize>(input: &[u16], output: &mut [f32]) {
    let chunks = input.len() / N;
    input
        .chunks_exact(chunks)
        .into_iter()
        .map(|chunk| chunk.iter().fold(0f32, |x, y| x + *y as f32))
        .map(|x| x / chunks as f32)
        .enumerate()
        .for_each(|(i, v)| output[i] = v);
}

pub fn heimdall_timestamp(time: &DateTime<Utc>) -> String {
    format!(
        "{}-{:02}-{:02}-{:02}:{:02}:{:02}",
        time.year(),
        time.month(),
        time.day(),
        time.hour(),
        time.minute(),
        time.second()
    )
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
        let pol_x = Complex { re: 32i8, im: 64i8 };
        let pol_y = Complex {
            re: -128i8,
            im: 127i8,
        };
        assert_eq!(37633u16, stokes_i(pol_x, pol_y))
    }
}
