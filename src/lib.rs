// Size of FFT in the FPGA
pub const CHANNELS: usize = 2048;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Complex<T> {
    pub re: T,
    pub im: T,
}

impl<T> Complex<T> {
    pub fn new(re: T, im: T) -> Self {
        Self { re, im }
    }
}

/// The type of raw channel data out of the FPGA
pub type ComplexByte = Complex<i8>;
