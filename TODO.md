# Rendering Pipeline TODO

This document tracks rendering-fidelity gaps found by comparing the current pipeline with the
field and texture semantics used by Meddle and MeddleTools.

MeddleTools is a behavioral reference, not an authoritative copy of the in-game shaders. Its
`shaders.blend` materials are hand-crafted approximations and the project is licensed under
AGPL-3.0-or-later. Keep WGSL implementations independent and use MeddleTools primarily to verify
texture roles, UV routing, material keys, ColorTable fields, and expected feature composition.

## P0 - Visible Correctness

- [x] Use a per-fragment view direction under the perspective camera.
  - Pass camera position and interpolated world position to fragment shading instead of reusing the
    model-center `view_dir` for the whole viewport.
  - Use the same vector for GGX/Fresnel, environment reflection, rim, ambient view fill, and
    two-sided normal orientation.
  - Keep the center view direction only for the camera-relative preview-light contract and a
    degenerate-position fallback.
  - Add a `ViewDirection` debug mode and native WGPU perspective-plane regression.

- [x] Fix two-sided shading-normal orientation.
  - Apply the correction after every Final world-space normal path, including geometric, primary
    TBN, secondary-map TBN, tile-array, and flow-based normals.
  - Do not rely only on `front_facing`; imported vertex normals may disagree with triangle winding.
  - Preserve tangent-space handedness when the normal is flipped.
  - Add an offscreen regression test that renders the same two-sided surface from both directions
    and compares luminance and specular response.
  - Relevant code: `crates/xiv-companion-render/src/renderer/model.wgsl` (`resolve_normal`).

- [x] Correct ColorTable specular color-space and HDR anisotropy handling.
  - `BakedColorTableMaps` preserves a compatibility sRGB RGBA8 ramp plus linear float RGB and
    unclamped anisotropy Alpha; renderer uses `Rgba16Float` for canonical baked ramps so installed
    values up to 7 are not truncated by UNORM.
  - Keep original packed `g_SamplerSpecularMap0` handling separate; MeddleTools treats that input
    as Non-Color and it is not equivalent to the baked ColorTable specular ramp.

- [x] Preserve installed HDR ColorTable diffuse/specular RGB ranges.
  - Full installed coverage reaches `6.7929688` for Diffuse and `4900` for Specular; both are linear
    ColorTable values and cannot be represented by the compatibility sRGB RGBA8 ramps alone.
  - Canonical baked diffuse/specular textures carry linear float A/B payloads and upload as
    `Rgba16Float`; Compatibility `base × colorset` decodes the source base from sRGB, multiplies in
    linear space, and keeps a float result beside its byte fallback.
  - Native WGPU pre-compose readback directly matches both installed maxima within half-float
    precision.
  - Installed audit rejects non-finite or out-of-range (`abs(value) > 65504`) values for every
    ColorTable channel uploaded through `Rgba16Float`.

- [x] Preserve HDR ColorTable emissive values in the canonical baked texture.
  - Installed ColorTable data reaches `61.46875`, so the compatibility sRGB RGBA8 ramp alone loses
    real emission intensity above 1.0.
  - Keep a linear float payload beside the byte ramp and upload canonical baked emissive as
    `Rgba16Float`; byte-only/source emissive textures retain the existing sRGB mip path.
  - Data and native WGPU fixtures verify float payload preservation, distinguish emission 2.0 from
    8.0 after tone mapping, and read the pre-compose HDR scene to match a source value of
    `61.46875` within `Rgba16Float` precision.
  - Relevant code: `crates/xiv-companion-data/src/model.rs` (`bake_color_table_maps`) and
    `crates/xiv-companion-render/src/renderer/model.rs` (`emissive_texture_view`).

- [x] Move scene lighting to an HDR intermediate target and add output tone mapping.
  - Replace the `Rgba8Unorm` scene/bright intermediates with an appropriate float format such as
    `Rgba16Float`, subject to WebGPU format support.
  - Apply exposure and a documented tone-mapping operator during the final compose pass.
  - Perform output encoding exactly once according to the target surface format.
  - Verify that metal highlights and emissive values above 1.0 retain detail before composition.
  - Relevant code: `crates/xiv-companion-render/src/renderer/model.rs` (`POST_FORMAT`) and
    `crates/xiv-companion-render/src/renderer/postprocess.wgsl` (`compose_fs`).

