//! Static validation of the WGSL, including the ray-query shader - wgpu only
//! compiles shaders at `create_shader_module`, which is to say at run time.

use naga::valid::{Capabilities, ValidationFlags, Validator};
use splat_tracer::tracer::{compose, DISPLAY_WGSL, GEOM_WGSL, PREP_WGSL, TRACE_WGSL};

fn validate(name: &str, src: &str) {
    let full = compose(src);
    let module = match naga::front::wgsl::parse_str(&full) {
        Ok(m) => m,
        Err(e) => panic!("{name}: parse failed\n{}", e.emit_to_string(&full)),
    };
    let mut v = Validator::new(ValidationFlags::all(), Capabilities::all());
    if let Err(e) = v.validate(&module) {
        panic!("{name}: validation failed\n{}", e.emit_to_string(&full));
    }
}

#[test]
fn geom_is_valid() {
    validate("geom", GEOM_WGSL);
}

#[test]
fn prep_is_valid() {
    validate("prep", PREP_WGSL);
}

#[test]
fn trace_is_valid() {
    validate("trace", TRACE_WGSL);
}

#[test]
fn display_is_valid() {
    validate("display", DISPLAY_WGSL);
}

/// The Frame uniform is written by hand on both sides; eleven vec4, 176 bytes.
#[test]
fn frame_uniform_is_176_bytes() {
    assert_eq!(std::mem::size_of::<splat_tracer::tracer::FrameUniform>(), 176);
}
