// ============================================================================
// Tone mapping.
//
// A galaxy's surface density spans four or five decades between the bulge and
// the outer disk. Accumulating that additively straight into an 8-bit surface
// clips the core to flat white and crushes the faint half of the disk into the
// first code value - which is exactly how a structured galaxy comes out
// looking like a uniform smudge.
//
// So the stars accumulate into an Rgba16Float target and get stretched here:
// an exponential shoulder that cannot clip, then a gamma lift that opens up
// the faint end. That is the astrophotographer's stretch, and it is the same
// reason № 01 maps flame density logarithmically.
// ============================================================================

struct Tone {
    // x: exposure   y: gamma   z: black point   w: unused
    p: vec4<f32>,
};

@group(0) @binding(0) var hdr: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> T: Tone;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VOut {
    // one oversized triangle, no vertex buffer
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    var o: VOut;
    o.uv = vec2<f32>(x, y);
    o.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return o;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let l = textureSample(hdr, samp, in.uv).rgb;

    // shoulder: maps [0, inf) into [0, 1) with no hard clip, so the bulge
    // keeps its internal gradient instead of flattening
    let c = vec3<f32>(1.0) - exp(-max(l, vec3<f32>(0.0)) * T.p.x);

    // lift: the outer disk and the arms live in the bottom few percent of the
    // range, and this is what makes them readable
    let g = pow(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / max(T.p.y, 0.01)));

    // black point, so empty sky stays black rather than grey-lifted noise
    let b = clamp((g - vec3<f32>(T.p.z)) / max(1.0 - T.p.z, 0.001), vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(b, 1.0);
}
