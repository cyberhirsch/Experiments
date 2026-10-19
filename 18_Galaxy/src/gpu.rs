//! Device bring-up, the star buffers, and the two passes that fill and draw
//! them. Kept deliberately thin: the simulation layers (event clock, hazards,
//! colonisation) will hang off the same buffers rather than owning their own.

use crate::galaxy::{GenParams, BYTES_POS, BYTES_STATE};
use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;

/// Conservative `max_compute_workgroups_per_dimension`; every backend
/// guarantees at least this, and every one measured so far reports exactly it.
pub const MAX_WG_PER_DIM: u32 = 65535;

/// Stars accumulate here before tone mapping. 8-bit would clip the bulge to
/// flat white and crush the outer disk into one code value.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ToneUniform {
    /// exposure, gamma, black point, unused
    pub p: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub eye: [f32; 4],
    /// exposure, point alpha, luminosity weight, unused
    pub tune: [f32; 4],
    /// colour mode, system count, now (decades), front window (decades)
    pub opts: [f32; 4],
}

/// What the adapter will actually let us allocate. Reported rather than
/// assumed - the PRD's 1e9-system figure lives or dies on these two numbers,
/// and they are driver policy, not arithmetic.
#[derive(Clone, Debug)]
pub struct DeviceReport {
    pub name: String,
    pub backend: String,
    pub max_buffer_size: u64,
    pub max_storage_binding: u64,
    pub max_workgroups: u32,
}

impl DeviceReport {
    /// Largest system count that fits in ONE position buffer. Position is the
    /// widest per-system array at 12 bytes, so it binds first.
    pub fn max_systems_single_buffer(&self) -> u64 {
        self.max_storage_binding.min(self.max_buffer_size) / BYTES_POS
    }
}

pub struct Scene {
    pub pos: wgpu::Buffer,
    pub state: wgpu::Buffer,
    pub colony: wgpu::Buffer,
    pub gen_params: wgpu::Buffer,
    pub camera: wgpu::Buffer,
    pub gen_bg: wgpu::BindGroup,
    pub draw_bg: wgpu::BindGroup,
    pub gen_pipeline: wgpu::ComputePipeline,
    pub draw_pipeline: wgpu::RenderPipeline,
    pub capacity: u32,
}

