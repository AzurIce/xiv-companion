// Model shader module, part of the `package` WESL package
// (compiled to OUT_DIR/model.wgsl by the build script).

struct Camera {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
    view_dir: vec4<f32>,
    camera_position: vec4<f32>,
    options: vec4<f32>, // x: normal mapping, y: normal y sign, z: uv scroll time, w: debug mode
    dynamic_emissive_color: vec4<f32>, // runtime material dynamic emissive multiplier
};

struct Material {
    diffuse_color: vec4<f32>,
    emissive_color: vec4<f32>, // a: has emissive texture
    specular_color: vec4<f32>,
    params: vec4<f32>, // x: has base, y: metalness, z: has normal, w: has mask
    properties: vec4<f32>, // x: has ColorTable material properties texture, y: has specular texture, z: apply vertex color, w: legacy Compatibility specular mode
    render: vec4<f32>, // x: render mode, y: opacity, z: alpha mode 0=opaque 1=mask 2=blend 3=glass, w: alpha threshold
    alpha_params: vec4<f32>, // x: aperture, y: offset, z: shadow alpha threshold, w: transparency
    alpha_policy_params: vec4<f32>, // x: source 0=opaque 1=base alpha 2=normal blue 3=material transparency 4=normal alpha, y: lighting, z: dither depth, w: prepared pass
    alpha_composition_params: vec4<f32>, // x: 0xAD94E254, vertex alpha remap toward one
    water_deep_color: vec4<f32>,
    water_refraction_color: vec4<f32>,
    water_whitecap_color: vec4<f32>,
    extra_properties: vec4<f32>, // x: tile, y: sheen, z: sphere, w: tile matrix
    shader_params: vec4<f32>, // x: normal, y: multi normal, z: detail normal, w: multi detail normal
    tile_params: vec4<f32>, // x: tile index, y: tile alpha, zw: tile repeat uv
    toon_sheen_params: vec4<f32>, // x: toon index, y: toon light scale, z: sheen rate, w: sheen tint rate
    toon_params: vec4<f32>, // x: light spec aperture, y: reflection scale, z: spec index, w: prepared toon
    sheen_sphere_params: vec4<f32>, // x: sheen aperture, y: sphere map index, z: baked specular alpha is anisotropy
    detail_params: vec4<f32>, // x: detail id, y: multi detail id, z: GetMultiValues detail blend
    array_params: vec4<f32>, // x: tile layers, y: detail layers, z: has tile pair, w: has detail pair
    tile_lod_params: vec4<f32>, // x: tile bias, y: packed generic ramps, z: modern blend shaping, w: packed tile ramps
    colorset_params: vec4<f32>, // x: compose full-resolution base diffuse with the colorset ramp in the shader
    detail_color: vec4<f32>,
    multi_detail_color: vec4<f32>,
    shader_diffuse_color: vec4<f32>,
    shader_multi_diffuse_color: vec4<f32>,
    shader_emissive_color: vec4<f32>,
    shader_multi_emissive_color: vec4<f32>,
    outline_params: vec4<f32>, // rgb: outline color, a: outline width
    specular_color_mask: vec4<f32>,
    surface_params: vec4<f32>, // x: ssao mask, y: texture mip bias, z: shadow pos offset, w: character mip bias
    detail_color_uv_scale: vec4<f32>, // xy: detail color repeat, zw: multi detail color repeat
    detail_normal_uv_scale: vec4<f32>, // xy: detail normal repeat, zw: multi detail normal repeat
    uv_scroll: vec4<f32>, // xy: uv0 scroll multiplier, zw: uv1 scroll multiplier
    lightshaft_color: vec4<f32>,
    lightshaft_tex_anim: vec4<f32>,
    lightshaft_tex_u: vec4<f32>,
    lightshaft_tex_v: vec4<f32>,
    lightshaft_ray: vec4<f32>,
    uv_sources0: vec4<f32>, // x: base, y: normal, z: mask, w: material map
    uv_sources1: vec4<f32>, // x: multi, y: specular, z: emissive, w: material properties
    uv_sources2: vec4<f32>, // x: tile, y: sheen, z: sphere, w: tile matrix
    uv_sources3: vec4<f32>, // x: ColorTable index, y: other
    uv_scroll_masks0: vec4<f32>, // base, normal, mask, material map
    uv_scroll_masks1: vec4<f32>, // multi, specular, emissive, material properties
    uv_scroll_masks2: vec4<f32>, // tile, sheen, sphere, tile matrix
    uv_scroll_masks3: vec4<f32>, // ColorTable index, other
    feature_params: vec4<f32>, // x: use flow0 tangent, y: water, z: secondary bindings, w: bg specular channels
    family_params: vec4<f32>, // x: exact characterlegacy, y: exact character/legacy Final
    secondary_map_params: vec4<f32>, // xyz: secondary color/normal/specular present, w: GetMultiValues blend
    draw_role_params: vec4<f32>, // x: lightshaft, y: transparent crest fallback, z: base material fallback
    debug_color: vec4<f32>, // xyz: mesh/draw-role debug color
    unsupported_color: vec4<f32>, // rgb: unsupported-input diagnostic color
    // 角色颜色通道（uniform 尾部）：RGB 为 squared RGB 线性色，W 为激活/强度。
    // 无 character_colors 的材质全零，全部分支不激活（武器/装备渲染不受影响）。
    character_skin: vec4<f32>, // rgb: SkinColor, a: 激活
    character_lip: vec4<f32>, // rgb: LipColor, a: 唇釉混合权重
    character_main: vec4<f32>, // rgb: MainColor 发色, a: 激活
    character_mesh: vec4<f32>, // rgb: MeshColor 挑染, a: 混合权重
    character_left_iris: vec4<f32>, // rgb: 左眼色, a: 激活
    character_right_iris: vec4<f32>, // rgb: 右眼色, a: 激活（分侧未实现，备用）
    character_option: vec4<f32>, // rgb: OptionColor 特征色, a: 激活
    character_decal: vec4<f32>, // rgb: DecalColor 面妆色, a: 不透明度
    character_decal_uv: vec4<f32>, // x: UV multiplier, y: UV offset, z: reversed, w: 贴图存在
    character_params: vec4<f32>, // x: hair family 用 mask R 通道作明暗细节, y: obj/face 脸部毛发（alpha 整形）, zw: 保留
    // bg 域逐贴图 UV 缩放（仅 Bg 家族非 1）：xy = map0，zw = map1。
    uv_scale_a: vec4<f32>, // g_ColorUVScale
    uv_scale_b: vec4<f32>, // g_NormalUVScale
    uv_scale_c: vec4<f32>, // g_SpecularUVScale
    // iris.shpk 专用（仅 Iris 家族材质非零）：按 MeddleTools iris.shpk 节点组。
    iris_white_eye: vec4<f32>,  // rgb: g_WhiteEyeColor（眼白乘色）
    iris_ring_color: vec4<f32>, // rgb: g_IrisRingColor, a: 启用
    iris_ring_a: vec4<f32>,     // x: g_IrisRingEmissiveIntensity, yz: g_IrisRingUvRadius.xy, w: 左眼 limbal 强度
    iris_ring_b: vec4<f32>,     // xy: g_IrisRingUvFadeWidth.xy, z: 右眼 limbal 强度
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) bitangent: vec4<f32>,
    @location(4) color: vec4<f32>,
    @location(5) uv1: vec2<f32>,
    @location(6) uv2: vec2<f32>,
    @location(7) uv3: vec2<f32>,
    @location(8) color1: vec4<f32>,
    @location(9) normal1: vec3<f32>,
    @location(10) bitangent1: vec4<f32>,
    @location(11) flow0: vec4<f32>,
    @location(12) flow1: vec4<f32>,
    @location(13) joints: vec2<u32>, // u8×8 打包（低字节在前）：槽位 0..3 在 x，4..7 在 y
    @location(14) weights0: vec4<f32>,
    @location(15) weights1: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv0: vec2<f32>,
    @location(2) bitangent: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) uv1: vec2<f32>,
    @location(5) uv2: vec2<f32>,
    @location(6) uv3: vec2<f32>,
    @location(7) flow0: vec4<f32>,
    @location(8) flow1: vec4<f32>,
    @location(9) normal1: vec3<f32>,
    @location(10) bitangent1: vec4<f32>,
    @location(11) color1: vec4<f32>,
    @location(12) world_position: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<uniform> material: Material;

