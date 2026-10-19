//! CPU-side checks: the PLY path, and the two references against each other.

use glam::Vec3;
use splat_tracer::camera::Camera;
use splat_tracer::{reference, scene, sh, RenderParams};

#[test]
fn ply_round_trip() {
    let s = scene::synthetic(257, 3, 1.0);
    let dir = std::env::temp_dir().join("splat_tracer_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("roundtrip.ply");
    scene::write_ply(&path, &s).unwrap();
    let r = scene::load_ply(&path).unwrap();
    assert_eq!(r.len(), s.len());
    assert_eq!(r.sh_degree, 3);
    let close = |a: f32, b: f32, tol: f32| (a - b).abs() <= tol * (1.0 + b.abs());
    for i in 0..s.len() {
        for k in 0..3 {
            assert!(close(r.pos[i][k], s.pos[i][k], 1e-6));
            assert!(close(r.scale[i][k], s.scale[i][k], 1e-5), "scale {i}");
        }
        for k in 0..4 {
            assert!(close(r.rot[i][k], s.rot[i][k], 1e-5), "rot {i}");
        }
        assert!(close(r.opacity[i], s.opacity[i], 1e-5), "opacity {i}");
        for k in 0..sh::FLOATS {
            assert!(close(r.sh[i * sh::FLOATS + k], s.sh[i * sh::FLOATS + k], 1e-6), "sh {i}/{k}");
        }
    }
}

/// For an affine projection the peak response of a 3D Gaussian along a ray
/// IS the projected 2D Gaussian of EWA splatting - marginalising a Gaussian
/// leaves the minimum of its exponent. So for splats small against their
/// distance, the ray tracer and the 3DGS rasteriser must agree closely; what
/// remains is perspective (EWA linearises at the centre) and sort order
/// (centre depth versus distance along the ray). A large disagreement here
/// means one of the two references is wrong.
#[test]
fn raster_and_trace_agree_where_the_formulations_coincide() {
    let mut s = scene::synthetic(3000, 21, 1.0);
    for sc in s.scale.iter_mut() {
        for v in sc.iter_mut() {
            *v = (*v * 0.25).max(0.004);
        }
    }
    let cam = Camera::look_at(
        Vec3::new(0.2, -0.3, -4.5),
        Vec3::ZERO,
        Vec3::new(0.0, -1.0, 0.0),
        160,
        120,
        45f32.to_radians(),
    );
    let p = RenderParams::default();
    let a = reference::render_raster(&s, &cam, &p);
    let b = reference::render_traced(&s, &cam, &p);
    let db = reference::psnr(&a, &b);
    let lit = a.iter().filter(|v| v[0] + v[1] + v[2] > 0.02).count();
    println!("raster vs trace, small splats: {db:.2} dB, {lit}/{} lit", a.len());
    assert!(lit > a.len() / 5, "scene barely covers the frame");
    assert!(db > 30.0, "references disagree where they should not: {db:.2} dB");

    // and the dilation has to be doing the same job in both: switch it off
    // in the tracer only and the agreement must get worse, not better
    let p0 = RenderParams { dilation_px2: 0.0, ..p };
    let c = reference::render_traced(&s, &cam, &p0);
    let db0 = reference::psnr(&a, &c);
    println!("raster vs trace without dilation: {db0:.2} dB");
    assert!(db0 < db, "removing the tracer's dilation should hurt, not help ({db0:.2} vs {db:.2})");
}

/// A non-finite value in one splat must never reach the BVH builder, and an
/// absurd scale must be clamped before it wrecks float precision.
#[test]
fn sanitize_drops_poison_and_clamps_giants() {
    let mut s = scene::synthetic(1000, 17, 1.0);
    s.pos[10][1] = f32::NAN;
    s.scale[20][0] = f32::INFINITY;
    s.rot[30][2] = f32::NEG_INFINITY;
    s.opacity[40] = f32::NAN;
    let st = s.sh_stride();
    s.sh[50 * st + 5] = f32::NAN;
    s.scale[60] = [1.0e9, 0.1, 0.1];
    let keep_pos = s.pos[11]; // the splat after the first dropped one
    let r = s.sanitize();
    assert_eq!(r.dropped, 5, "{r:?}");
    assert_eq!(r.clamped, 1, "{r:?}");
    assert_eq!(s.len(), 995);
    assert_eq!(s.sh.len(), 995 * st);
    assert_eq!(s.pos[10], keep_pos, "compaction shifted the wrong splats");
    assert!(s.scale.iter().flatten().all(|v| v.is_finite() && *v < 100.0));
    assert!(s.pos.iter().flatten().all(|v| v.is_finite()));
}

#[test]
fn subset_is_deterministic_and_sized() {
    let s = scene::synthetic(10_000, 3, 1.0);
    let a = splat_tracer::safety::subset(&s, 1234);
    let b = splat_tracer::safety::subset(&s, 1234);
    assert_eq!(a.len(), 1234);
    assert_eq!(a.sh.len(), 1234 * s.sh_stride());
    assert_eq!(a.pos, b.pos);
    assert_eq!(a.pos[0], s.pos[0]);
    let all = splat_tracer::safety::subset(&s, 50_000);
    assert_eq!(all.len(), s.len(), "asking for more than exists returns everything");
}

#[test]
fn sh_degree_zero_is_the_dc_term() {
    let mut shv = [0.0f32; sh::FLOATS];
    shv[0] = 1.0;
    shv[1] = -0.5;
    shv[2] = 0.25;
    for k in 3..sh::FLOATS {
        shv[k] = 7.0; // must be ignored at degree 0
    }
    let c = sh::eval(0, &shv, [0.0, 0.0, 1.0]);
    let expect = [sh::C0 + 0.5, -0.5 * sh::C0 + 0.5, 0.25 * sh::C0 + 0.5];
    for k in 0..3 {
        assert!((c[k] - expect[k]).abs() < 1e-6, "{c:?} vs {expect:?}");
    }
}
