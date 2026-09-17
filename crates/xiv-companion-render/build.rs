//! Build-time shader compilation.
//!
//! The shaders in `src/renderer/shaders/` form a WESL package and are
//! compiled to plain WGSL artifacts in `OUT_DIR` by wesl-rs:
//!
//! - `main.wesl` (package root, entry points) + modules → `model.wgsl`
//! - `postprocess.wesl` (bloom/compose root) → `postprocess.wgsl`
//!
//! The renderer and the shader tests include the artifacts via
//! `env!("OUT_DIR")`, so the compiled text is exactly what ships.

fn main() {
    #[cfg(feature = "renderer")]
    {
        let mut compiler = wesl::Wesl::new("src/renderer/shaders");
        compiler
            .set_mangler(wesl::ManglerKind::None)
            .set_options(wesl::CompileOptions {
                // Keep every declaration from every module in the output:
                // the shader is one closed module set and predictable output
                // (no reachability pruning) keeps diffs reviewable.
                strip: false,
                ..Default::default()
            });
        compiler.build_artifact(&"package::main".parse().unwrap(), "model");
        compiler.build_artifact(&"package::postprocess".parse().unwrap(), "postprocess");
    }
}
