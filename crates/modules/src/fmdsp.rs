//! Shared oversampling pieces of the FM modules: the decimation filter and the interpolation
//! weights. Built once in `new`, never on the audio thread.

/// Kaiser-windowed sinc low-pass at the base-rate Nyquist, `N` taps for a `factor`x internal
/// rate (33 taps at 2x, 65 at 4x: 17.5 kHz passband and 30.5 kHz stop band at 48 kHz, over
/// 70 dB down, unity gain at DC). The group delay is 8 base-rate samples either way.
pub fn lowpass<const N: usize>(factor: usize) -> [f32; N] {
    fn bessel_i0(x: f64) -> f64 {
        let (mut sum, mut term) = (1.0, 1.0);
        for k in 1..40 {
            term *= (x / (2.0 * k as f64)).powi(2);
            sum += term;
        }
        sum
    }
    let cutoff = 0.5 / factor as f64;
    let mid = (N / 2) as f64;
    let mut h = [0.0f64; N];
    for (i, v) in h.iter_mut().enumerate() {
        let x = i as f64 - mid;
        let sinc = if x == 0.0 {
            2.0 * cutoff
        } else {
            (2.0 * std::f64::consts::PI * cutoff * x).sin() / (std::f64::consts::PI * x)
        };
        let r = x / mid;
        *v = sinc * bessel_i0(7.0 * (1.0 - r * r).max(0.0).sqrt()) / bessel_i0(7.0);
    }
    let total: f64 = h.iter().sum();
    h.map(|v| (v / total) as f32)
}

/// 6-point Lagrange weights for the four quarter points (t = 1/4, 1/2, 3/4, 1) of the interval
/// between the third and fourth of six consecutive samples (oldest first).
pub fn weights4() -> [[f32; 6]; 4] {
    let nodes = [-2.0f64, -1.0, 0.0, 1.0, 2.0, 3.0];
    let mut w = [[0.0f32; 6]; 4];
    for (k, row) in w.iter_mut().enumerate() {
        let t = (k + 1) as f64 / 4.0;
        for (j, cell) in row.iter_mut().enumerate() {
            let mut l = 1.0;
            for (m, &pm) in nodes.iter().enumerate() {
                if m != j {
                    l *= (t - pm) / (nodes[j] - pm);
                }
            }
            *cell = l as f32;
        }
    }
    w
}
