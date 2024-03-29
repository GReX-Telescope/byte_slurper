//! This module is responsible for exfilling packet data to heimdall

use std::{collections::HashMap, io::Write};

use byte_slice_cast::AsByteSlice;
use chrono::{Datelike, TimeZone, Timelike, Utc};
use crossbeam_channel::Sender;
use hifitime::{Epoch, TimeUnits};
use lending_iterator::LendingIterator;
use psrdada::client::DadaClient;
use sigproc_filterbank::write::WriteFilterbank;
use tracing::{debug, info, warn};

use crate::{
    capture::{unpack, PayloadBytes},
    complex::ComplexByte,
    CaptureConfig,
};

// Set by hardware (in MHz)
const LOWBAND_MID_FREQ: f64 = 1280.06103516;
const BANDWIDTH: f64 = 250.0;

/// Convert a chronno DateTime into a heimdall-compatible timestamp string
fn heimdall_timestamp(time: &Epoch) -> String {
    let unix = time.to_unix_seconds();
    let time = Utc.timestamp_opt(unix as i64, 0).unwrap();
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

fn fullness(c: &rtrb::Consumer<PayloadBytes>) -> f32 {
    c.slots() as f32 / c.buffer().capacity() as f32
}

pub fn add_stokes_avg(
    output: &mut Vec<f32>,
    pol_a: &Vec<ComplexByte>,
    pol_b: &Vec<ComplexByte>,
    cc: &CaptureConfig,
) {
    assert_eq!(output.len(), cc.channels);
    assert_eq!(pol_a.len(), cc.channels);
    assert_eq!(pol_b.len(), cc.channels);

    for i in 0..cc.channels {
        output[i] += stokes_i(pol_a[i], pol_b[i]) as f32 / cc.avgs as f32;
    }
}

/// Basically the same as the dada consumer, except write to a filterbank instead with no chunking
pub fn filterbank_consumer(
    mut consumer: rtrb::Consumer<PayloadBytes>,
    tcp_sender: Sender<Vec<f32>>,
    cc: &CaptureConfig,
    payload_start: Epoch,
) {
    let mut pol_a = vec![ComplexByte::default(); cc.channels];
    let mut pol_b = vec![ComplexByte::default(); cc.channels];
    let mut avg = vec![0f32; cc.channels];
    let mut avg_cnt = 0usize;
    let mut fullness_rising_edge = false;
    let mut payload_n = 0u64;
    // Create the file
    let mut file = std::fs::File::create(format!(
        "grex-{}.fil",
        heimdall_timestamp(&Epoch::now().unwrap())
    ))
    .unwrap();
    // Create the filterbank context
    let mut fb = WriteFilterbank::new(cc.channels, 1);
    // Setup the header stuff
    fb.fch1 = Some(LOWBAND_MID_FREQ); // Start of band + half the step size
    fb.foff = Some(BANDWIDTH / cc.channels as f64);
    fb.tsamp = Some(cc.tsamp() as f64);
    // We will capture the timestamp on the first packet
    let mut first_payload = true;
    loop {
        // Check fullness and report
        if fullness(&consumer) >= 0.9 && !fullness_rising_edge {
            warn!("The raw UDP byte ringbuffer is 90% full");
            fullness_rising_edge = true;
        } else if fullness(&consumer) < 0.9 && fullness_rising_edge {
            fullness_rising_edge = false;
        }
        let payload = if let Ok(pl) = consumer.pop() {
            pl
        } else {
            continue;
        };
        unpack(&payload, &mut pol_a, &mut pol_b, &mut payload_n);
        // Timestamp first one
        if first_payload {
            first_payload = false;
            // Each payload represents cc.candence timesteps after payload_start
            let payload_offset = (payload_n as f64 * cc.cadence as f64).seconds();
            let payload_epoch = payload_start + payload_offset;
            fb.tstart = Some(payload_epoch.to_mjd_utc_days());
            // Write out the header
            file.write_all(&fb.header_bytes()).unwrap();
        }
        // Add to averages
        add_stokes_avg(&mut avg, &pol_a, &pol_b, cc);
        avg_cnt += 1;
        if avg_cnt == cc.avgs {
            avg_cnt = 0;
            let _ = tcp_sender.try_send(avg.clone());
            // Zero the first and last 250 samples because aliasing
            (avg[0..=250]).fill(0.0);
            (avg[1797..=2047]).fill(0.0);
            // Stream to FB
            file.write_all(&fb.pack(&avg)).unwrap();
            // Reset averages
            avg.fill(0.0);
        }
    }
}

/// Grab bytes from the capture thread to get them all the way to heimdall.
/// This doesn't need to be realtime, because we have cushion from the rtrb.
/// This function needs to run at less than the cadence (8.192us) (on average).
pub fn dada_consumer(
    key: i32,
    mut consumer: rtrb::Consumer<PayloadBytes>,
    tcp_sender: Sender<Vec<f32>>,
    cc: &CaptureConfig,
    payload_start: Epoch,
) {
    let mut fullness_rising_edge = false;
    // Containers for parsed spectra
    let mut pol_a = vec![ComplexByte::default(); cc.channels];
    let mut pol_b = vec![ComplexByte::default(); cc.channels];
    let mut payload_n = 0u64;
    // Averaging window
    let mut avg = vec![0f32; cc.channels];
    let mut avg_cnt = 0usize;
    // DADA window
    let mut stokes_cnt = 0usize;
    // We will capture the timestamp on the first packet
    let mut first_payload = true;
    // Send the header (heimdall only wants one)
    let mut header = HashMap::from([
        ("NCHAN".to_owned(), cc.channels.to_string()),
        ("BW".to_owned(), "250".to_owned()),
        ("FREQ".to_owned(), "1405".to_owned()),
        ("NPOL".to_owned(), "1".to_owned()),
        ("NBIT".to_owned(), "16".to_owned()),
        ("OBS_OFFSET".to_owned(), 0.to_string()),
        ("TSAMP".to_owned(), (cc.tsamp() * 1e6).to_string()),
    ]);
    // Connect to the PSRDADA buffer on this thread
    let mut client = DadaClient::new(key).expect("Could not connect to PSRDADA buffer");
    // Grab PSRDADA writing context
    let (mut hc, mut dc) = client.split();
    let mut data_writer = dc.writer();
    info!("DADA header pushed, starting main loop");
    // Start the main consumer loop
    loop {
        // Grab the next psrdada block we can write to (BLOCKING)
        let mut block = data_writer.next().unwrap();
        loop {
            // Check fullness and report
            if fullness(&consumer) >= 0.9 && !fullness_rising_edge {
                warn!("The raw UDP byte ringbuffer is 90% full");
                fullness_rising_edge = true;
            } else if fullness(&consumer) < 0.9 && fullness_rising_edge {
                fullness_rising_edge = false;
            }
            // Busy wait until we get data. This will peg the CPU at 100%, but that's ok
            // we don't want to give the time to the kernel with yeild, as that has a 15ms penalty
            let payload = if let Ok(pl) = consumer.pop() {
                pl
            } else {
                continue;
            };
            // Unpack payload to spectra
            unpack(&payload, &mut pol_a, &mut pol_b, &mut payload_n);
            // Timestamp first one
            if first_payload {
                first_payload = false;
                // Each payload represents cc.candence timesteps after payload_start
                let payload_offset = (payload_n as f64 * cc.cadence as f64).seconds();
                let payload_epoch = payload_start + payload_offset;
                let timestamp_str = heimdall_timestamp(&payload_epoch);
                header.insert("UTC_START".to_owned(), timestamp_str);
                // Write the single header
                // Safety: All these header keys and values are valid
                unsafe { hc.push_header(&header).unwrap() };
            }
            // TODO: Push to a time-domain buffer that we might want to dump
            // Generate stokes for this sample and push to averaging window
            // This is a transpose operation because the average calculation needs the time axis
            // to be contiguous as that's what we're summing over
            add_stokes_avg(&mut avg, &pol_a, &pol_b, cc);
            avg_cnt += 1;
            // If we've filled the averaging window, move on to the next step
            if avg_cnt == cc.avgs {
                // Reset the counter
                avg_cnt = 0;
                // Zero the first and last 250 samples because aliasing
                (avg[0..=250]).fill(0.0);
                (avg[1797..=2047]).fill(0.0);
                // Write this block
                block.write_all(avg.as_byte_slice()).unwrap();
                // Send this average over to the TCP listener, we don't care if this errors
                let _ = tcp_sender.try_send(avg.clone());
                // Reset the averages
                avg.fill(0.0);
                stokes_cnt += 1;
                // If we've filled the window, commit it to PSRDADA
                if stokes_cnt == cc.samples {
                    debug!("Commiting window to PSRDADA");
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
