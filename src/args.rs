//! Argument parsing for running from the command line

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Hexadecimal (sans leading 0x) PSRDADA key to create as source for heimdall
    #[clap(short, long, value_parser = valid_dada_key)]
    pub key: i32,
    /// Network device to capture packets from (MTU must be set to 9000)
    #[clap(short, long)]
    pub device_name: String,
    /// Port to capture UDP data from
    #[clap(short, long, default_value_t = 60000)]
    #[clap(value_parser = clap::value_parser!(u16).range(1..))]
    pub port: u16,
    /// Ring buffer capacity
    #[clap(short, long, default_value_t = 256)]
    pub capacity: usize,
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
