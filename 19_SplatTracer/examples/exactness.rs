//! Debug aid: where exactly does the GPU depart from the CPU reference?
//! Sweeps one setting at a time on the synthetic test scene and reports PSNR
//! and, for the worst pixel, how many splats each side composited.

use glam::Vec3;
use splat_tracer::camera::Camera;
use splat_tracer::tracer::{FrameUniform, Gpu, Proxy, Tracer};
use splat_tracer::{reference, scene, RenderParams};

fn main() {
    let g = Gpu::headless().expect("gpu");
    let s = scene::synthetic(600, 7, 1.0);
    let cam = Camera::look_at(Vec3::new(0.35, -0.45, -3.1), Vec3::ZERO, Vec3::NEG_Y, 128, 96, 50f32.to_radians());
    let base = RenderParams::default();
    let cpu = reference::render_traced(&s, &cam, &base);

    for (label, kmax, p) in [
        ("kmax 16, k 16", 16u32, base),
        ("kmax 32, k 16", 32, base),
        ("kmax 32, k 32", 32, RenderParams { k: 32, ..base }),
        ("kmax 32, k 8", 32, RenderParams { k: 8, ..base }),
        ("kmax 32, k 2", 32, RenderParams { k: 2, ..base }),
    ] {
        let mut t = Tracer::with_kmax(&g, kmax);
        t.set_scene(&g, &s, Proxy::Box).unwrap();
        let img = t.render(&g, &cam, &p, 1, false);
        let cpu_p = if p.k == base.k { cpu.clone() } else { reference::render_traced(&s, &cam, &p) };
        let db = reference::psnr(&img, &cpu_p);
        // worst pixel
        let (wi, wd) = img
            .iter()
            .zip(&cpu_p)
            .enumerate()
            .map(|(i, (a, b))| (i, (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0f32, f32::max)))
            .fold((0, 0.0f32), |acc, x| if x.1 > acc.1 { x } else { acc });
        // stats for that pixel from a stats-enabled sample
        let f = FrameUniform::new(&cam, &p, s.len() as u32, s.sh_degree, 0, false, false).with_stats(true);
        t.trace_sample(&g, &f, true);
        g.wait();
        let st = t.read_stats(&g)[wi];
        // CPU composited count for that pixel
        let prep = reference::prepare(&s, &cam, &p);
        let (x, y) = (wi % 128, wi / 128);
        let (o, d) = cam.primary_ray(x as f32 + 0.5, y as f32 + 0.5);
        let mut hits: Vec<(f32, f32)> = prep
            .iter()
            .filter(|g| g.opacity > 0.0)
            .map(|g| reference::eval_hit(g, o, d, p.alpha_max))
            .filter(|&(t, a)| a >= p.alpha_min && t > 0.0)
            .collect();
        hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mut tr = 1.0f32;
        let mut n = 0;
        for (_, a) in &hits {
            let test = tr * (1.0 - a);
            if test < p.t_min {
                break;
            }
            tr = test;
            n += 1;
        }
        let min_gap = hits[..n.min(hits.len())]
            .windows(2)
            .map(|w| (w[1].0 - w[0].0) / w[0].0.abs().max(1e-6))
            .fold(f32::INFINITY, f32::min);
        println!(
            "{label:<16} PSNR {db:8.2}  worst px ({x},{y}) off {wd:.2e}   GPU composited {} in {} passes, CPU composited {n} of {} candidates, tightest relative depth gap {min_gap:.1e}",
            st[6],
            st[5],
            hits.len()
        );
    }
}
