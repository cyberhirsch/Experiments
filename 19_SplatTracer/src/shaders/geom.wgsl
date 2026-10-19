// ============================================================================
// Axis-aligned proxy boxes for the RT cores. One invocation per splat.
//
// Camera-INDEPENDENT on purpose: wgpu 30 has no BVH refit, so anything the
// acceleration structure is built from is paid for with a full rebuild. The
// footprint a splat can reach does grow with the 3DGS dilation, and that is
// camera-dependent - so the box is sized with a dilation BOUND (`b2`) the
// host guarantees is at least what any current view needs. Evaluation in the
// tracer stays exact per camera; the proxy only has to contain it.
//
// Only the Box proxy is built here. The Oriented proxy is a TLAS instance
// per splat, and wgpu builds instance transforms on the CPU (tracer.rs).
// ============================================================================

struct Geom {
    b2:        f32,   // isotropic variance bound: dilation + covariance bias
    alpha_min: f32,
    n:         u32,
    mode:      u32,   // unused, kept for layout
};

@group(0) @binding(0) var<uniform> G: Geom;
@group(0) @binding(1) var<storage, read> pos_op: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> scl: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> rot: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> aabb: array<f32>;   // 6 per splat

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>,
        @builtin(num_workgroups) nwg: vec3<u32>) {
    let i = gid.x + gid.y * nwg.x * 256u;
    if (i >= G.n) { return; }
    let p = pos_op[i];
    let mu = p.xyz;
    let op = p.w;

    // faint splats get zero-volume boxes: at worst they report a splat whose
    // opacity is below alpha_min, which the tracer rejects
    var ext = vec3<f32>(0.0);
    if (op >= G.alpha_min) {
        let qr = rot[i];
        let q = qr / max(length(qr), 1e-12);
        let w = q.x; let x = q.y; let y = q.z; let z = q.w;
        let R0 = vec3<f32>(1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - w * z), 2.0 * (x * z + w * y));
        let R1 = vec3<f32>(2.0 * (x * y + w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - w * x));
        let R2 = vec3<f32>(2.0 * (x * z - w * y), 2.0 * (y * z + w * x), 1.0 - 2.0 * (x * x + y * y));
        let s = scl[i].xyz;
        let se = sqrt(s * s + vec3<f32>(G.b2));
        // radius of the alpha_min contour in the splat's canonical space:
        // opacity * exp(-k^2/2) = alpha_min  =>  k = sqrt(2 ln(opacity / alpha_min))
        let ks = sqrt(2.0 * log(op / G.alpha_min));
        ext = ks * vec3<f32>(length(R0 * se), length(R1 * se), length(R2 * se));
    }
    let o6 = i * 6u;
    aabb[o6] = mu.x - ext.x; aabb[o6 + 1u] = mu.y - ext.y; aabb[o6 + 2u] = mu.z - ext.z;
    aabb[o6 + 3u] = mu.x + ext.x; aabb[o6 + 4u] = mu.y + ext.y; aabb[o6 + 5u] = mu.z + ext.z;
}
