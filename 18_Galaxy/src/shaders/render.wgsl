// ============================================================================
// Star field render.
//
// Vertex pulling into a PointList: no vertex buffer, no index buffer, one
// vertex per system read straight from the storage buffer the compute pass
// wrote. At 10^8 points a quad-based sprite would cost six vertices each and
// an overdraw bill on top; a one-pixel additive point is both cheaper and the
// physically honest thing to draw, since what a galaxy image shows IS the
// accumulated density along the line of sight.
// ============================================================================

struct Camera {
    view_proj:   mat4x4<f32>,
    eye:         vec4<f32>,
    // x: exposure  y: point alpha  z: luminosity weight  w: unused
    tune:        vec4<f32>,
    // x: colour mode  y: system count  z: now (decades)  w: front window (decades)
    opts:        vec4<f32>,
};

@group(0) @binding(0) var<uniform> C: Camera;
@group(0) @binding(1) var<storage, read> pos: array<f32>;
@group(0) @binding(2) var<storage, read> state: array<u32>;
// bits 0..31: arrival in decades + 1, 0 = never reached; bit 31: colonising
@group(0) @binding(3) var<storage, read> colony: array<u32>;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0)       rgb:  vec3<f32>,
    @location(1)       a:    f32,
};

// Approximate main-sequence colours, O through M.
fn class_colour(c: u32) -> vec3<f32> {
    switch (c) {
        case 0u:  { return vec3<f32>(0.61, 0.69, 1.00); }   // O
        case 1u:  { return vec3<f32>(0.67, 0.75, 1.00); }   // B
        case 2u:  { return vec3<f32>(0.79, 0.84, 1.00); }   // A
        case 3u:  { return vec3<f32>(0.97, 0.97, 1.00); }   // F
        case 4u:  { return vec3<f32>(1.00, 0.96, 0.84); }   // G
        case 5u:  { return vec3<f32>(1.00, 0.82, 0.63); }   // K
        default:  { return vec3<f32>(1.00, 0.66, 0.45); }   // M
    }
}

// Rough L/Lsun by class, log-compressed. Full mass-luminosity spread is 10^5
// to 1, which at one pixel per star blows every hot star to white and hides
// the disk; the weight slider moves between honest and legible.
fn class_lum(c: u32) -> f32 {
    switch (c) {
        case 0u:  { return 12.0; }
        case 1u:  { return 6.0;  }
        case 2u:  { return 3.0;  }
        case 3u:  { return 1.8;  }
        case 4u:  { return 1.0;  }
        case 5u:  { return 0.5;  }
        default:  { return 0.15; }
    }
}

fn component_colour(k: u32) -> vec3<f32> {
    switch (k) {
        case 0u:  { return vec3<f32>(0.55, 0.78, 1.00); }   // thin disk
        case 1u:  { return vec3<f32>(1.00, 0.85, 0.55); }   // thick disk
        case 2u:  { return vec3<f32>(1.00, 0.62, 0.40); }   // bar / bulge
        default:  { return vec3<f32>(0.70, 0.70, 0.85); }   // halo
    }
}

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VOut {
    let o = vi * 3u;
    let p = vec3<f32>(pos[o], pos[o + 1u], pos[o + 2u]);
    let s = state[vi];

    let comp = s & 7u;
    let cls  = (s >> 3u) & 7u;
    let hab  = f32((s >> 6u) & 1023u) / 1023.0;

    var out: VOut;
    out.clip = C.view_proj * vec4<f32>(p, 1.0);

    var rgb: vec3<f32>;
    var gain = 1.0;
    let mode = u32(C.opts.x);
    if (mode == 1u) {
        rgb = component_colour(comp);
    } else if (mode == 2u) {
        // habitability: cold indigo through to warm white at the top
        rgb = mix(vec3<f32>(0.10, 0.12, 0.30), vec3<f32>(1.00, 0.94, 0.78), hab);
    } else if (mode == 3u) {
        // Colonisation. The substrate recedes; the colonised set is drawn at
        // "now", and the front - what arrived in the last window - burns
        // brightest, so the wave reads as a wave and not just a stain.
        let c = colony[vi];
        let t = c & 0x7FFFFFFFu;
        let open = (c & 0x80000000u) != 0u;
        let now = C.opts.z;
        if (t == 0u || f32(t - 1u) > now) {
            rgb = class_colour(cls) * 0.30;
            gain = 0.35;
        } else {
            let age = now - f32(t - 1u);
            let front = exp(-age / max(C.opts.w, 1.0));
            // colonising colonies gold, dead-end colonies a cool blue
            let base = select(vec3<f32>(0.40, 0.62, 0.95), vec3<f32>(1.00, 0.76, 0.32), open);
            rgb = mix(base, vec3<f32>(1.0, 1.0, 1.0), front * 0.8);
            gain = 1.6 + 4.0 * front;
        }
    } else {
        rgb = class_colour(cls);
    }

    // luminosity weight at 0 treats every system as one sample - which is what
    // a colonisation substrate actually is, one site per point
    let lw = mix(1.0, class_lum(cls), C.tune.z);
    out.rgb = rgb;
    out.a   = C.tune.y * lw * C.tune.x * gain;
    return out;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.rgb * in.a, in.a);
}
