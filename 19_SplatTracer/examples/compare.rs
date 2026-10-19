//! Does the tracer reproduce the scan?
//!
//!   cargo run --release --example compare -- <scene.ply> [--width 960] [--views 5] [--out DIR] [--bench]
//!
//! Renders the capture from its own training cameras (or an orbit, if no
//! cameras.json is found) three ways - the 3DGS rasteriser on the CPU, the RT
//! tracer with the 3DGS dilation re-created, and the pure 3DGRT tracer without
//! it - and reports PSNR of each tracer against the rasteriser, plus timings
//! and throughput at the render size and at 1080p. View 0 is written out as
//! PNGs for inspection.

use glam::Vec3;
use splat_tracer::camera::{self, Camera};
use splat_tracer::reference;
use splat_tracer::scene;
use splat_tracer::tracer::{Gpu, Proxy, Tracer};
use splat_tracer::safety::{self, Verdict};
use splat_tracer::RenderParams;
use std::path::PathBuf;
use std::time::Instant;

fn arg<T: std::str::FromStr>(args: &[String], name: &str, def: T) -> T {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(def)
}

fn scaled(cam: &Camera, width: u32) -> Camera {
    let h = ((cam.height as f32 * width as f32 / cam.width as f32).round() as u32).max(8);
    cam.resized(width, h)
}

fn main() {
    let threads = splat_tracer::polite_threads();
    eprintln!("cpu threads {threads} (SPLAT_THREADS=n to change)");
    let args: Vec<String> = std::env::args().collect();
    let Some(ply) = args.get(1).filter(|a| !a.starts_with("--")).map(PathBuf::from) else {
        eprintln!("usage: compare <scene.ply> [--width 960] [--views 5] [--out DIR]");
        std::process::exit(2);
    };
    let width: u32 = arg(&args, "--width", 960);
    let nviews: usize = arg(&args, "--views", 5);
    let out: PathBuf = arg(&args, "--out", PathBuf::from("compare_out"));
    // Box unless asked: it is the proxy measured safe at full scale
    let proxy = if args.iter().any(|a| a == "--oriented") { Proxy::Oriented } else { Proxy::Box };
    std::fs::create_dir_all(&out).expect("output dir");

    let t = Instant::now();
    let s = scene::load_ply(&ply).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    println!(
        "scene      {} splats, SH degree {}, parsed in {:.2} s",
        s.len(),
        s.sh_degree,
        t.elapsed().as_secs_f32()
    );

    let views: Vec<(String, Camera)> = match camera::find_cameras_json(&ply)
        .and_then(|p| camera::load_cameras_json(&p).ok())
        .filter(|v| !v.is_empty())
    {
        Some(v) => {
            let step = (v.len() / nviews.max(1)).max(1);
            v.iter()
                .step_by(step)
                .take(nviews)
                .map(|tv| (tv.name.clone(), scaled(&tv.camera, width)))
                .collect()
        }
        None => {
            println!("cameras    no cameras.json found - using an orbit around the robust centre, up = -Y");
            let (c, e) = s.robust_bounds();
            let c = Vec3::from(c);
            let r = Vec3::from(e).max_element() * 1.6;
            (0..nviews)
                .map(|i| {
                    let a = i as f32 / nviews as f32 * std::f32::consts::TAU;
                    let eye = c + Vec3::new(a.cos() * r, -0.3 * r, a.sin() * r);
                    let h = width * 9 / 16;
                    (format!("orbit {i}"), Camera::look_at(eye, c, Vec3::NEG_Y, width, h, 50f32.to_radians()))
                })
                .collect()
        }
    };

    let gpu = Gpu::headless().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    println!("gpu        {}", gpu.name);
    let p = RenderParams::default();
    if let Some((_, c0)) = views.first() {
        let c0 = *c0;
        let cam_at = |w: u32, h: u32| c0.resized(w, h);
        println!("ramp ({} proxy)", proxy.name());
        println!("{}", safety::header());
        let v = safety::ramp(
            &gpu, &s, proxy, &p, &cam_at, c0.width, c0.height, &safety::Limits::default(),
            &mut |st| println!("{}", safety::row(st)),
        );
        if let Verdict::Stopped { reason, .. } = v {
            println!("STOPPED: {reason}");
            std::process::exit(3);
        }
    }
    let mut tr = Tracer::new(&gpu);
    let t = Instant::now();
    tr.set_scene(&gpu, &s, proxy).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    gpu.wait();
    println!("upload     {:.2} s, proxy {}", t.elapsed().as_secs_f32(), proxy.name());

    let p0 = RenderParams { dilation_px2: 0.0, ..p };

    println!();
    println!(
        "{:<28} {:>9} {:>12} {:>12} {:>10} {:>9} {:>10}",
        "view", "size", "traced dB", "3DGRT dB", "build ms", "prep ms", "trace ms"
    );
    let mut sum = (0.0, 0.0);
    for (vi, (name, cam)) in views.iter().enumerate() {
        let raster = reference::render_raster(&s, cam, &p);
        let traced = tr.render(&gpu, cam, &p, 1, false);
        let pure = tr.render(&gpu, cam, &p0, 1, false);
        let tm = tr.time(&gpu, cam, &p, 3);
        let a = reference::psnr(&traced, &raster);
        let b = reference::psnr(&pure, &raster);
        sum.0 += a;
        sum.1 += b;
        let short: String = name.chars().take(28).collect();
        println!(
            "{short:<28} {:>9} {a:>12.2} {b:>12.2} {:>10.1} {:>9.2} {:>10.2}",
            format!("{}x{}", cam.width, cam.height),
            tm.build_ms,
            tm.prep_ms,
            tm.trace_ms
        );
        if vi == 0 {
            let (w, h) = (cam.width, cam.height);
            let save = |f: &str, img: &[[f32; 3]]| {
                let path = out.join(f);
                if let Err(e) = reference::save_png(&path, w, h, img) {
                    eprintln!("{e}");
                }
            };
            save("view0_raster_3dgs.png", &raster);
            save("view0_traced.png", &traced);
            save("view0_traced_pure_3dgrt.png", &pure);
            let diff: Vec<[f32; 3]> = traced
                .iter()
                .zip(&raster)
                .map(|(x, y)| {
                    let d = ((x[0] - y[0]).abs() + (x[1] - y[1]).abs() + (x[2] - y[2]).abs()) * 8.0 / 3.0;
                    [d, d, d]
                })
                .collect();
            save("view0_diff_x8.png", &diff);
        }
    }
    let n = views.len().max(1) as f64;
    println!("{:<28} {:>9} {:>12.2} {:>12.2}", "mean", "", sum.0 / n, sum.1 / n);

    // throughput at a common reference size - opt-in: it holds the GPU at
    // full load for several seconds
    let bench = args.iter().any(|a| a == "--bench");
    if let (true, Some((_, cam))) = (bench, views.first()) {
        let hd = Camera { ..cam.resized(1920, (1920.0 * cam.height as f32 / cam.width as f32) as u32) };
        let tm = tr.time(&gpu, &hd, &p, 5);
        let rays = hd.width as f64 * hd.height as f64;
        println!();
        println!(
            "1080p-class {}x{}: build {:.1} ms (once), prep {:.2} ms, trace {:.2} ms \
             -> {:.1} fps, {:.0} M primary rays/s",
            hd.width,
            hd.height,
            tm.build_ms,
            tm.prep_ms,
            tm.trace_ms,
            1000.0 / tm.trace_ms,
            rays / (tm.trace_ms * 1e-3) / 1e6
        );
    }
    println!();
    println!("images     {}", out.canonicalize().unwrap_or(out).display());
}
