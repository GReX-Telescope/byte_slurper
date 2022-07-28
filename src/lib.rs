use std::collections::HashMap;

use chrono::{DateTime, Datelike, Timelike, Utc};
use num_complex::Complex;

pub const PAYLOAD_SIZE: usize = 8192;
pub const WORD_SIZE: usize = 8;
pub const CHANNELS: usize = 2048;

pub const AVG_SIZE: usize = 8; // At tsamp of 8.192 us, this gives us 1 stoke per 65.536us
pub const NSAMP: usize = 8192; // At stoke time of 65.536, this is a little more than a second
pub const WINDOW_SIZE: usize = CHANNELS * NSAMP;
// A buffer for the running average
pub const AVG_WINDOW_SIZE: usize = AVG_SIZE * CHANNELS;
// We can figure out sample time
pub const TSAMP: f32 = 8.192e-6 * AVG_SIZE as f32;

pub type ComplexByte = Complex<i8>;

// Upcast to avoid overflow
fn square_byte(byte: i8) -> i16 {
    byte as i16 * byte as i16
}

// If we need to, these can be unchecked-add
fn norm_sq(cb: ComplexByte) -> i16 {
    square_byte(cb.re) + square_byte(cb.im)
}

fn stokes_i(pol_x: ComplexByte, pol_b: ComplexByte) -> i16 {
    norm_sq(pol_x) + norm_sq(pol_b)
}

pub fn gen_stokes_i<const N: usize>(
    pol_x: &[ComplexByte; N],
    pol_y: &[ComplexByte; N],
    output: &mut [i16],
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

pub fn avg_from_window(input: &[i16], output: &mut [i16], n: usize) {
    // [ch0,ch1..chN,ch0,ch1...]
    for (i, chunk) in input.chunks_exact(n).enumerate() {
        let sum: i32 = chunk.iter().fold(0i32, |acc, x| acc + *x as i32);
        output[i] = (sum / n as i32) as i16
    }
}

pub fn gen_header(
    nchan: u32,
    bw: f32,
    freq: f32,
    npol: u32,
    nbit: u32,
    tsamp: f32,
    utc_start: &str,
) -> HashMap<String, String> {
    HashMap::from([
        ("NCHAN".to_owned(), nchan.to_string()),
        ("BW".to_owned(), bw.to_string()),
        ("FREQ".to_owned(), freq.to_string()),
        ("NPOL".to_owned(), npol.to_string()),
        ("NBIT".to_owned(), nbit.to_string()),
        ("TSAMP".to_owned(), tsamp.to_string()),
        ("UTC_START".to_owned(), utc_start.to_owned()),
    ])
}

pub fn heimdall_timestamp(time: DateTime<Utc>) -> String {
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
