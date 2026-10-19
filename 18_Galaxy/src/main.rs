//! № 18 — Galaxy Colonization Simulator
//!
//! M1: the substrate - a procedurally generated Milky Way from published
//! density models, generated and drawn on the GPU, with the device's real
//! allocation limits reported so the PRD's 1e9-system target can be checked
//! against hardware instead of arithmetic.
//!
//! M4-lite: Landis percolation from the Sun. The neighbour graph and the
//! shortest-path colonisation run on the CPU, exact and tested; the GPU
//! draws the result with "now" as a threshold on the arrival field.

use galaxy_colonization::{colonize, galaxy, gpu};

use colonize::{colonise, pack, reach_fraction, Graph, Rule};
use galaxy::{Budget, Galaxy};
use gpu::{CameraUniform, DeviceReport, HdrTarget, Scene, ToneUniform, Tonemap};
use glam::{Mat4, Vec3};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

const KPC_PER_LY: f64 = 3.066_013_8e-4;
/// neighbours computed per system; the k slider picks a prefix of these
const K_MAX: usize = 12;

struct Camera {
    yaw: f32,
    pitch: f32,
    dist: f32,
    target: Vec3,
}

impl Default for Camera {
    fn default() -> Self {
        // near face-on: spiral structure is invisible from within the plane,
        // which is precisely why mapping the real Milky Way's arms is hard
        Self { yaw: 0.9, pitch: 1.25, dist: 46.0, target: Vec3::ZERO }
    }
}

impl Camera {
    fn eye(&self) -> Vec3 {
        let (sp, cp) = self.pitch.sin_cos();
        let (sy, cy) = self.yaw.sin_cos();
        self.target + self.dist * Vec3::new(cp * cy, cp * sy, sp)
    }
    #[allow(deprecated)]
    fn view_proj(&self, aspect: f32) -> Mat4 {
        let eye = self.eye();
        // galactic plane is XY, so +Z is up
        let view = Mat4::look_at_rh(eye, self.target, Vec3::Z);
        let proj = Mat4::perspective_rh(45f32.to_radians(), aspect, 0.02, 900.0);
        proj * view
    }
}

struct Render {
    /// per-system deposit into the HDR buffer, before any stretch
    alpha: f32,
    lum_weight: f32,
    colour_mode: usize,
    /// tone pass
    exposure: f32,
    gamma: f32,
    black: f32,
}

impl Default for Render {
    fn default() -> Self {
        // alpha is small and exposure does the lifting: nothing clips in an
        // Rgba16Float target, so headroom costs nothing and the bulge keeps
        // its gradient instead of flattening to white
        Self { alpha: 0.012, lum_weight: 0.35, colour_mode: 0, exposure: 1.4, gamma: 2.6, black: 0.0 }
    }
}

struct ColonyStats {
    n: usize,
    reached: usize,
    open_reached: usize,
    latest_yr: f32,
    secs: f32,
}

/// Everything the panel edits, in one place so the UI builder takes one
/// borrow instead of nine.
struct Controls {
    galaxy: Galaxy,
    render: Render,
    camera: Camera,
    pending_capacity_m: f32,

    // Landis
    p: f32,
    k: usize,
    v_c: f32,
    col_seed: u32,

    // playback over the arrival field
    now_yr: f32,
    front_yr: f32,
    playing: bool,
    play_rate: f32, // years per wall-clock second

    sweep: Vec<[f64; 2]>,
    stats: Option<ColonyStats>,
}

impl Default for Controls {
    fn default() -> Self {
        let galaxy = Galaxy::default();
        Self {
            pending_capacity_m: galaxy.count as f32 / 1.0e6,
            galaxy,
            render: Render::default(),
            camera: Camera::default(),
            p: 0.5,
            k: 6,
            v_c: 0.01,
            col_seed: 1,
            now_yr: 1.0e6,
            front_yr: 2.0e5,
            playing: false,
            play_rate: 2.0e6,
            sweep: Vec::new(),
            stats: None,
        }
    }
}

#[derive(Default)]
struct Actions {
    realloc: Option<u32>,
    regen: bool,
    build_graph: bool,
    colonise: bool,
    sweep: bool,
}

