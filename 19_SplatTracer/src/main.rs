//! № 19 — Splat Tracer, v001.
//!
//! Drop a 3DGS `.ply` on the window (or pass it on the command line). If a
//! `cameras.json` sits beside it, the training views are offered and the
//! orbit takes its "up" from the capture.

use glam::Vec3;
use splat_tracer::camera::{self, Camera, TrainingView};
use splat_tracer::reference;
use splat_tracer::scene::{self, Splats};
use splat_tracer::tracer::{FrameUniform, Gpu, Proxy, Tracer};
use splat_tracer::RenderParams;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

const PANEL_W: f32 = 340.0;

// ------------------------------------------------------------------ view ---

#[derive(Clone, Copy, PartialEq)]
enum Up {
    NegY,
    PosY,
    PosZ,
    NegZ,
    Capture,
}

struct Orbit {
    target: Vec3,
    yaw: f32,
    pitch: f32,
    dist: f32,
    up: Up,
    capture_up: Vec3,
    fov_deg: f32,
}

impl Orbit {
    fn up_vec(&self) -> Vec3 {
        match self.up {
            Up::NegY => Vec3::NEG_Y,
            Up::PosY => Vec3::Y,
            Up::PosZ => Vec3::Z,
            Up::NegZ => Vec3::NEG_Z,
            Up::Capture => self.capture_up,
        }
    }
    /// Two horizontal axes for the chosen up.
    fn frame(&self) -> (Vec3, Vec3, Vec3) {
        let u = self.up_vec().normalize_or(Vec3::NEG_Y);
        let a = u.any_orthonormal_vector();
        let b = u.cross(a);
        (a, b, u)
    }
    fn eye(&self) -> Vec3 {
        let (a, b, u) = self.frame();
        let (sp, cp) = self.pitch.sin_cos();
        let (sy, cy) = self.yaw.sin_cos();
        self.target + self.dist * (cp * (cy * a + sy * b) + sp * u)
    }
    fn camera(&self, w: u32, h: u32) -> Camera {
        Camera::look_at(self.eye(), self.target, self.up_vec(), w, h, self.fov_deg.to_radians())
    }
    /// Take over from a fixed camera without a jump: target straight ahead
    /// at `dist`, angles recovered in this orbit's own frame.
    fn adopt(&mut self, cam: &Camera, dist: f32) {
        self.dist = dist.max(1e-3);
        self.target = cam.pos + cam.fwd * self.dist;
        let (a, b, u) = self.frame();
        let v = (cam.pos - self.target) / self.dist;
        self.pitch = v.dot(u).clamp(-1.0, 1.0).asin();
        self.yaw = v.dot(b).atan2(v.dot(a));
        self.fov_deg = cam.fov_y().to_degrees();
    }
}

/// A training camera placed in a viewport of different aspect: keep its
/// vertical field of view, keep pixels square, re-centre.
fn fit(cam: &Camera, w: u32, h: u32) -> Camera {
    let s = h as f32 / cam.height.max(1) as f32;
    let f = cam.fy * s;
    Camera { width: w, height: h, fx: f, fy: f, cx: w as f32 * 0.5, cy: h as f32 * 0.5, ..*cam }
}

// --------------------------------------------------------------- display ---

struct Display {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    view: wgpu::Buffer,
    bg: Option<(wgpu::BindGroup, (u32, u32))>,
}

impl Display {
    fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Self {
        let d = &gpu.device;
        let m = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("display"),
            source: wgpu::ShaderSource::Wgsl(
                splat_tracer::tracer::compose(splat_tracer::tracer::DISPLAY_WGSL).into(),
            ),
        });
        let bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("display bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pl = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("display"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("display"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &m,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &m,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let view = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("display view"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { pipeline, bgl, view, bg: None }
    }

    fn bind_group(&mut self, gpu: &Gpu, t: &Tracer) -> &wgpu::BindGroup {
        let key = (t.width, t.height);
        if self.bg.as_ref().map(|b| b.1) != Some(key) {
            let bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("display bg"),
                layout: &self.bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: t.accum.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.view.as_entire_binding() },
                ],
            });
            self.bg = Some((bg, key));
        }
        &self.bg.as_ref().unwrap().0
    }
}

