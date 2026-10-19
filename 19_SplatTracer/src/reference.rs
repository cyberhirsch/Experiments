//! Two CPU references and the metric that compares them.
//!
//! `render_traced` is the GPU tracer's formulation evaluated by brute force:
//! every splat against every pixel ray, sorted, composited. It exists to prove
//! the GPU plumbing - ray query, bounding boxes, hit buffer, re-casting - is
//! exact. Brute force, so only for small scenes.
//!
//! `render_raster` is the 3DGS forward pass (EWA splatting, per-tile depth
//! sort, early termination) - what a capture *looks like* in the renderer it
//! was trained with. Comparing the tracer against it measures how faithfully
//! a ray tracer reproduces a raster-trained scan, which is the question v001
//! has to answer before anything is built on top.

use crate::camera::Camera;
use crate::scene::Splats;
use crate::{sh, RenderParams};
use glam::{Mat3, Vec3};
use rayon::prelude::*;

/// Per-splat data as the prep shader leaves it: world-to-canonical matrix
/// rows, centre, colour for this camera, opacity (0 = culled).
#[derive(Clone, Copy, Debug)]
pub struct Prepared {
    pub m: [Vec3; 3],
    pub mu: Vec3,
    pub color: [f32; 3],
    pub opacity: f32,
    pub aabb: [f32; 6],
}

/// Rotation matrix from a unit (w, x, y, z) quaternion, rows R0..R2 - the
/// 3DGS `build_rotation` layout.
pub fn rot_rows(q: [f32; 4]) -> [Vec3; 3] {
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt().max(1e-12);
    let (w, x, y, z) = (q[0] / n, q[1] / n, q[2] / n, q[3] / n);
    [
        Vec3::new(1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - w * z), 2.0 * (x * z + w * y)),
        Vec3::new(2.0 * (x * y + w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - w * x)),
        Vec3::new(2.0 * (x * z - w * y), 2.0 * (y * z + w * x), 1.0 - 2.0 * (x * x + y * y)),
    ]
}

/// Mirror of `prep.wgsl`. Kept line-for-line parallel so the two can be
/// compared by eye as well as by test.
pub fn prepare_one(s: &Splats, i: usize, cam: &Camera, p: &RenderParams) -> Prepared {
    let mu = Vec3::from(s.pos[i]);
    let op = s.opacity[i];
    let depth = (mu - cam.pos).dot(cam.fwd);
    if depth <= p.near || op < p.alpha_min {
        return Prepared {
            m: [Vec3::ZERO; 3],
            mu,
            color: [0.0; 3],
            opacity: 0.0,
            aabb: [mu.x, mu.y, mu.z, mu.x, mu.y, mu.z],
        };
    }
    let r = rot_rows(s.rot[i]);
    let sc = Vec3::from(s.scale[i]);
    // 3DGS adds 0.3 px^2 after projection; at depth z one pixel spans z/f
    // world units, so the same variance in 3D is 0.3 (z/f)^2
    let f = (cam.fx * cam.fy).sqrt();
    let px_world = depth / f;
    let b2 = p.dilation_px2 * px_world * px_world + p.cov_bias * p.cov_bias;
    let se = (sc * sc + Vec3::splat(b2)).map(f32::sqrt);
    let col = |c: usize| Vec3::new(r[0][c], r[1][c], r[2][c]);
    let m = [col(0) / se.x, col(1) / se.y, col(2) / se.z];
    let dir = (mu - cam.pos).normalize_or_zero();
    let deg = p.sh_degree.min(s.sh_degree);
    let color = sh::eval(deg, s.sh_of(i), dir.to_array());
    // the box holds every point where alpha can still reach alpha_min:
    // opacity * exp(-k^2/2) = alpha_min  =>  k = sqrt(2 ln(opacity/alpha_min))
    let ks = (2.0 * (op / p.alpha_min).ln()).sqrt();
    let ext = Vec3::new((r[0] * se).length(), (r[1] * se).length(), (r[2] * se).length()) * ks;
    Prepared {
        m,
        mu,
        color,
        opacity: op,
        aabb: [
            mu.x - ext.x,
            mu.y - ext.y,
            mu.z - ext.z,
            mu.x + ext.x,
            mu.y + ext.y,
            mu.z + ext.z,
        ],
    }
}

pub fn prepare(s: &Splats, cam: &Camera, p: &RenderParams) -> Vec<Prepared> {
    (0..s.len()).into_par_iter().map(|i| prepare_one(s, i, cam, p)).collect()
}

