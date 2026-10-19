//! Guard rails for running on a desktop GPU that also draws the desktop.
//!
//! A GPU job cannot be interrupted once submitted. If one runs past the OS
//! watchdog (~2 s on Windows) the driver is reset - and when a ray-tracing
//! job is what hung, that reset can fail and take the machine with it. It
//! did, twice, while this renderer was being built: once on a synthetic
//! volume benchmark, once on the first full-scale run of a proxy mode that
//! had only ever seen 600 splats.
//!
//! So: nothing new goes straight to full scale. `ramp` grows a run in steps -
//! a deterministic subset of the splats first, then the image - timing every
//! GPU job, extrapolating the next step before taking it, and stopping with a
//! reason instead of finding out the hard way.
//!
//! For the Oriented proxy this is the ONLY guard: the shader's per-ray report
//! cap cannot see instance entries that miss (see `RenderParams::max_reports`).
//! The margin is in the limits: 64-px tiles are 1/16 of the job that crashed,
//! and a 100 ms tile limit sits 20x under the ~2 s watchdog, so the work would
//! have to jump twentyfold between two steps to reach it.

use crate::camera::Camera;
use crate::scene::Splats;
use crate::sh;
use crate::tracer::{FrameUniform, Gpu, Proxy, Tracer};
use crate::RenderParams;

/// Every n-th splat: deterministic, and it keeps the scene's overall shape.
pub fn subset(s: &Splats, n: usize) -> Splats {
    let total = s.len();
    if n >= total {
        return s.clone();
    }
    let st = s.sh_stride();
    let step = total as f64 / n as f64;
    let idx: Vec<usize> = (0..n).map(|k| ((k as f64 * step) as usize).min(total - 1)).collect();
    Splats {
        pos: idx.iter().map(|&i| s.pos[i]).collect(),
        scale: idx.iter().map(|&i| s.scale[i]).collect(),
        rot: idx.iter().map(|&i| s.rot[i]).collect(),
        opacity: idx.iter().map(|&i| s.opacity[i]).collect(),
        sh: idx.iter().flat_map(|&i| s.sh[i * st..(i + 1) * st].iter().copied()).collect(),
        sh_degree: s.sh_degree,
        sanitized: s.sanitized,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Longest any single traced tile may take. Well under the OS watchdog,
    /// and short enough that the desktop never visibly stalls.
    pub tile_ms: f64,
    /// Longest the BVH build may take on the GPU (submit to done) - it is one
    /// job and cannot be tiled. CPU-side instance preparation is not counted:
    /// it cannot stall the desktop's GPU.
    pub build_ms: f64,
    pub start_splats: usize,
    pub start_width: u32,
    /// Tile edge used while ramping.
    pub tile: u32,
    /// Growth per splat step.
    pub factor: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { tile_ms: 100.0, build_ms: 500.0, start_splats: 50_000, start_width: 160, tile: 64, factor: 4 }
    }
}

#[derive(Clone, Debug)]
pub struct Step {
    pub splats: usize,
    pub width: u32,
    pub height: u32,
    /// wall clock, CPU instance preparation included
    pub build_ms: f64,
    /// the GPU job alone - what the limit applies to
    pub build_gpu_ms: f64,
    pub max_tile_ms: f64,
    pub frame_ms: f64,
    pub capped: u32,
}

pub enum Verdict {
    /// Every step stayed inside the limits, up to the requested size.
    Safe(Vec<Step>),
    /// Stopped before a step that measured - or was predicted - over a limit.
    Stopped { steps: Vec<Step>, reason: String },
}

