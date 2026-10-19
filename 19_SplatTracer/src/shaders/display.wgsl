// Accumulation buffer to screen. 3DGS colour is already display space - the
// splats were fitted to sRGB photographs - so this divides by the sample
// count and writes; no transfer curve, or the scan would come out washed.

struct View {
    // render width, height, origin x, origin y (window pixels)
    dim: vec4<u32>,
};

@group(0) @binding(0) var<storage, read> accum: array<vec4<f32>>;
@group(0) @binding(1) var<uniform> V: View;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    return vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let x = i32(p.x) - i32(V.dim.z);
    let y = i32(p.y) - i32(V.dim.w);
    if (x < 0 || y < 0 || x >= i32(V.dim.x) || y >= i32(V.dim.y)) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let a = accum[u32(y) * V.dim.x + u32(x)];
    return vec4<f32>(clamp(a.xyz / max(a.w, 1.0), vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
