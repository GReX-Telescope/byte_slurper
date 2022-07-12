use rustfft::{algorithm::Radix4, num_complex::Complex, num_traits::FromPrimitive, Fft};
// For each RXed payload from UDP, we will have 16384 samples at 2ns

// To create a stream of channels, we need to
// * multiply with window function
// * Fold by (N) taps
// * FFT
//
// As the output is 2048 and our input in 16384, we'll choose 8 taps

pub(crate) fn channelize<T, const N: usize>(input: &[u8], fft: &Radix4<T>) -> [Complex<T>; N]
where
    T: rustfft::FftNum
        + std::default::Default
        + std::ops::AddAssign
        + std::ops::SubAssign
        + std::ops::MulAssign
        + std::ops::DivAssign
        + std::ops::RemAssign,
{
    assert_eq!(
        input.len() % N,
        0,
        "Output size must divde evenly input size"
    );
    let taps = input.len() / N;
    // Multiply time series by window function
    // TODO!
    // Fold
    let mut folded = [Default::default(); N];
    // for chunk in input.chunks_exact(taps) {
    //     for i in 0..chunk.len() {
    //         folded[i] += Complex::<T>::from_u8(chunk[i]).unwrap();
    //     }
    // }
    // FFT
    fft.process(&mut folded);
    // Return
    folded
}