/// Grow a run from `limits.start_splats` at `limits.start_width` up to the
/// full scene at `target_w` x `target_h`, one measured step at a time.
/// `cam` maps an image size to the camera to use at that size.
pub fn ramp(
    gpu: &Gpu,
    s: &Splats,
    proxy: Proxy,
    p: &RenderParams,
    cam: &dyn Fn(u32, u32) -> Camera,
    target_w: u32,
    target_h: u32,
    lim: &Limits,
    log: &mut dyn FnMut(&Step),
) -> Verdict {
    let full = s.len();
    let aspect = target_h as f64 / target_w.max(1) as f64;
    let size = |w: u32| (w, ((w as f64 * aspect).round() as u32).max(8));
    let mut steps: Vec<Step> = Vec::new();
    let mut n = lim.start_splats.min(full).max(1);
    let (mut w, mut h) = size(lim.start_width.min(target_w));

    let measure = |tr: &mut Tracer, sc: &Splats, w: u32, h: u32| -> Step {
        let c = cam(w, h);
        tr.resize(gpu, w, h);
        tr.tile = lim.tile;
        let (build_ms, build_gpu_ms) = match tr.ensure_geometry(gpu, &c, p) {
            Some(ms) => (ms, tr.scene.as_ref().map(|s| s.last_build_gpu_ms).unwrap_or(ms)),
            None => (0.0, 0.0),
        };
        let base = FrameUniform::new(&c, p, sc.len() as u32, sc.sh_degree, 0, false, false);
        // Untimed warm-up first: the first dispatch after a build pays one-time
        // costs that would otherwise be extrapolated into a false stop. It is
        // itself within the limits the previous step predicted, and uses the
        // waiting variant so no more than one tile is ever queued.
        let _ = tr.trace_sample_timed(gpu, &base, true);
        let times = tr.trace_sample_timed(gpu, &base, true);
        Step {
            splats: sc.len(),
            width: w,
            height: h,
            build_ms,
            build_gpu_ms,
            max_tile_ms: times.iter().cloned().fold(0.0, f64::max),
            frame_ms: times.iter().sum(),
            capped: tr.read_capped(gpu),
        }
    };

    // 1. splats, at the small image
    loop {
        let owned;
        let sc: &Splats = if n < full {
            owned = subset(s, n);
            &owned
        } else {
            s
        };
        let mut tr = Tracer::new(gpu);
        if let Err(e) = tr.set_scene(gpu, sc, proxy) {
            return Verdict::Stopped { steps, reason: e };
        }
        let st = measure(&mut tr, sc, w, h);
        log(&st);
        let over = if st.build_gpu_ms > lim.build_ms {
            Some(format!(
                "BVH build took {:.0} ms on the GPU at {} splats (limit {:.0})",
                st.build_gpu_ms, n, lim.build_ms
            ))
        } else if st.max_tile_ms > lim.tile_ms {
            Some(format!("a tile took {:.0} ms at {} splats (limit {:.0})", st.max_tile_ms, n, lim.tile_ms))
        } else {
            None
        };
        let (build, tile) = (st.build_gpu_ms, st.max_tile_ms);
        steps.push(st);
        if let Some(reason) = over {
            return Verdict::Stopped { steps, reason };
        }
        if n >= full {
            break;
        }
        // extrapolate before stepping: linear in splat count is the
        // conservative guess for both the build and the per-ray work
        let next = (n * lim.factor).min(full);
        let f = next as f64 / n as f64;
        if build * f > lim.build_ms {
            return Verdict::Stopped {
                steps,
                reason: format!(
                    "stopped before {next} splats: GPU build predicted at {:.0} ms (limit {:.0})",
                    build * f,
                    lim.build_ms
                ),
            };
        }
        if tile * f > lim.tile_ms {
            return Verdict::Stopped {
                steps,
                reason: format!(
                    "stopped before {next} splats: slowest tile predicted at {:.0} ms (limit {:.0})",
                    tile * f,
                    lim.tile_ms
                ),
            };
        }
        n = next;
    }

    // 2. the image, at full splat count. Tiles stay the same size, so this
    //    mostly checks that no region of the frame is far costlier than the
    //    part the small image happened to sample.
    if w < target_w {
        let mut tr = Tracer::new(gpu);
        if let Err(e) = tr.set_scene(gpu, s, proxy) {
            return Verdict::Stopped { steps, reason: e };
        }
        while w < target_w {
            (w, h) = size((w * 2).min(target_w));
            let st = measure(&mut tr, s, w, h);
            log(&st);
            let bad = st.max_tile_ms > lim.tile_ms;
            let t = st.max_tile_ms;
            steps.push(st);
            if bad {
                return Verdict::Stopped {
                    steps,
                    reason: format!("a tile took {t:.0} ms at {w}x{h} (limit {:.0})", lim.tile_ms),
                };
            }
        }
    }
    Verdict::Safe(steps)
}

/// One formatted table row.
pub fn row(st: &Step) -> String {
    format!(
        "{:>10} {:>11} {:>10.1} {:>10.1} {:>13.2} {:>11.1} {:>9}",
        st.splats,
        format!("{}x{}", st.width, st.height),
        st.build_ms,
        st.build_gpu_ms,
        st.max_tile_ms,
        st.frame_ms,
        st.capped
    )
}

pub fn header() -> String {
    format!(
        "{:>10} {:>11} {:>10} {:>10} {:>13} {:>11} {:>9}",
        "splats", "image", "build ms", "gpu ms", "worst tile ms", "frame ms", "capped"
    )
}

/// Floats of SH per splat - re-exported for callers building subsets.
pub fn sh_stride(degree: u32) -> usize {
    sh::stride(degree)
}