/// Mirror of `eval` in `trace.wgsl`: parameter of the ray's closest approach
/// to the splat centre in the splat's own metric, and the alpha there. The
/// origin is made relative to the centre BEFORE the (possibly enormous)
/// canonical scaling - done the other way round, a 1 mm splat seen from 10 m
/// loses every significant digit to cancellation.
#[inline]
pub fn eval_hit(g: &Prepared, o: Vec3, d: Vec3, alpha_max: f32) -> (f32, f32) {
    let rel = o - g.mu;
    let op = Vec3::new(g.m[0].dot(rel), g.m[1].dot(rel), g.m[2].dot(rel));
    let dp = Vec3::new(g.m[0].dot(d), g.m[1].dot(d), g.m[2].dot(d));
    let dd = dp.dot(dp).max(1e-30);
    let t = -op.dot(dp) / dd;
    let c = op + dp * t;
    let a = (g.opacity * (-0.5 * c.dot(c)).exp()).min(alpha_max);
    (t, a)
}

/// Front-to-back compositing with the 3DGS termination rule. Returns colour
/// and the transmittance left for the background.
#[inline]
fn composite<'a>(hits: impl Iterator<Item = (f32, [f32; 3])>, t_min: f32) -> ([f32; 3], f32) {
    let mut c = [0.0f32; 3];
    let mut tr = 1.0f32;
    for (a, col) in hits {
        let test = tr * (1.0 - a);
        if test < t_min {
            break;
        }
        for k in 0..3 {
            c[k] += tr * a * col[k];
        }
        tr = test;
    }
    (c, tr)
}

/// Brute-force ray-traced reference, pixel centres, pinhole.
pub fn render_traced(s: &Splats, cam: &Camera, p: &RenderParams) -> Vec<[f32; 3]> {
    let prep = prepare(s, cam, p);
    let (w, h) = (cam.width as usize, cam.height as usize);
    (0..w * h)
        .into_par_iter()
        .map(|idx| {
            let (x, y) = (idx % w, idx / w);
            let (o, d) = cam.primary_ray(x as f32 + 0.5, y as f32 + 0.5);
            let mut hits: Vec<(f32, f32, usize)> = Vec::new();
            for (i, g) in prep.iter().enumerate() {
                if g.opacity <= 0.0 {
                    continue;
                }
                let (t, a) = eval_hit(g, o, d, p.alpha_max);
                if a < p.alpha_min || t <= 0.0 {
                    continue;
                }
                hits.push((t, a, i));
            }
            hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let (c, tr) = composite(hits.iter().map(|&(_, a, i)| (a, prep[i].color)), p.t_min);
            [
                c[0] + tr * p.background[0],
                c[1] + tr * p.background[1],
                c[2] + tr * p.background[2],
            ]
        })
        .collect()
}

// ------------------------------------------------------------ rasteriser ---

struct Projected {
    xy: [f32; 2],
    conic: [f32; 3],
    opacity: f32,
    color: [f32; 3],
}

const TILE: usize = 16;