impl Scene {
    pub fn new(device: &wgpu::Device, capacity: u32) -> Self {
        let format = HDR_FORMAT;
        let n = capacity as u64;
        let pos = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("star positions"),
            size: n * BYTES_POS,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        // per-system colonisation record, written by the CPU model:
        // bits 0..31 arrival in decades + 1 (0 = never), bit 31 = colonising
        let colony = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("colony"),
            size: n * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let state = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("star state"),
            size: n * BYTES_STATE,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let gen_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gen params"),
            size: std::mem::size_of::<GenParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let gen_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("generate"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/generate.wgsl"))),
        });
        let draw_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("render"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/render.wgsl"))),
        });

        let uniform_entry = |b: u32, vis: wgpu::ShaderStages| wgpu::BindGroupLayoutEntry {
            binding: b,
            visibility: vis,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let storage_entry = |b: u32, vis: wgpu::ShaderStages, read_only: bool| {
            wgpu::BindGroupLayoutEntry {
                binding: b,
                visibility: vis,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }
        };

        let gen_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gen bgl"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::COMPUTE),
                storage_entry(1, wgpu::ShaderStages::COMPUTE, false),
                storage_entry(2, wgpu::ShaderStages::COMPUTE, false),
            ],
        });
        let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("draw bgl"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::VERTEX),
                storage_entry(1, wgpu::ShaderStages::VERTEX, true),
                storage_entry(2, wgpu::ShaderStages::VERTEX, true),
                storage_entry(3, wgpu::ShaderStages::VERTEX, true),
            ],
        });

        let gen_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gen bg"),
            layout: &gen_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: gen_params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: pos.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: state.as_entire_binding() },
            ],
        });
        let draw_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw bg"),
            layout: &draw_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: camera.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: pos.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: state.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: colony.as_entire_binding() },
            ],
        });

        let gen_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gen pl"),
            bind_group_layouts: &[Some(&gen_layout)],
            immediate_size: 0,
        });
        let gen_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("generate"),
            layout: Some(&gen_pl),
            module: &gen_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let draw_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("draw pl"),
            bind_group_layouts: &[Some(&draw_layout)],
            immediate_size: 0,
        });
        // Additive, premultiplied: the fragment emits rgb*a so One/One is the
        // correct accumulation. No depth buffer - a galaxy image IS the
        // integral along the ray, and sorting 10^8 points would cost more
        // than the draw.
        let draw_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("stars"),
            layout: Some(&draw_pl),
            vertex: wgpu::VertexState {
                module: &draw_module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &draw_module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::PointList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pos,
            state,
            colony,
            gen_params,
            camera,
            gen_bg,
            draw_bg,
            gen_pipeline,
            draw_pipeline,
            capacity,
        }
    }

    /// Pull the positions back for the CPU-side neighbour graph. One staging
    /// buffer, one copy, one map - 240 MB at 20 M systems, well under a
    /// second on PCIe.
    pub fn read_positions(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        count: u32,
    ) -> Vec<[f32; 3]> {
        let bytes = count as u64 * BYTES_POS;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pos staging"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("readback"),
        });
        enc.copy_buffer_to_buffer(&self.pos, 0, &staging, 0, bytes);
        queue.submit(Some(enc.finish()));
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv().expect("map").expect("map failed");
        let data = slice.get_mapped_range().expect("mapped range");
        let f: &[f32] = bytemuck::cast_slice(&data[..]);
        let out: Vec<[f32; 3]> = f.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
        drop(data);
        staging.unmap();
        out
    }

    pub fn upload_colony(&self, queue: &wgpu::Queue, packed: &[u32]) {
        queue.write_buffer(&self.colony, 0, bytemuck::cast_slice(packed));
    }

    /// Fill the buffers. One dispatch, one invocation per system.
    pub fn generate(&self, device: &wgpu::Device, queue: &wgpu::Queue, p: &GenParams) {
        queue.write_buffer(&self.gen_params, 0, bytemuck::bytes_of(p));
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("generate"),
        });
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("generate"),
                timestamp_writes: None,
            });
            cp.set_pipeline(&self.gen_pipeline);
            cp.set_bind_group(0, &self.gen_bg, &[]);
            // 65535 workgroups * 256 = 16.8 M invocations is the 1-D ceiling on
            // this class of hardware, well under the counts this is built for,
            // so the grid runs 2-D and the shader rebuilds the linear index.
            let groups = p.count.div_ceil(256).max(1);
            let x = groups.min(MAX_WG_PER_DIM);
            let y = groups.div_ceil(x);
            cp.dispatch_workgroups(x, y, 1);
        }
        queue.submit(Some(enc.finish()));
    }
}

// ---------------------------------------------------------------------------
// Tone mapping: the stars land in an HDR target and are stretched from there.
// ---------------------------------------------------------------------------

/// Built once. The pipeline and sampler do not care about window size.
pub struct Tonemap {
    pub pipeline: wgpu::RenderPipeline,
    pub params: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

/// Rebuilt on every resize.
pub struct HdrTarget {
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
}

impl Tonemap {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tonemap"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/tonemap.wgsl"))),
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tone params"),
            size: std::mem::size_of::<ToneUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Nearest: the blit is 1:1, so filtering would only cost bandwidth and
        // sidesteps any question of whether Rgba16Float is filterable here.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tone sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tone bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tone pl"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tonemap"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
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
        Self { pipeline, params, layout, sampler }
    }

    pub fn target(&self, device: &wgpu::Device, width: u32, height: u32) -> HdrTarget {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hdr"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tone bg"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: self.params.as_entire_binding() },
            ],
        });
        HdrTarget { view, bind_group }
    }
}