// ----------------------------------------------------------------- state ---

struct Loaded {
    /// kept so the proxy kind can be switched without re-reading the file
    splats: Arc<Splats>,
    path: PathBuf,
    n: usize,
    degree: u32,
    load_s: f32,
    upload_s: f32,
    gpu_bytes: u64,
    views: Vec<TrainingView>,
    center: Vec3,
    extent: f32,
}

enum LoadMsg {
    Done(Result<(Splats, PathBuf, f32), String>),
}

struct Ui {
    params: RenderParams,
    proxy: Proxy,
    max_spp: u32,
    jitter: bool,
    aperture: f32,
    focus: f32,
    view: Option<usize>, // training view index, None = orbit
    status: String,
    /// set by the watchdog when tiles run too long; cleared by Restart
    paused: bool,
    capped: u32,
}

struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    gpu: Gpu,
    config: wgpu::SurfaceConfiguration,
    tracer: Tracer,
    display: Display,

    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,

    ui: Ui,
    orbit: Orbit,
    loaded: Option<Loaded>,
    loader: Option<Receiver<LoadMsg>>,
    loading_path: Option<PathBuf>,

    spp: u32,
    dirty: bool,
    render_w: u32,
    render_h: u32,

    rotating: bool,
    panning: bool,
    last_cursor: Option<(f32, f32)>,
    over_ui: bool,

    last_frame: Instant,
    frame_ms: f32,
}