@group(1) @binding(1)
var base_color_texture: texture_2d<f32>;

@group(1) @binding(2)
var base_color_sampler: sampler;

@group(1) @binding(3)
var normal_texture: texture_2d<f32>;

@group(1) @binding(4)
var mask_texture: texture_2d<f32>;

@group(1) @binding(5)
var emissive_texture: texture_2d<f32>;

@group(1) @binding(6)
var material_properties_texture: texture_2d<f32>;

@group(1) @binding(7)
var specular_texture: texture_2d<f32>;

@group(1) @binding(8)
var normal_sampler: sampler;

@group(1) @binding(9)
var tile_properties_texture: texture_2d<f32>;

@group(1) @binding(10)
var sheen_properties_texture: texture_2d<f32>;

@group(1) @binding(11)
var sphere_properties_texture: texture_2d<f32>;

@group(1) @binding(12)
var tile_matrix_texture: texture_2d<f32>;

@group(1) @binding(13)
var tile_matrix_sampler: sampler;

@group(1) @binding(14)
var color_table_index_texture: texture_2d<f32>;

@group(1) @binding(15)
var colorset_diffuse_texture: texture_2d<f32>;

@group(1) @binding(16)
var multi_map_texture: texture_2d<f32>;

@group(1) @binding(17)
var tile_array_pair_texture: texture_2d<f32>;

@group(1) @binding(18)
var detail_array_pair_texture: texture_2d<f32>;

@group(1) @binding(19)
var mask_sampler: sampler;

@group(1) @binding(20)
var emissive_sampler: sampler;

@group(1) @binding(21)
var material_properties_sampler: sampler;

@group(1) @binding(22)
var specular_sampler: sampler;

@group(1) @binding(23)
var tile_sampler: sampler;

@group(1) @binding(24)
var sheen_sampler: sampler;

@group(1) @binding(25)
var sphere_sampler: sampler;

@group(1) @binding(26)
var index_sampler: sampler;

@group(1) @binding(28)
var multi_map_sampler: sampler;

@group(1) @binding(29)
var tile_array_sampler: sampler;

@group(1) @binding(30)
var detail_array_sampler: sampler;

@group(1) @binding(31)
var decal_texture: texture_2d<f32>;

@group(1) @binding(32)
var decal_sampler: sampler;

struct FragmentOutput {
    @location(0) color: vec4<f32>,
};
