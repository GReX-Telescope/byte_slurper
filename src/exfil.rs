//! This module is responsible for exfilling packet data to heimdall

use std::{
    collections::HashMap,
    fs::File,
    io::{prelude::*, Write},
};

use byte_slice_cast::AsByteSlice;
use chrono::{DateTime, Datelike, Timelike, Utc};
use crossbeam_channel::{Receiver, Sender};
use lending_iterator::LendingIterator;
use psrdada::{builder::DadaClientBuilder, client::DadaClient};
use sigproc_filterbank::write::{PackSpectra, WriteFilterbank};
use tracing::info;

use crate::{
    capture::{unpack, PayloadBytes},
    complex::ComplexByte,
};

// Set by FPGA
pub const CHANNELS: usize = 2048;
// How many averages do we take (as the power of 2)
pub const AVG_SIZE_POW: usize = 5; // 262.144 us
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
const NSAMP: usize = 4096;

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

fn mjd(time: &DateTime<Utc>) -> f32 {
    let a = time.month() - 14 / 12;
    let b = 1461 * (time.year() as u32 + 4800 + a);
    let c = 367 * (time.month() - 2 - 12 * a);
    let e = (time.year() as u32 + 4900 + a) / 100;
    let jdn = b / 4 + c / 12 - (3 * e) / 4 + time.day() - 32075;
    // then the time since UTC noon
    let sec_since_midnight = time.num_seconds_from_midnight();
    let sec_since_last_noon = if sec_since_midnight < 43200 {
        sec_since_midnight + 43200 // Add half a day
    } else {
        sec_since_midnight - 43200 // Remove half a day
    };
    let frac_day = sec_since_last_noon as f32 / 86400f32;
    jdn as f32 + frac_day - 2400000.5
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

/// Basically the same as the dada consumer, except write to a single instead
pub fn filterbank_consumer(
    mut consumer: rtrb::Consumer<PayloadBytes>,
    tcp_sender: Sender<[u16; CHANNELS]>,
) {
    let mut pol_a = [ComplexByte::default(); CHANNELS];
    let mut pol_b = [ComplexByte::default(); CHANNELS];
    let mut avg_window = [0u16; AVG_WINDOW_SIZE];
    let mut avg = [0u16; CHANNELS];
    let mut avg_cnt = 0usize;
    // Create the file
    let mut file = File::create(format!("grex-{}.fil", heimdall_timestamp(&Utc::now()))).unwrap();
    // Create the filterbank context
    let mut fb = WriteFilterbank::new(CHANNELS, 1);
    // Get current  time
    let now = Utc::now();
    // Setup the header stuff
    fb.fch1 = Some(1280.06103516); // Start of band + half the step size
    fb.foff = Some(250.0);
    fb.tsamp = Some(TSAMP as f64);
    fb.tstart = Some(mjd(&now) as f64);
    // Write out the header
    file.write_all(&fb.header_bytes()).unwrap();
    loop {
        let payload;
        if let Ok(pl) = consumer.pop() {
            payload = pl;
        } else {
            continue;
        }
        unpack(&payload, &mut pol_a, &mut pol_b);
        push_to_avg_window(&mut avg_window, &pol_a, &pol_b, avg_cnt);
        // Stream to FB
        file.write_all(&fb.pack(&avg)).unwrap();
        avg_cnt += 1;
        if avg_cnt == AVG_SIZE {
            avg_cnt = 0;
            avg_from_window(&avg_window, AVG_SIZE_POW, &mut avg);
            let _ = tcp_sender.try_send(avg);
            fb.push(&avg);
        }
    }
}

/// Grab bytes from the capture thread to get them all the way to heimdall.
/// This doesn't need to be realtime, because we have cushion from the rtrb.
/// This function needs to run at less than 8us (on average).
pub fn dada_consumer(
    client_builder: DadaClientBuilder,
    //key: i32,
    mut consumer: rtrb::Consumer<PayloadBytes>,
    tcp_sender: Sender<[u16; CHANNELS]>,
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
    // Send the header (heimdall only wants one)
    let header = HashMap::from([
        ("NCHAN".to_owned(), CHANNELS.to_string()),
        ("BW".to_owned(), "250".to_owned()),
        ("FREQ".to_owned(), "1405".to_owned()),
        ("NPOL".to_owned(), "1".to_owned()),
        ("NBIT".to_owned(), "16".to_owned()),
        ("OBS_OFFSET".to_owned(), 0.to_string()),
        // ("FILE_SIZE".to_owned(), (2 * WINDOW_SIZE).to_string()),
        // ("HDR_SIZE".to_owned(), 4096.to_string()),
        ("TSAMP".to_owned(), (TSAMP * 1e6).to_string()),
        ("UTC_START".to_owned(), heimdall_timestamp(&Utc::now())),
    ]);
    // Finish building the PSRDADA client on this thread
    let mut client = client_builder.build().unwrap();
    // let mut client = DadaClient::new(key).unwrap();
    // Grab PSRDADA writing context
    let (mut hc, mut dc) = client.split();
    let mut data_writer = dc.writer();
    // Write the single header
    // Safety: All these header keys and values are valid
    unsafe { hc.push_header(&header).unwrap() };
    // Start the main consumer loop
    loop {
        // Grab the next psrdada block we can write to (BLOCKING)
        let mut block = data_writer.next().unwrap();
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
                stokes_cnt += 1;
                // If we've filled the window, generate the header and send it to PSRDADA
                if stokes_cnt == NSAMP {
                    // Reset the stokes counter
                    stokes_cnt = 0;
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