## P1 - Missing Material Inputs

- [x] Resolve the material environment-map support boundary.
  - Investigation outcome (MeddleTools `shaders.blend` tree parse): `g_SamplerEnvMap` only reaches
    the crystal group interface and has no downstream consumer; the bg tree has no EnvMap at all.
    No verified mapping semantics exist, so no speculative GPU sampling was added.
  - `PreparedTextureBindings::environment` and its prepared sampling policy stay available as
    data; the prepared `environment_mapping` unsupported flag and the `UnsupportedInputs` debug
    view surface the boundary.
  - The procedural studio environment remains the preview environment by design (no environment
    map semantics are available to fall back from).
  - Installed weapon audit has no environment sampler coverage, so there is no content to
    regression-test a mapping against.

- [x] Implement the MeddleTools-verified water subset and expose the remaining inputs.
  - Investigation outcome (MeddleTools `shaders.blend` tree parse): the verified semantics are
    exactly the implemented subset — `g_WaterDeepColor` as base color, primary `g_SamplerWaveMap`
    through the `NTNormal_Fix` R/G reconstruct as normal, and `g_Transparency` as alpha
    (MeddleTools also carries interface-level IOR/transmission defaults).
  - `water_wave1` and `water_whitecap` only reach the water group interface with no downstream
    consumer in MeddleTools, so they stay explicitly unsupported instead of receiving fabricated
    scrolling/whitecap/refraction behavior; a fixture proving their contribution is impossible by
    design.
  - The primary wave stays on the shared normal binding because the material bind group already
    uses 15 of the 16 sampled-textures-per-stage slots allowed by the WebGPU/downlevel (WebGL2)
    limit; three independent water slots would exceed it without changing a single pixel.
  - The existing synthetic water fixture (`render_mock_water_material_snapshot`) proves the
    primary wave and transparency contributions.

- [x] Make GPU texture formats follow `PreparedTextureSampling::color_space` consistently.
  - Centralize the mapping from prepared color space to texture format/mip semantic.
  - Audit base, emissive, specular, secondary maps, ColorTable-derived maps, detail arrays,
    environment maps, and water maps.
  - Keep special packed-normal and mixed color/data textures explicit rather than inferring their
    semantics from names.
  - Add table-driven tests covering every `ModelTextureKind`.

## P1 - Shader Family Separation

- [x] Stop silently rendering known-incomplete shader families through the generic surface model.
  - Consume `PreparedMaterialUnsupportedInputs` in the renderer instead of using it only as data
    metadata.
  - Define an explicit fallback or diagnostic behavior for unsupported inputs.
  - Prioritize `Crystal`, `CharacterGlass`, `CharacterReflection`, `CharacterScroll`, and
    `LightShaft` families used by visible weapon parts and effects.

- [x] Validate ColorTable lighting-field composition against MeddleTools node topology.
  - Treat Metalness, Roughness, GlossStrength, and SpecularStrength as independent inputs until a
    verified relationship is established.
  - Replace empirical formulas such as scaling roughness by GlossStrength and mapping
    SpecularStrength directly to an arbitrary dielectric F0 range.
  - Verify SheenRate, SheenTint, SheenAptitude, SphereIndex, SphereMask, and anisotropy separately.
  - Document which behavior is verified, approximated, or unsupported for every shader family.
  - Preserve baked specular-ramp Alpha as the verified anisotropy input, without treating packed
    `g_SamplerSpecularMap0` Alpha as the same semantic. The preview uses a tangent-oriented
    anisotropic GGX NDF while retaining the existing Smith approximation; a native WGPU fixture
    locks zero-anisotropy rotational invariance and nonzero directional response.
  - The checked MeddleTools character graph exposes Sheen/Sphere ramp sockets but leaves both at a
    dead-end mix-group interface. Their parsed constants, float ramps, bindings, and direct debug
    views remain available, but they no longer use empirical Final-lighting formulas. Nonzero
    SheenRate (857 resources/1335 references) and SphereMask (121 resources/183 references) are
    locked by the installed audit and surface as distinct `UnsupportedInputs` hues. A native WGPU
    fixture locks identical neutral/active Final output plus visible, distinct diagnostics.
  - Meddle only establishes the five `g_Toon*` names/defaults, while MeddleTools has no Toon node,
    socket, texture, or mapping. The installed audit covers 6399 material resources across
    character/legacy/glass/skin and finds zero MTRL overrides for every Toon constant. WGSL no
    longer invents golden-ratio lookup phases or fixed diffuse/specular bands; generic PBR uses a
    fixed preview specular scale, and future non-default Toon inputs produce a dedicated diagnostic.
    A native WGPU fixture locks identical default/override Final output and the override hue.

