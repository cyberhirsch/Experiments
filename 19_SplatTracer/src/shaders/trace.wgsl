// ============================================================================
// The splat tracer. One invocation per pixel.
//
// Splats are semi-transparent and overlap by the hundred, and RT hardware is
// built to find ONE closest hit. 3DGRT's answer, done here with ray query
// instead of OptiX any-hit programs:
//
//   traverse, and for every candidate proxy the RT cores report, evaluate the
//   splat's peak response along the ray; keep the k nearest in a sorted
//   buffer; once the buffer is full, shrink the ray's far end so the hardware
//   stops visiting anything beyond. Then shade the k in order, front to back,
//   and - if the ray is not yet opaque - re-cast from the last one.
//
// Order is by each splat's own peak distance t*, not by where the ray first
// touches its proxy, so the result equals sorting every hit along the ray
// and blending them all: each pass gathers exactly the k nearest splats
// beyond the previous pass. (3DGRT orders by proxy distance, an
// approximation; here the proxy only decides what gets looked at.)
//
// Proxies are always BOXES, because a box is reported whenever the ray's
// remaining segment overlaps it. A triangle proxy is reported only where
// the ray crosses its surface - and after a re-cast, a long proxy can hold
// the whole remaining segment inside it, cross no surface, and silently
// drop its splat. Tightness comes from orienting the box instead: one TLAS
// instance per splat, and the RT cores test the unit box in the splat's
// own frame.
// ============================================================================

enable wgpu_ray_query;

@group(0) @binding(0) var<uniform> F: Frame;
@group(0) @binding(1) var acc: acceleration_structure;
@group(0) @binding(2) var<storage, read> derived: array<Derived>;
@group(0) @binding(3) var<storage, read_write> accum: array<vec4<f32>>;
// per-pixel traversal statistics, written only when F.lens.w is set:
// reported, rejected-behind, rejected-faint, rejected-far, accepted, passes,
// composited, unused
@group(0) @binding(4) var<storage, read_write> stats: array<u32>;
// [0] rays that hit the work cap this sample; cleared by the host per sample
@group(0) @binding(5) var<storage, read_write> counters: array<atomic<u32>, 4>;

// Set when a ray exhausted its budget of box reports. A GPU job cannot be
// interrupted from outside, and Windows resets the driver - or hangs trying -
// if one runs past ~2 s. With one BLAS of boxes (Box proxy) every box the ray
// overlaps is reported here, so the budget bounds the whole traversal. With
// one instance per splat (Oriented) it does not: entering an instance whose
// tilted box the ray then misses is never reported, and costs work this
// counter cannot see.
var<private> capped: bool;

var<private> st_rep: u32;
var<private> st_behind: u32;
var<private> st_faint: u32;
var<private> st_far: u32;
var<private> st_acc: u32;
var<private> st_pass: u32;
var<private> st_comp: u32;

const KMAX: u32 = 16u;
const MAX_PASSES: u32 = 512u;

// ---------------------------------------------------------------- rng -------
fn pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}
var<private> rs: u32;
fn rnd() -> f32 { rs = pcg(rs); return f32(rs >> 8u) * (1.0 / 16777216.0); }

// Shirley-Chiu concentric map: uniform on the disk without the polar map's
// clumping at the centre.
fn disk(u: vec2<f32>) -> vec2<f32> {
    let a = 2.0 * u - vec2<f32>(1.0);
    if (a.x == 0.0 && a.y == 0.0) { return vec2<f32>(0.0); }
    var r: f32; var th: f32;
    if (abs(a.x) > abs(a.y)) { r = a.x; th = 0.78539816 * (a.y / a.x); }
    else { r = a.y; th = 1.57079633 - 0.78539816 * (a.x / a.y); }
    return r * vec2<f32>(cos(th), sin(th));
}

// Parameter of the ray's closest approach to the splat centre in the splat's
// own metric, and alpha there. The origin is made relative to the centre
// BEFORE the canonical scaling: done the other way round, a 1 mm splat seen
// from 10 m loses every significant digit to cancellation.
fn eval(i: u32, o: vec3<f32>, d: vec3<f32>) -> vec2<f32> {
    let g = derived[i];
    let rel = o - vec3<f32>(g.r0.w, g.r1.w, g.r2.w);
    let op = vec3<f32>(dot(g.r0.xyz, rel), dot(g.r1.xyz, rel), dot(g.r2.xyz, rel));
    let dp = vec3<f32>(dot(g.r0.xyz, d), dot(g.r1.xyz, d), dot(g.r2.xyz, d));
    let dd = max(dot(dp, dp), 1e-30);
    let t = -dot(op, dp) / dd;
    let c = op + t * dp;
    let a = min(F.tone.z, g.c.w * exp(-0.5 * dot(c, c)));
    return vec2<f32>(t, a);
}

