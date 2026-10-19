//! Scale test on synthetic volumes: BVH build cost per million splats, trace
//! throughput at 1080p, GPU memory. Not a stand-in for a real capture -
//! splats here fill a volume instead of hugging surfaces - but it isolates
//! the RT cores' cost from the character of any one scan.
//!
//!   cargo run --release --example scale -- [millions ...]    default: 0.25 0.5

use glam::Vec3;
use splat_tracer::camera::Camera;
use splat_tracer::scene;
use splat_tracer::tracer::{Gpu, Proxy, Tracer};
use splat_tracer::safety::{self, Verdict};
use splat_tracer::RenderParams;

fn main() {
    let threads = splat_tracer::polite_threads();
    eprintln!("cpu threads {threads} (SPLAT_THREADS=n to change)");
    let mut counts: Vec<f64> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    if counts.is_empty() {
        // gentle by default: larger counts only when asked for on the command line
        counts = vec![0.25, 0.5];
    }
    let gpu = Gpu::headless().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    println!("gpu {}", gpu.name);
    println!(
        "{:>9} {:>10} {:>12} {:>12} {:>9} {:>10} {:>12}",
        "splats", "GPU MiB", "build ms", "ms / Msplat", "prep ms", "trace ms", "M rays/s"
    );
    let p = RenderParams::default();
    for m in counts {
        let n = (m * 1e6) as usize;
        let mut s = scene::synthetic(n, 1234, 1.0);
        // shrink so density per unit volume stays capture-like as n grows:
        // a splat's linear size falls with the cube root of the count
        let k = (200_000.0 / n as f32).cbrt();
        for sc in s.scale.iter_mut() {
            for v in sc.iter_mut() {
                *v *= k;
            }
        }
        let cam_at = |w: u32, h: u32| {
            Camera::look_at(Vec3::new(0.4, -0.5, -2.6), Vec3::ZERO, Vec3::NEG_Y, w, h, 55f32.to_radians())
        };
        // a synthetic volume froze this machine once: ramp to 1080p first
        let v = safety::ramp(
            &gpu, &s, Proxy::Box, &p, &cam_at, 1920, 1080, &safety::Limits::default(),
            &mut |_| {},
        );
        if let Verdict::Stopped { reason, .. } = v {
            println!("{:>9} STOPPED: {reason}", n);
            continue;
        }
        let mut tr = Tracer::new(&gpu);
        if let Err(e) = tr.set_scene(&gpu, &s, Proxy::Box) {
            println!("{:>9} {e}", n);
            continue;
        }
        let cam = cam_at(1920, 1080);
        let tm = tr.time(&gpu, &cam, &p, 3);
        let mib = tr.scene.as_ref().map(|g| g.bytes).unwrap_or(0) as f64 / (1u64 << 20) as f64;
        println!(
            "{:>9} {:>10.0} {:>12.1} {:>12.2} {:>9.2} {:>10.2} {:>12.0}",
            n,
            mib,
            tm.build_ms,
            tm.build_ms / m,
            tm.prep_ms,
            tm.trace_ms,
            1920.0 * 1080.0 / (tm.trace_ms * 1e-3) / 1e6
        );
    }
}
