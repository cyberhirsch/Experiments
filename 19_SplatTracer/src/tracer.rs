//! The GPU tracer: device bring-up, scene buffers, proxy geometry, the
//! acceleration structure, and the prep -> trace sequence.
//!
//! Hardware ray tracing in wgpu is EXPERIMENTAL and Vulkan-only (measured on
//! the reference RTX 3090: DX12 exposes none of it). The version is pinned
//! with `=` in Cargo.toml for that reason - a minor bump is allowed to break
//! this API, and would. wgpu 30 also has no BVH refit ("only rebuild is
//! implemented"), which is why the proxies are camera-independent: see
//! `ensure_geometry`.

use crate::camera::Camera;
use crate::scene::Splats;
use crate::{sh, RenderParams, K_MAX};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::borrow::Cow;
use std::time::Instant;
use wgpu::util::DeviceExt;

const COMMON: &str = include_str!("shaders/common.wgsl");
pub const GEOM_WGSL: &str = include_str!("shaders/geom.wgsl");
pub const PREP_WGSL: &str = include_str!("shaders/prep.wgsl");
pub const TRACE_WGSL: &str = include_str!("shaders/trace.wgsl");
pub const DISPLAY_WGSL: &str = include_str!("shaders/display.wgsl");

/// WGSL has no include, and `enable` directives must precede every
/// declaration - so they are lifted out, then the shared header, then the rest.
pub fn compose(src: &str) -> String {
    let mut head = String::new();
    let mut body = String::new();
    for line in src.lines() {
        if line.trim_start().starts_with("enable ") {
            head.push_str(line);
            head.push('\n');
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    format!("{head}{COMMON}\n{body}")
}

/// The shape the RT cores test in place of each splat. Always a box: a box
/// is reported whenever the ray's remaining segment overlaps it, which is
/// what keeps re-casting exact (see trace.wgsl for why triangles are not).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Proxy {
    /// Axis-aligned, all splats in one BLAS, built on the GPU. Loose for thin
    /// splats tilted off the axes - most of the box is empty space, and every
    /// ray through it comes back to the shader to be rejected. The default:
    /// it is the one measured safe on a full-scale scan.
    #[default]
    Box,
    /// One TLAS instance per splat over a shared unit box, transformed into
    /// the splat's own frame: the box follows the splat's axes. Instances are
    /// written by the CPU in wgpu 30, so building costs more - but the
    /// geometry is camera-independent and rarely rebuilt. Unproven at full
    /// scale: its first 5.8 M-splat run coincided with a driver failure, so
    /// it only runs through `safety::ramp` until measured. NOTE the per-ray
    /// report cap does not bound this mode (see `RenderParams::max_reports`):
    /// an instance's world bounds are up to sqrt(3) looser per axis than the
    /// Box proxy's boxes, and entries that miss the tilted box are invisible
    /// to the shader.
    Oriented,
}

impl Proxy {
    pub fn instanced(self) -> bool {
        self == Proxy::Oriented
    }
    pub fn name(self) -> &'static str {
        match self {
            Proxy::Box => "box",
            Proxy::Oriented => "oriented box",
        }
    }
}

/// Mirror of `Frame` in common.wgsl: eleven vec4, 176 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FrameUniform {
    pub cam_pos: [f32; 4],
    pub cam_right: [f32; 4],
    pub cam_down: [f32; 4],
    pub cam_fwd: [f32; 4],
    pub img: [f32; 4],
    pub lens: [f32; 4],
    pub tone: [f32; 4],
    pub misc: [f32; 4],
    pub bg: [f32; 4],
    pub ints: [u32; 4],
    pub geo: [u32; 4],
}

