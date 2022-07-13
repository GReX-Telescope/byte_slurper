use num_complex::Complex;

pub const PAYLOAD_SIZE: usize = 8192;
pub const WORD_SIZE: usize = 8;
pub const CHANNELS: usize = 2048;

pub type ComplexByte = Complex<u8>;

pub fn total_power_spectra<const N: usize>(
    pol_a: &[ComplexByte; N],
    pol_b: &[ComplexByte; N],
) -> [f32; N] {
    let mut spectra = [0f32; N];
    for i in 0..N {
        let pol_a_float = Complex::new(pol_a[i].re as f32 / 255_f32, pol_a[i].im as f32 / 255_f32);
        let pol_b_float = Complex::new(pol_b[i].re as f32 / 255_f32, pol_b[i].im as f32 / 255_f32);
        spectra[i] = pol_a_float.norm() + pol_b_float.norm();
    }
    spectra
}

pub fn payload_to_spectra(
    payload: &[u8; PAYLOAD_SIZE],
    pol_a: &mut [ComplexByte; CHANNELS],
    pol_b: &mut [ComplexByte; CHANNELS],
) {
    assert_eq!(PAYLOAD_SIZE, CHANNELS * 4);
    for (i, word) in payload.chunks_exact(WORD_SIZE).enumerate() {
        // Each word contains two frequencies for each polarization
        // [A1 B1 A2 B2]
        // Where each channel is [Re Im]
        let a1 = ComplexByte {
            re: word[7],
            im: word[6],
        };
        let a2 = ComplexByte {
            re: word[5],
            im: word[4],
        };
        let b1 = ComplexByte {
            re: word[3],
            im: word[2],
        };
        let b2 = ComplexByte {
            re: word[1],
            im: word[0],
        };
        // Update spectra
        pol_a[2 * i] = a1;
        pol_a[2 * i + 1] = a2;
        pol_b[2 * i] = b1;
        pol_b[2 * i + 1] = b2;
    }
}