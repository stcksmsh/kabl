//! Small `no_std` helpers (there is no `libm` in a module). Plain `f32` math, no tables.

/// Flushes denormals: below 1e-20 returns 0. Apply to recursive state once per block.
pub fn flush(x: f32) -> f32 {
    if x.abs() < 1e-20 {
        0.0
    } else {
        x
    }
}

/// 2^x for |x| <= 8 (clamped; degree-7 Taylor on the fraction).
pub fn exp2(x: f32) -> f32 {
    let x = x.clamp(-8.0, 8.0);
    let t = x as i32;
    let i = (if (t as f32) > x { t - 1 } else { t }) as f32;
    let f = (x - i) * core::f32::consts::LN_2;
    let p = 1.0
        + f * (1.0
            + f * (0.5
                + f * (1.0 / 6.0
                    + f * (1.0 / 24.0 + f * (1.0 / 120.0 + f * (1.0 / 720.0 + f / 5040.0))))));
    p * f32::from_bits(((i as i32 + 127) as u32) << 23)
}

/// `(sin x, cos x)` for 0 <= x <= pi (Taylor, error under 1e-4).
pub fn sin_cos(x: f32) -> (f32, f32) {
    let x2 = x * x;
    let s = x
        * (1.0
            - x2 / 6.0
                * (1.0
                    - x2 / 20.0
                        * (1.0
                            - x2 / 42.0
                                * (1.0 - x2 / 72.0 * (1.0 - x2 / 110.0 * (1.0 - x2 / 156.0))))));
    let c = 1.0
        - x2 / 2.0
            * (1.0
                - x2 / 12.0
                    * (1.0
                        - x2 / 30.0 * (1.0 - x2 / 56.0 * (1.0 - x2 / 90.0 * (1.0 - x2 / 132.0)))));
    (s, c)
}
