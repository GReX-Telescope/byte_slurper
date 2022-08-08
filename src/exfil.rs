//! This module is responsible for exfilling packet data to heimdall

use std::{collections::HashMap, io::Write};

use byte_slice_cast::AsByteSlice;
use chrono::{DateTime, Datelike, Timelike, Utc};
use crossbeam_channel::{Receiver, Sender};
use lending_iterator::LendingIterator;
use psrdada::builder::DadaClientBuilder;

use crate::{
    capture::{unpack, PayloadBytes},
    complex::ComplexByte,
};

// Set by FPGA
pub const CHANNELS: usize = 2048;
// How many averages do we take (as the power of 2)
pub const AVG_SIZE_POW: usize = 2;
// 2^3 = 8 averages
const AVG_SIZE: usize = 2usize.pow(AVG_SIZE_POW as u32);
// How big is the averaging window (elements, not bytes)
pub const AVG_WINDOW_SIZE: usize = AVG_SIZE * CHANNELS;
// How big is the psrdada window (elements, not bytes)
pub const WINDOW_SIZE: usize = CHANNELS * NSAMP;
// Sample time after averaging
const TSAMP: f32 = 8.192e-6 * AVG_SIZE as f32;
// How many of the averaged time slices do we put in the window we're sending to heimdall
// At stoke time of 65.536, this is a little more than a second
const NSAMP: usize = 32768;

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

pub fn push_to_avg_window(
    window: &mut [u16],
    pol_a: &[ComplexByte],
    pol_b: &[ComplexByte],
    row: usize,
) {
    assert_eq!(window.len(), AVG_WINDOW_SIZE);
    assert_eq!(pol_a.len(), CHANNELS);
    assert_eq!(pol_b.len(), CHANNELS);
    for i in 0..CHANNELS {
        window[(i * AVG_SIZE) + row] = stokes_i(pol_a[i], pol_b[i]);
    }
}

/// Average from a fixed window into the output with a power of 2 (`pow`) window size
pub fn avg_from_window(input: &[u16], pow: usize, output: &mut [u16]) {
    assert_eq!(input.len(), AVG_WINDOW_SIZE);
    assert_eq!(output.len(), CHANNELS);
    input
        .chunks_exact(2usize.pow(pow as u32))
        .into_iter()
        .map(|chunk| chunk.iter().fold(0u32, |x, y| x + *y as u32))
        .map(|x| (x >> pow) as u16)
        .enumerate()
        .for_each(|(i, v)| output[i] = v);
}

/// Grab bytes from the capture thread to get them all the way to heimdall.
/// This doesn't need to be realtime, because we have cushion from the rtrb.
/// This function needs to run at less than 8us (on average).
pub fn exfil_consumer(
    client_builder: DadaClientBuilder,
    mut consumer: rtrb::Consumer<PayloadBytes>,
    tcp_sender: Sender<[u16; CHANNELS]>,
    ctrlc_r: Receiver<()>,
) {
    // Containers for parsed spectra
    let mut pol_a = [ComplexByte::default(); CHANNELS];
    let mut pol_b = [ComplexByte::default(); CHANNELS];
    // Averaging window
    let mut avg_window = [0u16; AVG_WINDOW_SIZE];
    let mut avg = [0u16; CHANNELS];
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
        // Grab the next psrdada block we can write to (BLOCKING)
        let mut block = data_writer.next().unwrap();
        loop {
            // Check for ctrlc
            if ctrlc_r.try_recv().is_ok() {
                return;
            }
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
            push_to_avg_window(&mut avg_window, &pol_a, &pol_b, avg_cnt);
            avg_cnt += 1;
            // If we've filled the averaging window, move on to the next step
            if avg_cnt == AVG_SIZE {
                // Reset the counter
                avg_cnt = 0;
                // Generate the average from the window and add to the correct position in the output block
                avg_from_window(&avg_window, AVG_SIZE_POW, &mut avg);
                // Send this average over to the TCP listender, we don't care if this errors
                let _ = tcp_sender.try_send(avg);
                block.write_all(avg.as_byte_slice()).unwrap();
                // If this was the first one, update the start time
                if stokes_cnt == 0 {
                    first_sample_time = Utc::now();
                }
                stokes_cnt += 1;
                // If we've filled the window, generate the header and send it to PSRDADA
                if stokes_cnt == NSAMP {
                    // Reset the stokes counter
                    stokes_cnt = 0;
                    header
                        .entry("UTC_START".to_owned())
                        .or_insert_with(|| heimdall_timestamp(&first_sample_time));
                    // Safety: All these header keys and values are valid
                    unsafe { hc.push_header(&header).unwrap() };
                    // Commit data and update
                    block.commit();
                    //Break to finish the write
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complex::Complex;

    #[test]
    fn test_stokes() {
        let pol_x = Complex { re: -1i8, im: -1i8 };
        let pol_y = Complex { re: -1i8, im: -1i8 };
        assert_eq!(4u16, stokes_i(pol_x, pol_y))
    }
}