impl FrameUniform {
    pub fn new(
        cam: &Camera,
        p: &RenderParams,
        n: u32,
        scene_degree: u32,
        frame: u32,
        jitter: bool,
        accumulate: bool,
    ) -> Self {
        Self {
            cam_pos: [cam.pos.x, cam.pos.y, cam.pos.z, p.near],
            cam_right: [cam.right.x, cam.right.y, cam.right.z, cam.fx],
            cam_down: [cam.down.x, cam.down.y, cam.down.z, cam.fy],
            cam_fwd: [cam.fwd.x, cam.fwd.y, cam.fwd.z, 0.0],
            img: [cam.width as f32, cam.height as f32, cam.cx, cam.cy],
            lens: [cam.aperture, cam.focus, if jitter { 1.0 } else { 0.0 }, 0.0],
            tone: [p.alpha_min, p.t_min, p.alpha_max, p.dilation_px2],
            // w: floats of SH stored per splat, so the shader can index a
            // compacted buffer
            misc: [p.cov_bias, 0.0, 0.0, sh::stride(scene_degree) as f32],
            bg: [
                p.background[0],
                p.background[1],
                p.background[2],
                if accumulate { 1.0 } else { 0.0 },
            ],
            ints: [n, p.k.clamp(1, K_MAX), p.sh_degree.min(scene_degree), frame],
            geo: [0, p.max_reports.max(1), p.flag_capped as u32, 0],
        }
    }

    pub fn with_stats(mut self, on: bool) -> Self {
        self.lens[3] = if on { 1.0 } else { 0.0 };
        self
    }

    pub fn with_tile(mut self, x0: u32, y0: u32) -> Self {
        self.misc[1] = x0 as f32;
        self.misc[2] = y0 as f32;
        self
    }
}

/// Mirror of `Geom` in geom.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GeomUniform {
    b2: f32,
    alpha_min: f32,
    n: u32,
    mode: u32,
}

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub name: String,
    pub limits: wgpu::Limits,
}

impl Gpu {
    /// Vulkan only: that is where wgpu exposes ray query.
    pub fn instance() -> wgpu::Instance {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::VULKAN;
        wgpu::Instance::new(desc)
    }

    pub fn headless() -> Result<Self, String> {
        Self::new(Self::instance(), None)
    }

    pub fn new(instance: wgpu::Instance, surface: Option<&wgpu::Surface<'_>>) -> Result<Self, String> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: surface,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .map_err(|e| format!("no Vulkan adapter: {e}"))?;
        let info = adapter.get_info();
        if !adapter.features().contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY) {
            return Err(format!(
                "{} does not expose hardware ray query through wgpu/Vulkan",
                info.name
            ));
        }
        let limits = adapter.limits();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("splat tracer"),
            required_features: wgpu::Features::EXPERIMENTAL_RAY_QUERY,
            required_limits: limits.clone(),
            // SAFETY: ray query is marked experimental in wgpu 30; accepting
            // that is the price of reaching the RT cores from this stack.
            experimental_features: unsafe { wgpu::ExperimentalFeatures::enabled() },
            ..Default::default()
        }))
        .map_err(|e| format!("device: {e}"))?;
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            name: format!("{} ({:?})", info.name, info.backend),
            limits,
        })
    }

    pub fn wait(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }
}

pub struct SceneGpu {
    pub n: u32,
    pub sh_degree: u32,
    pub proxy: Proxy,
    _pos_op: wgpu::Buffer,
    _scl: wgpu::Buffer,
    _rot: wgpu::Buffer,
    _sh: wgpu::Buffer,
    pub derived: wgpu::Buffer,
    /// Box: one AABB per splat. Oriented: the single unit box [-1, 1]^3.
    aabb: wgpu::Buffer,
    aabb_size: wgpu::BlasAABBGeometrySizeDescriptor,
    /// Oriented only: what the CPU needs to write instance transforms
    cpu: Option<CpuSplats>,
    pub blas: wgpu::Blas,
    pub tlas: wgpu::Tlas,
    prep_bg: wgpu::BindGroup,
    geom_bg: wgpu::BindGroup,
    geom_uniform: wgpu::Buffer,
    /// bounds of the splat centres, for the dilation bound
    lo: Vec3,
    hi: Vec3,
    built_b2: Option<f32>,
    built_alpha: f32,
    /// Oriented: instances whose transform came out non-finite and were
    /// masked off rather than handed to the BVH builder
    pub invalid_instances: u32,
    pub bytes: u64,
    pub builds: u32,
    /// wall clock of the last build, CPU work included
    pub last_build_ms: f64,
    /// submit-to-done of the last build: the GPU job the OS watchdog sees.
    /// wgpu serialises TLAS instances on the CPU while encoding, so finishing
    /// the encoder first keeps that out of this number.
    pub last_build_gpu_ms: f64,
}

