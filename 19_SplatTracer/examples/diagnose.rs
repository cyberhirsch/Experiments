//! Where does a ray's time go? Per-pixel traversal counters from the GPU,
//! plus the shape of the scene's bounding boxes from the CPU.
//!
//!   cargo run --release --example diagnose -- <scene.ply> [--width 640]
//!
//! Light by design: one sample at a small size.

use glam::Vec3;
use splat_tracer::camera::Camera;
use splat_tracer::reference;
use splat_tracer::scene;
use splat_tracer::tracer::{FrameUniform, Gpu, Proxy, Tracer};
use splat_tracer::safety::{self, Verdict};
use splat_tracer::RenderParams;
use std::time::Instant;

fn pct(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * q) as usize]
}

fn main() {
    let threads = splat_tracer::polite_threads();
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("usage: diagnose <scene.ply> [--width 640]");
    let width: u32 = args
        .iter()
        .position(|a| a == "--width")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(640);
    eprintln!("cpu threads {threads}");
    let s = scene::load_ply(std::path::Path::new(path)).expect("load");
    println!(
        "sanitised   {} splats dropped (non-finite values), {} scales clamped",
        s.sanitized.dropped, s.sanitized.clamped
    );
    let (c, e) = s.robust_bounds();
    let c = Vec3::from(c);
    let extent = Vec3::from(e).max_element();
    let r = extent * 1.6;
    let h = width * 9 / 16;
    let cam = Camera::look_at(c + Vec3::new(r, -0.3 * r, 0.0), c, Vec3::NEG_Y, width, h, 50f32.to_radians());
    let kmax: u32 = args
        .iter()
        .position(|a| a == "--k")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(16);
    let p = RenderParams { k: kmax, ..RenderParams::default() };
    let gpu_only = args.iter().any(|a| a == "--gpu-only");
    // Box unless asked: it is the proxy measured safe at full scale
    let proxy = if args.iter().any(|a| a == "--oriented") { Proxy::Oriented } else { Proxy::Box };

    // ---- CPU: how big are the boxes the RT cores have to test? ----------
    let prep = if gpu_only { Vec::new() } else { reference::prepare(&s, &cam, &p) };
    let mut ext: Vec<f64> = prep
        .iter()
        .filter(|g| g.opacity > 0.0)
        .map(|g| {
            let a = g.aabb;
            ((a[3] - a[0]).max(a[4] - a[1]).max(a[5] - a[2]) / extent) as f64
        })
        .collect();
    let live = ext.len();
    if !gpu_only {
    let big10 = ext.iter().filter(|&&x| x > 0.10).count();
    let big50 = ext.iter().filter(|&&x| x > 0.50).count();
    let big100 = ext.iter().filter(|&&x| x > 1.00).count();
    println!("scene       {} splats, {} live for this camera, robust extent {:.2}", s.len(), live, extent);
    println!(
        "box size / scene extent   p50 {:.4}  p90 {:.4}  p99 {:.4}  p99.9 {:.4}  max {:.2}",
        pct(&mut ext, 0.5),
        pct(&mut ext, 0.9),
        pct(&mut ext, 0.99),
        pct(&mut ext, 0.999),
        pct(&mut ext, 1.0)
    );
    println!("boxes larger than 10% / 50% / 100% of the scene: {big10} / {big50} / {big100}");
    let mut op: Vec<f64> = s.opacity.iter().map(|&o| o as f64).collect();
    println!(
        "opacity     p10 {:.3}  p50 {:.3}  p90 {:.3}   below 1/255: {}",
        pct(&mut op, 0.1),
        pct(&mut op, 0.5),
        pct(&mut op, 0.9),
        s.opacity.iter().filter(|&&o| o < 1.0 / 255.0).count()
    );
    // AABB looseness: box volume against the ellipsoid it bounds. A thin disc
    // tilted 45 degrees fills a small fraction of its axis-aligned box, and
    // every ray through the empty part is a report the shader rejects.
    let mut fill: Vec<f64> = (0..s.len())
        .step_by((s.len() / 200_000).max(1))
        .filter(|&i| prep[i].opacity > 0.0)
        .map(|i| {
            let a = prep[i].aabb;
            let bv = ((a[3] - a[0]) * (a[4] - a[1]) * (a[5] - a[2])) as f64;
            let ks = (2.0 * (s.opacity[i] / p.alpha_min).ln()).sqrt() as f64;
            let sc = s.scale[i];
            let ev = 4.0 / 3.0 * std::f64::consts::PI * ks.powi(3) * (sc[0] * sc[1] * sc[2]) as f64;
            if bv > 0.0 { ev / bv } else { 1.0 }
        })
        .collect();
    println!(
        "ellipsoid / box volume    p10 {:.4}  p50 {:.4}  p90 {:.4}   (1.0 = box is tight)",
        pct(&mut fill, 0.1),
        pct(&mut fill, 0.5),
        pct(&mut fill, 0.9)
    );
    }

    // ---- GPU: what does each ray actually do? --------------------------
    let gpu = Gpu::headless().expect("gpu");

    // Never straight to full scale: grow the run and stop on the way if any
    // GPU job measures - or is predicted - over the limits.
    let eye = cam.pos;
    let fov = cam.fov_y();
    let cam_at = |w: u32, h: u32| Camera::look_at(eye, c, Vec3::NEG_Y, w, h, fov);
    println!();
    println!("ramp ({} proxy)", proxy.name());
    println!("{}", safety::header());
    let verdict = safety::ramp(
        &gpu, &s, proxy, &p, &cam_at, cam.width, cam.height, &safety::Limits::default(),
        &mut |st| println!("{}", safety::row(st)),
    );
    if let Verdict::Stopped { reason, .. } = verdict {
        println!("STOPPED: {reason}");
        // non-zero, so a calling script does not go on to the next step
        std::process::exit(3);
    }
    let mut tr = Tracer::with_kmax(&gpu, kmax);
    tr.set_scene(&gpu, &s, proxy).expect("upload");
    tr.resize(&gpu, cam.width, cam.height);
    let build = tr.ensure_geometry(&gpu, &cam, &p).unwrap_or(0.0);
    println!("proxy       {} - geometry + BVH build {build:.1} ms", proxy.name());
    let base = FrameUniform::new(&cam, &p, s.len() as u32, s.sh_degree, 0, false, false).with_stats(true);
    // warm-up sample: the first dispatch pays for pipeline and cache set-up
    tr.trace_sample(&gpu, &base, true);
    gpu.wait();
    let t = Instant::now();
    tr.trace_sample(&gpu, &base, true);
    gpu.wait();
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let st = tr.read_stats(&gpu);
    let col = |k: usize| -> Vec<f64> { st.iter().map(|v| v[k] as f64).collect() };
    let names = ["reported", "rejected: behind tmin", "rejected: faint", "rejected: beyond k-th", "accepted", "passes", "composited"];
    println!();
    println!("one sample, {}x{}, k = {}, {ms:.1} ms including prep (after warm-up)", cam.width, cam.height, tr.kmax);
    println!("{:<24} {:>10} {:>10} {:>10} {:>10} {:>12}", "per ray", "mean", "p50", "p90", "p99", "max");
    let mut totals = [0.0f64; 7];
    for k in 0..7 {
        let mut v = col(k);
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        totals[k] = mean;
        println!(
            "{:<24} {:>10.1} {:>10.0} {:>10.0} {:>10.0} {:>12.0}",
            names[k],
            mean,
            pct(&mut v, 0.5),
            pct(&mut v, 0.9),
            pct(&mut v, 0.99),
            pct(&mut v, 1.0)
        );
    }
    println!();
    println!(
        "useful fraction of reports (accepted / reported): {:.1}%",
        100.0 * totals[4] / totals[0].max(1.0)
    );
    println!(
        "re-reports of already-shaded splats (behind / reported): {:.1}%",
        100.0 * totals[1] / totals[0].max(1.0)
    );

    // how concentrated is the cost? a GPU runs rays in lockstep groups of 32
    // and a group is as slow as its slowest ray
    let mut rep: Vec<f64> = col(0);
    rep.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let total: f64 = rep.iter().sum();
    let share = |q: f64| rep[..((rep.len() as f64 * q) as usize).max(1)].iter().sum::<f64>() / total;
    println!(
        "share of all reports from the costliest 0.1% / 1% / 10% of rays: {:.1}% / {:.1}% / {:.1}%",
        share(0.001) * 100.0,
        share(0.01) * 100.0,
        share(0.1) * 100.0
    );
    // warp-level cost: group pixels 8x4 (one lockstep group) and take the max
    let (w, h) = (cam.width as usize, cam.height as usize);
    let mut warp_cost = 0.0f64;
    for gy in (0..h).step_by(4) {
        for gx in (0..w).step_by(8) {
            let mut m = 0u32;
            for y in gy..(gy + 4).min(h) {
                for x in gx..(gx + 8).min(w) {
                    m = m.max(st[y * w + x][0]);
                }
            }
            warp_cost += m as f64 * 32.0;
        }
    }
    println!(
        "lockstep inflation: groups pay for {:.1}x the reports their rays actually need",
        warp_cost / total.max(1.0)
    );

    // where does it sit in the picture? log-scaled heat map of reports
    let lmax = (rep[0] + 1.0).ln();
    let heat: Vec<[f32; 3]> = st
        .iter()
        .map(|v| {
            let x = ((v[0] as f64 + 1.0).ln() / lmax) as f32;
            [x.powf(0.8), (x * 1.5 - 0.5).clamp(0.0, 1.0), (x * 3.0 - 2.0).clamp(0.0, 1.0)]
        })
        .collect();
    let _ = std::fs::create_dir_all("compare_out");
    let _ = reference::save_png(std::path::Path::new("compare_out/diagnose_reports_heat.png"), cam.width, cam.height, &heat);
    println!("heat map    compare_out/diagnose_reports_heat.png (log scale, white = {} reports)", rep[0]);
}
