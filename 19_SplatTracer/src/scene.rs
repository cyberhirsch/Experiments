//! Splat scenes: the standard 3DGS PLY, activated on load.
//!
//! A 3DGS PLY stores parameters *before* activation, exactly as the optimiser
//! held them: log-scales, opacity logits, an unnormalised quaternion. They are
//! activated once here - exp, sigmoid, normalise - so every consumer downstream
//! (GPU, both references) works in the same physical units.

use crate::sh;
use rayon::prelude::*;
use std::path::Path;

/// Activated splats. Quaternions are (w, x, y, z) - `rot_0` is the real part
/// in the 3DGS convention.
#[derive(Clone, Debug, Default)]
pub struct Splats {
    pub pos: Vec<[f32; 3]>,
    pub scale: Vec<[f32; 3]>,
    pub rot: Vec<[f32; 4]>,
    pub opacity: Vec<f32>,
    /// `sh::stride(sh_degree)` floats per splat, coefficient-major.
    pub sh: Vec<f32>,
    pub sh_degree: u32,
    /// what `sanitize` had to do on load
    pub sanitized: Sanitized,
}

/// Splats a GPU must never be handed. A single non-finite box poisons a BVH,
/// and traversing a poisoned BVH is a classic way to hang the RT cores.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sanitized {
    /// non-finite position, scale, rotation, opacity or colour: removed
    pub dropped: usize,
    /// scale larger than 10x the scene's robust extent: clamped
    pub clamped: usize,
}

impl Splats {
    pub fn len(&self) -> usize {
        self.pos.len()
    }
    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }
    /// Drop what cannot be traced, clamp what would wreck precision. Run on
    /// every load; the counts are kept in `sanitized`.
    pub fn sanitize(&mut self) -> Sanitized {
        let st = self.sh_stride();
        let n = self.len();
        let ok: Vec<bool> = (0..n)
            .into_par_iter()
            .map(|i| {
                self.pos[i].iter().all(|v| v.is_finite())
                    && self.scale[i].iter().all(|v| v.is_finite() && *v >= 0.0)
                    && self.rot[i].iter().all(|v| v.is_finite())
                    && self.opacity[i].is_finite()
                    && self.sh[i * st..(i + 1) * st].iter().all(|v| v.is_finite())
            })
            .collect();
        let dropped = ok.iter().filter(|k| !**k).count();
        if dropped > 0 {
            let mut j = 0;
            for i in 0..n {
                if ok[i] {
                    self.pos[j] = self.pos[i];
                    self.scale[j] = self.scale[i];
                    self.rot[j] = self.rot[i];
                    self.opacity[j] = self.opacity[i];
                    self.sh.copy_within(i * st..(i + 1) * st, j * st);
                    j += 1;
                }
            }
            self.pos.truncate(j);
            self.scale.truncate(j);
            self.rot.truncate(j);
            self.opacity.truncate(j);
            self.sh.truncate(j * st);
        }
        let (_, e) = self.robust_bounds();
        let cap = 10.0 * e.iter().cloned().fold(0.0f32, f32::max).max(1e-3);
        let mut clamped = 0;
        for sc in self.scale.iter_mut() {
            if sc.iter().any(|v| *v > cap) {
                clamped += 1;
                for v in sc.iter_mut() {
                    *v = v.min(cap);
                }
            }
        }
        for o in self.opacity.iter_mut() {
            *o = o.clamp(0.0, 1.0);
        }
        self.sanitized = Sanitized { dropped, clamped };
        self.sanitized
    }

    pub fn sh_stride(&self) -> usize {
        sh::stride(self.sh_degree)
    }
    pub fn sh_of(&self, i: usize) -> &[f32] {
        let st = self.sh_stride();
        &self.sh[i * st..(i + 1) * st]
    }

    /// Robust centre and extent: per-axis 10th and 90th percentiles, so the
    /// floaters every capture drags along do not set the camera distance.
    pub fn robust_bounds(&self) -> ([f32; 3], [f32; 3]) {
        if self.pos.is_empty() {
            return ([0.0; 3], [1.0; 3]);
        }
        let mut lo = [0.0f32; 3];
        let mut hi = [0.0f32; 3];
        let stride = (self.pos.len() / 200_000).max(1);
        for a in 0..3 {
            let mut v: Vec<f32> = self.pos.iter().step_by(stride).map(|p| p[a]).collect();
            v.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
            lo[a] = v[v.len() / 10];
            hi[a] = v[(v.len() * 9) / 10];
        }
        let c = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5];
        let e = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
        (c, e)
    }
}

