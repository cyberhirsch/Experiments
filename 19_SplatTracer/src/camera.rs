//! Pinhole camera with a thin lens, in the COLMAP convention every 3DGS
//! capture uses: image x runs right, image y runs DOWN, the camera looks along
//! +forward. A training camera from `cameras.json` drops straight in.

use glam::Vec3;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub pos: Vec3,
    pub right: Vec3,
    pub down: Vec3,
    pub fwd: Vec3,
    pub width: u32,
    pub height: u32,
    pub fx: f32,
    pub fy: f32,
    pub cx: f32,
    pub cy: f32,
    /// Lens radius in world units; 0 is a pinhole.
    pub aperture: f32,
    /// Distance along `fwd` to the plane in focus.
    pub focus: f32,
}

impl Camera {
    /// `up` is the world direction that should read as up on screen.
    pub fn look_at(pos: Vec3, target: Vec3, up: Vec3, width: u32, height: u32, fov_y: f32) -> Self {
        let fwd = (target - pos).normalize_or(Vec3::Z);
        let mut right = fwd.cross(up);
        if right.length_squared() < 1e-10 {
            // looking straight along `up`: any perpendicular will do
            right = fwd.any_orthonormal_vector();
        }
        let right = right.normalize();
        let down = fwd.cross(right);
        let fy = 0.5 * height as f32 / (0.5 * fov_y).tan();
        Self {
            pos,
            right,
            down,
            fwd,
            width,
            height,
            fx: fy,
            fy,
            cx: width as f32 * 0.5,
            cy: height as f32 * 0.5,
            aperture: 0.0,
            focus: (target - pos).length().max(1e-3),
        }
    }

    /// Same view, different resolution: the field of view is kept, so the
    /// focal length scales with the image.
    pub fn resized(&self, width: u32, height: u32) -> Self {
        let sx = width as f32 / self.width.max(1) as f32;
        let sy = height as f32 / self.height.max(1) as f32;
        Self {
            width,
            height,
            fx: self.fx * sx,
            fy: self.fy * sy,
            cx: self.cx * sx,
            cy: self.cy * sy,
            ..*self
        }
    }

    /// Pinhole ray through continuous pixel coordinates (pixel centres sit at
    /// half-integers). Direction is unit length.
    pub fn primary_ray(&self, px: f32, py: f32) -> (Vec3, Vec3) {
        let x = (px - self.cx) / self.fx;
        let y = (py - self.cy) / self.fy;
        (self.pos, (self.right * x + self.down * y + self.fwd).normalize())
    }

    pub fn fov_y(&self) -> f32 {
        2.0 * (0.5 * self.height as f32 / self.fy).atan()
    }
}

/// A camera the scene was trained from.
#[derive(Clone, Debug)]
pub struct TrainingView {
    pub name: String,
    pub camera: Camera,
}

/// 3DGS writes `cameras.json` beside its output. Each entry's `rotation` is
/// the camera-to-world matrix - its COLUMNS are the camera's right, down and
/// forward axes in world space - `position` the camera centre, and fx/fy the
/// focal lengths in pixels of the original image.
pub fn load_cameras_json(path: &Path) -> Result<Vec<TrainingView>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("cameras.json: {e}"))?;
    let arr = v.as_array().ok_or("cameras.json: expected an array")?;
    let mut out = Vec::with_capacity(arr.len());
    for (i, e) in arr.iter().enumerate() {
        let num = |k: &str| e.get(k).and_then(|x| x.as_f64()).map(|x| x as f32);
        let (Some(w), Some(h), Some(fx), Some(fy)) = (num("width"), num("height"), num("fx"), num("fy")) else {
            return Err(format!("cameras.json entry {i}: missing width/height/fx/fy"));
        };
        let p = e.get("position").and_then(|x| x.as_array()).ok_or("missing position")?;
        let r = e.get("rotation").and_then(|x| x.as_array()).ok_or("missing rotation")?;
        let pf = |a: &serde_json::Value| a.as_f64().unwrap_or(0.0) as f32;
        let m = |row: usize, col: usize| -> f32 {
            r.get(row)
                .and_then(|x| x.as_array())
                .and_then(|x| x.get(col))
                .map(pf)
                .unwrap_or(0.0)
        };
        let col = |c: usize| Vec3::new(m(0, c), m(1, c), m(2, c));
        let name = e
            .get("img_name")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("view {i}"));
        out.push(TrainingView {
            name,
            camera: Camera {
                pos: Vec3::new(pf(&p[0]), pf(&p[1]), pf(&p[2])),
                right: col(0),
                down: col(1),
                fwd: col(2),
                width: w as u32,
                height: h as u32,
                fx,
                fy,
                cx: w * 0.5,
                cy: h * 0.5,
                aperture: 0.0,
                focus: 3.0,
            },
        });
    }
    Ok(out)
}

/// The usual 3DGS output layout puts the PLY three levels below
/// `cameras.json` (`<scene>/point_cloud/iteration_N/point_cloud.ply`); look in
/// the PLY's own folder and up to three above it.
pub fn find_cameras_json(ply: &Path) -> Option<PathBuf> {
    let mut dir = ply.parent();
    for _ in 0..4 {
        let d = dir?;
        let c = d.join("cameras.json");
        if c.is_file() {
            return Some(c);
        }
        dir = d.parent();
    }
    None
}