struct CpuSplats {
    pos: Vec<[f32; 3]>,
    scale: Vec<[f32; 3]>,
    rot: Vec<[f32; 4]>,
    opacity: Vec<f32>,
    /// smallest proxy half-axis: keeps instance matrices invertible for the
    /// hardware when a splat is thinner than float precision can carry
    floor: f32,
}

/// What `time` measured, in milliseconds.
#[derive(Clone, Copy, Debug, Default)]
pub struct Timing {
    /// proxy generation + BVH build - paid at load and when the view leaves
    /// the dilation bound, not per frame
    pub build_ms: f64,
    /// per-camera preparation: colour, exact dilation, culling
    pub prep_ms: f64,
    /// one traced sample over the whole image
    pub trace_ms: f64,
}

pub struct Tracer {
    /// hit-buffer capacity the trace shader was compiled with
    pub kmax: u32,
    geom: wgpu::ComputePipeline,
    prep: wgpu::ComputePipeline,
    trace: wgpu::ComputePipeline,
    geom_bgl: wgpu::BindGroupLayout,
    prep_bgl: wgpu::BindGroupLayout,
    trace_bgl: wgpu::BindGroupLayout,
    pub frame: wgpu::Buffer,
    pub scene: Option<SceneGpu>,
    pub accum: wgpu::Buffer,
    /// eight u32 of traversal statistics per pixel, filled when requested
    pub stats: wgpu::Buffer,
    /// four atomic u32; [0] = rays that hit the work cap in the last sample
    counters: wgpu::Buffer,
    pub width: u32,
    pub height: u32,
    /// Edge of the square tile one submission traces. Small enough that a
    /// single submission on a heavy scene stays in the milliseconds.
    pub tile: u32,
    trace_bg: Option<wgpu::BindGroup>,
}