#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

// ------------------------------------------------------------------- PLY ----

#[derive(Clone, Copy, Debug, PartialEq)]
enum Ty {
    F32,
    F64,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
}

impl Ty {
    fn parse(s: &str) -> Option<Ty> {
        Some(match s {
            "float" | "float32" => Ty::F32,
            "double" | "float64" => Ty::F64,
            "uchar" | "uint8" => Ty::U8,
            "char" | "int8" => Ty::I8,
            "ushort" | "uint16" => Ty::U16,
            "short" | "int16" => Ty::I16,
            "uint" | "uint32" => Ty::U32,
            "int" | "int32" => Ty::I32,
            _ => return None,
        })
    }
    fn size(self) -> usize {
        match self {
            Ty::U8 | Ty::I8 => 1,
            Ty::U16 | Ty::I16 => 2,
            Ty::F32 | Ty::U32 | Ty::I32 => 4,
            Ty::F64 => 8,
        }
    }
    #[inline]
    fn read(self, b: &[u8]) -> f32 {
        match self {
            Ty::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            Ty::F64 => f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) as f32,
            Ty::U8 => b[0] as f32,
            Ty::I8 => b[0] as i8 as f32,
            Ty::U16 => u16::from_le_bytes([b[0], b[1]]) as f32,
            Ty::I16 => i16::from_le_bytes([b[0], b[1]]) as f32,
            Ty::U32 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
            Ty::I32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
        }
    }
}

#[derive(Clone, Copy)]
struct Field {
    off: usize,
    ty: Ty,
}

pub fn load_ply(path: &Path) -> Result<Splats, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut s = parse_ply(&bytes)?;
    s.sanitize();
    if s.is_empty() {
        return Err("no splat in the file survived sanitising".into());
    }
    Ok(s)
}

