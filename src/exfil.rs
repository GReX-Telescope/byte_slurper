//! This module is responsible for exfilling packet data to heimdall

use std::{collections::HashMap, io::Write};

use byte_slice_cast::AsByteSlice;
use byte_slurper::{ComplexByte, CHANNELS};
use chrono::{DateTime, Datelike, Timelike, Utc};
use lending_iterator::LendingIterator;
use psrdada::builder::DadaClientBuilder;

use crate::capture::{unpack, PayloadBytes};

// How many averages do we take (as the power of 2)
const AVG_SIZE_POW: usize = 3;
// 2^3 = 8 averages
const AVG_SIZE: usize = 2usize.pow(AVG_SIZE_POW as u32);
// How big is the averaging window (elements, not bytes)
const AVG_WINDOW_SIZE: usize = AVG_SIZE * CHANNELS;
// How big is the psrdada window (elements, not bytes)
pub const WINDOW_SIZE: usize = CHANNELS * NSAMP;
// Sample time after averaging
const TSAMP: f32 = 8.192e-6 * AVG_SIZE as f32;
// How many of the averaged time slices do we put in the window we're sending to heimdall
// At stoke time of 65.536, this is a little more than a second
const NSAMP: usize = 16384;

/// Convert a chronno DateTime into a heimdall-compatible timestamp string
fn heimdall_timestamp(time: &DateTime<Utc>) -> String {
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

fn square_byte(byte: i8) -> u16 {
    byte.unsigned_abs() as u16 * byte.unsigned_abs() as u16
}

fn norm_sq(cb: ComplexByte) -> u16 {
    square_byte(cb.re) + square_byte(cb.im)
}

pub fn stokes_i(pol_x: ComplexByte, pol_y: ComplexByte) -> u16 {
    norm_sq(pol_x) + norm_sq(pol_y)
}

/// Average from a fixed window into the output with a power of 2 (`pow`) window size
fn avg_from_window(input: &[u16], pow: usize) -> Vec<u16> {
    input
        .chunks_exact(2usize.pow(pow as u32))
        .into_iter()
        .map(|chunk| chunk.iter().fold(0u32, |x, y| x + *y as u32))
        .map(|x| (x >> pow) as u16)
        .collect()
}

/// Grab bytes from the capture thread to get them all the way to heimdall.
/// This doesn't need to be realtime, because we have cushion from the rtrb.
pub fn exfil_consumer(
    client_builder: DadaClientBuilder,
    mut consumer: rtrb::Consumer<PayloadBytes>,
) -> ! {
    // Containers for parsed spectra
    let mut pol_a = [ComplexByte::default(); CHANNELS];
    let mut pol_b = [ComplexByte::default(); CHANNELS];
    // Averaging window - heap allocated so we don't blow our stack
    let mut avg_window = vec![0u16; AVG_WINDOW_SIZE];
    let mut avg_cnt = 0usize;
    // DADA window
    let mut stokes_cnt = 0usize;
    // Setup our timer for the samples
    // This will need to come from the data eventually
    let mut first_sample_time = Utc::now();
    // Create header skeleton
    let mut header = HashMap::from([
        ("NCHAN".to_owned(), CHANNELS.to_string()),
        ("BW".to_owned(), "250".to_owned()),
        ("FREQ".to_owned(), "1405".to_owned()),
        ("NPOL".to_owned(), "1".to_owned()),
        ("NBIT".to_owned(), "16".to_owned()),
        ("TSAMP".to_owned(), (TSAMP * 1e6).to_string()),
        (
            "UTC_START".to_owned(),
            heimdall_timestamp(&first_sample_time),
        ),
    ]);
    // Finish building the PSRDADA client on this thread
    let mut client = client_builder.build().unwrap();
    // Grab PSRDADA writing context
    let (mut hc, mut dc) = client.split();
    let mut data_writer = dc.writer();
    // Start the main consumer loop
    loop {
        // Grab the next psrdada block we can write to
        // let mut block = data_writer.next().unwrap();
        loop {
            // Busy wait until we get data. This will peg the CPU at 100%, but that's ok
            // we don't want to give the time to the kernel with yeild, as that has a 15ms penalty
            let payload;
            if let Ok(pl) = consumer.pop() {
                payload = pl;
            } else {
                continue;
            }
            // Unpack payload to spectra
            unpack(&payload, &mut pol_a, &mut pol_b);
            // TODO: Push to a time-domain buffer that we might want to dump
            // Generate stokes for this sample and push to averaging window
            // This is a transpose operation because the average calculation needs the time axis
            // to be contiguous as that's what we're summing over
            for i in 0..CHANNELS {
                avg_window[(i * AVG_SIZE) + avg_cnt] = stokes_i(pol_a[i], pol_b[i]);
            }
            avg_cnt += 1;
            // If we've filled the averaging window, move on to the next step
            if avg_cnt == AVG_SIZE {
                // Reset the counter
                avg_cnt = 0;
                // // Generate the average from the window and add to the correct position in the output block
                // let avg = avg_from_window(&avg_window, AVG_SIZE_POW);
                // // block
                // //     .write_all(avg_from_window(avg.as_byte_slice())
                // //     .unwrap();
                // // If this was the first one, update the start time
                // if stokes_cnt == 0 {
                //     first_sample_time = Utc::now();
                // }
                // stokes_cnt += 1;
                // // If we've filled the window, generate the header and send it to PSRDADA
                // if stokes_cnt == NSAMP {
                //     // Reset the stokes counter
                //     stokes_cnt = 0;
                //     header
                //         .entry("UTC_START".to_owned())
                //         .or_insert_with(|| heimdall_timestamp(&first_sample_time));
                //     // Safety: All these header keys and values are valid
                //     // unsafe { hc.push_header(&header).unwrap() };
                //     // Commit data and update
                //     // block.commit();
                //     //Break to finish the write
                //     break;
                // }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use byte_slurper::Complex;

    use super::*;

    #[test]
    fn test_stokes() {
        let pol_x = Complex { re: -1i8, im: -1i8 };
        let pol_y = Complex { re: -1i8, im: -1i8 };
        assert_eq!(4u16, stokes_i(pol_x, pol_y))
    }
}
