use num_complex::Complex;

pub const PAYLOAD_SIZE: usize = 8192;
pub const WORD_SIZE: usize = 8;
pub const CHANNELS: usize = 2048;

pub type ComplexByte = Complex<i8>;

pub fn stokes_i<const N: usize>(
    pol_x: &[ComplexByte; N],
    pol_y: &[ComplexByte; N],
    output: &mut [f32; N],
) {
    for i in 0..N {
        let pol_x_cast = Complex::new(pol_x[i].re as f32 / 127f32, pol_x[i].im as f32 / 127f32);
        let pol_y_cast = Complex::new(pol_y[i].re as f32 / 127f32, pol_y[i].im as f32 / 127f32);
        output[i] = pol_x_cast.norm_sqr() + pol_y_cast.norm_sqr();
    }
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
            re: word[7] as i8,
            im: word[6] as i8,
        };
        let a2 = ComplexByte {
            re: word[5] as i8,
            im: word[4] as i8,
        };
        let b1 = ComplexByte {
            re: word[3] as i8,
            im: word[2] as i8,
        };
        let b2 = ComplexByte {
            re: word[1] as i8,
            im: word[0] as i8,
        };
        // Update spectra
        pol_a[2 * i] = a1;
        pol_a[2 * i + 1] = a2;
        pol_b[2 * i] = b1;
        pol_b[2 * i + 1] = b2;
    }
}

pub fn vsum_mut<const N: usize>(a: &[f32; N], b: &mut [f32; N], n: u32) {
    for i in 0..N {
        b[i] += a[i] / n as f32
    }
}
