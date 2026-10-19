// ============================================================================
// Galaxy generation.
//
// One invocation per sampled system. Every component is drawn by INVERSE
// TRANSFORM where an exact inverse exists, and only the spiral-arm modulation
// falls back to rejection - so generation is a single pass with no iteration
// except a short bounded arm loop.
//
// The one that matters: an exponential disk's radial law is exactly a Gamma.
// Surface density S(R) ~ exp(-R/h) gives P(R) ~ R*exp(-R/h) dR, which IS
// Gamma(shape 2, scale h), and a Gamma(2,1) draw is just -ln(u1*u2). No
// Newton iteration on the CDF, no rejection, no bias. Two logs and done.
//
// Parameters follow Juric et al. (2008) for the disks and halo, Wegg &
// Gerhard (2013) for the bar, Reid et al. (2019) for the arm geometry, and
// Kroupa (2001) for the mass function. Every default is in galaxy.rs with
// its source.
// ============================================================================

struct Params {
    count:         u32,
    seed:          u32,
    arm_count:     u32,
    _pad0:         u32,

    // cumulative component fractions: thin | thick | bulge | halo (= 1.0)
    cf_thin:       f32,
    cf_thick:      f32,
    cf_bulge:      f32,
    cf_halo:       f32,

    thin_hr:       f32,
    thin_hz:       f32,
    thick_hr:      f32,
    thick_hz:      f32,

    bulge_x0:      f32,
    bulge_y0:      f32,
    bulge_z0:      f32,
    bar_angle:     f32,

    halo_n:        f32,
    halo_q:        f32,
    halo_rmin:     f32,
    halo_rmax:     f32,

    arm_pitch:     f32,
    arm_r0:        f32,
    arm_width:     f32,
    arm_contrast:  f32,

    disk_rmax:     f32,
    sun_r:         f32,
    metal_grad:    f32,
    _pad1:         f32,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read_write> pos: array<f32>;    // 3 per system
@group(0) @binding(2) var<storage, read_write> state: array<u32>;  // 1 per system

// ---------------------------------------------------------------- rng -------
// PCG: one multiply-xorshift round. Cheap, and decorrelates the low bits that
// a plain LCG leaves visibly structured - which in a star field reads as
// lattice banding along the arms.
var<private> rng_state: u32;

fn pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}
fn seed_rng(i: u32) { rng_state = pcg(i ^ (P.seed * 2654435761u)); }
fn next_u32() -> u32 { rng_state = pcg(rng_state); return rng_state; }

// half-open (0,1] - never returns 0, so log() below is always finite
fn rnd() -> f32 { return (f32(next_u32() >> 8u) + 1.0) * (1.0 / 16777216.0); }

fn rnd_normal() -> f32 {
    // Box-Muller; one of the pair is discarded, which is still cheaper than
    // carrying the spare through a shader with no persistent state
    let u1 = rnd();
    let u2 = rnd();
    return sqrt(-2.0 * log(u1)) * cos(6.28318530718 * u2);
}

// ------------------------------------------------------------ spiral --------
// Logarithmic arms r = r0 * exp(k*(theta - theta_i)), k = tan(pitch). Inverted
// for a known R this gives the arm's angle directly, so the distance to the
// nearest arm is one wrap per arm rather than a search along the curve.
fn arm_weight(r: f32, theta: f32) -> f32 {
    if (r < 0.2 || P.arm_count == 0u || P.arm_contrast <= 0.0) { return 0.0; }
    let k = tan(P.arm_pitch);
    let base = log(r / P.arm_r0) / k;
    let two_pi = 6.28318530718;
    var best = 1e9;
    for (var i = 0u; i < P.arm_count; i = i + 1u) {
        let off = two_pi * f32(i) / f32(P.arm_count);
        var d = theta - (base + off);
        d = d - two_pi * round(d / two_pi);        // wrap to [-pi, pi]
        let s = abs(d) * r;                        // arc length to the centreline
        best = min(best, s);
    }
    let w = exp(-0.5 * (best / P.arm_width) * (best / P.arm_width));
    return w * P.arm_contrast;
}

// Exponential disk: R ~ Gamma(2, h) exactly, z ~ two-sided exponential.
fn sample_disk(hr: f32, hz: f32) -> vec3<f32> {
    var r = 0.0;
    var theta = 0.0;
    // arm modulation by rejection on theta; the radial law is untouched, so
    // arms redistribute stars in angle without altering the disk's profile
    for (var t = 0u; t < 12u; t = t + 1u) {
        r = -hr * log(rnd() * rnd());
        if (r > P.disk_rmax) { continue; }
        theta = 6.28318530718 * rnd();
        let w = arm_weight(r, theta);
        if (rnd() * (1.0 + P.arm_contrast) <= 1.0 + w) { break; }
    }
    r = min(r, P.disk_rmax);
    let z = -hz * log(rnd()) * select(-1.0, 1.0, rnd() > 0.5);
    return vec3<f32>(r * cos(theta), r * sin(theta), z);
}