impl State {
    fn new(window: Arc<Window>) -> Result<Self, String> {
        let instance = Gpu::instance();
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("surface: {e}"))?;
        let gpu = Gpu::new(instance, Some(&surface))?;
        let size = window.inner_size();
        let caps = surface.get_capabilities(&gpu.adapter);
        // 3DGS colour is already display space: write it untouched
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
            // a progressive renderer should run flat out, not at refresh rate
            wgpu::PresentMode::Immediate
        } else {
            wgpu::PresentMode::AutoVsync
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&gpu.device, &config);
        let tracer = Tracer::new(&gpu);
        let display = Display::new(&gpu, format);

        let egui_ctx = egui::Context::default();
        egui_ctx.set_visuals(egui::Visuals::dark());
        let egui_state =
            egui_winit::State::new(egui_ctx.clone(), egui::ViewportId::ROOT, &window, None, None, None);
        let egui_renderer =
            egui_wgpu::Renderer::new(&gpu.device, format, egui_wgpu::RendererOptions::default());

        Ok(Self {
            window,
            surface,
            gpu,
            config,
            tracer,
            display,
            egui_ctx,
            egui_state,
            egui_renderer,
            ui: Ui {
                params: RenderParams::default(),
                proxy: Proxy::default(),
                max_spp: 256,
                jitter: true,
                aperture: 0.0,
                focus: 3.0,
                view: None,
                status: "Drop a 3DGS .ply onto the window.".into(),
                paused: false,
                capped: 0,
            },
            orbit: Orbit {
                target: Vec3::ZERO,
                yaw: 0.6,
                pitch: 0.3,
                dist: 4.0,
                up: Up::NegY,
                capture_up: Vec3::NEG_Y,
                fov_deg: 50.0,
            },
            loaded: None,
            loader: None,
            loading_path: None,
            spp: 0,
            dirty: true,
            render_w: 1,
            render_h: 1,
            rotating: false,
            panning: false,
            last_cursor: None,
            over_ui: false,
            last_frame: Instant::now(),
            frame_ms: 0.0,
        })
    }

    fn start_load(&mut self, path: PathBuf) {
        if self.loader.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let p = path.clone();
        std::thread::spawn(move || {
            let t = Instant::now();
            let r = scene::load_ply(&p).map(|s| (s, p, t.elapsed().as_secs_f32()));
            let _ = tx.send(LoadMsg::Done(r));
        });
        self.ui.status = format!("loading {} …", path.display());
        self.loading_path = Some(path);
        self.loader = Some(rx);
    }

    fn poll_load(&mut self) {
        let Some(rx) = self.loader.as_ref() else { return };
        let Ok(LoadMsg::Done(r)) = rx.try_recv() else { return };
        self.loader = None;
        self.loading_path = None;
        match r {
            Err(e) => self.ui.status = format!("load failed: {e}"),
            Ok((s, path, load_s)) => {
                let t = Instant::now();
                if let Err(e) = self.tracer.set_scene(&self.gpu, &s, self.ui.proxy) {
                    self.ui.status = format!("upload failed: {e}");
                    return;
                }
                self.gpu.wait();
                let upload_s = t.elapsed().as_secs_f32();
                let (c, e) = s.robust_bounds();
                let center = Vec3::from(c);
                let extent = Vec3::from(e).max_element().max(1e-3);
                let views = camera::find_cameras_json(&path)
                    .and_then(|p| camera::load_cameras_json(&p).ok())
                    .unwrap_or_default();
                self.orbit.target = center;
                self.orbit.dist = extent * 1.6;
                self.ui.focus = self.orbit.dist;
                if let Some(v0) = views.first() {
                    // the capture knows which way is up; the file does not
                    self.orbit.capture_up = -v0.camera.down;
                    self.orbit.up = Up::Capture;
                    self.ui.view = Some(0);
                    self.ui.focus = (center - v0.camera.pos).dot(v0.camera.fwd).max(0.1);
                } else {
                    self.ui.view = None;
                }
                self.ui.status = format!(
                    "{} splats · SH degree {} · {} training views",
                    fmt_count(s.len()),
                    s.sh_degree,
                    views.len()
                );
                let gpu_bytes = self.tracer.scene.as_ref().map(|g| g.bytes).unwrap_or(0);
                let n = s.len();
                let degree = s.sh_degree;
                self.loaded = Some(Loaded {
                    splats: Arc::new(s),
                    path,
                    n,
                    degree,
                    load_s,
                    upload_s,
                    gpu_bytes,
                    views,
                    center,
                    extent,
                });
                self.dirty = true;
            }
        }
    }

    fn current_camera(&self) -> Camera {
        let (w, h) = (self.render_w, self.render_h);
        let mut cam = match (self.ui.view, &self.loaded) {
            (Some(i), Some(l)) if i < l.views.len() => fit(&l.views[i].camera, w, h),
            _ => self.orbit.camera(w, h),
        };
        cam.aperture = self.ui.aperture;
        cam.focus = self.ui.focus;
        cam
    }

    /// Leave a training view for the orbit without the picture jumping.
    fn detach_to_orbit(&mut self) {
        if let (Some(i), Some(l)) = (self.ui.view, &self.loaded) {
            if let Some(v) = l.views.get(i) {
                let cam = v.camera;
                let d = (l.center - cam.pos).dot(cam.fwd).max(l.extent * 0.25);
                self.orbit.adopt(&cam, d);
            }
        }
        self.ui.view = None;
    }

    fn save_png(&mut self) {
        let Some(l) = &self.loaded else { return };
        let img = self.tracer.read_image(&self.gpu);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let dir = l.path.parent().unwrap_or(Path::new("."));
        let out = dir.join(format!("splat_tracer_{stamp}_{}spp.png", self.spp));
        self.ui.status = match reference::save_png(&out, self.tracer.width, self.tracer.height, &img) {
            Ok(()) => format!("saved {}", out.display()),
            Err(e) => format!("save failed: {e}"),
        };
    }

    // ------------------------------------------------------------- panel ---

    fn build_ui(&mut self, root: &mut egui::Ui) -> (f32, bool) {
        let mut dirty = false;
        let mut save = false;
        let mut switch_proxy = false;
        let fps = if self.frame_ms > 0.0 { 1000.0 / self.frame_ms } else { 0.0 };
        let px = (self.render_w as f64) * (self.render_h as f64);
        let resp = egui::Panel::right("panel").default_size(PANEL_W).show(root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("№ 19 — Splat Tracer");
                ui.label(egui::RichText::new("gaussian splats on the RT cores · v001").italics().weak());
                ui.separator();
                ui.label(egui::RichText::new(&self.ui.status).small());
                ui.label(egui::RichText::new(&self.gpu.name).weak().small());

                if let Some(l) = &self.loaded {
                    ui.separator();
                    ui.label(egui::RichText::new("SCENE").strong());
                    ui.label(format!(
                        "{} splats · SH {} · {:.2} GiB on GPU",
                        fmt_count(l.n),
                        l.degree,
                        l.gpu_bytes as f64 / (1u64 << 30) as f64
                    ));
                    ui.label(
                        egui::RichText::new(format!(
                            "parse {:.2} s · upload {:.2} s · {}",
                            l.load_s,
                            l.upload_s,
                            l.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()
                        ))
                        .weak()
                        .small(),
                    );

                    ui.separator();
                    ui.label(egui::RichText::new("VIEW").strong());
                    let label = match self.ui.view {
                        Some(i) => l.views.get(i).map(|v| v.name.clone()).unwrap_or_default(),
                        None => "orbit".into(),
                    };
                    let mut pick = self.ui.view;
                    egui::ComboBox::from_label("camera")
                        .selected_text(label)
                        .height(360.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut pick, None, "orbit");
                            for (i, v) in l.views.iter().enumerate() {
                                ui.selectable_value(&mut pick, Some(i), &v.name);
                            }
                        });
                    if pick != self.ui.view {
                        self.ui.view = pick;
                        dirty = true;
                    }
                    if self.ui.view.is_none() {
                        ui.horizontal(|ui| {
                            ui.label("up");
                            for (u, name) in [
                                (Up::Capture, "capture"),
                                (Up::NegY, "−Y"),
                                (Up::PosY, "+Y"),
                                (Up::PosZ, "+Z"),
                                (Up::NegZ, "−Z"),
                            ] {
                                if u == Up::Capture && l.views.is_empty() {
                                    continue;
                                }
                                dirty |= ui.selectable_value(&mut self.orbit.up, u, name).changed();
                            }
                        });
                        dirty |= ui
                            .add(egui::Slider::new(&mut self.orbit.fov_deg, 10.0..=110.0).text("fov (deg)"))
                            .changed();
                    }
                    ui.label(
                        egui::RichText::new("drag: orbit · right-drag: pan · wheel: dolly")
                            .weak()
                            .small(),
                    );

                    ui.separator();
                    ui.label(egui::RichText::new("RENDER").strong());
                    let before = self.ui.proxy;
                    ui.horizontal(|ui| {
                        ui.label("proxy");
                        ui.selectable_value(&mut self.ui.proxy, Proxy::Oriented, "oriented");
                        ui.selectable_value(&mut self.ui.proxy, Proxy::Box, "box");
                    });
                    if self.ui.proxy != before {
                        switch_proxy = true;
                    }
                    if let Some(s) = self.tracer.scene.as_ref() {
                        ui.label(
                            egui::RichText::new(format!(
                                "BVH builds {} · last {:.1} ms (only when the view leaves the dilation bound)",
                                s.builds, s.last_build_ms
                            ))
                            .weak()
                            .small(),
                        );
                    }
                    let p = &mut self.ui.params;
                    dirty |= ui
                        .add(egui::Slider::new(&mut p.dilation_px2, 0.0..=1.0).text("dilation (px²)"))
                        .changed();
                    ui.label(
                        egui::RichText::new(
                            "0.3 matches the 3DGS rasteriser the scan was trained with. \
                             0 is pure 3DGRT - thin splats vanish.",
                        )
                        .weak()
                        .small(),
                    );
                    dirty |= ui
                        .add(
                            egui::Slider::new(&mut p.cov_bias, 0.0..=(l.extent * 0.01).max(1e-4))
                                .text("covariance bias"),
                        )
                        .changed();
                    dirty |= ui.add(egui::Slider::new(&mut p.k, 1..=16).text("hit buffer k")).changed();
                    dirty |= ui
                        .add(
                            egui::Slider::new(&mut p.alpha_min, 1.0 / 255.0..=0.1)
                                .logarithmic(true)
                                .text("alpha min"),
                        )
                        .changed();
                    dirty |= ui.add(egui::Slider::new(&mut p.sh_degree, 0..=3).text("SH degree")).changed();
                    ui.horizontal(|ui| {
                        ui.label("background");
                        dirty |= ui.color_edit_button_rgb(&mut p.background).changed();
                    });

                    ui.separator();
                    ui.label(egui::RichText::new("LENS").strong());
                    dirty |= ui
                        .add(
                            egui::Slider::new(&mut self.ui.aperture, 0.0..=(l.extent * 0.05).max(1e-3))
                                .text("aperture"),
                        )
                        .changed();
                    dirty |= ui
                        .add(
                            egui::Slider::new(&mut self.ui.focus, (l.extent * 0.01).max(1e-3)..=l.extent * 10.0)
                                .logarithmic(true)
                                .text("focus distance"),
                        )
                        .changed();

                    ui.separator();
                    ui.label(egui::RichText::new("SAMPLING").strong());
                    ui.add(egui::Slider::new(&mut self.ui.max_spp, 1..=4096).logarithmic(true).text("max spp"));
                    dirty |= ui.checkbox(&mut self.ui.jitter, "jitter (antialiasing)").changed();
                    dirty |= ui
                        .add(
                            egui::Slider::new(&mut self.ui.params.max_reports, 512..=262_144)
                                .logarithmic(true)
                                .text("max reports / ray"),
                        )
                        .changed();
                    dirty |= ui.checkbox(&mut self.ui.params.flag_capped, "show capped rays in magenta").changed();
                    ui.label(
                        egui::RichText::new(format!(
                            "{} rays hit the cap last restart - the guard that keeps a GPU job \
                             from running past the OS watchdog",
                            self.ui.capped
                        ))
                        .weak()
                        .small(),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Restart").clicked() {
                            dirty = true;
                        }
                        if ui.button("Save PNG").clicked() {
                            save = true;
                        }
                    });

                    ui.separator();
                    ui.label(format!(
                        "{} spp · {:.1} ms/frame · {:.0} fps",
                        self.spp, self.frame_ms, fps
                    ));
                    ui.label(
                        egui::RichText::new(format!(
                            "{}×{} · {:.0} M primary rays/s · tile {}",
                            self.render_w,
                            self.render_h,
                            px * fps as f64 / 1e6,
                            self.tracer.tile
                        ))
                        .weak()
                        .small(),
                    );
                }
            });
        });
        if save {
            self.save_png();
        }
        if switch_proxy {
            if let Some(l) = &self.loaded {
                let s = l.splats.clone();
                if let Err(e) = self.tracer.set_scene(&self.gpu, &s, self.ui.proxy) {
                    self.ui.status = format!("proxy switch failed: {e}");
                } else {
                    self.ui.status = format!("proxy: {}", self.ui.proxy.name());
                    dirty = true;
                }
            }
        }
        self.dirty |= dirty;
        (resp.response.rect.width(), dirty)
    }

    // ------------------------------------------------------------- frame ---

    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.gpu.device, &self.config);
        self.dirty = true;
    }

    fn frame(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32() * 1000.0;
        self.last_frame = now;
        self.frame_ms = if self.frame_ms == 0.0 { dt } else { self.frame_ms * 0.9 + dt * 0.1 };
        self.poll_load();

        let raw = self.egui_state.take_egui_input(&self.window);
        let ctx = self.egui_ctx.clone();
        let mut panel_w = PANEL_W;
        let out = ctx.run_ui(raw, |ui| {
            let (w, _) = self.build_ui(ui);
            panel_w = w;
        });
        self.over_ui = ctx.is_pointer_over_egui();
        self.egui_state
            .handle_platform_output(&self.window, out.platform_output.clone());

        // the render area is the window minus the panel
        let ppp = self.egui_ctx.pixels_per_point();
        let rw = (self.config.width as f32 - panel_w * ppp).max(16.0) as u32;
        let rh = self.config.height.max(16);
        if (rw, rh) != (self.render_w, self.render_h) {
            self.render_w = rw;
            self.render_h = rh;
            self.tracer.resize(&self.gpu, rw, rh);
            self.dirty = true;
        }

        use wgpu::CurrentSurfaceTexture as Cst;
        let frame = match self.surface.get_current_texture() {
            Cst::Success(f) | Cst::Suboptimal(f) => f,
            Cst::Outdated | Cst::Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
                return;
            }
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let jobs = self.egui_ctx.tessellate(out.shapes, ppp);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: ppp,
        };
        // trace one more sample, or restart the accumulation - tile by tile,
        // each tile its own submission, so the desktop keeps its GPU slots
        let scene = self.tracer.scene.as_ref().map(|s| (s.n, s.sh_degree));
        if let Some((n, deg)) = scene {
            if self.dirty {
                self.ui.paused = false;
            }
            if !self.ui.paused && (self.dirty || self.spp < self.ui.max_spp) {
                let restart = self.dirty;
                let cam = self.current_camera();
                let f = FrameUniform::new(
                    &cam,
                    &self.ui.params,
                    n,
                    deg,
                    if restart { 0 } else { self.spp },
                    self.ui.jitter,
                    !restart,
                );
                if restart {
                    // cheap unless the view outgrew the proxies' dilation bound
                    self.tracer.ensure_geometry(&self.gpu, &cam, &self.ui.params);
                }
                let t = Instant::now();
                let tiles = self.tracer.trace_sample(&self.gpu, &f, restart);
                // keep each submission short: aim for 2-12 ms of GPU work per
                // tile, measured including the wait, one step at a time
                self.gpu.wait();
                let per_tile = t.elapsed().as_secs_f32() * 1000.0 / tiles.max(1) as f32;
                if per_tile > 12.0 && self.tracer.tile > 64 {
                    self.tracer.tile /= 2;
                } else if per_tile < 2.0 && self.tracer.tile < 1024 {
                    self.tracer.tile *= 2;
                }
                // Watchdog. A job cannot be stopped once running, so the only
                // lever is not to submit the next one: if tiles are this slow
                // even after shrinking, stop accumulating and say so.
                if per_tile > 250.0 && self.tracer.tile <= 64 {
                    self.ui.paused = true;
                    self.ui.status = format!(
                        "paused: tiles took {per_tile:.0} ms - too heavy at these settings. \
                         Lower max reports, k or the window size, then Restart."
                    );
                }
                if restart {
                    // one tiny readback per restart, not per frame
                    self.ui.capped = self.tracer.read_capped(&self.gpu);
                }
                self.spp = if restart { 1 } else { self.spp + 1 };
                self.dirty = false;
            }
        }
        let mut enc = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas {
                self.egui_renderer
                    .update_texture(&self.gpu.device, &self.gpu.queue, *id, delta);
            }
        }
        self.egui_renderer
            .update_buffers(&self.gpu.device, &self.gpu.queue, &mut enc, &jobs, &screen);

        let dim: [u32; 4] = [self.tracer.width, self.tracer.height, 0, 0];
        self.gpu
            .queue
            .write_buffer(&self.display.view, 0, bytemuck::cast_slice(&dim));
        let show = self.tracer.scene.is_some();
        let bg = self.display.bind_group(&self.gpu, &self.tracer).clone();
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("display"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.02, g: 0.02, b: 0.025, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if show {
                rp.set_pipeline(&self.display.pipeline);
                rp.set_bind_group(0, &bg, &[]);
                rp.draw(0..3, 0..1);
            }
            let mut rp = rp.forget_lifetime();
            self.egui_renderer.render(&mut rp, &jobs, &screen);
        }
        for id in &out.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
        self.gpu.queue.submit(Some(enc.finish()));
        self.gpu.queue.present(frame);
        self.window.request_redraw();
    }
}