// Returns colour and the transmittance left for the background.
fn trace(o: vec3<f32>, d: vec3<f32>) -> vec4<f32> {
    let k = clamp(F.ints.y, 1u, KMAX);
    let instanced = F.geo.x == 1u;
    var C = vec3<f32>(0.0);
    var T = 1.0;
    var tmin = 0.0;

    for (var ps = 0u; ps < MAX_PASSES; ps = ps + 1u) {
        st_pass = st_pass + 1u;
        var ht: array<f32, KMAX>;
        var ha: array<f32, KMAX>;
        var hi: array<u32, KMAX>;
        var hc = 0u;

        var rq: ray_query;
        rayQueryInitialize(&rq, acc, RayDesc(RAY_FLAG_NONE, 0xFFu, tmin, 1e30, o, d));
        while (rayQueryProceed(&rq)) {
            let cand = rayQueryGetCandidateIntersection(&rq);
            if (cand.kind != RAY_QUERY_INTERSECTION_AABB) { continue; }
            // which splat: its instance carries the index, or it is the
            // primitive's position in the single BLAS
            var i = cand.primitive_index;
            if (instanced) { i = cand.instance_custom_data; }
            st_rep = st_rep + 1u;
            if (st_rep > F.geo.y) {
                rayQueryTerminate(&rq);
                capped = true;
                break;
            }
            let e = eval(i, o, d);

            var take = false;
            if (e.x <= tmin) {
                // strictly beyond the last pass: that is what makes re-casting exact
                st_behind = st_behind + 1u;
            } else if (e.y < F.tone.x) {
                st_faint = st_faint + 1u;
            } else if (hc == k && e.x >= ht[k - 1u]) {
                st_far = st_far + 1u;
            } else {
                // the hardware may report a primitive twice; composite once
                var dup = false;
                for (var j = 0u; j < hc; j = j + 1u) { if (hi[j] == i) { dup = true; break; } }
                take = !dup;
            }

            if (take) {
                // insertion into the sorted buffer; when full, the farthest is dropped
                var j: u32;
                if (hc < k) { j = hc; hc = hc + 1u; } else { j = k - 1u; }
                loop {
                    if (j == 0u) { break; }
                    if (ht[j - 1u] <= e.x) { break; }
                    ht[j] = ht[j - 1u]; ha[j] = ha[j - 1u]; hi[j] = hi[j - 1u];
                    j = j - 1u;
                }
                ht[j] = e.x; ha[j] = e.y; hi[j] = i;
                st_acc = st_acc + 1u;
            }

            // Buffer full: commit at the k-th distance so the RT cores cull
            // every box beginning beyond it. A box always encloses its splat's
            // peak, so nothing culled can be nearer than t_k.
            if (take && hc == k) {
                // bound to a `let`: naga 30 does not emit an expression passed
                // straight into this call, and the validator rejects it
                let tk = ht[k - 1u];
                rayQueryGenerateIntersection(&rq, tk);
            }
        }

        // an interrupted traversal has not seen everything nearer than what it
        // collected; compositing that batch could put a far splat in front of
        // a near one. Stop with what is certain.
        if (capped) { return vec4<f32>(C, T); }

        // shade this batch front to back, with the 3DGS termination rule
        for (var j = 0u; j < hc; j = j + 1u) {
            let a = ha[j];
            let test_t = T * (1.0 - a);
            if (test_t < F.tone.y) { return vec4<f32>(C, T); }
            C = C + T * a * derived[hi[j]].c.xyz;
            T = test_t;
            st_comp = st_comp + 1u;
        }
        if (hc < k) { break; }      // the traversal found fewer than k: nothing left
        tmin = ht[k - 1u];
    }
    return vec4<f32>(C, T);
}

// One dispatch covers one tile. A whole frame in one dispatch can run for
// seconds on a heavy scene, and while it does Windows cannot draw the
// desktop; tiles submitted separately give the OS a gap after each one.
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = u32(F.img.x);
    let h = u32(F.img.y);
    let px = gid.x + u32(F.misc.y);
    let py = gid.y + u32(F.misc.z);
    if (px >= w || py >= h) { return; }
    rs = pcg(px * 1973u + py * 9277u + F.ints.w * 26699u) | 1u;

    var sx = f32(px) + 0.5;
    var sy = f32(py) + 0.5;
    if (F.lens.z > 0.5) {
        sx = f32(px) + rnd();
        sy = f32(py) + rnd();
    }
    let dx = (sx - F.img.z) / F.cam_right.w;
    let dy = (sy - F.img.w) / F.cam_down.w;
    var dir = normalize(F.cam_right.xyz * dx + F.cam_down.xyz * dy + F.cam_fwd.xyz);
    var org = F.cam_pos.xyz;

    // thin lens: every ray through the lens meets at the focus plane
    if (F.lens.x > 0.0) {
        let focus = org + dir * (F.lens.y / dot(dir, F.cam_fwd.xyz));
        let l = disk(vec2<f32>(rnd(), rnd())) * F.lens.x;
        org = org + F.cam_right.xyz * l.x + F.cam_down.xyz * l.y;
        dir = normalize(focus - org);
    }

    let r = trace(org, dir);
    var col = r.xyz + r.w * F.bg.xyz;
    let idx = py * w + px;
    if (capped) {
        atomicAdd(&counters[0], 1u);
        if ((F.geo.z & 1u) != 0u) { col = vec3<f32>(1.0, 0.0, 1.0); }
    }
    if (F.lens.w > 0.5) {
        let b = idx * 8u;
        stats[b] = st_rep; stats[b + 1u] = st_behind; stats[b + 2u] = st_faint;
        stats[b + 3u] = st_far; stats[b + 4u] = st_acc; stats[b + 5u] = st_pass;
        stats[b + 6u] = st_comp; stats[b + 7u] = select(0u, 1u, capped);
    }
    if (F.bg.w > 0.5) {
        accum[idx] = accum[idx] + vec4<f32>(col, 1.0);
    } else {
        accum[idx] = vec4<f32>(col, 1.0);
    }
}