// Boxy bar as a triaxial gaussian, long axis rotated to the bar angle.
// The true profile (Dwek G2) is boxier than a gaussian; for a colonisation
// substrate the difference is a slightly softer bar edge and nothing else.
fn sample_bulge() -> vec3<f32> {
    let x = rnd_normal() * P.bulge_x0;
    let y = rnd_normal() * P.bulge_y0;
    let z = rnd_normal() * P.bulge_z0;
    let c = cos(P.bar_angle);
    let s = sin(P.bar_angle);
    return vec3<f32>(x * c - y * s, x * s + y * c, z);
}

// Flattened power-law halo, rho ~ r^-n. Mass inside r goes as r^(3-n), which
// inverts in closed form between an inner core and an outer cutoff.
fn sample_halo() -> vec3<f32> {
    let e = 3.0 - P.halo_n;
    let a = pow(P.halo_rmin, e);
    let b = pow(P.halo_rmax, e);
    let r = pow(a + rnd() * (b - a), 1.0 / e);
    // uniform direction on the sphere
    let ct = 2.0 * rnd() - 1.0;
    let st = sqrt(max(0.0, 1.0 - ct * ct));
    let ph = 6.28318530718 * rnd();
    return vec3<f32>(r * st * cos(ph), r * st * sin(ph), r * ct * P.halo_q);
}

// ---------------------------------------------------------------- IMF -------
// Kroupa (2001) broken power law. Inverse transform on each segment, with the
// segment chosen by its integrated weight. Returns mass in solar units.
fn sample_kroupa() -> f32 {
    let m0 = 0.08; let m1 = 0.5; let m2 = 120.0;
    let a1 = 1.3;  let a2 = 2.3;
    // integral of m^-a over each segment
    let i1 = (pow(m1, 1.0 - a1) - pow(m0, 1.0 - a1)) / (1.0 - a1);
    // continuity: the high segment is scaled so the two meet at m1
    let k2 = pow(m1, a2 - a1);
    let i2 = k2 * (pow(m2, 1.0 - a2) - pow(m1, 1.0 - a2)) / (1.0 - a2);
    let u = rnd() * (i1 + i2);
    if (u < i1) {
        let e = 1.0 - a1;
        return pow(pow(m0, e) + u * e, 1.0 / e);
    }
    let e = 1.0 - a2;
    return pow(pow(m1, e) + (u - i1) * e / k2, 1.0 / e);
}

// Harvard class from mass, coarse main-sequence boundaries.
fn spectral_class(m: f32) -> u32 {
    if (m >= 16.0) { return 0u; }   // O
    if (m >=  2.1) { return 1u; }   // B
    if (m >=  1.4) { return 2u; }   // A
    if (m >=  1.04) { return 3u; }  // F
    if (m >=  0.8) { return 4u; }   // G
    if (m >=  0.45) { return 5u; }  // K
    return 6u;                      // M
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>,
        @builtin(num_workgroups) nwg: vec3<u32>) {
    // The dispatch is 2-D because one dimension caps at 65535 workgroups;
    // gid.x already spans nwg.x*256, so the row stride is exactly that.
    let i = gid.x + gid.y * nwg.x * 256u;
    if (i >= P.count) { return; }
    seed_rng(i);

    let u = rnd();
    var p: vec3<f32>;
    var comp: u32;
    if (u < P.cf_thin)       { p = sample_disk(P.thin_hr,  P.thin_hz);  comp = 0u; }
    else if (u < P.cf_thick) { p = sample_disk(P.thick_hr, P.thick_hz); comp = 1u; }
    else if (u < P.cf_bulge) { p = sample_bulge();                      comp = 2u; }
    else                     { p = sample_halo();                       comp = 3u; }

    let mass = sample_kroupa();
    let cls  = spectral_class(mass);

    // Metallicity falls outward and is low in the halo; planet occurrence
    // scales with it, so this is what gates habitability later. Normalised
    // to 1.0 at the solar radius in the thin disk.
    let rr = length(p.xy);
    var metal = exp(-P.metal_grad * (rr - P.sun_r));
    if (comp == 1u) { metal = metal * 0.45; }      // thick disk is older, poorer
    if (comp == 3u) { metal = metal * 0.05; }      // halo, poorer still
    if (comp == 2u) { metal = metal * 1.30; }      // bar/bulge, enriched
    metal = clamp(metal, 0.0, 2.0);

    // Habitability: long-lived star, enough metal to build planets. F/G/K
    // carry it; O/B die before biology finishes, M are flare-prone and
    // tidally locking. This is the gate a colonisation pass reads.
    var hab = clamp(metal / 1.4, 0.0, 1.0);
    if (cls <= 1u)      { hab = hab * 0.02; }
    else if (cls == 2u) { hab = hab * 0.25; }
    else if (cls == 6u) { hab = hab * 0.35; }
    let habq = u32(clamp(hab, 0.0, 1.0) * 1023.0);

    // packed: [0..2] component  [3..5] class  [6..15] habitability
    //         [16..31] reserved for colonisation state
    state[i] = (comp & 7u) | ((cls & 7u) << 3u) | ((habq & 1023u) << 6u);

    let o = i * 3u;
    pos[o]      = p.x;
    pos[o + 1u] = p.y;
    pos[o + 2u] = p.z;
}
