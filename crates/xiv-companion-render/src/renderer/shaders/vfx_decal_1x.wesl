import package::vfx_decal_common::{DecalVertexOutput, decal_blend, decal_multiply, decal_screen, vs_decal};

@group(2) @binding(1) var scene_depth: texture_depth_2d;

@fragment
fn fs_decal_multiply(input: DecalVertexOutput) -> @location(0) vec4<f32> {
    return decal_multiply(input, textureLoad(scene_depth, vec2<i32>(input.clip_position.xy), 0));
}

@fragment
fn fs_decal_screen(input: DecalVertexOutput) -> @location(0) vec4<f32> {
    return decal_screen(input, textureLoad(scene_depth, vec2<i32>(input.clip_position.xy), 0));
}

@fragment
fn fs_decal_blend(input: DecalVertexOutput) -> @location(0) vec4<f32> {
    return decal_blend(input, textureLoad(scene_depth, vec2<i32>(input.clip_position.xy), 0));
}
