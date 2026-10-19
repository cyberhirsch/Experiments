//! The experimental part, proven: the GPU tracer on the RT cores against the
//! brute-force CPU evaluation of the same formulation - for both proxy
//! shapes. Any fault in ray query, in a proxy's coverage, in the hit buffer's
//! ordering, in the traversal shrink or in the re-cast between batches shows
//! up here as a mismatch.

use glam::Vec3;
use splat_tracer::camera::Camera;
use splat_tracer::tracer::{Gpu, Proxy, Tracer};
use splat_tracer::{reference, scene, RenderParams};

fn gpu() -> Option<Gpu> {
    match Gpu::headless() {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("skipping GPU test: {e}");
            None
        }
    }
}

fn cam() -> Camera {
    Camera::look_at(
        Vec3::new(0.35, -0.45, -3.1),
        Vec3::ZERO,
        Vec3::new(0.0, -1.0, 0.0),
        128,
        96,
        50f32.to_radians(),
    )
}

fn compare(g: &Gpu, t: &mut Tracer, s: &scene::Splats, p: &RenderParams, what: &str) -> f64 {
    let c = cam();
    let a = t.render(g, &c, p, 1, false);
    let b = reference::render_traced(s, &c, p);
    let db = reference::psnr(&a, &b);
    let mx = reference::max_abs(&a, &b);
    let lit = b.iter().filter(|v| v[0] + v[1] + v[2] > 0.02).count();
    println!(
        "{what:<48} PSNR {db:7.2} dB   max |diff| {mx:.2e}   lit {lit}/{}",
        b.len()
    );
    assert!(lit > b.len() / 4, "{what}: scene barely covers the frame ({lit} lit) - test is vacuous");
    assert!(db > 50.0, "{what}: GPU and CPU disagree, {db:.2} dB");
    assert!(mx < 2e-2, "{what}: worst pixel off by {mx}");
    db
}

#[test]
fn gpu_matches_cpu_reference() {
    let Some(g) = gpu() else { return };
    let s = scene::synthetic(600, 7, 1.0);
    for proxy in [Proxy::Box, Proxy::Oriented] {
        let mut t = Tracer::new(&g);
        t.set_scene(&g, &s, proxy).unwrap();
        let n = proxy.name();

        let p = RenderParams::default();
        compare(&g, &mut t, &s, &p, &format!("{n}: k=16, 3DGS dilation"));

        // k = 2 forces dozens of re-casts per pixel; the batches must stitch
        // together into exactly the same sorted sequence
        let p2 = RenderParams { k: 2, ..p };
        compare(&g, &mut t, &s, &p2, &format!("{n}: k=2 (heavy re-casting)"));

        let p3 = RenderParams { dilation_px2: 0.0, ..p };
        compare(&g, &mut t, &s, &p3, &format!("{n}: no dilation (pure 3DGRT)"));

        let p4 = RenderParams { sh_degree: 0, background: [0.2, 0.4, 0.9], ..p };
        compare(&g, &mut t, &s, &p4, &format!("{n}: SH degree 0, coloured background"));

        // 32-px tiles cut the 128x96 frame into 4x3 separate submissions; the
        // image must not know it was drawn in pieces
        t.tile = 32;
        compare(&g, &mut t, &s, &p, &format!("{n}: 32-px tiles, 12 submissions"));
    }
}

/// The proxies are camera-independent, sized for a dilation bound. Moving the
/// camera far enough out must trigger a rebuild, and the image from out
/// there must still match - a stale, too-small proxy would clip splats.
#[test]
fn geometry_rebuilds_when_the_view_outgrows_it() {
    let Some(g) = gpu() else { return };
    let s = scene::synthetic(600, 9, 1.0);
    let mut t = Tracer::new(&g);
    t.set_scene(&g, &s, Proxy::Oriented).unwrap();
    let p = RenderParams::default();
    let near = cam();
    let _ = t.render(&g, &near, &p, 1, false);
    let builds = t.scene.as_ref().unwrap().builds;
    assert_eq!(builds, 1);

    // three times farther out at the SAME field of view: one pixel now spans
    // 3x the world, the dilation bound needed grows ~9x in variance, past the
    // 2.25x headroom. (Narrowing the FOV would raise the focal length and
    // cancel most of that - a zoomed-in far view needs no rebuild at all.)
    let far = Camera::look_at(
        Vec3::new(1.05, -1.35, -9.3),
        Vec3::ZERO,
        Vec3::new(0.0, -1.0, 0.0),
        128,
        96,
        50f32.to_radians(),
    );
    let a = t.render(&g, &far, &p, 1, false);
    assert!(t.scene.as_ref().unwrap().builds > builds, "far view did not trigger a rebuild");
    let b = reference::render_traced(&s, &far, &p);
    let db = reference::psnr(&a, &b);
    println!("after rebuild for a far view: {db:.2} dB");
    assert!(db > 50.0, "far view mismatch after rebuild: {db:.2} dB");
}

/// A scene stored at degree 0 - what SuperSplat exports - uses the compacted
/// 3-float SH stride on the GPU. Same proof, other layout.
#[test]
fn degree_zero_storage_matches() {
    let Some(g) = gpu() else { return };
    let mut s = scene::synthetic(600, 5, 1.0);
    let full = std::mem::take(&mut s.sh);
    s.sh = full.chunks(splat_tracer::sh::FLOATS).flat_map(|c| c[..3].to_vec()).collect();
    s.sh_degree = 0;
    assert_eq!(s.sh.len(), s.len() * 3);
    let mut t = Tracer::new(&g);
    t.set_scene(&g, &s, Proxy::Oriented).unwrap();
    compare(&g, &mut t, &s, &RenderParams::default(), "oriented: degree-0 storage, 3 floats/splat");
}

/// Progressive accumulation with jitter must converge on something close to
/// the centre-sampled image, not drift away from it.
#[test]
fn accumulation_is_an_average() {
    let Some(g) = gpu() else { return };
    let s = scene::synthetic(600, 11, 1.0);
    let mut t = Tracer::new(&g);
    t.set_scene(&g, &s, Proxy::Oriented).unwrap();
    let p = RenderParams::default();
    let c = cam();
    let centre = t.render(&g, &c, &p, 1, false);
    let avg = t.render(&g, &c, &p, 64, true);
    let db = reference::psnr(&centre, &avg);
    println!("64-spp jittered vs centre-sampled: {db:.2} dB");
    // antialiasing changes edges, so this is a loose bound on purpose: it
    // catches a broken running sum (which lands near 0-10 dB), not AA itself
    assert!(db > 22.0, "accumulated image drifted from the centre-sampled one: {db:.2} dB");
}
