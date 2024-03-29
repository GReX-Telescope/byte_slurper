//! Argument parsing for running from the command line

use std::net::SocketAddr;

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Hexadecimal (sans leading 0x) PSRDADA key to create as source for heimdall.
    /// If not set, output will be written to filterbank files.
    #[clap(short, long, value_parser = valid_dada_key)]
    pub key: Option<i32>,
    /// Network device to capture packets from (MTU must be set to 9000)
    #[clap(short, long)]
    pub device_name: String,
    /// The ip and socket address of the SNAP board
    #[clap(long)]
    pub fpga_addr: SocketAddr,
    /// Port to capture UDP data from
    #[clap(short, long, default_value_t = 60000)]
    #[clap(value_parser = clap::value_parser!(u16).range(1..))]
    pub port: u16,
    /// Port to send TCP average spectra to
    #[clap(short, long, default_value_t = 4242)]
    #[clap(value_parser = clap::value_parser!(u16).range(1..))]
    pub listen_port: u16,
    /// Ring buffer capacity
    #[clap(long, default_value_t = 16384)]
    pub capacity: usize,
    /// Number of channels
    #[clap(long, default_value_t = 2048)]
    pub channels: usize,
    /// Number of samples
    #[clap(long, default_value_t = 65536)]
    pub samples: usize,
    /// How many samples to average
    #[clap(long, default_value_t = 4)]
    pub avgs: usize,
    /// The cadence (in seconds) we expect the packets to arrive at
    #[clap(long, default_value_t = 8.192e-6)]
    pub cadence: f32,
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,
}

/// Match verbosity filter with tracing subscriber log levels
pub fn convert_filter(filter: log::LevelFilter) -> tracing_subscriber::filter::LevelFilter {
    match filter {
        log::LevelFilter::Off => tracing_subscriber::filter::LevelFilter::OFF,
        log::LevelFilter::Error => tracing_subscriber::filter::LevelFilter::ERROR,
        log::LevelFilter::Warn => tracing_subscriber::filter::LevelFilter::WARN,
        log::LevelFilter::Info => tracing_subscriber::filter::LevelFilter::INFO,
        log::LevelFilter::Debug => tracing_subscriber::filter::LevelFilter::DEBUG,
        log::LevelFilter::Trace => tracing_subscriber::filter::LevelFilter::TRACE,
    }
}

fn valid_dada_key(s: &str) -> Result<i32, String> {
    i32::from_str_radix(s, 16).map_err(|_| "Invalid hex litteral".to_string())
}
