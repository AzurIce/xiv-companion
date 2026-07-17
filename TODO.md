# Rendering Pipeline TODO

This document tracks rendering-fidelity gaps found by comparing the current pipeline with the
field and texture semantics used by Meddle and MeddleTools.

MeddleTools is a behavioral reference, not an authoritative copy of the in-game shaders. Its
`shaders.blend` materials are hand-crafted approximations and the project is licensed under
AGPL-3.0-or-later. Keep WGSL implementations independent and use MeddleTools primarily to verify
texture roles, UV routing, material keys, ColorTable fields, and expected feature composition.

## P0 - Visible Correctness

- [ ] Fix two-sided shading-normal orientation.
  - Apply the correction after every world-space normal path, including geometric, primary TBN,
    secondary-map TBN, tile-array, detail-array, and flow-based normals.
  - Do not rely only on `front_facing`; imported vertex normals may disagree with triangle winding.
  - Preserve tangent-space handedness when the normal is flipped.
  - Add an offscreen regression test that renders the same two-sided surface from both directions
    and compares luminance and specular response.
  - Relevant code: `crates/xiv-companion-render/src/renderer/model.wgsl` (`resolve_normal`).

- [ ] Correct ColorTable specular color-space handling.
  - `BakedColorTableMaps::specular_rgba` contains sRGB-encoded RGB and linear anisotropy alpha.
  - Create the GPU texture with sRGB RGB decoding while preserving alpha as linear data.
  - Keep original packed `g_SamplerSpecularMap0` handling separate; MeddleTools treats that input
    as Non-Color and it is not equivalent to the baked ColorTable specular ramp.
  - Add a test that checks the sampled linear GPU value against the source ColorTable value.
  - Relevant code: `crates/xiv-companion-data/src/model.rs` (`bake_color_table_maps`) and
    `crates/xiv-companion-render/src/renderer/model.rs` (`specular_texture_view`).

- [ ] Move scene lighting to an HDR intermediate target and add output tone mapping.
  - Replace the `Rgba8Unorm` scene/bright intermediates with an appropriate float format such as
    `Rgba16Float`, subject to WebGPU format support.
  - Apply exposure and a documented tone-mapping operator during the final compose pass.
  - Perform output encoding exactly once according to the target surface format.
  - Verify that metal highlights and emissive values above 1.0 retain detail before composition.
  - Relevant code: `crates/xiv-companion-render/src/renderer/model.rs` (`POST_FORMAT`) and
    `crates/xiv-companion-render/src/renderer/postprocess.wgsl` (`compose_fs`).

## P1 - Missing Material Inputs

- [ ] Bind and sample material environment maps.
  - Carry `PreparedTextureBindings::environment` and its prepared sampling policy into the GPU
    bind group.
  - Implement the material-specific mapping semantics before applying roughness/Fresnel response.
  - Retain the procedural studio environment only as a preview fallback when no environment map is
    available.
  - Add a regression fixture where toggling the environment texture changes only the reflection
    contribution.

- [ ] Implement the complete water texture path.
  - Bind `water_wave`, `water_wave1`, and `water_whitecap` independently.
  - Do not alias the primary wave texture to the generic normal slot as the final implementation.
  - Reproduce the verified UV routing, scrolling, normal composition, whitecap mask, refraction
    color, deep color, transparency, and blend behavior.
  - Add a focused render fixture that proves both wave maps and the whitecap texture contribute.
  - Relevant code: `effective_normal_texture` in
    `crates/xiv-companion-render/src/renderer/model.rs`.

- [ ] Make GPU texture formats follow `PreparedTextureSampling::color_space` consistently.
  - Centralize the mapping from prepared color space to texture format/mip semantic.
  - Audit base, emissive, specular, secondary maps, ColorTable-derived maps, detail arrays,
    environment maps, and water maps.
  - Keep special packed-normal and mixed color/data textures explicit rather than inferring their
    semantics from names.
  - Add table-driven tests covering every `ModelTextureKind`.