pub fn parse_ply(bytes: &[u8]) -> Result<Splats, String> {
    // header is ASCII up to and including "end_header" plus its newline
    let marker = b"end_header";
    let hpos = bytes
        .windows(marker.len())
        .position(|w| w == marker)
        .ok_or("not a PLY file: no end_header")?;
    let mut body = hpos + marker.len();
    if bytes.get(body) == Some(&b'\r') {
        body += 1;
    }
    if bytes.get(body) == Some(&b'\n') {
        body += 1;
    }
    let header = std::str::from_utf8(&bytes[..hpos]).map_err(|_| "PLY header is not text")?;

    let mut lines = header.lines();
    if lines.next().map(str::trim) != Some("ply") {
        return Err("not a PLY file".into());
    }
    let mut format_ok = false;
    let mut count = 0usize;
    let mut in_vertex = false;
    let mut seen_element = false;
    let mut props: Vec<(String, Ty)> = Vec::new();
    for line in lines {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t.as_slice() {
            ["format", "binary_little_endian", _] => format_ok = true,
            ["format", f, _] => return Err(format!("unsupported PLY format '{f}' - need binary_little_endian")),
            ["element", "vertex", n] => {
                if seen_element {
                    return Err("vertex element must come first".into());
                }
                seen_element = true;
                in_vertex = true;
                count = n.parse().map_err(|_| "bad vertex count")?;
            }
            ["element", other, _] => {
                if !seen_element {
                    return Err(format!(
                        "PLY starts with element '{other}' - a compressed or non-3DGS layout, not supported"
                    ));
                }
                in_vertex = false;
            }
            ["property", "list", ..] if in_vertex => {
                return Err("list properties in the vertex element are not supported".into());
            }
            ["property", ty, name] if in_vertex => {
                let ty = Ty::parse(ty).ok_or_else(|| format!("unknown PLY type '{ty}'"))?;
                props.push((name.to_string(), ty));
            }
            _ => {}
        }
    }
    if !format_ok {
        return Err("PLY is not binary_little_endian".into());
    }

    let mut stride = 0usize;
    let mut field = std::collections::HashMap::new();
    for (name, ty) in &props {
        field.insert(name.clone(), Field { off: stride, ty: *ty });
        stride += ty.size();
    }
    let get = |n: &str| -> Result<Field, String> {
        field
            .get(n)
            .copied()
            .ok_or_else(|| format!("missing property '{n}' - not a 3DGS splat PLY"))
    };
    let fx = [get("x")?, get("y")?, get("z")?];
    let fdc = [get("f_dc_0")?, get("f_dc_1")?, get("f_dc_2")?];
    let fop = get("opacity")?;
    let fsc = [get("scale_0")?, get("scale_1")?, get("scale_2")?];
    let frot = [get("rot_0")?, get("rot_1")?, get("rot_2")?, get("rot_3")?];
    let mut frest = Vec::new();
    while let Some(f) = field.get(&format!("f_rest_{}", frest.len())) {
        frest.push(*f);
    }
    // f_rest is channel-major: all of red's higher coefficients, then green's,
    // then blue's - the 3DGS exporter transposes before it flattens.
    if frest.len() % 3 != 0 {
        return Err(format!("{} f_rest values is not a multiple of 3", frest.len()));
    }
    let per_ch = frest.len() / 3;
    let coeffs = 1 + per_ch;
    let degree = match coeffs {
        1 => 0,
        4 => 1,
        9 => 2,
        16 => 3,
        n => return Err(format!("{n} SH coefficients per channel is not a full degree")),
    };

    let need = body + count * stride;
    if bytes.len() < need {
        return Err(format!(
            "PLY truncated: header promises {count} splats ({need} bytes), file has {}",
            bytes.len()
        ));
    }
    let data = &bytes[body..need];

    let st = sh::stride(degree);
    let mut s = Splats {
        pos: vec![[0.0; 3]; count],
        scale: vec![[0.0; 3]; count],
        rot: vec![[0.0; 4]; count],
        opacity: vec![0.0; count],
        sh: vec![0.0; count * st],
        sh_degree: degree,
        sanitized: Sanitized::default(),
    };

    const CH: usize = 16384;
    s.pos
        .par_chunks_mut(CH)
        .zip(s.scale.par_chunks_mut(CH))
        .zip(s.rot.par_chunks_mut(CH))
        .zip(s.opacity.par_chunks_mut(CH))
        .zip(s.sh.par_chunks_mut(CH * st))
        .enumerate()
        .for_each(|(ci, ((((pos, scale), rot), opacity), shc))| {
            for j in 0..pos.len() {
                let row = &data[(ci * CH + j) * stride..];
                let r = |f: Field| f.ty.read(&row[f.off..]);
                pos[j] = [r(fx[0]), r(fx[1]), r(fx[2])];
                scale[j] = [r(fsc[0]).exp(), r(fsc[1]).exp(), r(fsc[2]).exp()];
                let q = [r(frot[0]), r(frot[1]), r(frot[2]), r(frot[3])];
                let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
                rot[j] = if n > 0.0 {
                    [q[0] / n, q[1] / n, q[2] / n, q[3] / n]
                } else {
                    [1.0, 0.0, 0.0, 0.0]
                };
                opacity[j] = sigmoid(r(fop));
                let out = &mut shc[j * st..(j + 1) * st];
                for c in 0..3 {
                    out[c] = r(fdc[c]);
                    for k in 1..coeffs {
                        out[k * 3 + c] = r(frest[c * per_ch + (k - 1)]);
                    }
                }
            }
        });
    Ok(s)
}

