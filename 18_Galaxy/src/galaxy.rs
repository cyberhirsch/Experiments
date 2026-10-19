//! Milky Way structural model and the memory arithmetic that bounds it.
//!
//! Every default below carries its source. The PRD asks for a bibliography
//! per parameter; this module is it, and the UI reads these strings straight
//! out so a value on screen can always be traced.

use bytemuck::{Pod, Zeroable};

/// Uniform block for `generate.wgsl`. 28 scalars = 112 bytes, a multiple of
/// 16, and no vector members - so std140 padding cannot silently shift a
/// field out from under the shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GenParams {
    pub count: u32,
    pub seed: u32,
    pub arm_count: u32,
    pub _pad0: u32,

    // cumulative, so the shader picks a component with one compare chain
    pub cf_thin: f32,
    pub cf_thick: f32,
    pub cf_bulge: f32,
    pub cf_halo: f32,

    pub thin_hr: f32,
    pub thin_hz: f32,
    pub thick_hr: f32,
    pub thick_hz: f32,

    pub bulge_x0: f32,
    pub bulge_y0: f32,
    pub bulge_z0: f32,
    pub bar_angle: f32,

    pub halo_n: f32,
    pub halo_q: f32,
    pub halo_rmin: f32,
    pub halo_rmax: f32,

    pub arm_pitch: f32,
    pub arm_r0: f32,
    pub arm_width: f32,
    pub arm_contrast: f32,

    pub disk_rmax: f32,
    pub sun_r: f32,
    pub metal_grad: f32,
    pub _pad1: f32,
}

/// Editable model, in physical units (kpc, radians, dimensionless).
#[derive(Clone, Copy, Debug)]
pub struct Galaxy {
    pub count: u32,
    pub seed: u32,

    /// Stellar mass fractions. Licquia & Newman (2015) put the disk at
    /// 4.4e10 Msun and the bulge near 0.9e10; the halo is ~1e9. These are
    /// counts of sampled systems, not mass, so they run slightly flatter
    /// than the mass ratios - the bulge is old and low-mass-dominated.
    pub frac_thin: f32,
    pub frac_thick: f32,
    pub frac_bulge: f32,
    pub frac_halo: f32,

    /// Juric et al. (2008), SDSS star counts: thin disk scale length 2.6 kpc,
    /// scale height 300 pc; thick disk 3.6 kpc and 900 pc.
    pub thin_hr: f32,
    pub thin_hz: f32,
    pub thick_hr: f32,
    pub thick_hz: f32,

    /// Wegg & Gerhard (2013) bar axis ratios, and a bar angle of 27 deg to
    /// the Sun-centre line.
    pub bulge_x0: f32,
    pub bulge_y0: f32,
    pub bulge_z0: f32,
    pub bar_angle_deg: f32,

    /// Juric et al. (2008): halo power law index 2.77, flattening q = 0.64.
    pub halo_n: f32,
    pub halo_q: f32,
    pub halo_rmin: f32,
    pub halo_rmax: f32,

    /// Reid et al. (2019) VLBI parallaxes give arm pitch angles of roughly
    /// 9-14 deg; 12.5 is a reasonable single value for a four-arm model.
    pub arm_count: u32,
    pub arm_pitch_deg: f32,
    pub arm_r0: f32,
    pub arm_width: f32,
    pub arm_contrast: f32,

    pub disk_rmax: f32,
    /// GRAVITY Collaboration (2019): R0 = 8.178 kpc. The PRD says 8.2.
    pub sun_r: f32,
    /// Radial metallicity gradient, roughly -0.06 dex/kpc, expressed here as
    /// the exponential rate that reproduces it.
    pub metal_grad: f32,
}

