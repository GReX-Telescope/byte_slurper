use chrono::{DateTime, Datelike, Timelike, Utc};
use num_complex::Complex;

pub const PAYLOAD_SIZE: usize = 8192;
pub const WORD_SIZE: usize = 8;
pub const CHANNELS: usize = 2048;

pub const AVG_SIZE: usize = 8; // At tsamp of 8.192 us, this gives us 1 stoke per 65.536us
pub const NSAMP: usize = 16384; // At stoke time of 65.536, this is a little more than a second
pub const WINDOW_SIZE: usize = CHANNELS * NSAMP;
// A buffer for the running average
pub const AVG_WINDOW_SIZE: usize = AVG_SIZE * CHANNELS;
// We can figure out sample time
pub const TSAMP: f32 = 8.192e-6 * AVG_SIZE as f32;

pub type ComplexByte = Complex<i8>;

// Upcast to avoid overflow
fn square_byte(byte: i8) -> u16 {
    (byte as i16 * byte as i16) as u16
}

// If we need to, these can be unchecked-add
fn norm_sq(cb: ComplexByte) -> u16 {
    square_byte(cb.re) + square_byte(cb.im)
}

fn stokes_i(pol_x: ComplexByte, pol_b: ComplexByte) -> u16 {
    norm_sq(pol_x) + norm_sq(pol_b)
}

pub fn gen_stokes_i<const N: usize>(
    pol_x: &[ComplexByte; N],
    pol_y: &[ComplexByte; N],
    output: &mut [u16],
) {
    for i in 0..N {
        output[i] = stokes_i(pol_x[i], pol_y[i]);
    }
}

pub fn payload_to_spectra(
    payload: &[u8],
    pol_a: &mut [ComplexByte; CHANNELS],
    pol_b: &mut [ComplexByte; CHANNELS],
) {
    for (i, word) in payload.chunks_exact(WORD_SIZE).enumerate() {
        // Each word contains two frequencies for each polarization
        // [A1 B1 A2 B2]
        // Where each channel is [Re Im]
        let a1 = ComplexByte {
            re: word[7] as i8,
            im: word[6] as i8,
        };
        let a2 = ComplexByte {
            re: word[5] as i8,
            im: word[4] as i8,
        };
        let b1 = ComplexByte {
            re: word[3] as i8,
            im: word[2] as i8,
        };
        let b2 = ComplexByte {
            re: word[1] as i8,
            im: word[0] as i8,
        };
        // Update spectra
        pol_a[2 * i] = a1;
        pol_a[2 * i + 1] = a2;
        pol_b[2 * i] = b1;
        pol_b[2 * i + 1] = b2;
    }
}

pub fn avg_from_window(input: &[u16], output: &mut [u16], n: usize) {
    // [ch0,ch1..chN,ch0,ch1...]
    for (i, chunk) in input.chunks_exact(n).enumerate() {
        let sum: u32 = chunk.iter().fold(0u32, |acc, x| acc + *x as u32);
        output[i] = (sum / n as u32) as u16
    }
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