/// Write the standard 3DGS layout, inverse-activated - used by the tests to
/// round-trip, and to hand synthetic scenes to other tools.
pub fn write_ply(path: &Path, s: &Splats) -> Result<(), String> {
    let deg = s.sh_degree as usize;
    let coeffs = (deg + 1) * (deg + 1);
    let per_ch = coeffs - 1;
    let mut h = String::new();
    h += "ply\nformat binary_little_endian 1.0\n";
    h += &format!("element vertex {}\n", s.len());
    for n in ["x", "y", "z", "nx", "ny", "nz", "f_dc_0", "f_dc_1", "f_dc_2"] {
        h += &format!("property float {n}\n");
    }
    for i in 0..per_ch * 3 {
        h += &format!("property float f_rest_{i}\n");
    }
    h += "property float opacity\n";
    for n in ["scale_0", "scale_1", "scale_2", "rot_0", "rot_1", "rot_2", "rot_3"] {
        h += &format!("property float {n}\n");
    }
    h += "end_header\n";
    let mut out = h.into_bytes();
    for i in 0..s.len() {
        let mut row: Vec<f32> = Vec::with_capacity(17 + per_ch * 3);
        row.extend_from_slice(&s.pos[i]);
        row.extend_from_slice(&[0.0, 0.0, 0.0]);
        let shv = s.sh_of(i);
        row.extend_from_slice(&[shv[0], shv[1], shv[2]]);
        for c in 0..3 {
            for k in 1..coeffs {
                row.push(shv[k * 3 + c]);
            }
        }
        let o = s.opacity[i].clamp(1e-7, 1.0 - 1e-7);
        row.push((o / (1.0 - o)).ln());
        row.extend(s.scale[i].iter().map(|v| v.ln()));
        row.extend_from_slice(&s.rot[i]);
        for v in row {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    std::fs::write(path, out).map_err(|e| format!("{}: {e}", path.display()))
}

// ------------------------------------------------------------- synthetic ---

struct Lcg(u64);
impl Lcg {
    fn f(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
}

/// A random but plausible scene for tests: anisotropic splats of widely
/// varying size inside a box, full degree-3 colour. Deterministic in `seed`.
pub fn synthetic(n: usize, seed: u64, half: f32) -> Splats {
    let mut r = Lcg(seed ^ 0x9E37_79B9_7F4A_7C15);
    let mut s = Splats {
        sh_degree: 3,
        sh: vec![0.0; n * sh::FLOATS],
        ..Default::default()
    };
    // (sanitized stays at its default: nothing here needs it)
    for i in 0..n {
        s.pos.push([r.range(-half, half), r.range(-half, half), r.range(-half, half)]);
        // log-uniform sizes over two decades, then stretched: splats in real
        // captures are mostly needles and discs, not spheres
        let base = (r.range(-4.0, -1.0) as f32).exp() * half;
        s.scale.push([
            base * r.range(0.2, 1.0),
            base * r.range(0.2, 1.0),
            base * r.range(0.05, 1.0),
        ]);
        let q = [r.range(-1.0, 1.0), r.range(-1.0, 1.0), r.range(-1.0, 1.0), r.range(-1.0, 1.0)];
        let qn = (q.iter().map(|v| v * v).sum::<f32>()).sqrt().max(1e-6);
        s.rot.push([q[0] / qn, q[1] / qn, q[2] / qn, q[3] / qn]);
        s.opacity.push(r.range(0.05, 1.0));
        let shv = &mut s.sh[i * sh::FLOATS..(i + 1) * sh::FLOATS];
        for c in 0..3 {
            shv[c] = r.range(-1.2, 1.2);
        }
        for k in 1..sh::COEFFS {
            for c in 0..3 {
                shv[k * 3 + c] = r.range(-0.25, 0.25);
            }
        }
    }
    s
}