impl Default for Galaxy {
    fn default() -> Self {
        Self {
            count: 20_000_000,
            seed: 1,

            frac_thin: 0.65,
            frac_thick: 0.12,
            frac_bulge: 0.21,
            frac_halo: 0.02,

            thin_hr: 2.6,
            thin_hz: 0.30,
            thick_hr: 3.6,
            thick_hz: 0.90,

            bulge_x0: 0.90,
            bulge_y0: 0.38,
            bulge_z0: 0.26,
            bar_angle_deg: 27.0,

            halo_n: 2.77,
            halo_q: 0.64,
            halo_rmin: 1.0,
            halo_rmax: 100.0,

            arm_count: 4,
            arm_pitch_deg: 12.5,
            arm_r0: 3.5,
            arm_width: 0.35,
            arm_contrast: 1.6,

            disk_rmax: 20.0,
            sun_r: 8.2,
            metal_grad: 0.14,
        }
    }
}

impl Galaxy {
    pub fn to_gpu(&self) -> GenParams {
        // normalise, then accumulate - a fraction set that does not sum to 1
        // would otherwise quietly starve the halo
        let s = self.frac_thin + self.frac_thick + self.frac_bulge + self.frac_halo;
        let s = if s > 0.0 { s } else { 1.0 };
        let t = self.frac_thin / s;
        let k = self.frac_thick / s;
        let b = self.frac_bulge / s;

        GenParams {
            count: self.count,
            seed: self.seed,
            arm_count: self.arm_count,   // 0 disables the arms entirely
            _pad0: 0,

            cf_thin: t,
            cf_thick: t + k,
            cf_bulge: t + k + b,
            cf_halo: 1.0,

            thin_hr: self.thin_hr,
            thin_hz: self.thin_hz,
            thick_hr: self.thick_hr,
            thick_hz: self.thick_hz,

            bulge_x0: self.bulge_x0,
            bulge_y0: self.bulge_y0,
            bulge_z0: self.bulge_z0,
            bar_angle: self.bar_angle_deg.to_radians(),

            halo_n: self.halo_n,
            halo_q: self.halo_q,
            halo_rmin: self.halo_rmin.max(0.01),
            halo_rmax: self.halo_rmax.max(self.halo_rmin + 0.1),

            arm_pitch: self.arm_pitch_deg.to_radians(),
            arm_r0: self.arm_r0.max(0.1),
            arm_width: self.arm_width.max(0.01),
            arm_contrast: self.arm_contrast,

            disk_rmax: self.disk_rmax,
            sun_r: self.sun_r,
            metal_grad: self.metal_grad,
            _pad1: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Memory arithmetic.
//
// The PRD budgets 16 bytes per system - position f32x3 plus a packed u32 -
// and concludes 1e9 systems fit a 24 GB card. That is right for the substrate
// and wrong for the simulation, because Section 6 also requires a per-system
// next_event_time and Section 8 a spatial hash over the same points, neither
// of which is in the 16. Those are counted here so the shortfall shows up on
// screen instead of at 3 a.m. three months in.
// ---------------------------------------------------------------------------

pub const BYTES_POS: u64 = 12; // f32 x 3
pub const BYTES_STATE: u64 = 4; // packed u32
/// Event times are stored as u32 DECADES, not f64 years. A 10^10 yr run needs
/// 10^9 decades; u32 reaches 4.29e9, so a decade tick covers the full span
/// with four times the headroom and half the footprint of an f64 - and 10 yr
/// resolution is far finer than the 10^2-10^4 yr hop the fast clock needs.
pub const BYTES_EVENT: u64 = 4;

pub struct Budget {
    pub systems: u64,
    pub substrate: u64,
    pub events: u64,
    pub hash: u64,
    pub total: u64,
}

impl Budget {
    pub fn for_count(n: u64) -> Self {
        let substrate = n * (BYTES_POS + BYTES_STATE);
        let events = n * BYTES_EVENT;
        // Sparse spatial hash: keep the system arrays sorted by cell so no
        // per-point index array is needed, leaving only the cell table. Sized
        // at two entries per occupied cell, 8 bytes each, assuming ~32 systems
        // per cell at the neighbour-search radius.
        let cells = (n / 32).max(1);
        let hash = cells * 2 * 8;
        Self { systems: n, substrate, events, hash, total: substrate + events + hash }
    }
    pub fn gib(v: u64) -> f64 {
        v as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}
