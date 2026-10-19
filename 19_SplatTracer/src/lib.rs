//! № 19 — Splat Tracer.
//!
//! A path tracer specialised on Gaussian splats, on the RT cores. v001 is the
//! foundation everything else stands on: trace a scan exactly enough that it
//! reproduces what the scan looks like, and prove it with numbers before
//! adding light, shadow or reflection on top.

pub mod camera;
pub mod reference;
pub mod safety;
pub mod scene;
pub mod sh;
pub mod tracer;

/// Render settings shared by the GPU tracer and both CPU references, so a
/// comparison between them compares formulations and nothing else.
#[derive(Clone, Copy, Debug)]
pub struct RenderParams {
    /// Contributions below this are skipped: 1/255, as in 3DGS.
    pub alpha_min: f32,
    /// Stop when the next splat would push transmittance below this: 1e-4,
    /// the 3DGS rule, including its quirk that the splat which would cross the
    /// line is not composited at all.
    pub t_min: f32,
    /// Per-splat alpha is clamped to 0.99, as in 3DGS.
    pub alpha_max: f32,
    /// Screen-space variance added to every splat, in pixels squared. 3DGS
    /// adds 0.3 after projection and trains with it on, so its splats are
    /// allowed to be thinner than a pixel and still show up. Re-created here
    /// in 3D at each splat's depth; 0 gives the pure 3DGRT formulation.
    pub dilation_px2: f32,
    /// World-space standard deviation added isotropically to every splat -
    /// what Octane exposes as Covariance Bias.
    pub cov_bias: f32,
    /// Splat centres closer to the camera than this are culled, as in 3DGS.
    pub near: f32,
    /// Highest spherical-harmonic degree used, clamped to what the scene has.
    pub sh_degree: u32,
    /// Hit buffer size: how many splats one traversal collects before
    /// shading them in order and re-casting from the last.
    pub k: u32,
    pub background: [f32; 3],
    /// Hard ceiling on box reports one ray may process, across all its
    /// re-casts. A ray that hits it stops, keeps what it has composited, and
    /// is counted.
    ///
    /// In Box mode this bounds the ray's whole traversal: every box the ray
    /// overlaps is reported to the shader. In Oriented mode it does NOT - the
    /// ray first enters each instance's world-space bounds, and an entry that
    /// then misses the tilted box inside never reaches the shader, so that
    /// work is invisible here and uncounted. Oriented mode is guarded only by
    /// measured tile times (`safety::ramp`).
    pub max_reports: u32,
    /// Paint capped rays magenta instead of hiding them.
    pub flag_capped: bool,
}

impl Default for RenderParams {
    fn default() -> Self {
        Self {
            alpha_min: 1.0 / 255.0,
            t_min: 1.0e-4,
            alpha_max: 0.99,
            dilation_px2: 0.3,
            cov_bias: 0.0,
            near: 0.2,
            sh_degree: 3,
            k: 16,
            background: [0.0, 0.0, 0.0],
            // measured on a 5.8 M-splat outdoor scan: the p99 ray needs ~7 000,
            // one report costs ~0.25 us, so the worst ray stops within ~4 ms
            max_reports: 16_384,
            flag_capped: false,
        }
    }
}

/// Largest hit buffer a tracer can be compiled for. The buffer lives in
/// registers (spilling to local memory past a point), so capacity is a
/// compile-time choice; `RenderParams::k` picks a prefix of it at run time.
pub const K_MAX: u32 = 64;

/// Size the global CPU thread pool to half the machine, so a parse or a
/// reference render never takes the whole PC with it. `SPLAT_THREADS=n`
/// overrides - that is the way to say "use everything".
pub fn polite_threads() -> usize {
    let all = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let n = std::env::var("SPLAT_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or((all / 2).max(1))
        .clamp(1, all);
    let _ = rayon::ThreadPoolBuilder::new().num_threads(n).build_global();
    n
}