fn fmt_count(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.2} M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{:.1} k", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}

// -------------------------------------------------------------------- app ---

#[derive(Default)]
struct App {
    state: Option<State>,
    initial: Option<PathBuf>,
    fatal: Option<String>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("№ 19 — Splat Tracer")
            .with_inner_size(winit::dpi::LogicalSize::new(1600, 950));
        let window = Arc::new(el.create_window(attrs).expect("window"));
        match State::new(window) {
            Ok(mut s) => {
                if let Some(p) = self.initial.take() {
                    s.start_load(p);
                }
                self.state = Some(s);
            }
            Err(e) => {
                self.fatal = Some(e);
                el.exit();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(s) = self.state.as_mut() else { return };
        let consumed = s.egui_state.on_window_event(&s.window, &event).consumed;
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(sz) => s.resize(sz.width, sz.height),
            WindowEvent::RedrawRequested => s.frame(),
            WindowEvent::DroppedFile(path) => s.start_load(path),
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                if down && (consumed || s.over_ui) {
                    return;
                }
                match button {
                    MouseButton::Left => s.rotating = down,
                    MouseButton::Right | MouseButton::Middle => s.panning = down,
                    _ => {}
                }
                if !down {
                    s.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = (position.x as f32, position.y as f32);
                if (s.rotating || s.panning) && s.loaded.is_some() {
                    if let Some((px, py)) = s.last_cursor {
                        let (dx, dy) = (x - px, y - py);
                        if dx != 0.0 || dy != 0.0 {
                            s.detach_to_orbit();
                            if s.rotating {
                                s.orbit.yaw -= dx * 0.006;
                                s.orbit.pitch = (s.orbit.pitch + dy * 0.006).clamp(-1.55, 1.55);
                            } else {
                                let cam = s.orbit.camera(s.render_w, s.render_h);
                                let k = s.orbit.dist / cam.fy;
                                s.orbit.target += (-cam.right * dx - cam.down * dy) * k;
                            }
                            s.dirty = true;
                        }
                    }
                }
                s.last_cursor = Some((x, y));
            }
            WindowEvent::MouseWheel { delta, .. } if !consumed && !s.over_ui => {
                if s.loaded.is_some() {
                    let d = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                    };
                    s.detach_to_orbit();
                    s.orbit.dist = (s.orbit.dist * (1.0 - d * 0.1)).max(1e-3);
                    s.dirty = true;
                }
            }
            _ => {}
        }
    }
}

fn main() {
    splat_tracer::polite_threads();
    let el = EventLoop::new().expect("event loop");
    el.set_control_flow(ControlFlow::Poll);
    let mut app = App { initial: std::env::args().nth(1).map(PathBuf::from), ..Default::default() };
    el.run_app(&mut app).expect("run");
    if let Some(e) = app.fatal {
        eprintln!("№ 19 could not start: {e}");
        std::process::exit(1);
    }
}
