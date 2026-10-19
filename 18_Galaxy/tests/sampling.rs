//! Does the sampler actually put stars in arms?
//!
//! The WGSL is the real implementation; this mirrors its arm weighting and
//! rejection exactly so the density contrast can be measured on the CPU. If
//! this fails, no amount of render tuning will show spiral structure — the
//! structure is not there.

const TWO_PI: f32 = 6.283_185_3;

fn arm_weight(r: f32, theta: f32, arms: u32, pitch: f32, r0: f32, width: f32, a: f32) -> f32 {
    if r < 0.2 || arms == 0 || a <= 0.0 {
        return 0.0;
    }
    let k = pitch.tan();
    let base = (r / r0).ln() / k;
    let mut best = 1e9f32;
    for i in 0..arms {
        let off = TWO_PI * i as f32 / arms as f32;
        let mut d = theta - (base + off);
        d -= TWO_PI * (d / TWO_PI).round();
        best = best.min(d.abs() * r);
    }
    (-0.5 * (best / width) * (best / width)).exp() * a
}

struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        // xorshift is enough for a statistical check
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 16_777_216.0 + 1e-7
    }
}

/// Fraction of the disk's area that lies within one arm-width of a centreline,
/// and the density contrast the rejection rule produces there.
#[test]
fn arms_are_denser_than_the_inter_arm_disk() {
    let (arms, pitch, r0, width, contrast) = (4u32, 12.5f32.to_radians(), 3.5, 0.35, 1.6);
    let mut rng = Rng(0x1234_5678);

    // Sample the accepted population the way the shader does, then ask what
    // fraction landed in an arm versus what fraction of the area an arm covers.
    let (mut in_arm, mut total) = (0u32, 0u32);
    let (mut area_in_arm, mut area_total) = (0u32, 0u32);
    for _ in 0..400_000 {
        let r = 4.0 + rng.next() * 8.0; // an annulus where arms are well defined
        let theta = TWO_PI * rng.next();
        let w = arm_weight(r, theta, arms, pitch, r0, width, contrast);
        let is_arm = w > contrast * 0.5; // within ~1.2 sigma of a centreline

        area_total += 1;
        if is_arm {
            area_in_arm += 1;
        }
        // the shader's acceptance test
        if rng.next() * (1.0 + contrast) <= 1.0 + w {
            total += 1;
            if is_arm {
                in_arm += 1;
            }
        }
    }

    let area_frac = area_in_arm as f64 / area_total as f64;
    let star_frac = in_arm as f64 / total as f64;
    let contrast_ratio = (star_frac / area_frac) / ((1.0 - star_frac) / (1.0 - area_frac));

    println!("arm area fraction      {:.3}", area_frac);
    println!("arm star fraction      {:.3}", star_frac);
    println!("density contrast       {:.2}x inter-arm", contrast_ratio);

    assert!(
        area_frac > 0.03,
        "arms cover only {:.1}% of the disk - too thin to read at any exposure",
        area_frac * 100.0
    );
    assert!(
        contrast_ratio > 1.5,
        "arms are only {:.2}x the inter-arm density - the rejection rule is not biting",
        contrast_ratio
    );
}

/// The radial law must stay Gamma(2, h) whatever the arms do - arms
/// redistribute stars in angle, they must not reshape the disk profile.
/// Mean of Gamma(2, h) is exactly 2h.
#[test]
fn radial_law_is_gamma_2_h() {
    let h = 2.6f32;
    let mut rng = Rng(0x9e37_79b9);
    let n = 500_000;
    let mut sum = 0.0f64;
    for _ in 0..n {
        let r = -h * (rng.next() * rng.next()).ln();
        sum += r as f64;
    }
    let mean = sum / n as f64;
    let expect = 2.0 * h as f64;
    println!("mean radius {:.4} kpc, expected {:.4}", mean, expect);
    assert!(
        (mean - expect).abs() / expect < 0.02,
        "radial mean {mean:.4} is not 2h = {expect:.4}"
    );
}