fn storage(b: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: b,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform(b: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: b,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn dispatch_2d(cp: &mut wgpu::ComputePass<'_>, n: u32) {
    // one dimension caps at 65535 workgroups; spread over y and let the
    // shader rebuild the linear index from num_workgroups
    let groups = n.div_ceil(256).max(1);
    let x = groups.min(65535);
    cp.dispatch_workgroups(x, groups.div_ceil(x), 1);
}

impl Tracer {
    pub fn new(gpu: &Gpu) -> Self {
        Self::with_kmax(gpu, 32)
    }

    pub fn with_kmax(gpu: &Gpu, kmax: u32) -> Self {
        let kmax = kmax.clamp(1, K_MAX);
        let d = &gpu.device;
        let module = |label: &str, src: &str| {
            d.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(Cow::Owned(compose(src))),
            })
        };
        let geom_m = module("geom", GEOM_WGSL);
        let prep_m = module("prep", PREP_WGSL);
        let trace_src = TRACE_WGSL.replace("const KMAX: u32 = 16u;", &format!("const KMAX: u32 = {kmax}u;"));
        let trace_m = module("trace", &trace_src);

        let geom_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("geom bgl"),
            entries: &[
                uniform(0),
                storage(1, true),
                storage(2, true),
                storage(3, true),
                storage(4, false),
            ],
        });
        let prep_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("prep bgl"),
            entries: &[
                uniform(0),
                storage(1, true),
                storage(2, true),
                storage(3, true),
                storage(4, true),
                storage(5, false),
            ],
        });
        let trace_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("trace bgl"),
            entries: &[
                uniform(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::AccelerationStructure { vertex_return: false },
                    count: None,
                },
                storage(2, true),
                storage(3, false),
                storage(4, false),
                storage(5, false),
            ],
        });
        let pipe = |label: &str, bgl: &wgpu::BindGroupLayout, m: &wgpu::ShaderModule| {
            let pl = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(bgl)],
                immediate_size: 0,
            });
            d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                module: m,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let geom = pipe("geom", &geom_bgl, &geom_m);
        let prep = pipe("prep", &prep_bgl, &prep_m);
        let trace = pipe("trace", &trace_bgl, &trace_m);
        let frame = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let accum = Self::make_accum(d, 1, 1);
        let stats = Self::make_stats(d, 1, 1);
        let counters = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("counters"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            kmax,
            geom,
            prep,
            trace,
            geom_bgl,
            prep_bgl,
            trace_bgl,
            frame,
            scene: None,
            accum,
            stats,
            counters,
            width: 1,
            height: 1,
            // start small; the app grows it once it has seen how long a tile takes
            tile: 64,
            trace_bg: None,
        }
    }

    fn make_accum(d: &wgpu::Device, w: u32, h: u32) -> wgpu::Buffer {
        d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("accum"),
            size: (w.max(1) as u64) * (h.max(1) as u64) * 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn make_stats(d: &wgpu::Device, w: u32, h: u32) -> wgpu::Buffer {
        d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("stats"),
            size: (w.max(1) as u64) * (h.max(1) as u64) * 32,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    /// Upload a scene. The proxies and the BVH are built on first use, by
    /// `ensure_geometry`; everything camera-dependent waits for `prepare`.
    pub fn set_scene(&mut self, gpu: &Gpu, s: &Splats, proxy: Proxy) -> Result<(), String> {
        let d = &gpu.device;
        let n = s.len() as u32;
        if n == 0 {
            return Err("scene has no splats".into());
        }
        let bind_cap = gpu.limits.max_storage_buffer_binding_size as u64;
        let gib = |b: u64| b as f64 / (1u64 << 30) as f64;
        let sh_bytes = s.sh.len() as u64 * 4;
        let aabb_bytes = if proxy == Proxy::Box { n as u64 * 24 } else { 24 };
        for (what, b) in [("SH coefficients", sh_bytes), ("proxy boxes", aabb_bytes)] {
            if b > bind_cap {
                return Err(format!(
                    "{n} splats need {:.2} GiB of {what}; one storage binding holds {:.2} GiB \
                     on this device. Splitting across bindings is not in v001.",
                    gib(b),
                    gib(bind_cap)
                ));
            }
        }
        match proxy {
            Proxy::Box if n > gpu.limits.max_blas_primitive_count => {
                return Err(format!(
                    "{n} splats exceed the device's {} primitives per BLAS",
                    gpu.limits.max_blas_primitive_count
                ));
            }
            // an instance's custom index has 24 bits, and the TLAS its own cap
            Proxy::Oriented if n > gpu.limits.max_tlas_instance_count.min(1 << 24) => {
                return Err(format!(
                    "{n} splats exceed {} instances per TLAS - use the Box proxy",
                    gpu.limits.max_tlas_instance_count.min(1 << 24)
                ));
            }
            _ => {}
        }

        let pos_op: Vec<[f32; 4]> = s
            .pos
            .iter()
            .zip(&s.opacity)
            .map(|(p, o)| [p[0], p[1], p[2], *o])
            .collect();
        let scl: Vec<[f32; 4]> = s.scale.iter().map(|v| [v[0], v[1], v[2], 0.0]).collect();
        let init = |label: &str, bytes: &[u8]| {
            d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytes,
                usage: wgpu::BufferUsages::STORAGE,
            })
        };
        let pos_op_b = init("pos_op", bytemuck::cast_slice(&pos_op));
        let scl_b = init("scale", bytemuck::cast_slice(&scl));
        let rot_b = init("rot", bytemuck::cast_slice(&s.rot));
        let sh_b = init("sh", bytemuck::cast_slice(&s.sh));
        let derived = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("derived"),
            size: n as u64 * 64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        // Box: written by the geom shader, then read by the BVH build.
        // Oriented: the one unit box every instance shares, written here.
        let aabb = if proxy == Proxy::Box {
            d.create_buffer(&wgpu::BufferDescriptor {
                label: Some("aabb"),
                size: aabb_bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::BLAS_INPUT,
                mapped_at_creation: false,
            })
        } else {
            d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("unit box"),
                contents: bytemuck::cast_slice(&[-1.0f32, -1.0, -1.0, 1.0, 1.0, 1.0]),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::BLAS_INPUT,
            })
        };
        // one splat reported twice by one traversal would be composited twice;
        // the hardware promises not to, and the shader dedupes anyway
        let aabb_size = wgpu::BlasAABBGeometrySizeDescriptor {
            primitive_count: if proxy == Proxy::Box { n } else { 1 },
            flags: wgpu::AccelerationStructureGeometryFlags::NO_DUPLICATE_ANY_HIT_INVOCATION,
        };
        let blas = d.create_blas(
            &wgpu::CreateBlasDescriptor {
                label: Some("splats"),
                flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
                update_mode: wgpu::AccelerationStructureUpdateMode::Build,
            },
            wgpu::BlasGeometrySizeDescriptors::AABBs { descriptors: vec![aabb_size.clone()] },
        );
        let tlas = d.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some("scene"),
            max_instances: if proxy == Proxy::Box { 1 } else { n },
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        });

        let geom_uniform = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("geom params"),
            size: std::mem::size_of::<GeomUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let geom_bg = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("geom bg"),
            layout: &self.geom_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: geom_uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: pos_op_b.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: scl_b.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: rot_b.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: aabb.as_entire_binding() },
            ],
        });
        let prep_bg = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("prep bg"),
            layout: &self.prep_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.frame.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: pos_op_b.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: scl_b.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: rot_b.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: sh_b.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: derived.as_entire_binding() },
            ],
        });

        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for p in &s.pos {
            let v = Vec3::from(*p);
            lo = lo.min(v);
            hi = hi.max(v);
        }
        let cpu = if proxy.instanced() {
            let diag = (hi - lo).length().max(1e-6);
            Some(CpuSplats {
                pos: s.pos.clone(),
                scale: s.scale.clone(),
                rot: s.rot.clone(),
                opacity: s.opacity.clone(),
                floor: diag * 1e-7,
            })
        } else {
            None
        };
        // instances: 64 bytes each on the GPU
        let inst_bytes = if proxy.instanced() { n as u64 * 64 } else { 0 };
        let bytes = n as u64 * (16 + 16 + 16 + 64) + sh_bytes + aabb_bytes + inst_bytes;
        self.scene = Some(SceneGpu {
            n,
            sh_degree: s.sh_degree,
            proxy,
            _pos_op: pos_op_b,
            _scl: scl_b,
            _rot: rot_b,
            _sh: sh_b,
            derived,
            aabb,
            aabb_size,
            cpu,
            blas,
            tlas,
            prep_bg,
            geom_bg,
            geom_uniform,
            lo,
            hi,
            built_b2: None,
            built_alpha: 0.0,
            invalid_instances: 0,
            bytes,
            builds: 0,
            last_build_ms: 0.0,
            last_build_gpu_ms: 0.0,
        });
        self.trace_bg = None;
        Ok(())
    }

    /// Make sure the proxies contain every splat as THIS camera will evaluate
    /// it, rebuilding them and the BVH only if not. The 3DGS dilation grows
    /// with depth, so the proxies are sized for the farthest splat centre this
    /// view can see, with 1.5x headroom so that ordinary orbiting does not
    /// trigger rebuilds; they are rebuilt tighter if the bound becomes 4x
    /// looser than needed. Returns the build time if a build happened.
    pub fn ensure_geometry(&mut self, gpu: &Gpu, cam: &Camera, p: &RenderParams) -> Option<f64> {
        let s = self.scene.as_mut()?;
        let mut dmax = 0.0f32;
        for c in 0..8 {
            let corner = Vec3::new(
                if c & 1 == 0 { s.lo.x } else { s.hi.x },
                if c & 2 == 0 { s.lo.y } else { s.hi.y },
                if c & 4 == 0 { s.lo.z } else { s.hi.z },
            );
            dmax = dmax.max((corner - cam.pos).length());
        }
        let f = (cam.fx * cam.fy).sqrt().max(1e-6);
        let need = p.dilation_px2 * (dmax / f).powi(2) + p.cov_bias * p.cov_bias;
        let rebuild = match s.built_b2 {
            None => true,
            Some(b) => need > b || need < b / 16.0 || p.alpha_min != s.built_alpha,
        };
        if !rebuild {
            return None;
        }
        let b2 = need * 2.25;
        let t = Instant::now();
        let mut enc = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("geometry") });
        match &s.cpu {
            None => {
                // Box: boxes on the GPU, one BLAS over all of them
                let gu = GeomUniform { b2, alpha_min: p.alpha_min, n: s.n, mode: 0 };
                gpu.queue.write_buffer(&s.geom_uniform, 0, bytemuck::bytes_of(&gu));
                let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("geom"),
                    timestamp_writes: None,
                });
                cp.set_pipeline(&self.geom);
                cp.set_bind_group(0, &s.geom_bg, &[]);
                dispatch_2d(&mut cp, s.n);
                drop(cp);
                s.tlas[0] = Some(wgpu::TlasInstance::new(
                    &s.blas,
                    [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                    0,
                    0xFF,
                ));
            }
            Some(c) => {
                // Oriented: the unit box mapped onto each splat's own axes,
                // scaled to the alpha_min contour. A faint splat gets mask 0 and
                // is never visited at all.
                use rayon::prelude::*;
                let alpha_min = p.alpha_min;
                let floor = c.floor;
                // a non-finite transform poisons the BVH, and a poisoned BVH is
                // a classic way to hang the RT cores: validate every one
                let xf: Vec<Option<[f32; 12]>> = (0..s.n as usize)
                    .into_par_iter()
                    .map(|i| {
                        let op = c.opacity[i];
                        if op < alpha_min {
                            return None;
                        }
                        let r = crate::reference::rot_rows(c.rot[i]);
                        let sc = c.scale[i];
                        let ks = (2.0 * (op / alpha_min).ln()).sqrt();
                        let se = [
                            (sc[0] * sc[0] + b2).sqrt().max(floor) * ks,
                            (sc[1] * sc[1] + b2).sqrt().max(floor) * ks,
                            (sc[2] * sc[2] + b2).sqrt().max(floor) * ks,
                        ];
                        let m = c.pos[i];
                        let x = [
                            r[0].x * se[0], r[0].y * se[1], r[0].z * se[2], m[0],
                            r[1].x * se[0], r[1].y * se[1], r[1].z * se[2], m[1],
                            r[2].x * se[0], r[2].y * se[1], r[2].z * se[2], m[2],
                        ];
                        if x.iter().all(|v| v.is_finite()) { Some(x) } else { None }
                    })
                    .collect();
                let invalid = (0..s.n as usize)
                    .filter(|&i| c.opacity[i] >= alpha_min && xf[i].is_none())
                    .count() as u32;
                s.invalid_instances = invalid;
                if let Some(slots) = s.tlas.get_mut_slice(0..s.n as usize) {
                    for (i, (slot, x)) in slots.iter_mut().zip(xf).enumerate() {
                        *slot = Some(match x {
                            Some(m) => wgpu::TlasInstance::new(&s.blas, m, i as u32, 0xFF),
                            None => wgpu::TlasInstance::new(&s.blas, [0.0; 12], i as u32, 0),
                        });
                    }
                }
            }
        }
        enc.build_acceleration_structures(
            std::iter::once(&wgpu::BlasBuildEntry {
                blas: &s.blas,
                geometry: wgpu::BlasGeometries::AabbGeometries(vec![wgpu::BlasAabbGeometry {
                    size: &s.aabb_size,
                    stride: 24,
                    aabb_buffer: &s.aabb,
                    primitive_offset: 0,
                }]),
            }),
            std::iter::once(&s.tlas),
        );
        let cb = enc.finish();
        let t_gpu = std::time::Instant::now();
        gpu.queue.submit(Some(cb));
        gpu.wait();
        s.last_build_gpu_ms = t_gpu.elapsed().as_secs_f64() * 1e3;
        let ms = t.elapsed().as_secs_f64() * 1e3;
        s.built_b2 = Some(b2);
        s.built_alpha = p.alpha_min;
        s.builds += 1;
        s.last_build_ms = ms;
        Some(ms)
    }

    pub fn resize(&mut self, gpu: &Gpu, w: u32, h: u32) {
        let (w, h) = (w.max(1), h.max(1));
        if (w, h) == (self.width, self.height) {
            return;
        }
        self.accum = Self::make_accum(&gpu.device, w, h);
        self.stats = Self::make_stats(&gpu.device, w, h);
        self.width = w;
        self.height = h;
        self.trace_bg = None;
    }

    fn trace_bind_group(&mut self, gpu: &Gpu) -> Option<&wgpu::BindGroup> {
        if self.trace_bg.is_none() {
            let s = self.scene.as_ref()?;
            self.trace_bg = Some(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("trace bg"),
                layout: &self.trace_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.frame.as_entire_binding() },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::AccelerationStructure(&s.tlas),
                    },
                    wgpu::BindGroupEntry { binding: 2, resource: s.derived.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: self.accum.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: self.stats.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 5, resource: self.counters.as_entire_binding() },
                ],
            }));
        }
        self.trace_bg.as_ref()
    }

    /// Write the frame uniform, stamping in what only the tracer knows.
    pub fn write_frame(&self, gpu: &Gpu, f: &FrameUniform) {
        let mut f = *f;
        f.geo[0] = self.scene.as_ref().map(|s| s.proxy.instanced() as u32).unwrap_or(0);
        gpu.queue.write_buffer(&self.frame, 0, bytemuck::bytes_of(&f));
    }

    /// Per-camera preparation: colour, exact dilation, culling. Cheap - one
    /// pass over the splats - and no BVH work.
    pub fn encode_prepare(&self, enc: &mut wgpu::CommandEncoder) {
        let Some(s) = self.scene.as_ref() else { return };
        let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("prep"),
            timestamp_writes: None,
        });
        cp.set_pipeline(&self.prep);
        cp.set_bind_group(0, &s.prep_bg, &[]);
        dispatch_2d(&mut cp, s.n);
    }

    fn encode_trace_tile(&mut self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, tw: u32, th: u32) {
        let trace = self.trace.clone();
        let Some(bg) = self.trace_bind_group(gpu) else { return };
        let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("trace tile"),
            timestamp_writes: None,
        });
        cp.set_pipeline(&trace);
        cp.set_bind_group(0, bg, &[]);
        cp.dispatch_workgroups(tw.div_ceil(8), th.div_ceil(8), 1);
    }

    /// One sample for every pixel, tile by tile, each tile its own submission.
    /// `prepare` runs the per-camera pass first. The geometry must already
    /// cover this camera - call `ensure_geometry` when the view changes.
    /// Returns the number of submissions made.
    pub fn trace_sample(&mut self, gpu: &Gpu, base: &FrameUniform, prepare: bool) -> u32 {
        self.trace_tiles(gpu, base, prepare, false).0
    }

    /// Same, but waits for each tile and times it: the per-job wall clock the
    /// OS watchdog cares about. Slower - for measuring, not for rendering.
    pub fn trace_sample_timed(&mut self, gpu: &Gpu, base: &FrameUniform, prepare: bool) -> Vec<f64> {
        self.trace_tiles(gpu, base, prepare, true).1
    }

    fn trace_tiles(&mut self, gpu: &Gpu, base: &FrameUniform, prepare: bool, timed: bool) -> (u32, Vec<f64>) {
        if self.scene.is_none() {
            return (0, Vec::new());
        }
        let t = self.tile.clamp(32, 4096);
        let mut n = 0;
        let mut times = Vec::new();
        let mut y0 = 0;
        while y0 < self.height {
            let mut x0 = 0;
            while x0 < self.width {
                // write-then-submit per tile: a buffer written twice before one
                // submit is seen with its last value by every dispatch in it
                self.write_frame(gpu, &base.with_tile(x0, y0));
                let mut enc = gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("tile") });
                if n == 0 {
                    enc.clear_buffer(&self.counters, 0, None);
                    if prepare {
                        self.encode_prepare(&mut enc);
                    }
                }
                let tw = t.min(self.width - x0);
                let th = t.min(self.height - y0);
                self.encode_trace_tile(gpu, &mut enc, tw, th);
                let start = Instant::now();
                gpu.queue.submit(Some(enc.finish()));
                if timed {
                    gpu.wait();
                    times.push(start.elapsed().as_secs_f64() * 1e3);
                }
                n += 1;
                x0 += t;
            }
            y0 += t;
        }
        (n, times)
    }

    /// Rays that hit the per-ray work cap in the last traced sample.
    pub fn read_capped(&self, gpu: &Gpu) -> u32 {
        let raw = self.readback(gpu, &self.counters, 16);
        bytemuck::cast_slice::<u8, u32>(&raw)[0]
    }

    fn readback(&self, gpu: &Gpu, src: &wgpu::Buffer, bytes: u64) -> Vec<u8> {
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(src, 0, &staging, 0, bytes);
        gpu.queue.submit(Some(enc.finish()));
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        gpu.wait();
        rx.recv().expect("map").expect("map failed");
        let data = slice.get_mapped_range().expect("mapped range");
        let out = data[..].to_vec();
        drop(data);
        staging.unmap();
        out
    }

    /// Read the accumulation buffer and resolve it: colour / sample count.
    pub fn read_image(&self, gpu: &Gpu) -> Vec<[f32; 3]> {
        let raw = self.readback(gpu, &self.accum, self.width as u64 * self.height as u64 * 16);
        bytemuck::cast_slice::<u8, [f32; 4]>(&raw)
            .iter()
            .map(|a| {
                let n = a[3].max(1.0);
                [a[0] / n, a[1] / n, a[2] / n]
            })
            .collect()
    }

    /// Per-pixel traversal counters from the last traced sample (only
    /// meaningful if it was traced with the stats flag on).
    pub fn read_stats(&self, gpu: &Gpu) -> Vec<[u32; 8]> {
        let raw = self.readback(gpu, &self.stats, self.width as u64 * self.height as u64 * 32);
        bytemuck::cast_slice::<u8, [u32; 8]>(&raw).to_vec()
    }

    /// Headless: prepare once, trace `spp` samples, resolve. With `jitter`
    /// off every sample is the pixel centre - the setting the references use.
    pub fn render(&mut self, gpu: &Gpu, cam: &Camera, p: &RenderParams, spp: u32, jitter: bool) -> Vec<[f32; 3]> {
        self.resize(gpu, cam.width, cam.height);
        let (n, deg) = match &self.scene {
            Some(s) => (s.n, s.sh_degree),
            None => return vec![[0.0; 3]; (cam.width * cam.height) as usize],
        };
        self.ensure_geometry(gpu, cam, p);
        for f in 0..spp.max(1) {
            let base = FrameUniform::new(cam, p, n, deg, f, jitter, f > 0);
            self.trace_sample(gpu, &base, f == 0);
            // never let more than one sample queue up ahead of the GPU
            gpu.wait();
        }
        self.read_image(gpu)
    }

    /// Medians over `reps` of: a forced proxy + BVH build, the per-camera
    /// pass, and one traced sample. Wall clock including submission - what
    /// an interactive frame actually pays.
    pub fn time(&mut self, gpu: &Gpu, cam: &Camera, p: &RenderParams, reps: u32) -> Timing {
        self.resize(gpu, cam.width, cam.height);
        let Some((n, deg)) = self.scene.as_ref().map(|s| (s.n, s.sh_degree)) else {
            return Timing::default();
        };
        let base = FrameUniform::new(cam, p, n, deg, 0, false, false);
        let (mut b, mut pr, mut tr) = (Vec::new(), Vec::new(), Vec::new());
        for _ in 0..reps.max(1) {
            if let Some(s) = self.scene.as_mut() {
                s.built_b2 = None;
            }
            b.push(self.ensure_geometry(gpu, cam, p).unwrap_or(0.0));

            self.write_frame(gpu, &base);
            let t = Instant::now();
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            self.encode_prepare(&mut enc);
            gpu.queue.submit(Some(enc.finish()));
            gpu.wait();
            pr.push(t.elapsed().as_secs_f64() * 1e3);

            let t = Instant::now();
            self.trace_sample(gpu, &base, false);
            gpu.wait();
            tr.push(t.elapsed().as_secs_f64() * 1e3);
        }
        let med = |v: &mut Vec<f64>| {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[v.len() / 2]
        };
        Timing { build_ms: med(&mut b), prep_ms: med(&mut pr), trace_ms: med(&mut tr) }
    }
}