/// Results coming back from the worker thread.
enum Job {
    Graph(Arc<Graph>, usize, f32),
    Colony { packed: Vec<u32>, stats: ColonyStats },
    Sweep(Vec<[f64; 2]>, f32),
}

struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    scene: Scene,
    tonemap: Tonemap,
    hdr: HdrTarget,
    report: DeviceReport,

    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,

    c: Controls,

    graph: Option<Arc<Graph>>,
    graph_secs: f32,
    seed_idx: usize,
    job_rx: Option<Receiver<Job>>,
    busy: Option<&'static str>,
    progress: Arc<AtomicU32>,

    dragging: bool,
    last_cursor: Option<(f32, f32)>,
    last_frame: Instant,
    fps: f32,
    gen_ms: f32,
}

impl State {
    fn new(window: Arc<Window>) -> Self {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            // Buckets round limits down to portable tiers. We want what this
            // card will really give, since that number is the whole question.
            apply_limit_buckets: false,
        }))
        .expect("no suitable GPU adapter");

        let info = adapter.get_info();
        // Ask for everything the adapter will give. The defaults cap storage
        // buffers at 128 MiB, which would make the whole scale question moot
        // before it was asked.
        let limits = adapter.limits();
        let report = DeviceReport {
            name: info.name.clone(),
            backend: format!("{:?}", info.backend),
            max_buffer_size: limits.max_buffer_size,
            max_storage_binding: limits.max_storage_buffer_binding_size as u64,
            max_workgroups: limits.max_compute_workgroups_per_dimension,
        };

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("galaxy"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            ..Default::default()
        }))
        .expect("device");

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let c = Controls::default();
        let capacity = c.galaxy.count;
        let scene = Scene::new(&device, capacity);
        let tonemap = Tonemap::new(&device, format);
        let hdr = tonemap.target(&device, config.width, config.height);

        let egui_ctx = egui::Context::default();
        egui_ctx.set_visuals(egui::Visuals::dark());
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            None,
            None,
            None,
        );
        let egui_renderer =
            egui_wgpu::Renderer::new(&device, format, egui_wgpu::RendererOptions::default());

        let mut s = Self {
            window,
            surface,
            device,
            queue,
            config,
            scene,
            tonemap,
            hdr,
            report,
            egui_ctx,
            egui_state,
            egui_renderer,
            c,
            graph: None,
            graph_secs: 0.0,
            seed_idx: 0,
            job_rx: None,
            busy: None,
            progress: Arc::new(AtomicU32::new(0)),
            dragging: false,
            last_cursor: None,
            last_frame: Instant::now(),
            fps: 0.0,
            gen_ms: 0.0,
        };
        s.regenerate();
        s
    }

    fn regenerate(&mut self) {
        let t = Instant::now();
        self.c.galaxy.count = self.c.galaxy.count.min(self.scene.capacity);
        let p = self.c.galaxy.to_gpu();
        self.scene.generate(&self.device, &self.queue, &p);
        // one blocking poll so the timing means something rather than
        // measuring how fast we can fill a command buffer
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        self.gen_ms = t.elapsed().as_secs_f32() * 1000.0;

        // new positions: the graph and every arrival time built on the old
        // ones are void. Clear on the GPU rather than uploading 80 MB of zeros.
        self.graph = None;
        self.c.stats = None;
        self.c.sweep.clear();
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("clear colony") });
        enc.clear_buffer(&self.scene.colony, 0, None);
        self.queue.submit(Some(enc.finish()));
    }

    fn reallocate(&mut self, capacity: u32) {
        // never ask for a binding the driver will refuse
        let ceiling = self.report.max_systems_single_buffer().min(u32::MAX as u64) as u32;
        let capacity = capacity.clamp(1, ceiling);
        self.scene = Scene::new(&self.device, capacity);
        self.c.galaxy.count = self.c.galaxy.count.min(capacity);
        self.regenerate();
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.hdr = self.tonemap.target(&self.device, w, h);
    }

    // ------------------------------------------------------------ jobs ----

    fn start_graph(&mut self) {
        if self.busy.is_some() {
            return;
        }
        let pos = self.scene.read_positions(&self.device, &self.queue, self.c.galaxy.count);
        let (tx, rx) = channel();
        let prog = self.progress.clone();
        let sun = [self.c.galaxy.sun_r, 0.0, 0.02];
        std::thread::spawn(move || {
            let t = Instant::now();
            let g = Graph::build(pos, K_MAX, &prog);
            let seed = g.nearest(sun);
            let _ = tx.send(Job::Graph(Arc::new(g), seed, t.elapsed().as_secs_f32()));
        });
        self.job_rx = Some(rx);
        self.busy = Some("building neighbour graph");
        self.progress.store(0, Ordering::Relaxed);
    }

    fn start_colonise(&mut self) {
        let Some(g) = self.graph.clone() else { return };
        if self.busy.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let rule = Rule { p: self.c.p, k: self.c.k, v_c: self.c.v_c, seed: self.c.col_seed };
        let src = self.seed_idx;
        std::thread::spawn(move || {
            let t = Instant::now();
            let out = colonise(&g, src, rule);
            let packed = pack(&out);
            let stats = ColonyStats {
                n: g.n,
                reached: out.reached,
                open_reached: out.open_reached,
                latest_yr: out.latest_arrival,
                secs: t.elapsed().as_secs_f32(),
            };
            let _ = tx.send(Job::Colony { packed, stats });
        });
        self.job_rx = Some(rx);
        self.busy = Some("colonising");
        self.progress.store(0, Ordering::Relaxed);
    }

    fn start_sweep(&mut self) {
        let Some(g) = self.graph.clone() else { return };
        if self.busy.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let prog = self.progress.clone();
        let (k, seed, src) = (self.c.k, self.c.col_seed, self.seed_idx);
        std::thread::spawn(move || {
            let t = Instant::now();
            let steps = 20u32;
            let pts: Vec<[f64; 2]> = (1..=steps)
                .map(|i| {
                    let p = i as f32 / steps as f32;
                    let f = reach_fraction(&g, src, p, k, seed);
                    prog.store(100 * i / steps, Ordering::Relaxed);
                    [p as f64, f]
                })
                .collect();
            let _ = tx.send(Job::Sweep(pts, t.elapsed().as_secs_f32()));
        });
        self.job_rx = Some(rx);
        self.busy = Some("sweeping P");
        self.progress.store(0, Ordering::Relaxed);
    }

    fn poll_jobs(&mut self) {
        let Some(rx) = self.job_rx.as_ref() else { return };
        let Ok(job) = rx.try_recv() else { return };
        self.job_rx = None;
        self.busy = None;
        match job {
            Job::Graph(g, seed, secs) => {
                self.graph = Some(g);
                self.seed_idx = seed;
                self.graph_secs = secs;
            }
            Job::Colony { packed, stats } => {
                self.scene.upload_colony(&self.queue, &packed);
                // land the playhead at the end of the run and show the front
                self.c.now_yr = stats.latest_yr.max(1.0);
                self.c.front_yr = (stats.latest_yr * 0.05).max(100.0);
                self.c.stats = Some(stats);
                self.c.render.colour_mode = 3;
            }
            Job::Sweep(pts, _secs) => {
                self.c.sweep = pts;
            }
        }
    }

    // -------------------------------------------------------------- ui ----

    /// Free function over explicit borrows rather than a `&mut self` method:
    /// egui 0.36 builds panels inside a root `Ui`, and the closure would
    /// otherwise want `self` whole while `run_ui` already holds the context.
    fn build_ui(
        root: &mut egui::Ui,
        c: &mut Controls,
        report: &DeviceReport,
        gen_ms: f32,
        fps: f32,
        busy: Option<(&str, u32)>,
        graph: Option<(usize, f32)>,
        seed_pos: Option<[f32; 3]>,
    ) -> Actions {
        let mut act = Actions::default();
        let galaxy_count = c.galaxy.count;
        let idle = busy.is_none();

        egui::Panel::right("panel")
            .default_size(360.0)
            .show(root, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("№ 18 — Galaxy");
                    ui.label(
                        egui::RichText::new("colonisation substrate · milestone 4-lite")
                            .italics()
                            .weak(),
                    );
                    ui.separator();

                    ui.collapsing("Device", |ui| {
                        ui.label(format!("{} ({})", report.name, report.backend));
                        ui.label(format!(
                            "max buffer        {:.2} GiB",
                            Budget::gib(report.max_buffer_size)
                        ));
                        ui.label(format!(
                            "max storage bind  {:.2} GiB",
                            Budget::gib(report.max_storage_binding)
                        ));
                        let m = report.max_systems_single_buffer();
                        ui.label(format!(
                            "→ {:.1} M systems per position buffer",
                            m as f64 / 1.0e6
                        ));
                        if m < 1_000_000_000 {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 170, 90),
                                format!(
                                    "1e9 systems needs {} position buffers",
                                    1_000_000_000u64.div_ceil(m.max(1))
                                ),
                            );
                        }
                    });

                    ui.collapsing("Memory budget", |ui| {
                        for n in [100_000_000u64, 500_000_000, 1_000_000_000] {
                            let b = Budget::for_count(n);
                            ui.label(format!(
                                "{:>5} M → {:.1} GiB  (sub {:.1} + evt {:.1} + hash {:.1})",
                                n / 1_000_000,
                                Budget::gib(b.total),
                                Budget::gib(b.substrate),
                                Budget::gib(b.events),
                                Budget::gib(b.hash),
                            ));
                        }
                    });

                    // ------------------------------------------- sampling --
                    ui.separator();
                    ui.label(egui::RichText::new("SAMPLING").strong());
                    let cap_m = (report.max_systems_single_buffer() as f32 / 1.0e6).max(1.0);
                    ui.add(
                        egui::Slider::new(&mut c.pending_capacity_m, 1.0..=cap_m)
                            .logarithmic(true)
                            .text("capacity (M)"),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "ceiling is one storage binding: {cap_m:.0} M. Past that the \
                             substrate must be split across bindings."
                        ))
                        .weak()
                        .small(),
                    );
                    if ui.add_enabled(idle, egui::Button::new("Reallocate & regenerate")).clicked() {
                        act.realloc = Some((c.pending_capacity_m * 1.0e6) as u32);
                    }
                    let mut cm = galaxy_count as f32 / 1.0e6;
                    if ui
                        .add(
                            egui::Slider::new(&mut cm, 0.1..=c.pending_capacity_m)
                                .logarithmic(true)
                                .text("systems (M)"),
                        )
                        .changed()
                    {
                        c.galaxy.count = (cm * 1.0e6) as u32;
                        act.regen = true;
                    }
                    if ui.add(egui::Slider::new(&mut c.galaxy.seed, 1..=9999).text("seed")).changed() {
                        act.regen = true;
                    }

                    // ------------------------------------------ structure --
                    ui.separator();
                    ui.collapsing("Structure", |ui| {
                        let g = &mut c.galaxy;
                        let mut d = false;
                        d |= ui.add(egui::Slider::new(&mut g.frac_thin, 0.0..=1.0).text("thin disk")).changed();
                        d |= ui.add(egui::Slider::new(&mut g.frac_thick, 0.0..=1.0).text("thick disk")).changed();
                        d |= ui.add(egui::Slider::new(&mut g.frac_bulge, 0.0..=1.0).text("bar / bulge")).changed();
                        d |= ui.add(egui::Slider::new(&mut g.frac_halo, 0.0..=0.2).text("halo")).changed();
                        ui.add_space(4.0);
                        d |= ui.add(egui::Slider::new(&mut g.thin_hr, 1.0..=6.0).text("thin hR (kpc)")).changed();
                        d |= ui.add(egui::Slider::new(&mut g.thin_hz, 0.05..=1.5).text("thin hz (kpc)")).changed();
                        d |= ui.add(egui::Slider::new(&mut g.disk_rmax, 5.0..=40.0).text("disk cutoff")).changed();
                        ui.add_space(4.0);
                        d |= ui.add(egui::Slider::new(&mut g.arm_count, 0..=8).text("arms")).changed();
                        d |= ui.add(egui::Slider::new(&mut g.arm_pitch_deg, 4.0..=30.0).text("pitch (deg)")).changed();
                        d |= ui.add(egui::Slider::new(&mut g.arm_width, 0.05..=1.5).text("arm width")).changed();
                        d |= ui.add(egui::Slider::new(&mut g.arm_contrast, 0.0..=6.0).text("arm contrast")).changed();
                        d |= ui.add(egui::Slider::new(&mut g.bar_angle_deg, 0.0..=90.0).text("bar angle (deg)")).changed();
                        act.regen |= d;
                    });

                    // --------------------------------------- colonisation --
                    ui.separator();
                    ui.label(egui::RichText::new("COLONISATION").strong());
                    ui.label(
                        egui::RichText::new(
                            "Landis (1998): each colony is colonising with probability P and \
                             reaches its k nearest systems. Arrival time is the shortest path \
                             from the Sun through colonising sites - computed once, exact.",
                        )
                        .weak()
                        .small(),
                    );
                    match graph {
                        None => {
                            if ui.add_enabled(idle, egui::Button::new("Build neighbour graph")).clicked() {
                                act.build_graph = true;
                            }
                            ui.label(
                                egui::RichText::new(format!(
                                    "k-d tree, {} nearest per system, on the CPU. Expect ~30 s at 20 M.",
                                    K_MAX
                                ))
                                .weak()
                                .small(),
                            );
                        }
                        Some((n, secs)) => {
                            ui.label(format!(
                                "graph: {:.1} M systems × {} neighbours, {:.1} s",
                                n as f64 / 1e6,
                                K_MAX,
                                secs
                            ));
                            if let Some(sp) = seed_pos {
                                let r = (sp[0] * sp[0] + sp[1] * sp[1]).sqrt();
                                ui.label(
                                    egui::RichText::new(format!(
                                        "seed: nearest sampled system to the Sun, R = {r:.3} kpc, z = {:.3} kpc",
                                        sp[2]
                                    ))
                                    .weak()
                                    .small(),
                                );
                            }
                        }
                    }
                    let have_graph = graph.is_some();
                    ui.add_enabled_ui(have_graph, |ui| {
                        ui.add(egui::Slider::new(&mut c.p, 0.0..=1.0).text("P  (colonising)"));
                        ui.add(egui::Slider::new(&mut c.k, 1..=K_MAX).text("k  (neighbours reached)"));
                        ui.add(
                            egui::Slider::new(&mut c.v_c, 0.0001..=1.0)
                                .logarithmic(true)
                                .text("ship speed (c)"),
                        );
                        ui.add(egui::Slider::new(&mut c.col_seed, 1..=9999).text("coin seed"));
                        ui.horizontal(|ui| {
                            if ui.add_enabled(idle, egui::Button::new("Colonise")).clicked() {
                                act.colonise = true;
                            }
                            if ui.add_enabled(idle, egui::Button::new("Sweep P")).clicked() {
                                act.sweep = true;
                            }
                        });
                    });
                    if let Some((what, pct)) = busy {
                        ui.add(egui::ProgressBar::new(pct as f32 / 100.0).text(what).animate(true));
                    }
                    if let Some(s) = &c.stats {
                        let f = s.reached as f64 / s.n as f64;
                        ui.label(format!(
                            "colonised {:.1} M of {:.1} M  ({:.1}%)",
                            s.reached as f64 / 1e6,
                            s.n as f64 / 1e6,
                            f * 100.0
                        ));
                        ui.label(format!(
                            "{:.1} M colonising · {:.1} M dead-end · last arrival {:.2} Myr · {:.1} s",
                            s.open_reached as f64 / 1e6,
                            (s.reached - s.open_reached) as f64 / 1e6,
                            s.latest_yr / 1e6,
                            s.secs
                        ));
                    }

                    // ----------------------------------------- playback ---
                    if c.stats.is_some() {
                        ui.add_space(4.0);
                        let max_yr = c.stats.as_ref().map(|s| s.latest_yr.max(1.0)).unwrap_or(1e9);
                        ui.add(
                            egui::Slider::new(&mut c.now_yr, 1.0e3..=max_yr * 1.05)
                                .logarithmic(true)
                                .text("now (yr)"),
                        );
                        ui.add(
                            egui::Slider::new(&mut c.front_yr, 100.0..=max_yr)
                                .logarithmic(true)
                                .text("front window (yr)"),
                        );
                        ui.horizontal(|ui| {
                            let label = if c.playing { "⏸ pause" } else { "▶ play" };
                            ui.toggle_value(&mut c.playing, label);
                            ui.add(
                                egui::Slider::new(&mut c.play_rate, 1.0e4..=1.0e8)
                                    .logarithmic(true)
                                    .text("yr / s"),
                            );
                        });
                    }

                    // -------------------------------------------- sweep ---
                    if !c.sweep.is_empty() {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("colonised fraction vs P — the percolation order parameter")
                                .weak()
                                .small(),
                        );
                        let pts: Vec<[f64; 2]> = c.sweep.clone();
                        egui_plot::Plot::new("sweep")
                            .height(150.0)
                            .include_x(0.0)
                            .include_x(1.0)
                            .include_y(0.0)
                            .include_y(1.0)
                            .allow_drag(false)
                            .allow_zoom(false)
                            .allow_scroll(false)
                            .show(ui, |pu| {
                                pu.line(egui_plot::Line::new("colonised", egui_plot::PlotPoints::from(pts)));
                            });
                    }

                    // ------------------------------------------- render ---
                    ui.separator();
                    ui.label(egui::RichText::new("RENDER").strong());
                    let r = &mut c.render;
                    egui::ComboBox::from_label("colour by")
                        .selected_text(
                            ["spectral class", "component", "habitability", "colonisation"][r.colour_mode],
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut r.colour_mode, 0, "spectral class");
                            ui.selectable_value(&mut r.colour_mode, 1, "component");
                            ui.selectable_value(&mut r.colour_mode, 2, "habitability");
                            ui.selectable_value(&mut r.colour_mode, 3, "colonisation");
                        });
                    ui.add(
                        egui::Slider::new(&mut r.exposure, 0.02..=20.0)
                            .logarithmic(true)
                            .text("exposure"),
                    );
                    ui.add(egui::Slider::new(&mut r.gamma, 1.0..=6.0).text("stretch (gamma)"));
                    ui.add(egui::Slider::new(&mut r.black, 0.0..=0.25).text("black point"));
                    ui.add(egui::Slider::new(&mut r.alpha, 0.001..=0.2).logarithmic(true).text("point deposit"));
                    ui.add(egui::Slider::new(&mut r.lum_weight, 0.0..=1.0).text("luminosity weight"));
                    ui.add(egui::Slider::new(&mut c.camera.dist, 1.0..=260.0).logarithmic(true).text("distance (kpc)"));
                    if ui.button("Reset view").clicked() {
                        c.camera = Camera::default();
                    }

                    ui.separator();
                    ui.label(format!("{:.0} fps · generate {:.1} ms", fps, gen_ms));
                    ui.label(
                        egui::RichText::new(format!(
                            "Sun at R = {:.2} kpc  ({:.0} ly across the disk cutoff)",
                            c.galaxy.sun_r,
                            (c.galaxy.disk_rmax as f64 * 2.0) / KPC_PER_LY
                        ))
                        .weak()
                        .small(),
                    );
                });
            });

        act
    }

    // ----------------------------------------------------------- frame ----

    fn frame(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        if dt > 0.0 {
            self.fps = self.fps * 0.9 + (1.0 / dt) * 0.1;
        }
        self.poll_jobs();
        if self.c.playing {
            self.c.now_yr += self.c.play_rate * dt.min(0.1);
            if let Some(s) = &self.c.stats {
                if self.c.now_yr > s.latest_yr * 1.05 {
                    self.c.now_yr = 1.0e3; // loop
                }
            }
        }

        let raw = self.egui_state.take_egui_input(&self.window);
        let ctx = self.egui_ctx.clone(); // Arc-backed, so this is a refcount bump
        let busy = self.busy.map(|b| (b, self.progress.load(Ordering::Relaxed)));
        let graph_info = self.graph.as_ref().map(|g| (g.n, self.graph_secs));
        let seed_pos = self.graph.as_ref().map(|g| g.pos[self.seed_idx]);
        let mut act = Actions::default();
        let out = ctx.run_ui(raw, |ui| {
            act = Self::build_ui(
                ui,
                &mut self.c,
                &self.report,
                self.gen_ms,
                self.fps,
                busy,
                graph_info,
                seed_pos,
            );
        });
        self.egui_state
            .handle_platform_output(&self.window, out.platform_output.clone());

        if let Some(cap) = act.realloc {
            self.reallocate(cap.max(1));
        } else if act.regen {
            self.regenerate();
        }
        if act.build_graph {
            self.start_graph();
        }
        if act.colonise {
            self.start_colonise();
        }
        if act.sweep {
            self.start_sweep();
        }

        use wgpu::CurrentSurfaceTexture as Cst;
        let frame = match self.surface.get_current_texture() {
            Cst::Success(f) | Cst::Suboptimal(f) => f,
            Cst::Outdated | Cst::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());

        let aspect = self.config.width as f32 / self.config.height.max(1) as f32;
        let cam = CameraUniform {
            view_proj: self.c.camera.view_proj(aspect).to_cols_array_2d(),
            eye: self.c.camera.eye().extend(1.0).to_array(),
            tune: [1.0, self.c.render.alpha, self.c.render.lum_weight, 0.0],
            opts: [
                self.c.render.colour_mode as f32,
                self.c.galaxy.count as f32,
                self.c.now_yr / 10.0,
                self.c.front_yr / 10.0,
            ],
        };
        self.queue
            .write_buffer(&self.scene.camera, 0, bytemuck::bytes_of(&cam));
        let tone = ToneUniform {
            p: [self.c.render.exposure, self.c.render.gamma, self.c.render.black, 0.0],
        };
        self.queue
            .write_buffer(&self.tonemap.params, 0, bytemuck::bytes_of(&tone));

        let ppp = self.egui_ctx.pixels_per_point();
        let jobs = self.egui_ctx.tessellate(out.shapes, ppp);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: ppp,
        };

        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        // 0.36 batches several deltas per texture id
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas {
                self.egui_renderer
                    .update_texture(&self.device, &self.queue, *id, delta);
            }
        }
        self.egui_renderer
            .update_buffers(&self.device, &self.queue, &mut enc, &jobs, &screen);

        // pass 1: accumulate the star field additively into the HDR target.
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("stars"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&self.scene.draw_pipeline);
            rp.set_bind_group(0, &self.scene.draw_bg, &[]);
            rp.draw(0..self.c.galaxy.count, 0..1);
        }

        // pass 2: one fullscreen triangle stretches it, then the panel on top
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tonemap"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&self.tonemap.pipeline);
            rp.set_bind_group(0, &self.hdr.bind_group, &[]);
            rp.draw(0..3, 0..1);

            let mut rp = rp.forget_lifetime();
            self.egui_renderer.render(&mut rp, &jobs, &screen);
        }

        for id in &out.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
        self.queue.submit(Some(enc.finish()));
        self.queue.present(frame);
        self.window.request_redraw();
    }
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("№ 18 — Galaxy Colonization Simulator")
            .with_inner_size(winit::dpi::LogicalSize::new(1600, 950));
        let window = Arc::new(el.create_window(attrs).expect("window"));
        self.state = Some(State::new(window));
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(s) = self.state.as_mut() else { return };
        let consumed = s.egui_state.on_window_event(&s.window, &event).consumed;

        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(sz) => s.resize(sz.width, sz.height),
            WindowEvent::RedrawRequested => s.frame(),
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } if !consumed => {
                s.dragging = state == ElementState::Pressed;
                if !s.dragging {
                    s.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = (position.x as f32, position.y as f32);
                if s.dragging {
                    if let Some((px, py)) = s.last_cursor {
                        s.c.camera.yaw -= (x - px) * 0.006;
                        s.c.camera.pitch = (s.c.camera.pitch + (y - py) * 0.006).clamp(-1.553, 1.553);
                    }
                }
                s.last_cursor = Some((x, y));
            }
            WindowEvent::MouseWheel { delta, .. } if !consumed => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                s.c.camera.dist = (s.c.camera.dist * (1.0 - d * 0.12)).clamp(0.4, 400.0);
            }
            _ => {}
        }
    }
}

fn main() {
    let el = EventLoop::new().expect("event loop");
    el.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    el.run_app(&mut app).expect("run");
}
