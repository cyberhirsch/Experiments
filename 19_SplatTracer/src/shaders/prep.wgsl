// ============================================================================
// Per-camera preparation. One invocation per splat.
//
// Everything that depends on the camera but not on the ray is done once here
// instead of once per hit: the splat's colour (3DGS evaluates SH along the
// camera-to-centre direction, not the ray) and the exact screen-space
// dilation re-created in 3D. The proxies the RT cores test are NOT made here
// - see geom.wgsl - so moving the camera never costs a BVH rebuild.
//
// Mirrors `reference::prepare_one` line for line.
// ============================================================================

@group(0) @binding(0) var<uniform> F: Frame;
@group(0) @binding(1) var<storage, read> pos_op: array<vec4<f32>>;   // centre, opacity
@group(0) @binding(2) var<storage, read> scl: array<vec4<f32>>;      // scale xyz
@group(0) @binding(3) var<storage, read> rot: array<vec4<f32>>;      // quaternion w,x,y,z
@group(0) @binding(4) var<storage, read> sh: array<f32>;             // (d+1)^2 * 3 per splat
@group(0) @binding(5) var<storage, read_write> derived: array<Derived>;

const C0: f32 = 0.28209479;
const C1: f32 = 0.48860251;

fn shc(b: u32, k: u32) -> vec3<f32> {
    let o = b + k * 3u;
    return vec3<f32>(sh[o], sh[o + 1u], sh[o + 2u]);
}

// Transcribed from the 3DGS forward pass (computeColorFromSH).
// `deg` never exceeds the stored degree - the host clamps it - so the reads
// below stay inside this splat's stride.
fn sh_color(i: u32, d: vec3<f32>, deg: u32) -> vec3<f32> {
    let b = i * u32(F.misc.w);
    var r = C0 * shc(b, 0u);
    if (deg > 0u) {
        let x = d.x; let y = d.y; let z = d.z;
        r = r - C1 * y * shc(b, 1u) + C1 * z * shc(b, 2u) - C1 * x * shc(b, 3u);
        if (deg > 1u) {
            let xx = x * x; let yy = y * y; let zz = z * z;
            let xy = x * y; let yz = y * z; let xz = x * z;
            r = r + 1.0925484 * xy * shc(b, 4u)
                  - 1.0925484 * yz * shc(b, 5u)
                  + 0.31539157 * (2.0 * zz - xx - yy) * shc(b, 6u)
                  - 1.0925484 * xz * shc(b, 7u)
                  + 0.5462742 * (xx - yy) * shc(b, 8u);
            if (deg > 2u) {
                r = r - 0.5900436 * y * (3.0 * xx - yy) * shc(b, 9u)
                      + 2.8906114 * xy * z * shc(b, 10u)
                      - 0.4570458 * y * (4.0 * zz - xx - yy) * shc(b, 11u)
                      + 0.37317634 * z * (2.0 * zz - 3.0 * xx - 3.0 * yy) * shc(b, 12u)
                      - 0.4570458 * x * (4.0 * zz - xx - yy) * shc(b, 13u)
                      + 1.4453057 * z * (xx - yy) * shc(b, 14u)
                      - 0.5900436 * x * (xx - 3.0 * yy) * shc(b, 15u);
            }
        }
    }
    return max(r + vec3<f32>(0.5), vec3<f32>(0.0));
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>,
        @builtin(num_workgroups) nwg: vec3<u32>) {
    // 2-D grid: one dimension caps at 65535 workgroups = 16.8 M splats
    let i = gid.x + gid.y * nwg.x * 256u;
    if (i >= F.ints.x) { return; }

    let p = pos_op[i];
    let mu = p.xyz;
    let op = p.w;
    let depth = dot(mu - F.cam_pos.xyz, F.cam_fwd.xyz);

    if (depth <= F.cam_pos.w || op < F.tone.x) {
        // Culled: zero opacity, so any report of its proxy is rejected as
        // faint rather than composited.
        derived[i] = Derived(vec4<f32>(0.0, 0.0, 0.0, mu.x),
                             vec4<f32>(0.0, 0.0, 0.0, mu.y),
                             vec4<f32>(0.0, 0.0, 0.0, mu.z),
                             vec4<f32>(0.0));
        return;
    }

    let qr = rot[i];
    let q = qr / max(length(qr), 1e-12);
    let w = q.x; let x = q.y; let y = q.z; let z = q.w;
    let R0 = vec3<f32>(1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - w * z), 2.0 * (x * z + w * y));
    let R1 = vec3<f32>(2.0 * (x * y + w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - w * x));
    let R2 = vec3<f32>(2.0 * (x * z - w * y), 2.0 * (y * z + w * x), 1.0 - 2.0 * (x * x + y * y));

    // 3DGS adds 0.3 px^2 after projection and trains with it on, so its
    // splats may be thinner than a pixel and still show. At depth z one pixel
    // spans z/f world units, so the same variance in 3D is 0.3 (z/f)^2.
    let s = scl[i].xyz;
    let f = sqrt(F.cam_right.w * F.cam_down.w);
    let px_world = depth / f;
    let b2 = F.tone.w * px_world * px_world + F.misc.x * F.misc.x;
    let se = sqrt(s * s + vec3<f32>(b2));

    // M = S^-1 R^T: row r is column r of R over the r-th scale
    let m0 = vec3<f32>(R0.x, R1.x, R2.x) / se.x;
    let m1 = vec3<f32>(R0.y, R1.y, R2.y) / se.y;
    let m2 = vec3<f32>(R0.z, R1.z, R2.z) / se.z;

    let dir = normalize(mu - F.cam_pos.xyz);
    let col = sh_color(i, dir, F.ints.z);
    derived[i] = Derived(vec4<f32>(m0, mu.x), vec4<f32>(m1, mu.y), vec4<f32>(m2, mu.z), vec4<f32>(col, op));
}
