// Shared by every pass. Concatenated in front of each shader at load time -
// WGSL has no include - so the Frame layout is written down exactly once.

struct Frame {
    cam_pos:   vec4<f32>,   // xyz; w = near (splat centres closer are culled)
    cam_right: vec4<f32>,   // xyz; w = fx
    cam_down:  vec4<f32>,   // xyz; w = fy
    cam_fwd:   vec4<f32>,   // xyz; w = unused
    img:       vec4<f32>,   // width, height, cx, cy
    lens:      vec4<f32>,   // aperture radius, focus distance, jitter (0/1), stats (0/1)
    tone:      vec4<f32>,   // alpha_min, T_min, alpha_max, dilation (px^2)
    misc:      vec4<f32>,   // covariance bias (world sigma), tile origin x, tile origin y, SH floats per splat
    bg:        vec4<f32>,   // background rgb; w = accumulate (0 overwrites)
    ints:      vec4<u32>,   // splat count, k, SH degree, frame index
    geo:       vec4<u32>,   // proxy (0 = boxes in one BLAS, 1 = instance per splat), max reports per ray, flags (bit 0: tint capped rays), unused
};

// One splat as the prep pass leaves it for the tracer, 64 bytes:
// rows of the world-to-canonical matrix M = S^-1 R^T with the centre tucked
// into .w, then colour for this camera and opacity (0 = culled).
struct Derived {
    r0: vec4<f32>,
    r1: vec4<f32>,
    r2: vec4<f32>,
    c:  vec4<f32>,
};
