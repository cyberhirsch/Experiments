//! Spherical-harmonic colour, transcribed from the 3DGS reference
//! (`computeColorFromSH` in its forward pass) so that a splat's colour here
//! is the colour it was trained to have.
//!
//! Coefficients are stored coefficient-major: `sh[k * 3 + c]` is coefficient
//! `k` (0..16) of channel `c`. The GPU shader reads the same layout.

pub const C0: f32 = 0.282_094_79;
pub const C1: f32 = 0.488_602_5;
pub const C2: [f32; 5] = [
    1.092_548_4,
    -1.092_548_4,
    0.315_391_57,
    -1.092_548_4,
    0.546_274_2,
];
pub const C3: [f32; 7] = [
    -0.590_043_6,
    2.890_611_4,
    -0.457_045_8,
    0.373_176_34,
    -0.457_045_8,
    1.445_305_7,
    -0.590_043_6,
];

/// Coefficients per splat, all channels, degree 3.
pub const COEFFS: usize = 16;
pub const FLOATS: usize = COEFFS * 3;

/// Floats per splat actually stored for a given degree: (d+1)^2 coefficients
/// times three channels. A degree-0 scan needs 3, not 48 - for six million
/// splats that is 70 MB instead of 1.1 GB.
pub const fn stride(degree: u32) -> usize {
    let c = (degree as usize + 1) * (degree as usize + 1);
    c * 3
}

#[inline]
fn c(sh: &[f32], k: usize) -> [f32; 3] {
    [sh[k * 3], sh[k * 3 + 1], sh[k * 3 + 2]]
}

#[inline]
fn madd(acc: &mut [f32; 3], w: f32, v: [f32; 3]) {
    acc[0] += w * v[0];
    acc[1] += w * v[1];
    acc[2] += w * v[2];
}

/// Colour of one splat seen along `dir` (unit, from camera to splat centre).
/// 3DGS adds 0.5 and clamps at zero from below only.
pub fn eval(deg: u32, sh: &[f32], dir: [f32; 3]) -> [f32; 3] {
    let mut r = [0.0f32; 3];
    madd(&mut r, C0, c(sh, 0));
    if deg > 0 {
        let [x, y, z] = dir;
        madd(&mut r, -C1 * y, c(sh, 1));
        madd(&mut r, C1 * z, c(sh, 2));
        madd(&mut r, -C1 * x, c(sh, 3));
        if deg > 1 {
            let (xx, yy, zz) = (x * x, y * y, z * z);
            let (xy, yz, xz) = (x * y, y * z, x * z);
            madd(&mut r, C2[0] * xy, c(sh, 4));
            madd(&mut r, C2[1] * yz, c(sh, 5));
            madd(&mut r, C2[2] * (2.0 * zz - xx - yy), c(sh, 6));
            madd(&mut r, C2[3] * xz, c(sh, 7));
            madd(&mut r, C2[4] * (xx - yy), c(sh, 8));
            if deg > 2 {
                madd(&mut r, C3[0] * y * (3.0 * xx - yy), c(sh, 9));
                madd(&mut r, C3[1] * xy * z, c(sh, 10));
                madd(&mut r, C3[2] * y * (4.0 * zz - xx - yy), c(sh, 11));
                madd(&mut r, C3[3] * z * (2.0 * zz - 3.0 * xx - 3.0 * yy), c(sh, 12));
                madd(&mut r, C3[4] * x * (4.0 * zz - xx - yy), c(sh, 13));
                madd(&mut r, C3[5] * z * (xx - yy), c(sh, 14));
                madd(&mut r, C3[6] * x * (xx - 3.0 * yy), c(sh, 15));
            }
        }
    }
    [
        (r[0] + 0.5).max(0.0),
        (r[1] + 0.5).max(0.0),
        (r[2] + 0.5).max(0.0),
    ]
}