## P1 - Shader Family Separation

- [ ] Stop silently rendering known-incomplete shader families through the generic surface model.
  - Consume `PreparedMaterialUnsupportedInputs` in the renderer instead of using it only as data
    metadata.
  - Define an explicit fallback or diagnostic behavior for unsupported inputs.
  - Prioritize `Crystal`, `CharacterGlass`, `CharacterReflection`, `CharacterScroll`, and
    `LightShaft` families used by visible weapon parts and effects.

- [ ] Validate ColorTable lighting-field composition against MeddleTools node topology.
  - Treat Metalness, Roughness, GlossStrength, and SpecularStrength as independent inputs until a
    verified relationship is established.
  - Replace empirical formulas such as scaling roughness by GlossStrength and mapping
    SpecularStrength directly to an arbitrary dielectric F0 range.
  - Verify SheenRate, SheenTint, SheenAptitude, SphereIndex, SphereMask, and anisotropy separately.
  - Document which behavior is verified, approximated, or unsupported for every shader family.

- [ ] Review remaining family-specific behavior.
  - AlphaMulti/AlphaMulti2/AlphaMulti3 value modes.
  - Character reflection, glass, stockings, tattoo, occlusion, and scroll variants.
  - Lightshaft clipping and animation parameters.
  - Ambient-occlusion masks, legacy specular modes, tile mip bias, and vertex movement.
  - Decal, crest, runtime skin color, sub-color, and other runtime-only inputs.

## P2 - Postprocessing And Presentation

- [ ] Recalibrate bloom after the HDR and tone-mapping changes.
  - Define the bright-pass threshold in scene-linear units.
  - Avoid embedding unrelated highlight extraction constants in the material shader.
  - Test emissive-only, dielectric, and metallic fixtures independently.

- [ ] Define a stable preview-lighting contract.
  - Specify key/fill/environment orientation, intensity, color temperature, and camera relationship.
  - Keep preview lighting separate from material semantics so lighting changes cannot compensate for
    incorrect normals or texture decoding.
  - Add fixed-camera reference renders for representative matte, glossy, metallic, emissive, and
    transparent materials.

## Verification Infrastructure

- [ ] Add GPU/offscreen pixel tests in addition to source-string and field-passthrough tests.
  - Two-sided normal orientation.
  - ColorTable diffuse/specular/emissive color-space decoding.
  - Metallic roughness and Fresnel response.
  - HDR preservation and tone-mapped output.
  - Environment, water, tile, detail, sheen, sphere, transparency, and lightshaft contributions.

- [ ] Add a small checked-in material fixture matrix.
  - Include representative legacy and Dawntrail ColorTables.
  - Include at least one single-value, multi-value, dyed, metallic, emissive, transparent, and
    double-sided material.
  - Store expected numeric probes or tightly controlled snapshots rather than relying only on
    manual inspection.

- [ ] Ensure renderer tests run with the `renderer` feature in the default CI verification path.
  - `cargo test -p xiv-companion-render` currently does not compile the renderer module.
  - Include `cargo test -p xiv-companion-render --features renderer` in CI or the repository's
    standard verification command.

## Reference Files

- `E:\repos\Meddle\Meddle\Meddle.Utils\Export\Model.cs`
- `E:\repos\Meddle\Meddle\Meddle.Utils\Export\Mesh.cs`
- `E:\repos\Meddle\Meddle\Meddle.Utils\Export\Vertex.cs`
- `E:\repos\Meddle\Meddle\Meddle.Utils\Files\Structs\Material\ColorTableRow.cs`
- `E:\repos\MeddleTools\MeddleTools\node_setup\node_configs.py`
- `E:\repos\MeddleTools\MeddleTools\node_setup\node_mappings.py`
- `E:\repos\MeddleTools\MeddleTools\bake\bake_utils.py`
- `E:\repos\MeddleTools\MeddleTools\shaders.blend`

