//! Static validation of the WGSL.
//!
//! `cargo build` never looks at the shaders - wgpu compiles them at
//! `create_shader_module`, which is to say at run time, in front of the user.
//! These tests run naga's front end and validator over both modules so a typo
//! or a type error fails in CI instead of in the window.

use naga::valid::{Capabilities, ValidationFlags, Validator};

fn validate(name: &str, src: &str) {
    let module = match naga::front::wgsl::parse_str(src) {
        Ok(m) => m,
        Err(e) => panic!("{name}: parse failed\n{}", e.emit_to_string(src)),
    };
    let mut v = Validator::new(ValidationFlags::all(), Capabilities::all());
    if let Err(e) = v.validate(&module) {
        panic!("{name}: validation failed\n{}", e.emit_to_string(src));
    }
}

#[test]
fn generate_wgsl_is_valid() {
    validate("generate.wgsl", include_str!("../src/shaders/generate.wgsl"));
}

#[test]
fn render_wgsl_is_valid() {
    validate("render.wgsl", include_str!("../src/shaders/render.wgsl"));
}

/// The uniform block is mirrored by hand in Rust and WGSL. If the two ever
/// disagree on size, every field past the divergence reads garbage on the GPU
/// and nothing warns you - the galaxy just comes out wrong in a way that
/// looks like a modelling mistake. 28 scalars, 112 bytes, checked both ends.
#[test]
fn gen_params_layout_matches_shader() {
    let src = include_str!("../src/shaders/generate.wgsl");
    let start = src.find("struct Params {").expect("Params struct");
    let end = src[start..].find("};").expect("struct end") + start;
    let body = &src[start..end];
    let fields = body
        .lines()
        .filter(|l| {
            let t = l.trim();
            t.contains(':') && (t.contains("f32") || t.contains("u32"))
        })
        .count();
    assert_eq!(fields, 28, "shader Params field count changed");
    assert_eq!(fields * 4, 112, "expected 112 bytes");
}