- [x] Review remaining family-specific behavior.
  - AlphaMulti/AlphaMulti2/AlphaMulti3 value modes.
  - Character reflection, glass, stockings, tattoo, occlusion, and scroll variants.
  - Lightshaft clipping and animation parameters.
  - Ambient-occlusion masks, legacy specular modes, and vertex movement.
  - Decal, crest, runtime skin color, sub-color, and other runtime-only inputs.
  - The 2026-07-19 full installed audit still contains only Character (8091 references), Skin
    (15), and CharacterGlass (6); there are no weapon Reflection/Scroll/Stockings/Tattoo/
    Occlusion/LightShaft families and no AlphaMulti values. `ApplyVertexMovement` remains Off for
    every audited material. Character AO mask has real coverage (198 resources/251 references,
    value 0.25); `g_SSAOMask` has four non-default resources/nine references with values near 1;
    and tile mip bias has three nonzero resources (`+1` and `-1`). Installed character/legacy DXBC
    now proves the tile formula and sampler scope: `max(log2(minAxis / 128), 0) + offset` biases only
    Tile Normal/ORB array samples. AO/SSAO and movement remain structured unsupported inputs and do
    not silently alter Final shading. The audit now locks these boundaries. `UnsupportedInputs`
    assigns distinct diagnostic hues to AlphaMulti, AO/SSAO, unsupported legacy specular, and vertex
    movement instead of collapsing them into the generic runtime-only gray. `ApplyVertexColor`
    remains parsed and directly debuggable, but MeddleTools only proves the On/Off key and not an
    RGB composition formula; the old generic `base * vertexColor.rgb` tint is therefore isolated
    from Final and reported as `vertexColorComposition` when enabled.
  - Installed `characterglass.shpk` has 38 pixel shaders: 34 sample Index/Normal/Table/TileNormal/
    TileOrb, 24 sample Mask/ReflectionArray/SphereMap, 19 sample Dissolve/Dissolve1, and 8 sample
    DepthWithWater/ViewPosition/Sky. 31 shaders discard and 32 write output alpha; those 32 split
    evenly between constant-one output and a dynamic `mad o0.w` formula. Normal samples never
    request a `.w` destination. MeddleTools maps this package to the character node group without a
    glass-specific downstream node, so Normal Blue remains a 45059-compatible preview fallback and
    the extra consumers remain an explicit `glassShaderParameters` boundary until the runtime
    scene/subview permutation and final composition are recovered from stronger evidence.
  - `g_SpecularColorMask` remains parsed/uniform/debug-visible, but MeddleTools has no consumer
    mapping and the field has only a three-channel default. Non-default values are now reported as
    `specularColorMaskComposition`; neither RGB multiplication nor a fabricated fourth-channel
    scalar enters Final.
  - `g_OutlineColor` / `g_OutlineWidth` remain parsed, encoded, and represented by a dormant
    pipeline, but the checked references do not prove a world-space `normal * width` extrusion.
    Non-default inputs report `outlineComposition` and do not automatically submit an outline draw.
  - `g_TextureMipBias` now follows installed DXBC rather than a broad generic sampling rule. Every
    consuming character/legacy/glass permutation adds the material value to the global PBR mip
    bias with the same sign; the direct sampler scope is primary Diffuse/Normal/Mask. The preview
    therefore biases only primary Base/Normal/Mask (including dither Base/Normal), while baked
    ramps, secondary maps, emissive, lightshaft, index/table, and pair atlases keep their own LOD
    policy. Installed character materials lock six non-default resources/nine references with only
    `+1` and `-1`; the other audited packages remain at zero.
  - `g_TileMipBiasOffset` now follows the independently verified tile-array LOD path. The renderer
    derives the minimum TileMatrix axis length, applies the game `1/128` threshold, adds the material
    offset with the same sign, and scales explicit pair-atlas gradients by `exp2(bias)`. Native WGPU
    coverage locks installed `-1/+1` behavior for both Tile Normal and Tile ORB. ColorTable tile
    ramps now preserve adjacent A/B TileIndex, TileAlpha, and TileMatrix texels per source index
    pixel. WGSL selects each layer, transforms each UV/normal, applies each matrix-derived LOD, and
    samples Tile Normal/ORB independently before mixing the two results by the installed
    `1-index.G` A-to-B interpolation factor. Modern `character.shpk` permutations additionally shape that
    weight with the even A-row ColorTable Anisotropy (Table texel 4/W) and the primary world-normal/view
    term; the renderer consumes this only when packed generic ramps and the modern package gate
    are both present. Legacy keeps the unshaped `1-index.G` path.
  - The installed DXBC audit propagates A/B row provenance per physical register lane, including
    packed swizzles, and locks ordered A-to-B ORB blends for all 720 character and 276 legacy pairs.