/// The 3DGS forward pass on the CPU: project, EWA, tile, sort, blend.
/// `p.dilation_px2` is the 2D low-pass (0.3 in 3DGS); `p.cov_bias` is ignored
/// - this is the renderer the scan was trained in, not a tunable one.
pub fn render_raster(s: &Splats, cam: &Camera, p: &RenderParams) -> Vec<[f32; 3]> {
    let (w, h) = (cam.width as usize, cam.height as usize);
    let tiles_x = w.div_ceil(TILE);
    let tiles_y = h.div_ceil(TILE);
    let tan_x = 0.5 * w as f32 / cam.fx;
    let tan_y = 0.5 * h as f32 / cam.fy;
    let deg = p.sh_degree.min(s.sh_degree);
    let wrot = Mat3::from_cols(cam.right, cam.down, cam.fwd).transpose(); // world -> view

    // 1. project every splat; collect (tile, depth) keys for the ones visible
    let per: Vec<Option<(Projected, f32, [usize; 4])>> = (0..s.len())
        .into_par_iter()
        .map(|i| {
            let mu = Vec3::from(s.pos[i]);
            let t = wrot * (mu - cam.pos);
            if t.z <= p.near {
                return None;
            }
            let op = s.opacity[i];
            // 3D covariance R S S^T R^T
            let r = rot_rows(s.rot[i]);
            let rm = Mat3::from_cols(
                Vec3::new(r[0].x, r[1].x, r[2].x),
                Vec3::new(r[0].y, r[1].y, r[2].y),
                Vec3::new(r[0].z, r[1].z, r[2].z),
            );
            let sm = Mat3::from_diagonal(Vec3::from(s.scale[i]));
            let l = rm * sm;
            let cov3 = l * l.transpose();
            // EWA: Jacobian of the perspective map, with the 3DGS guard band
            // clamping x/z and y/z at 1.3x the field of view
            let limx = 1.3 * tan_x;
            let limy = 1.3 * tan_y;
            let tx = (t.x / t.z).clamp(-limx, limx) * t.z;
            let ty = (t.y / t.z).clamp(-limy, limy) * t.z;
            let j = Mat3::from_cols(
                Vec3::new(cam.fx / t.z, 0.0, 0.0),
                Vec3::new(0.0, cam.fy / t.z, 0.0),
                Vec3::new(-cam.fx * tx / (t.z * t.z), -cam.fy * ty / (t.z * t.z), 0.0),
            );
            let tm = j * wrot;
            let cov2 = tm * cov3 * tm.transpose();
            let a = cov2.x_axis.x + p.dilation_px2;
            let b = cov2.y_axis.x;
            let c = cov2.y_axis.y + p.dilation_px2;
            let det = a * c - b * b;
            if det <= 0.0 {
                return None;
            }
            let conic = [c / det, -b / det, a / det];
            let mid = 0.5 * (a + c);
            let l1 = mid + (mid * mid - det).max(0.1).sqrt();
            let radius = (3.0 * l1.sqrt()).ceil();
            let xy = [cam.fx * t.x / t.z + cam.cx, cam.fy * t.y / t.z + cam.cy];
            let x0 = ((xy[0] - radius) / TILE as f32).floor().max(0.0) as isize;
            let y0 = ((xy[1] - radius) / TILE as f32).floor().max(0.0) as isize;
            let x1 = (((xy[0] + radius) / TILE as f32).ceil() as isize).min(tiles_x as isize);
            let y1 = (((xy[1] + radius) / TILE as f32).ceil() as isize).min(tiles_y as isize);
            if x1 <= x0 || y1 <= y0 {
                return None;
            }
            let dir = (mu - cam.pos).normalize_or_zero();
            let color = sh::eval(deg, s.sh_of(i), dir.to_array());
            Some((
                Projected { xy, conic, opacity: op, color },
                t.z,
                [x0 as usize, y0 as usize, x1 as usize, y1 as usize],
            ))
        })
        .collect();

    // 2. key every (tile, splat) pair by tile then depth - the 3DGS key layout
    let mut keys: Vec<(u64, u32)> = Vec::new();
    for (i, e) in per.iter().enumerate() {
        if let Some((_, z, [x0, y0, x1, y1])) = e {
            for ty in *y0..*y1 {
                for tx in *x0..*x1 {
                    let tile = (ty * tiles_x + tx) as u64;
                    keys.push(((tile << 32) | z.to_bits() as u64, i as u32));
                }
            }
        }
    }
    keys.par_sort_unstable_by_key(|k| k.0);
    let mut ranges = vec![(0usize, 0usize); tiles_x * tiles_y];
    let mut start = 0;
    while start < keys.len() {
        let tile = (keys[start].0 >> 32) as usize;
        let mut end = start;
        while end < keys.len() && (keys[end].0 >> 32) as usize == tile {
            end += 1;
        }
        ranges[tile] = (start, end);
        start = end;
    }

    // 3. blend each pixel through its tile's depth-sorted list
    (0..w * h)
        .into_par_iter()
        .map(|idx| {
            let (x, y) = (idx % w, idx / w);
            let tile = (y / TILE) * tiles_x + x / TILE;
            let (a, b) = ranges[tile];
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let hits = keys[a..b].iter().filter_map(|&(_, i)| {
                let (g, _, _) = per[i as usize].as_ref()?;
                let dx = g.xy[0] - px;
                let dy = g.xy[1] - py;
                let power = -0.5 * (g.conic[0] * dx * dx + g.conic[2] * dy * dy) - g.conic[1] * dx * dy;
                if power > 0.0 {
                    return None;
                }
                let al = (g.opacity * power.exp()).min(p.alpha_max);
                if al < p.alpha_min {
                    return None;
                }
                Some((al, g.color))
            });
            let (c, tr) = composite(hits, p.t_min);
            [
                c[0] + tr * p.background[0],
                c[1] + tr * p.background[1],
                c[2] + tr * p.background[2],
            ]
        })
        .collect()
}

/// Peak signal-to-noise ratio over RGB in [0,1], in dB.
pub fn psnr(a: &[[f32; 3]], b: &[[f32; 3]]) -> f64 {
    assert_eq!(a.len(), b.len());
    let mut se = 0.0f64;
    for (p, q) in a.iter().zip(b) {
        for k in 0..3 {
            let d = p[k].clamp(0.0, 1.0) as f64 - q[k].clamp(0.0, 1.0) as f64;
            se += d * d;
        }
    }
    let mse = se / (a.len() * 3) as f64;
    if mse <= 0.0 {
        f64::INFINITY
    } else {
        10.0 * (1.0 / mse).log10()
    }
}

/// Largest absolute channel difference - PSNR averages, this does not.
pub fn max_abs(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
    a.iter()
        .zip(b)
        .flat_map(|(p, q)| (0..3).map(move |k| (p[k] - q[k]).abs()))
        .fold(0.0, f32::max)
}

/// Save an RGB float image as 8-bit PNG. 3DGS colour is already display
/// space - it was fitted to sRGB photographs - so there is no transfer curve.
pub fn save_png(path: &std::path::Path, w: u32, h: u32, px: &[[f32; 3]]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    let data: Vec<u8> = px
        .iter()
        .flat_map(|c| c.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8))
        .collect();
    wr.write_image_data(&data).map_err(|e| e.to_string())
}
