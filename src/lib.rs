pub mod args;
pub mod capture;
pub mod complex;
pub mod exfil;
pub mod monitoring;

#[derive(Debug, Copy, Clone)]
/// Contains all the state for how to shape the data we're capturing
pub struct CaptureConfig {
    /// Number of frequency channels
    pub channels: usize,
    /// Number of samples to exfil
    pub samples: usize,
    /// Samples per average (downsampling)
    pub avgs: usize,
    /// Seconds per packet
    pub cadence: f32,
}

impl CaptureConfig {
    /// The size of the output buffer window in samples (not bytes)
    pub fn window_size(&self) -> usize {
        self.channels * self.samples
    }
    /// The sample time after averaging
    pub fn tsamp(&self) -> f32 {
        self.cadence * self.avgs as f32
    }
}