## P2 - Postprocessing And Presentation

- [x] Recalibrate bloom after the HDR and tone-mapping changes.
  - Define the bright-pass threshold in scene-linear units.
  - Avoid embedding unrelated highlight extraction constants in the material shader.
  - Test emissive-only, dielectric, and metallic fixtures independently.

- [x] Define a stable preview-lighting contract.
  - Specify key/fill/environment orientation, intensity, color temperature, and camera relationship.
  - Keep preview lighting separate from material semantics so lighting changes cannot compensate for
    incorrect normals or texture decoding.
  - Add fixed-camera reference renders for representative matte, glossy, metallic, emissive, and
    transparent materials.
  - The contract is centralized as named `PREVIEW_*` constants in Rust/WGSL: the direct key is
    camera-relative (`-0.45 right + 0.65 up + 0.65 view`) with warm RGB `(1.0, 0.95, 0.88)`;
    ambient/environment ground, sky, horizon, key/fill lobes, rim, direct diffuse/specular scales,
    exposure, and scene-linear bloom threshold are presentation parameters rather than material
    inputs. Unit tests lock the camera relationship and named shader constants. Synthetic
    fixed-camera WGPU fixtures cover matte/dielectric, glossy, metallic, emissive, transparent,
    two-sided, and roughness-sweep responses.

## Verification Infrastructure

- [x] Add GPU/offscreen pixel tests in addition to source-string and field-passthrough tests.
  - Two-sided normal orientation.
  - ColorTable diffuse/specular/emissive color-space decoding.
  - Metallic roughness and Fresnel response.
  - Normal-angle diffuse/specular response and baked ColorTable anisotropy direction.
  - HDR preservation and tone-mapped output.
  - Environment, sheen, sphere, BG detail, and alpha-shaping unsupported boundaries; water, tile,
    transparency, and lightshaft contributions.
  - Native WGPU fixtures now assert final pixels for two-sided orientation, baked
    diffuse/specular/emissive color spaces, metallic/Fresnel/roughness, HDR/tone mapping/bloom,
    water, tile arrays, detail-array debug selection, sheen/sphere diagnostics, transparency
    sorting/alpha, and lightshaft. Environment, Sheen/Sphere, BG detail, and alpha shaping have no
    verified final composition, so their fixtures assert explicit unsupported diagnostics and
    unchanged Final output rather than inventing contributions.

- [x] Add a small checked-in material fixture matrix.
  - Include representative legacy and Dawntrail ColorTables.
  - Include at least one single-value, multi-value, dyed, metallic, emissive, transparent, and
    double-sided material.
  - Store expected numeric probes or tightly controlled snapshots rather than relying only on
    manual inspection.
  - `tests/fixtures/material_fixture_matrix.json` now covers Legacy/Dawntrail, single/multi,
    dyed, metallic, emissive, transparent, and double-sided cases. The integration test drives the
    production ColorTable baker and prepared-material policy, checking exact sRGB/linear packed
    bytes, HDR float ramps, render pass, alpha source, shader family, and backface behavior.

- [x] Ensure renderer tests run with the `renderer` feature in the default CI verification path.
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
