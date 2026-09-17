//! Build-time shader compilation.
//!
//! The model shader is authored as a WESL package in
//! `src/renderer/model/` and compiled to plain WGSL at build time. The
//! artifact lands in `OUT_DIR/model.wgsl` and is included via
//! `include_wesl!("model")` from the renderer module.

fn main() {
    #[cfg(feature = "renderer")]
    {
        wesl::Wesl::new("src/renderer/model")
            .set_mangler(wesl::ManglerKind::None)
            .set_options(wesl::CompileOptions {
                // Keep every declaration from every module in the output:
                // the shader is one closed module set and predictable output
                // (no reachability pruning) keeps diffs reviewable.
                strip: false,
                ..Default::default()
            })
            .build_artifact(&"package::main".parse().unwrap(), "model");
    }
}
