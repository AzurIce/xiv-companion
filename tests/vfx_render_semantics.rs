#![cfg(feature = "render-test-support")]

use xiv_companion::renderer::test_support::{
    WeaponModelSnapshotOptions, render_weapon_model_snapshot_with_options,
};
use xiv_companion::renderer::{
    ModelRenderOptions, VfxTextureCubeMipInput, VfxTextureInput, VfxTextureMipInput,
    VfxTextureMipRgba16fInput,
};
use xiv_companion::{
    ModelBounds, ModelData, ModelMesh, ModelVertex, VfxDecal, VfxDrawModel, VfxDrawVertex,
    VfxMeshInstance, VfxQuad, VfxTextureCubeFormat,
};

fn quad(color: [f32; 4], blend_add: bool) -> VfxQuad {
    VfxQuad {
        particle_type: Some(xiv_companion_data::avfx::ParticleType::Quad),
        particle_index: 0,
        soft_particle: false,
        soft_particle_fade_range: 1.0,
        depth_offset_type: 0,
        depth_offset: 0.0,
        powder_single: false,
        windmill_uv_type: 0,
        disc: None,
        polygon: None,
        laser: None,
        line: None,
        polyline: None,
        decal: None,
        position: [0.0; 3],
        size: [1.0; 2],
        orientation: [0.0, 0.0, 0.0, 1.0],
        parent_basis: xiv_companion::VFX_IDENTITY_BASIS,
        movement_direction: [0.0, 0.0, 1.0],
        facing_parent_basis: xiv_companion::VFX_IDENTITY_BASIS,
        rotation_direction_base: xiv_companion_data::avfx::rotation_direction_base::NONE,
        color,
        draw_layer: 0,
        soft_key_offset: 0.0,
        draw_priority: 0,
        draw_order: None,
        pivot: [0.0; 2],
        texture_indexes: [0, -1, -1, -1],
        texture_uv_sets: [0; 4],
        combine_mode_tc1: [1, 1],
        combine_modes: [[0; 2]; 3],
        color_to_alpha: [false; 4],
        uv_origins: [[0.0; 2]; 4],
        uv_scales: [[1.0; 2]; 4],
        uv_by_pixel_position: [false; 4],
        uv_rotations: [0.0; 4],
        texture_borders: [[0; 2]; 4],
        texture_filters: [1; 4],
        texture1_is_shape_mask: false,
        texture1_enabled: true,
        texture1_use_screen_copy: false,
        draw_mode: if blend_add { 2 } else { 0 },
        depth_test: false,
        depth_write: false,
        cull_mode: 0,
        texture_distortion_index: -1,
        distortion_power: 0.0,
        distortion_targets: 0,
        uvd_origin: [0.0; 2],
        uvd_scale: [1.0; 2],
        uvd_rotation: 0.0,
        uvd_by_pixel_position: false,
        distortion_uv_set: 0,
        distortion_borders: [0; 2],
        distortion_filter: 1,
        texture_palette_index: -1,
        palette_offset: 0.0,
        palette_border: 0,
        palette_filter: 1,
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn point_auxiliary_actual_sampling_and_staged_playback_reach_gpu() {
    render_point_auxiliary_playback(AuxiliaryScenario::Point);
}

#[test]
#[ignore = "requires native wgpu"]
fn nested_point_auxiliary_sampling_and_staged_playback_reach_gpu() {
    render_point_auxiliary_playback(AuxiliaryScenario::NestedPoint);
}

#[test]
#[ignore = "requires native wgpu"]
fn point_root_revision_rotation_reaches_actual_playback_gpu() {
    render_point_auxiliary_playback(AuxiliaryScenario::RootRevision);
}

#[test]
#[ignore = "requires native wgpu"]
fn document_host_revision_getters_reach_actual_playback_gpu() {
    render_point_auxiliary_playback(AuxiliaryScenario::Document);
}

#[test]
#[ignore = "requires native wgpu"]
fn dynamic_document_birth_and_current_history_reach_actual_playback_gpu() {
    render_point_auxiliary_playback(AuxiliaryScenario::DynamicDocument);
}

#[test]
#[ignore = "requires native wgpu"]
fn linear_auxiliary_actual_sampling_and_staged_playback_reach_gpu() {
    render_point_auxiliary_playback(AuxiliaryScenario::Linear);
}

#[test]
#[ignore = "requires native wgpu"]
fn linear_global_direction_actual_continuous_sampling_reaches_gpu() {
    render_point_auxiliary_playback(AuxiliaryScenario::LinearGlobal);
}

#[test]
#[ignore = "native GPU snapshot"]
fn linear_initialization_root_revision_reaches_actual_playback_gpu() {
    render_point_auxiliary_playback(AuxiliaryScenario::LinearRootRevision);
}

#[test]
#[ignore = "native GPU snapshot"]
fn linear_construction_missing_goal_never_draws_phantom_root_children() {
    render_point_auxiliary_playback(AuxiliaryScenario::LinearMissingGoal);
}

#[test]
#[ignore = "requires native wgpu"]
fn dynamic_linear_birth_and_current_matrices_reach_actual_playback_gpu() {
    render_dynamic_binder_history_gpu(BinderHistoryCase::default());
}

#[test]
#[ignore = "native GPU, LinearAdjust height projection and PICd birth/current histories at 1x/4x"]
fn linear_adjust_height_projection_reaches_actual_playback_gpu() {
    render_dynamic_binder_history_gpu(BinderHistoryCase {
        horizontal: true,
        ..Default::default()
    });
}

#[test]
#[ignore = "requires native wgpu"]
fn dynamic_point_birth_and_current_matrices_reach_actual_playback_gpu() {
    render_dynamic_binder_history_gpu(BinderHistoryCase {
        point: true,
        ..Default::default()
    });
}

#[test]
#[ignore = "requires native wgpu"]
fn delayed_finite_binder_factory_and_retired_parent_tails_reach_actual_gpu() {
    render_dynamic_binder_history_gpu(BinderHistoryCase {
        lifecycle: true,
        ..Default::default()
    });
    render_dynamic_binder_history_gpu(BinderHistoryCase {
        point: true,
        lifecycle: true,
        ..Default::default()
    });
}

#[test]
#[ignore = "native GPU, animated Linear COF/Pos and PICd histories"]
fn linear_cof_and_position_curves_reach_actual_playback_gpu() {
    render_dynamic_binder_history_gpu(BinderHistoryCase {
        curves: true,
        ..Default::default()
    });
}

#[test]
#[ignore = "native GPU, Point bStG and PICd birth/current histories"]
fn point_start_global_flag_reaches_actual_playback_gpu() {
    render_dynamic_binder_history_gpu(BinderHistoryCase {
        point: true,
        global_point: true,
        ..Default::default()
    });
}

#[derive(Clone, Copy, Default)]
struct BinderHistoryCase {
    point: bool,
    lifecycle: bool,
    horizontal: bool,
    curves: bool,
    global_point: bool,
    scheduled: bool,
}

#[test]
fn scheduled_binder_birth_packets_match_independent_geometry() {
    build_binder_history_packets(BinderHistoryCase {
        curves: true,
        scheduled: true,
        ..Default::default()
    });
    build_binder_history_packets(BinderHistoryCase {
        point: true,
        scheduled: true,
        ..Default::default()
    });
    for case in [
        BinderHistoryCase::default(),
        BinderHistoryCase {
            horizontal: true,
            ..Default::default()
        },
        BinderHistoryCase {
            point: true,
            ..Default::default()
        },
        BinderHistoryCase {
            lifecycle: true,
            ..Default::default()
        },
        BinderHistoryCase {
            point: true,
            lifecycle: true,
            ..Default::default()
        },
        BinderHistoryCase {
            curves: true,
            ..Default::default()
        },
        BinderHistoryCase {
            point: true,
            global_point: true,
            ..Default::default()
        },
    ] {
        build_binder_history_packets(case);
    }
}

#[test]
#[ignore = "native GPU, scheduled Point/Linear birth age and PICd history at 1x/4x"]
fn scheduled_binder_birth_inputs_reach_actual_playback_gpu() {
    render_dynamic_binder_history_gpu(BinderHistoryCase {
        curves: true,
        scheduled: true,
        ..Default::default()
    });
    render_dynamic_binder_history_gpu(BinderHistoryCase {
        point: true,
        scheduled: true,
        ..Default::default()
    });
}

fn build_binder_history_packets(
    case: BinderHistoryCase,
) -> (Vec<VfxQuad>, Vec<VfxQuad>, Vec<VfxQuad>) {
    let BinderHistoryCase {
        point,
        lifecycle,
        horizontal,
        curves,
        global_point,
        scheduled,
    } = case;
    use xiv_companion_data::{
        AvfxFile, VfxBinderMatrix, VfxBinderTargetSnapshot, VfxPlayback, VfxRuntime, avfx::*,
    };
    let constant = |value| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z: value,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let mut packets = Vec::new();
    let mut references = Vec::new();
    let mut controls = Vec::new();
    for coord in 0..=3 {
        let x = -0.75 + coord as f32 * 0.5;
        let property = |id| AvfxBinderProperties {
            bind_point_id: id,
            bind_target_point_type: 3,
            coord_update_frame: -1,
            ..Default::default()
        };
        let mut file = AvfxFile {
            binders: vec![AvfxBinder {
                binder_type: if horizontal {
                    4
                } else if point {
                    0
                } else {
                    1
                },
                bind_point_id: 3,
                start_to_global_direction: global_point,
                life: -1,
                transform_scale: 255,
                following_target_orientation: true,
                vfx_scale_bias: 1.0,
                properties_start: Some(property(3)),
                properties_goal: (!point).then(|| property(4)),
                data: Some(AvfxBinderData {
                    carry_over_factor: (!point).then(|| constant(0.5)),
                    spring_strength: point.then(|| constant(1.0)),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            timelines: vec![AvfxTimeline {
                binder_index: -1,
                items: vec![AvfxTimelineItem {
                    enabled: true,
                    start_time: 0,
                    end_time: -1,
                    binder_index: 0,
                    emitter_index: 0,
                    effector_index: -1,
                    clip_index: -1,
                    platform: 0,
                }],
                ..Default::default()
            }],
            emitters: vec![AvfxEmitter {
                emitter_type: Some(EmitterType::Point),
                effector_index: -1,
                create_count: constant(1.0),
                create_interval: constant(1000.0),
                position: AvfxCurve3Axis {
                    x: Some(constant(0.125)),
                    ..Default::default()
                },
                particle_items: vec![AvfxEmitterItem {
                    enabled: true,
                    target_index: 0,
                    parameter_link: -1,
                    create_probability: 100,
                    create_count: 1,
                    parent_influence_coord: coord,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            particles: vec![AvfxParticle {
                particle_type: Some(ParticleType::Quad),
                collision_type: -1,
                rotation_direction_base: rotation_direction_base::NONE,
                position: AvfxCurve3Axis {
                    x: Some(constant(0.25)),
                    ..Default::default()
                },
                scale: AvfxCurve3Axis {
                    x: Some(constant(0.2)),
                    y: Some(constant(0.2)),
                    z: Some(constant(0.2)),
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let targets = |moved| {
            [3, 4]
                .into_iter()
                .enumerate()
                .map(|(endpoint, id)| {
                    let center = if moved {
                        [x, -0.875, 0.0]
                    } else {
                        [x - 0.75, 0.0, 0.0]
                    };
                    VfxBinderTargetSnapshot {
                        id,
                        matrix: VfxBinderMatrix {
                            basis: if moved {
                                [[0.0, 3.0, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]]
                            } else {
                                [[2.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]]
                            },
                            position: [
                                center[0]
                                    + if point {
                                        0.0
                                    } else if endpoint == 0 {
                                        -0.25
                                    } else {
                                        0.25
                                    },
                                center[1]
                                    + if horizontal && endpoint == 1 {
                                        0.5
                                    } else {
                                        0.0
                                    },
                                0.0,
                            ],
                        },
                    }
                })
                .collect::<Vec<_>>()
        };
        let animated = |start, end| AvfxCurve {
            keys: vec![
                AvfxCurveKey {
                    time: 0,
                    z: start,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                },
                AvfxCurveKey {
                    time: 1,
                    z: end,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                },
            ],
            ..Default::default()
        };
        if curves {
            file.binders[0].data.as_mut().unwrap().carry_over_factor = Some(animated(0.25, 0.75));
            file.binders[0]
                .properties_start
                .as_mut()
                .unwrap()
                .position
                .x = Some(animated(0.0, 0.25));
            file.binders[0].properties_goal.as_mut().unwrap().position.y =
                Some(animated(0.0, 0.25));
        }
        if lifecycle {
            file.binders[0].life = 2;
            file.binders[0]
                .properties_start
                .as_mut()
                .unwrap()
                .generate_delay = 1;
            file.timelines[0].items[0].end_time = 5;
            if !point {
                // Native regular Linear factory ignores PrpS.GenD.
                file.timelines[0].items[0].start_time = 1;
                file.timelines[0].items[0].end_time = 6;
            }
            file.particles[0].life.enabled = true;
            file.particles[0].life.value = 6.0;
        }
        if scheduled {
            file.timelines[0].items[0].start_time = 2;
            if point {
                file.emitters[0].position.x = Some(animated(0.125, 0.25));
            }
        }
        let mut playback = VfxPlayback::new(
            VfxRuntime::new(&file)
                .with_binder_target_snapshot(&targets(false))
                .unwrap(),
        );
        assert_eq!(playback.fallback_reason(), None);
        if lifecycle || scheduled {
            let mut empty = Vec::new();
            playback.sample(&mut empty, &mut Vec::new());
            assert!(empty.is_empty());
            playback
                .advance_with_binder_target_snapshot(
                    if scheduled { 3.0 / 30.0 } else { 2.0 / 30.0 },
                    &targets(false),
                )
                .unwrap();
        }
        playback
            .advance_with_binder_target_snapshot(1.0 / 30.0, &targets(true))
            .unwrap();
        assert_eq!(playback.fallback_reason(), None);
        if lifecycle {
            // Parent retires strictly beyond Life=2 and freezes the moved
            // matrix. Its emitter dies later; the original particle tail is
            // still alive at equality Life=6 despite missing targets.
            playback
                .advance_with_binder_target_snapshot(1.0 / 30.0, &targets(false))
                .unwrap();
            playback
                .advance_with_binder_target_snapshot(4.0 / 30.0, &[])
                .unwrap();
            assert_eq!(playback.fallback_reason(), None);
        }
        let mut quads = Vec::new();
        let mut meshes = Vec::new();
        playback.sample(&mut quads, &mut meshes);
        assert!(meshes.is_empty());
        assert_eq!(quads.len(), 1);
        packets.push(quads[0]);
        // Literal geometry, independent of the Binder/history implementation:
        // 0/1 retain birth emitter origin and use current auxiliary particle X;
        // 2 uses current M*A for both offsets; 3 retains the whole birth M*A.
        let mut reference = quads[0];
        let constant_position = match coord {
            0 | 1 => [x + 0.25, 0.0, 0.0],
            2 => [x, 0.25, 0.0],
            3 => [x, 0.0, 0.0],
            _ => unreachable!(),
        };
        reference.position = if scheduled && point {
            // First creation at age=1 reads emitter Pos.x=.25, not .125.
            match coord {
                0 | 1 => [x + 0.5, 0.0, 0.0],
                2 => [x, 0.625, 0.0],
                3 => [x + 0.25, 0.0, 0.0],
                _ => unreachable!(),
            }
        } else if scheduled && curves {
            // Both birth and current COF/Pos have reached key 1. Birth
            // basis is still initial, while mode 2 uses the moved basis.
            match coord {
                0 | 1 => [x + 0.4375, 0.1875, 0.0],
                2 => [x - 0.0625, 0.3125, 0.0],
                3 => [x + 0.1875, 0.1875, 0.0],
                _ => unreachable!(),
            }
        } else if curves {
            // Birth COF=.25 and Pos=0. Current COF=.75 and the blended
            // Pos=(.0625,.1875,0) rotates to (-.1875,.0625,0).
            match coord {
                0 | 1 => [x + 0.125, 0.0, 0.0],
                2 => [x - 0.0625, 0.3125, 0.0],
                3 => [x - 0.125, 0.0, 0.0],
                _ => unreachable!(),
            }
        } else {
            constant_position
        };
        reference.parent_basis = match coord {
            0 | 1 => [[3.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]],
            2 => [[0.0, 3.0, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]],
            3 => [[2.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]],
            _ => unreachable!(),
        };
        reference.orientation = [0.0, 0.0, 0.0, 1.0];
        reference.size = [0.1; 2];
        for (actual, expected) in quads[0]
            .position
            .into_iter()
            .zip(reference.position)
            .chain(
                quads[0]
                    .parent_basis
                    .into_iter()
                    .flatten()
                    .zip(reference.parent_basis.into_iter().flatten()),
            )
            .chain(quads[0].size.into_iter().zip(reference.size))
        {
            assert!(
                (actual - expected).abs() < 1e-5,
                "PICd {coord}: {actual} != {expected}"
            );
        }
        references.push(reference);
        // Wrongly reconstructing all existing particles as newly born at the
        // current targets loses both the initial origin and initial basis.
        reference.position = if scheduled && point {
            constant_position
        } else if scheduled && curves {
            // Wrong constructor age=0 loses the animated birth origin.
            match coord {
                0 | 1 => [x + 0.125, 0.0, 0.0],
                2 => [x - 0.0625, 0.3125, 0.0],
                3 => [x - 0.125, 0.0, 0.0],
                _ => unreachable!(),
            }
        } else if curves {
            constant_position
        } else if horizontal {
            let mut position = reference.position;
            position[1] += 0.25;
            position
        } else if coord < 2 {
            [x + 0.75, -0.5, 0.0]
        } else {
            [x, 0.25, 0.0]
        };
        if coord == 3 && !horizontal && !curves {
            reference.parent_basis = [[0.0, 3.0, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]];
        }
        controls.push(reference);
    }
    (packets, references, controls)
}

fn render_dynamic_binder_history_gpu(case: BinderHistoryCase) {
    let (packets, references, controls) = build_binder_history_packets(case);
    let BinderHistoryCase {
        point,
        lifecycle,
        horizontal,
        curves,
        global_point,
        scheduled,
    } = case;
    for samples in [1, 4] {
        let draw = |label, quads: &Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!(
                    "vfx-{}-{}-history-{label}-{samples}",
                    if horizontal {
                        "linear-adjust"
                    } else if point {
                        "point"
                    } else {
                        "linear"
                    },
                    if scheduled {
                        "scheduled-birth"
                    } else if global_point {
                        "start-global"
                    } else if curves {
                        "animated-curves"
                    } else if lifecycle {
                        "delayed-finite"
                    } else {
                        "dynamic"
                    }
                ),
                quads.clone(),
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )
        };
        let actual = draw("actual", &packets);
        let reference = draw("reference", &references);
        assert_image(&actual, &reference);
        let wrong = draw(
            if scheduled {
                "zero-birth-age"
            } else if curves {
                "constant-curves"
            } else if horizontal {
                "unflattened-goal"
            } else {
                "rebuilt-birth"
            },
            &controls,
        );
        assert!(
            reference
                .iter()
                .zip(wrong)
                .filter(|(a, b)| a.iter().zip(b).any(|(a, b)| (a - b).abs() > 0.01))
                .count()
                > 10
        );
    }
}

#[test]
#[ignore = "native GPU, Point/Linear RoTp=1/2/3 and PICd histories"]
fn binder_camera_rotation_and_history_reach_actual_playback_gpu() {
    use xiv_companion_data::{
        AvfxFile, VfxBinderCameraSnapshot, VfxBinderMatrix, VfxBinderTargetSnapshot, VfxPlayback,
        VfxRuntime, avfx::*,
    };
    let constant = |value| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z: value,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let mut packets = Vec::new();
    let mut references = Vec::new();
    let mut controls = Vec::new();
    for point in [false, true] {
        for rotation in 1..=3 {
            let y = -1.15 + ((if point { 3 } else { 0 }) + rotation - 1) as f32 * 0.46;
            for coord in 0..=3 {
                let x = -0.75 + coord as f32 * 0.5;
                let property = |id| AvfxBinderProperties {
                    bind_point_id: id,
                    bind_target_point_type: 3,
                    coord_update_frame: -1,
                    ..Default::default()
                };
                let file = AvfxFile {
                    binders: vec![AvfxBinder {
                        binder_type: if point { 0 } else { 1 },
                        rotation_type: rotation,
                        bind_point_id: 3,
                        life: -1,
                        transform_scale: 255,
                        following_target_orientation: true,
                        vfx_scale_bias: 1.0,
                        properties_start: Some(property(3)),
                        properties_goal: (!point).then(|| property(4)),
                        data: Some(AvfxBinderData {
                            carry_over_factor: (!point).then(|| constant(0.5)),
                            spring_strength: point.then(|| constant(1.0)),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    timelines: vec![AvfxTimeline {
                        binder_index: -1,
                        items: vec![AvfxTimelineItem {
                            enabled: true,
                            start_time: 0,
                            end_time: -1,
                            binder_index: 0,
                            emitter_index: 0,
                            effector_index: -1,
                            clip_index: -1,
                            platform: 0,
                        }],
                        ..Default::default()
                    }],
                    emitters: vec![AvfxEmitter {
                        emitter_type: Some(EmitterType::Point),
                        effector_index: -1,
                        create_count: constant(1.0),
                        create_interval: constant(1000.0),
                        position: AvfxCurve3Axis {
                            x: Some(constant(0.125)),
                            ..Default::default()
                        },
                        particle_items: vec![AvfxEmitterItem {
                            enabled: true,
                            target_index: 0,
                            parameter_link: -1,
                            create_probability: 100,
                            create_count: 1,
                            parent_influence_coord: coord,
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                    particles: vec![AvfxParticle {
                        particle_type: Some(ParticleType::Quad),
                        collision_type: -1,
                        rotation_direction_base: rotation_direction_base::NONE,
                        position: AvfxCurve3Axis {
                            x: Some(constant(0.25)),
                            ..Default::default()
                        },
                        scale: AvfxCurve3Axis {
                            x: Some(constant(0.2)),
                            y: Some(constant(0.2)),
                            z: Some(constant(0.2)),
                            ..Default::default()
                        },
                        ..Default::default()
                    }],
                    ..Default::default()
                };
                let targets = |moved| {
                    [3, 4]
                        .into_iter()
                        .enumerate()
                        .map(|(endpoint, id)| {
                            let center = if moved {
                                [x, y - 0.875, 0.0]
                            } else {
                                [x - 0.75, y, 0.0]
                            };
                            VfxBinderTargetSnapshot {
                                id,
                                matrix: VfxBinderMatrix {
                                    basis: if moved {
                                        [[0.0, 3.0, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]]
                                    } else {
                                        [[2.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]]
                                    },
                                    position: [
                                        center[0]
                                            + if point {
                                                0.0
                                            } else if endpoint == 0 {
                                                -0.25
                                            } else {
                                                0.25
                                            },
                                        center[1],
                                        0.0,
                                    ],
                                },
                            }
                        })
                        .collect::<Vec<_>>()
                };
                let camera = |moved| VfxBinderCameraSnapshot {
                    basis: if moved {
                        [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]
                    } else {
                        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
                    },
                    parallel_direction: [0.0, 0.0, 2.0],
                    position: if moved {
                        [x, y - 0.875, 3.0]
                    } else {
                        [x - 0.75, y, 3.0]
                    },
                };
                let mut playback = VfxPlayback::new(
                    VfxRuntime::new(&file)
                        .with_binder_target_snapshot(&targets(false))
                        .unwrap()
                        .with_binder_camera_snapshot(camera(false))
                        .unwrap(),
                );
                assert_eq!(playback.fallback_reason(), None);
                playback
                    .update_to_with_binder_target_and_camera_snapshot(
                        1.0 / 30.0,
                        &targets(true),
                        camera(true),
                    )
                    .unwrap();
                assert_eq!(playback.fallback_reason(), None);
                let mut quads = Vec::new();
                let mut meshes = Vec::new();
                playback.sample(&mut quads, &mut meshes);
                assert!(meshes.is_empty());
                assert_eq!(quads.len(), 1);
                packets.push(quads[0]);
                // Literal geometry, independent of the Binder/history implementation:
                // 0/1 retain birth emitter origin and use current auxiliary particle X;
                // 2 uses current M*A for both offsets; 3 retains the whole birth M*A.
                let mut reference = quads[0];
                reference.position = match coord {
                    0 | 1 => [x + 0.25, y, 0.0],
                    2 => [x, y + 0.25, 0.0],
                    3 => [x, y, 0.0],
                    _ => unreachable!(),
                };
                reference.parent_basis = match coord {
                    0 | 1 => [[3.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]],
                    2 => [[0.0, 3.0, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]],
                    3 => [[2.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]],
                    _ => unreachable!(),
                };
                reference.orientation = [0.0, 0.0, 0.0, 1.0];
                reference.size = [0.1; 2];
                for (actual, expected) in quads[0].position.into_iter().zip(reference.position) {
                    assert!(
                        (actual - expected).abs() < 1e-5,
                        "point={point} rotation={rotation} PICd={coord}: {actual} != {expected}"
                    );
                }
                for (actual, expected) in quads[0]
                    .parent_basis
                    .into_iter()
                    .flatten()
                    .zip(reference.parent_basis.into_iter().flatten())
                {
                    assert!((actual - expected).abs() < 1e-5);
                }
                references.push(reference);
                // Wrongly reconstructing all existing particles as newly born at the
                // current targets loses both the initial origin and initial basis.
                reference.position = if coord < 2 {
                    [x + 0.75, y - 0.5, 0.0]
                } else {
                    [x, y + 0.25, 0.0]
                };
                if coord == 3 {
                    reference.parent_basis = [[0.0, 3.0, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]];
                }
                controls.push(reference);
            }
        }
    }
    for samples in [1, 4] {
        let draw = |label, quads: &Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-binder-camera-rotation-{label}-{samples}"),
                quads.clone(),
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )
        };
        let actual = draw("actual", &packets);
        let reference = draw("reference", &references);
        assert_image(&actual, &reference);
        let wrong = draw("rebuilt-birth", &controls);
        assert!(
            reference
                .iter()
                .zip(wrong)
                .filter(|(a, b)| a.iter().zip(b).any(|(a, b)| (a - b).abs() > 0.01))
                .count()
                > 10
        );
    }
}

// Camera Binder fixture and analytic pixel reference. This exercises the actual
// preview adapter on GPU; the CPU companion first verifies the reference and
// birth/current input history without creating any graphics device.
fn preview_camera_binder_render_fixture(
    ags_enabled: bool,
) -> (
    ModelData,
    xiv_companion_data::WeaponVfxAttachments,
    Vec<xiv_companion::renderer::test_support::WeaponVfxCameraInput>,
    Vec<VfxQuad>,
) {
    use xiv_companion::renderer::test_support::WeaponVfxCameraInput;
    use xiv_companion_data::{WeaponVfxAttachment, WeaponVfxAttachments, WeaponVfxData, avfx::*};
    let constant = |z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let initial = WeaponVfxCameraInput {
        time: 0.0,
        yaw: -0.25,
        pitch: -0.15,
        zoom: 3.0,
        pan: [0.05, -0.1],
        roll: -0.4,
    };
    let moved = WeaponVfxCameraInput {
        time: 0.125,
        yaw: 0.55,
        pitch: 0.35,
        pan: [-0.05, 0.1],
        roll: 0.65,
        ..initial
    };
    let mut model = vfx_semantics_model();
    let template = model.meshes[0].clone();
    model.meshes.clear();
    let mut mounts = WeaponVfxAttachments::default();
    let mut references = Vec::new();
    for timeline in [false, true] {
        for coord in [2, 3] {
            let x = if coord == 2 { -0.4 } else { 0.4 };
            let y = if timeline { 0.3 } else { -0.3 };
            let path = format!("preview-camera-binder-{timeline}-{coord}.mdl");
            let file = AvfxFile {
                global: AvfxGlobalParameters {
                    ags_enabled,
                    // Scheduler Camera uses the MDL owner Euler rotation,
                    // independent of these authored compiled root angles.
                    revised_rotation: if ags_enabled && timeline {
                        [0.25, -0.5, 0.75]
                    } else {
                        [0.0; 3]
                    },
                    ..Default::default()
                },
                schedulers: vec![AvfxScheduler {
                    items: vec![AvfxSchedulerItem {
                        enabled: true,
                        start_time: 0,
                        timeline_index: 0,
                    }],
                    ..Default::default()
                }],
                timelines: vec![AvfxTimeline {
                    binder_index: if timeline { 0 } else { -1 },
                    items: vec![AvfxTimelineItem {
                        enabled: true,
                        start_time: 0,
                        end_time: -1,
                        binder_index: if timeline { -1 } else { 0 },
                        emitter_index: 0,
                        effector_index: -1,
                        clip_index: -1,
                        platform: 0,
                    }],
                    ..Default::default()
                }],
                binders: vec![AvfxBinder {
                    binder_type: 3,
                    rotation_type: 1,
                    life: -1,
                    adjust_to_screen_enabled: true,
                    properties_start: Some(AvfxBinderProperties {
                        generate_delay: 999,
                        coord_update_frame: -1,
                        ..Default::default()
                    }),
                    data: Some(AvfxBinderData {
                        distance: Some(constant(-2.8)),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                emitters: vec![AvfxEmitter {
                    emitter_type: Some(EmitterType::Point),
                    effector_index: -1,
                    create_count: constant(1.0),
                    create_interval: constant(1000.0),
                    position: AvfxCurve3Axis {
                        x: Some(constant(x)),
                        y: Some(constant(y)),
                        ..Default::default()
                    },
                    particle_items: vec![AvfxEmitterItem {
                        enabled: true,
                        target_index: 0,
                        parameter_link: -1,
                        create_count: 1,
                        create_probability: 100,
                        parent_influence_coord: coord,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                particles: vec![AvfxParticle {
                    particle_type: Some(ParticleType::Quad),
                    collision_type: -1,
                    rotation_direction_base: rotation_direction_base::NONE,
                    scale: AvfxCurve3Axis {
                        x: Some(constant(0.16)),
                        y: Some(constant(0.16)),
                        z: Some(constant(0.16)),
                        ..Default::default()
                    },
                    ..Default::default()
                }],
                ..Default::default()
            };
            let c = if coord == 2 { moved } else { initial };
            let (basis, eye) = analytic_preview_camera_binder_orbit(c);
            let mut q = quad([1.0; 4], false);
            q.texture_indexes = [-1; 4];
            q.texture1_enabled = false;
            q.combine_mode_tc1 = [0, 3];
            q.soft_particle_fade_range = 0.0;
            q.size = [0.08; 2];
            q.position = std::array::from_fn(|axis| {
                eye[axis] - 2.8 * basis[2][axis] + x * basis[0][axis] + y * basis[1][axis]
            });
            q.parent_basis = basis;
            references.push(q);
            mounts.attachments.push(WeaponVfxAttachment {
                model_path: path.clone(),
                data: WeaponVfxData {
                    file,
                    ..Default::default()
                },
            });
            model.meshes.push(ModelMesh {
                path,
                ..template.clone()
            });
        }
    }
    (
        model,
        mounts,
        vec![
            initial,
            WeaponVfxCameraInput {
                time: moved.time,
                ..initial
            },
            moved,
        ],
        references,
    )
}

fn analytic_preview_camera_binder_orbit(
    camera: xiv_companion::renderer::test_support::WeaponVfxCameraInput,
) -> ([[f32; 3]; 3], [f32; 3]) {
    let (sy, cy) = (camera.yaw.sin(), camera.yaw.cos());
    let (sp, cp) = (camera.pitch.sin(), camera.pitch.cos());
    let (sr, cr) = (camera.roll.sin(), camera.roll.cos());
    let right = [cy, 0.0, -sy];
    let up = [-sy * sp, cp, -cy * sp];
    let z = [sy * cp, sp, cy * cp];
    let r: [f32; 3] = std::array::from_fn(|axis| right[axis] * cr + up[axis] * sr);
    let u: [f32; 3] = std::array::from_fn(|axis| up[axis] * cr - right[axis] * sr);
    let eye = std::array::from_fn(|axis| {
        camera.zoom * z[axis] + camera.pan[0] * r[axis] + camera.pan[1] * u[axis]
    });
    ([r, u, z], eye)
}

#[test]
fn preview_camera_binder_gpu_reference_matches_complete_cpu_history() {
    use xiv_companion_data::{
        VfxBinderCameraSnapshot, VfxBinderMatrix, VfxCameraViewSnapshot, VfxPlayback,
    };
    for ags_enabled in [false, true] {
        let (_, mounts, inputs, references) = preview_camera_binder_render_fixture(ags_enabled);
        let view = |input| {
            let (basis, position) = analytic_preview_camera_binder_orbit(input);
            VfxCameraViewSnapshot {
                inverse_view: VfxBinderMatrix { basis, position },
                camera: VfxBinderCameraSnapshot {
                    basis,
                    position,
                    parallel_direction: [basis[2][0], 0.0, basis[2][2]],
                },
                screen_height: 128,
            }
        };
        for (mount, expected) in mounts.attachments.iter().zip(references) {
            let mut playback = VfxPlayback::new(
                mount
                    .data
                    .runtime()
                    .with_binder_object_snapshot(
                        xiv_companion_data::VfxBinderObjectSnapshot::IDENTITY,
                    )
                    .unwrap()
                    .with_preview_camera_host_rotation([0.0; 3])
                    .unwrap()
                    .with_preview_camera_view(view(inputs[0]))
                    .unwrap(),
            );
            assert_eq!(playback.fallback_reason(), None);
            for input in &inputs {
                playback
                    .update_preview_camera_view_to(input.time, None, view(*input))
                    .unwrap();
            }
            let (mut actual, mut meshes) = (Vec::new(), Vec::new());
            playback.sample(&mut actual, &mut meshes);
            assert_eq!(actual.len(), 1);
            for (a, b) in actual[0]
                .position
                .into_iter()
                .chain(actual[0].parent_basis.into_iter().flatten())
                .chain(actual[0].size)
                .zip(
                    expected
                        .position
                        .into_iter()
                        .chain(expected.parent_basis.into_iter().flatten())
                        .chain(expected.size),
                )
            {
                assert!((a - b).abs() < 3e-6, "{}: {a} != {b}", mount.model_path);
            }
        }
    }
}

#[test]
#[ignore = "native GPU; Camera Binder inverse view, Scheduler/Item and paused birth/current history"]
fn preview_camera_binder_complete_view_reaches_actual_attachment_gpu() {
    for ags_enabled in [false, true] {
        let (model, mounts, inputs, references) = preview_camera_binder_render_fixture(ags_enabled);
        let moved = *inputs.last().unwrap();
        for samples in [1, 4] {
            let options = |label: &str| {
                WeaponModelSnapshotOptions::new(format!(
                    "vfx-camera-binder-preview-ags{ags_enabled}-{label}-{samples}"
                ))
                .with_viewport(128, 128)
                .with_camera(moved.yaw, moved.pitch, moved.zoom, moved.pan)
                .with_render_options(ModelRenderOptions {
                    msaa_samples: samples,
                    camera_roll: moved.roll,
                    ..Default::default()
                })
                .with_hdr_scene_capture()
            };
            let mut mounted = options("actual").with_weapon_vfx(mounts.clone(), moved.time);
            mounted.weapon_vfx_camera_inputs = inputs.clone();
            mounted.require_staged_weapon_vfx = true;
            let actual = render_weapon_model_snapshot_with_options(mounted, &model).unwrap();
            assert_eq!(actual.weapon_vfx_quads.len(), references.len());
            for (a, b) in actual.weapon_vfx_quads.iter().zip(&references) {
                for (a, b) in a
                    .position
                    .into_iter()
                    .chain(a.parent_basis.into_iter().flatten())
                    .chain(a.size)
                    .zip(
                        b.position
                            .into_iter()
                            .chain(b.parent_basis.into_iter().flatten())
                            .chain(b.size),
                    )
                {
                    assert!((a - b).abs() < 3e-6, "{a} != {b}");
                }
            }
            let expected = render_weapon_model_snapshot_with_options(
                options("reference").with_vfx_quads(references.clone()),
                &model,
            )
            .unwrap();
            let expected_pixels = expected.hdr_scene_rgba.unwrap();
            assert_image(&actual.hdr_scene_rgba.unwrap(), &expected_pixels);
            // The reference must produce a visible difference when Initial inputs
            // are incorrectly rebuilt with the current view, preventing blank-pass success.
            let mut rebuilt = options("wrong-birth").with_weapon_vfx(mounts.clone(), moved.time);
            rebuilt.weapon_vfx_camera_inputs = vec![
                xiv_companion::renderer::test_support::WeaponVfxCameraInput { time: 0.0, ..moved },
            ];
            rebuilt.require_staged_weapon_vfx = true;
            let wrong = render_weapon_model_snapshot_with_options(rebuilt, &model)
                .unwrap()
                .hdr_scene_rgba
                .unwrap();
            assert!(
                expected_pixels
                    .iter()
                    .zip(wrong)
                    .filter(|(a, b)| a.iter().zip(b).any(|(a, b)| (a - b).abs() > 0.01))
                    .count()
                    > 3
            );
        }
    }
}

#[test]
#[ignore = "renders mounted Point/Linear cameras, paused input and birth history through the preview adapter"]
fn preview_camera_inputs_reach_actual_attachment_gpu() {
    use xiv_companion::renderer::test_support::WeaponVfxCameraInput;
    use xiv_companion_data::*;
    #[derive(Clone, Copy)]
    struct ReferenceVector {
        x: f32,
        y: f32,
        z: f32,
    }
    impl ReferenceVector {
        const Y: Self = Self {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        };
        fn new(x: f32, y: f32, z: f32) -> Self {
            Self { x, y, z }
        }
        fn from_array([x, y, z]: [f32; 3]) -> Self {
            Self { x, y, z }
        }
        fn to_array(self) -> [f32; 3] {
            [self.x, self.y, self.z]
        }
        fn normalize(self) -> Self {
            self * (1.0 / (self.x * self.x + self.y * self.y + self.z * self.z).sqrt())
        }
        fn cross(self, b: Self) -> Self {
            Self::new(
                self.y * b.z - self.z * b.y,
                self.z * b.x - self.x * b.z,
                self.x * b.y - self.y * b.x,
            )
        }
    }
    impl std::ops::Add for ReferenceVector {
        type Output = Self;
        fn add(self, b: Self) -> Self {
            Self::new(self.x + b.x, self.y + b.y, self.z + b.z)
        }
    }
    impl std::ops::Sub for ReferenceVector {
        type Output = Self;
        fn sub(self, b: Self) -> Self {
            Self::new(self.x - b.x, self.y - b.y, self.z - b.z)
        }
    }
    impl std::ops::Mul<f32> for ReferenceVector {
        type Output = Self;
        fn mul(self, k: f32) -> Self {
            Self::new(self.x * k, self.y * k, self.z * k)
        }
    }
    let constant = |z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let initial = WeaponVfxCameraInput {
        time: 0.0,
        yaw: -0.4,
        pitch: -0.2,
        zoom: 3.0,
        pan: [0.1, -0.05],
        roll: -0.5,
    };
    let moved = WeaponVfxCameraInput {
        time: 0.125,
        yaw: 0.6,
        pitch: 0.45,
        zoom: 3.0,
        pan: [0.05, -0.1],
        roll: 0.8,
    };
    // Analytic camera axes/position; no production view/inverse helper.
    let orbit = |c: WeaponVfxCameraInput| {
        let z = ReferenceVector::new(
            c.yaw.sin() * c.pitch.cos(),
            c.pitch.sin(),
            c.yaw.cos() * c.pitch.cos(),
        );
        let r = ReferenceVector::new(c.yaw.cos(), 0.0, -c.yaw.sin());
        let u = ReferenceVector::new(
            -c.yaw.sin() * c.pitch.sin(),
            c.pitch.cos(),
            -c.yaw.cos() * c.pitch.sin(),
        );
        let right = r * c.roll.cos() + u * c.roll.sin();
        let up = u * c.roll.cos() - r * c.roll.sin();
        (
            right,
            up,
            z,
            (right * c.pan[0] + up * c.pan[1]) + z * c.zoom,
        )
    };
    let facing =
        |rotation, position: ReferenceVector, c: WeaponVfxCameraInput, full_parallel: bool| {
            let (r, u, z, eye) = orbit(c);
            if rotation == 1 {
                return [r.to_array(), u.to_array(), z.to_array()];
            }
            let z = if rotation == 2 {
                if full_parallel {
                    z
                } else {
                    ReferenceVector::new(z.x, 0.0, z.z)
                }
            } else {
                eye - position
            };
            let z = z.normalize();
            let x = ReferenceVector::Y.cross(z).normalize();
            let y = z.cross(x);
            [x.to_array(), y.to_array(), z.to_array()]
        };
    let transform = |basis: [[f32; 3]; 3], x: f32| ReferenceVector::from_array(basis[0]) * x;
    let mut model = vfx_semantics_model();
    let template = model.meshes[0].clone();
    model.meshes.clear();
    let mut mounts = WeaponVfxAttachments::default();
    let mut references = Vec::new();
    let mut pose_updates = Vec::new();
    let mut wrong_horizontal = Vec::new();
    for point in [false, true] {
        for rotation in 1..=3 {
            for coord in 0..=3 {
                let y = -1.0 + ((if point { 3 } else { 0 }) + rotation - 1) as f32 * 0.4;
                let t = ReferenceVector::new(-0.9 + coord as f32 * 0.5, y, 0.0);
                let path = format!(
                    "camera-{}-{rotation}-{coord}.mdl",
                    if point { "point" } else { "linear" }
                );
                let property = |id| AvfxBinderProperties {
                    bind_point_id: id,
                    bind_target_point_type: 3,
                    coord_update_frame: -1,
                    ..Default::default()
                };
                let file = AvfxFile {
                    binders: vec![AvfxBinder {
                        binder_type: if point { 0 } else { 1 },
                        rotation_type: rotation,
                        bind_point_id: 3,
                        life: -1,
                        transform_scale: 255,
                        following_target_orientation: true,
                        vfx_scale_bias: 1.0,
                        properties_start: Some(property(3)),
                        properties_goal: (!point).then(|| property(4)),
                        data: Some(AvfxBinderData {
                            spring_strength: point.then(|| constant(1.0)),
                            carry_over_factor: (!point).then(|| constant(0.5)),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    timelines: vec![AvfxTimeline {
                        binder_index: -1,
                        items: vec![AvfxTimelineItem {
                            enabled: true,
                            start_time: 0,
                            end_time: -1,
                            binder_index: 0,
                            emitter_index: 0,
                            effector_index: -1,
                            clip_index: -1,
                            platform: 0,
                        }],
                        ..Default::default()
                    }],
                    emitters: vec![AvfxEmitter {
                        emitter_type: Some(EmitterType::Point),
                        effector_index: -1,
                        create_count: constant(1.0),
                        create_interval: constant(1000.0),
                        position: AvfxCurve3Axis {
                            x: Some(constant(0.125)),
                            ..Default::default()
                        },
                        particle_items: vec![AvfxEmitterItem {
                            enabled: true,
                            target_index: 0,
                            parameter_link: -1,
                            create_count: 1,
                            create_probability: 100,
                            parent_influence_coord: coord,
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                    particles: vec![AvfxParticle {
                        particle_type: Some(ParticleType::Quad),
                        collision_type: -1,
                        rotation_direction_base: rotation_direction_base::NONE,
                        position: AvfxCurve3Axis {
                            x: Some(constant(0.25)),
                            z: Some(constant(0.25)),
                            ..Default::default()
                        },
                        scale: AvfxCurve3Axis {
                            x: Some(constant(0.16)),
                            y: Some(constant(0.16)),
                            z: Some(constant(0.16)),
                            ..Default::default()
                        },
                        ..Default::default()
                    }],
                    ..Default::default()
                };
                let birth = facing(rotation, t, initial, false);
                let current_t = t + ReferenceVector::new(0.05, 0.05, 0.0);
                let now = facing(rotation, current_t, moved, false);
                let make_reference = |now: [[f32; 3]; 3]| {
                    let mut q = quad([1.0; 4], false);
                    q.texture_indexes = [-1; 4];
                    q.texture1_enabled = false;
                    q.combine_mode_tc1 = [0, 3];
                    q.soft_particle_fade_range = 0.0;
                    q.size = [0.08; 2];
                    q.position = (match coord {
                        0 | 1 => {
                            t + transform(birth, 0.125) + ReferenceVector::new(0.25, 0.0, 0.25)
                        }
                        2 => {
                            current_t
                                + transform(now, 0.375)
                                + ReferenceVector::from_array(now[2]) * 0.25
                        }
                        3 => {
                            t + transform(birth, 0.375)
                                + ReferenceVector::from_array(birth[2]) * 0.25
                        }
                        _ => unreachable!(),
                    })
                    .to_array();
                    q.parent_basis = match coord {
                        0 | 1 => VFX_IDENTITY_BASIS,
                        2 => now,
                        3 => birth,
                        _ => unreachable!(),
                    };
                    q
                };
                references.push(make_reference(now));
                wrong_horizontal.push(make_reference(facing(rotation, current_t, moved, true)));
                let skeleton = ModelSkeleton {
                    body_scaling: None,
                    bone_names: vec!["root".into()],
                    parent_indices: vec![-1],
                    rest_pose: vec![BoneTransform {
                        translation: t.to_array(),
                        ..BoneTransform::IDENTITY
                    }],
                };
                let mut pose = SkeletonPose::rest_pose(&skeleton);
                pose.set_translation(0, current_t.to_array()).unwrap();
                pose_updates.push((moved.time, path.clone(), pose));
                mounts.attachments.push(WeaponVfxAttachment {
                    model_path: path.clone(),
                    data: WeaponVfxData {
                        file,
                        skeleton: Some(skeleton),
                        bind_points: [3, 4]
                            .into_iter()
                            .map(|id| VfxBindPoint {
                                id,
                                parent_bone: Some("root".into()),
                                translate: [0.0; 3],
                                rotate: [0.0; 3],
                            })
                            .collect(),
                        ..Default::default()
                    },
                });
                model.meshes.push(ModelMesh {
                    path,
                    ..template.clone()
                });
            }
        }
    }
    for samples in [1, 4] {
        let options = |label: &str| {
            WeaponModelSnapshotOptions::new(format!("vfx-preview-camera-{label}-{samples}"))
                .with_viewport(64, 64)
                .with_camera(moved.yaw, moved.pitch, moved.zoom, moved.pan)
                .with_render_options(ModelRenderOptions {
                    msaa_samples: samples,
                    camera_roll: moved.roll,
                    ..Default::default()
                })
                .with_hdr_scene_capture()
        };
        let draw_geometry = |label: &str, geometry: Vec<VfxQuad>| {
            render_weapon_model_snapshot_with_options(
                options(label).with_vfx_quads(geometry),
                &model,
            )
            .unwrap()
            .hdr_scene_rgba
            .unwrap()
        };
        let draw_mounted = |label: &str, inputs: Vec<WeaponVfxCameraInput>| {
            let mut o = options(label).with_weapon_vfx(mounts.clone(), moved.time);
            o.weapon_vfx_camera_inputs = inputs;
            o.weapon_vfx_pose_updates = pose_updates.clone();
            o.require_staged_weapon_vfx = true;
            let result = render_weapon_model_snapshot_with_options(o, &model).unwrap();
            if label == "actual" {
                assert_eq!(result.weapon_vfx_quads.len(), references.len());
                for (index, (actual, expected)) in
                    result.weapon_vfx_quads.iter().zip(&references).enumerate()
                {
                    for (actual, expected) in actual
                        .position
                        .into_iter()
                        .chain(actual.parent_basis.into_iter().flatten())
                        .chain(actual.size)
                        .zip(
                            expected
                                .position
                                .into_iter()
                                .chain(expected.parent_basis.into_iter().flatten())
                                .chain(expected.size),
                        )
                    {
                        assert!(
                            (actual - expected).abs() < 3e-6,
                            "quad {index}: {actual} != {expected}"
                        );
                    }
                }
            }
            result.hdr_scene_rgba.unwrap()
        };
        // The camera changes at a paused timestamp after a positive time input.
        let actual = draw_mounted(
            "actual",
            vec![
                initial,
                WeaponVfxCameraInput {
                    time: moved.time,
                    ..initial
                },
                moved,
            ],
        );
        let expected = draw_geometry("reference", references.clone());
        assert_image(&actual, &expected);
        let rebuilt = draw_mounted(
            "rebuilt-birth",
            vec![WeaponVfxCameraInput { time: 0.0, ..moved }],
        );
        let wrong = draw_geometry("full-parallel", wrong_horizontal.clone());
        for (label, control) in [("rebuilt birth", rebuilt), ("full parallel", wrong)] {
            assert!(
                expected
                    .iter()
                    .zip(control)
                    .filter(|(a, b)| a.iter().zip(b).any(|(a, b)| (a - b).abs() > 0.01))
                    .count()
                    > 3,
                "{label} must distinguish birth history/horizontal camera"
            );
        }
    }
}

#[test]
#[ignore = "renders production attachment bone poses, owner identity and PICd birth/current histories at 1x/4x"]
fn bone_pose_attachment_history_reaches_actual_gpu() {
    render_bone_pose_attachment_history_gpu(false);
}

#[test]
#[ignore = "checks Point factory zero injection direction through LoDr and real bone attachment GPU at 1x/4x"]
fn point_factory_zero_direction_reaches_local_offset_gpu() {
    render_bone_pose_attachment_history_gpu(true);
}

fn render_bone_pose_attachment_history_gpu(point_zero_direction: bool) {
    use xiv_companion_data::*;
    let constant = |z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let mut mounts = WeaponVfxAttachments::default();
    let mut pose_updates = Vec::new();
    let mut references = Vec::new();
    let mut wrong_injection = Vec::new();
    let prefix = if point_zero_direction {
        "vfx-point-zero-direction"
    } else {
        "vfx-bone-attachment"
    };
    let mut model = vfx_semantics_model();
    let template = model.meshes[0].clone();
    model.meshes.clear();
    for point in [true, false] {
        for coord in 0..=3 {
            let x = -0.75 + coord as f32 * 0.5;
            let y = if point { 0.5 } else { -0.5 };
            let path = format!("{}-{coord}.mdl", if point { "point" } else { "linear" });
            let property = |id| AvfxBinderProperties {
                bind_point_id: id,
                bind_target_point_type: 3,
                coord_update_frame: -1,
                ..Default::default()
            };
            let file = AvfxFile {
                binders: vec![AvfxBinder {
                    binder_type: if point { 0 } else { 1 },
                    bind_point_id: 3,
                    life: -1,
                    transform_scale: 255,
                    following_target_orientation: true,
                    vfx_scale_bias: 1.0,
                    properties_start: Some(property(3)),
                    properties_goal: (!point).then(|| property(4)),
                    data: Some(AvfxBinderData {
                        spring_strength: point.then(|| constant(1.0)),
                        carry_over_factor: (!point).then(|| constant(0.5)),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                timelines: vec![AvfxTimeline {
                    binder_index: -1,
                    items: vec![AvfxTimelineItem {
                        enabled: true,
                        start_time: 0,
                        end_time: -1,
                        binder_index: 0,
                        emitter_index: 0,
                        effector_index: -1,
                        clip_index: -1,
                        platform: 0,
                    }],
                    ..Default::default()
                }],
                emitters: vec![AvfxEmitter {
                    emitter_type: Some(EmitterType::Point),
                    effector_index: -1,
                    create_count: constant(1.0),
                    create_interval: constant(1000.0),
                    position: AvfxCurve3Axis {
                        x: Some(constant(0.125)),
                        ..Default::default()
                    },
                    particle_items: vec![AvfxEmitterItem {
                        enabled: true,
                        target_index: 0,
                        parameter_link: -1,
                        create_count: 1,
                        create_probability: 100,
                        parent_influence_coord: coord,
                        local_direction: i32::from(point && point_zero_direction),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                particles: vec![AvfxParticle {
                    particle_type: Some(ParticleType::Quad),
                    collision_type: -1,
                    rotation_direction_base: rotation_direction_base::NONE,
                    position: AvfxCurve3Axis {
                        x: Some(constant(0.25)),
                        ..Default::default()
                    },
                    scale: AvfxCurve3Axis {
                        x: Some(constant(0.2)),
                        y: Some(constant(0.2)),
                        z: Some(constant(0.2)),
                        ..Default::default()
                    },
                    ..Default::default()
                }],
                ..Default::default()
            };
            let skeleton = ModelSkeleton {
                body_scaling: None,
                bone_names: vec!["root".into()],
                parent_indices: vec![-1],
                rest_pose: vec![BoneTransform {
                    translation: [x - 1.25, y, 0.0],
                    scale: if point && point_zero_direction {
                        [1.0, 0.5, 2.0]
                    } else {
                        [2.0, 0.5, 1.0]
                    },
                    rotation: if point && point_zero_direction {
                        quat_from_axis_angle([0.0, 1.0, 0.0], -std::f32::consts::FRAC_PI_2)
                    } else {
                        BoneTransform::IDENTITY.rotation
                    },
                    ..BoneTransform::IDENTITY
                }],
            };
            let mut pose = SkeletonPose::rest_pose(&skeleton);
            pose.set_transform(
                0,
                BoneTransform {
                    translation: [x, y - 1.625, 0.0],
                    scale: if point && point_zero_direction {
                        [1.0, 0.5, 3.0]
                    } else {
                        [3.0, 0.5, 1.0]
                    },
                    rotation: if point && point_zero_direction {
                        [0.5, -0.5, 0.5, 0.5]
                    } else {
                        quat_from_axis_angle([0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_2)
                    },
                },
            )
            .unwrap();
            let points = [3, 4]
                .into_iter()
                .enumerate()
                .map(|(endpoint, id)| VfxBindPoint {
                    id,
                    parent_bone: Some("root".into()),
                    translate: if point && point_zero_direction {
                        [0.0, 0.0, -0.25]
                    } else {
                        [if point { 0.25 } else { endpoint as f32 * 0.5 }, 0.0, 0.0]
                    },
                    rotate: if point && point_zero_direction {
                        [0.0, std::f32::consts::FRAC_PI_2, 0.0]
                    } else {
                        [0.0; 3]
                    },
                })
                .collect();
            // The reference uses independently authored raw target coordinates,
            // never the pose adapter. Its final geometry is literal as well.
            let targets = |moved| {
                [3, 4]
                    .into_iter()
                    .enumerate()
                    .map(|(endpoint, id)| {
                        let offset = if point { 0.0 } else { endpoint as f32 - 0.5 };
                        VfxBinderTargetSnapshot {
                            id,
                            matrix: VfxBinderMatrix {
                                basis: if moved {
                                    [[0.0, 3.0, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]]
                                } else {
                                    [[2.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]]
                                },
                                position: if moved {
                                    [x, y - 0.875 + offset * 1.5, 0.0]
                                } else {
                                    [x - 0.75 + offset, y, 0.0]
                                },
                            },
                        }
                    })
                    .collect::<Vec<_>>()
            };
            let mut playback = VfxPlayback::new(
                VfxRuntime::new(&file)
                    .with_binder_target_snapshot(&targets(false))
                    .unwrap(),
            );
            assert_eq!(playback.fallback_reason(), None);
            playback
                .advance_with_binder_target_snapshot(0.125, &targets(true))
                .unwrap();
            let mut quads = Vec::new();
            playback.sample(&mut quads, &mut Vec::new());
            assert_eq!(quads.len(), 1);
            let mut reference = quads[0];
            reference.position = match coord {
                0 | 1 => [x + 0.25, y, 0.0],
                2 => [x, y + 0.25, 0.0],
                3 => [x, y, 0.0],
                _ => unreachable!(),
            };
            reference.parent_basis = match coord {
                0 | 1 => [[3.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]],
                2 => [[0.0, 3.0, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]],
                3 => [[2.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]],
                _ => unreachable!(),
            };
            reference.orientation = [0.0, 0.0, 0.0, 1.0];
            reference.size = [0.1; 2];
            references.push(reference);
            let mut wrong = reference;
            if point {
                wrong.position = match coord {
                    0 | 1 | 3 => [x - 0.5, y, -0.25],
                    2 => [x, y - 0.5, -0.25],
                    _ => unreachable!(),
                };
                wrong.movement_direction = [1.0, 0.0, 0.0];
            }
            wrong_injection.push(wrong);
            mounts.attachments.push(WeaponVfxAttachment {
                model_path: path.clone(),
                data: WeaponVfxData {
                    file,
                    skeleton: Some(skeleton),
                    bind_points: points,
                    ..Default::default()
                },
            });
            pose_updates.push((0.125, path.clone(), pose));
            model.meshes.push(ModelMesh {
                path,
                ..template.clone()
            });
        }
    }
    let mut rebuilt = mounts.clone();
    for (attachment, (_, _, pose)) in rebuilt.attachments.iter_mut().zip(&pose_updates) {
        let skeleton = attachment.data.skeleton.as_mut().unwrap();
        skeleton.rest_pose[0] = pose.transform(0).unwrap();
    }
    for samples in [1, 4] {
        let draw = |name: &str, data: WeaponVfxAttachments, updates: bool| {
            let mut options = WeaponModelSnapshotOptions::new(format!("{prefix}-{name}-{samples}"))
                .with_viewport(64, 64)
                .with_camera(0.0, 0.0, 3.0, [0.0; 2])
                .with_render_options(ModelRenderOptions {
                    msaa_samples: samples,
                    ..Default::default()
                })
                .with_weapon_vfx(data, 0.125)
                .with_hdr_scene_capture();
            options.require_staged_weapon_vfx = true;
            if updates {
                options.weapon_vfx_pose_updates = pose_updates.clone();
            }
            render_weapon_model_snapshot_with_options(options, &model)
                .unwrap()
                .hdr_scene_rgba
                .unwrap()
        };
        let actual = draw("actual", mounts.clone(), true);
        let reference = render_with_geometry_and_samples(
            &format!("{prefix}-reference-{samples}"),
            references.clone(),
            vec![],
            vec![solid([255; 4])],
            square(),
            samples,
        );
        assert_image(&actual, &reference);
        let wrong = if point_zero_direction {
            render_with_geometry_and_samples(
                &format!("{prefix}-wrong-injection-{samples}"),
                wrong_injection.clone(),
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )
        } else {
            draw("rebuilt-birth", rebuilt.clone(), false)
        };
        assert!(
            reference
                .iter()
                .zip(wrong)
                .filter(|(a, b)| a.iter().zip(b).any(|(a, b)| (a - b).abs() > 0.01))
                .count()
                > 10
        );
    }
}

#[test]
fn scheduled_direct_emitter_packets_match_birth_age_and_document_history() {
    scheduled_direct_emitter_history_packets();
}

#[test]
fn shared_timeline_packets_keep_parent_and_constructor_history() {
    shared_timeline_history_packets();
}

#[test]
#[ignore = "native GPU, shared Timeline parent and same-input Item Binder constructor history"]
fn shared_timeline_parent_and_item_birth_reach_actual_playback_gpu() {
    let (packets, references, controls) = shared_timeline_history_packets();
    for samples in [1, 4] {
        let draw = |label, quads: &Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-timeline-shared-{label}-{samples}"),
                quads.clone(),
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )
        };
        let reference = draw("reference", &references);
        assert_image(&draw("actual", &packets), &reference);
        let wrong = draw("wrong-history", &controls);
        assert!(
            reference
                .iter()
                .zip(wrong)
                .filter(|(a, b)| a.iter().zip(b).any(|(a, b)| (a - b).abs() > 0.01))
                .count()
                > 10
        );
    }
}

#[test]
fn shared_timeline_loop_packets_keep_wrap_births_and_inner_retirement() {
    shared_timeline_loop_history_packets();
}

#[test]
#[ignore = "native GPU, shared Timeline loops and finite inner Point retirement"]
fn shared_timeline_loop_and_inner_retirement_reach_actual_gpu() {
    let (packets, references, controls) = shared_timeline_loop_history_packets();
    for samples in [1, 4] {
        let draw = |label, quads: &Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-timeline-loop-{label}-{samples}"),
                quads.clone(),
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )
        };
        let reference = draw("reference", &references);
        assert_image(&draw("actual", &packets), &reference);
        let wrong = draw("wrong-history", &controls);
        assert!(
            reference
                .iter()
                .zip(wrong)
                .filter(|(a, b)| a.iter().zip(b).any(|(a, b)| (a - b).abs() > 0.01))
                .count()
                > 10
        );
    }
}

#[test]
fn shared_trigger_packets_keep_same_prepare_delay_and_retired_birth_history() {
    shared_trigger_history_packets();
}

fn object_binder_history_packets() -> (Vec<VfxQuad>, Vec<VfxQuad>, Vec<VfxQuad>) {
    use xiv_companion_data::{
        VfxBinderMatrix, VfxBinderObjectSnapshot, VfxBinderTargetSnapshot, VfxPlayback, VfxRuntime,
        avfx::*,
    };
    let curve = |z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let mut packets = Vec::new();
    let mut references = Vec::new();
    let mut controls = Vec::new();
    for mirrored in [false, true] {
        for mode in 0..=3 {
            let x = -0.7 + mode as f32 * 0.45;
            let y = if mirrored { -0.3 } else { 0.3 };
            let sign = if mirrored { -1.0 } else { 1.0 };
            let file = AvfxFile {
                schedulers: vec![AvfxScheduler {
                    items: vec![AvfxSchedulerItem {
                        enabled: true,
                        start_time: 0,
                        timeline_index: 0,
                    }],
                    ..Default::default()
                }],
                timelines: vec![AvfxTimeline {
                    binder_index: -1,
                    items: vec![AvfxTimelineItem {
                        enabled: true,
                        start_time: 0,
                        end_time: -1,
                        binder_index: 0,
                        emitter_index: 0,
                        effector_index: -1,
                        clip_index: -1,
                        platform: 0,
                    }],
                    ..Default::default()
                }],
                binders: vec![AvfxBinder {
                    bind_point_id: 0,
                    life: -1,
                    following_target_orientation: true,
                    transform_scale: 0,
                    properties_start: Some(AvfxBinderProperties {
                        bind_point_id: 0,
                        bind_target_point_type: 0,
                        coord_update_frame: -1,
                        ..Default::default()
                    }),
                    data: Some(AvfxBinderData {
                        spring_strength: Some(curve(1.0)),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                emitters: vec![AvfxEmitter {
                    emitter_type: Some(EmitterType::Point),
                    effector_index: -1,
                    create_count: curve(1.0),
                    create_interval: curve(1000.0),
                    particle_items: vec![AvfxEmitterItem {
                        enabled: true,
                        target_index: 0,
                        create_time: 1,
                        create_count: 1,
                        create_probability: 100,
                        parameter_link: -1,
                        parent_influence_coord: mode,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                particles: vec![AvfxParticle {
                    particle_type: Some(ParticleType::Quad),
                    collision_type: -1,
                    rotation_direction_base: rotation_direction_base::NONE,
                    life: AvfxLife {
                        enabled: true,
                        value: 30.0,
                        ..Default::default()
                    },
                    scale: AvfxCurve3Axis {
                        x: Some(curve(0.16)),
                        y: Some(curve(0.16)),
                        z: Some(curve(0.16)),
                        ..Default::default()
                    },
                    ..Default::default()
                }],
                ..Default::default()
            };
            // q=(0,0,1,1) is deliberately nonunit: raw rotation columns
            // (-1,2,0), (-2,-1,0), (0,0,1), not a normalized quaternion.
            let initial = VfxBinderObjectSnapshot {
                position: [x, y, 0.0],
                quaternion: [0.0, 0.0, 1.0, 1.0],
                scale: [sign * 2.0, 3.0, 1.0],
            };
            let moved = VfxBinderObjectSnapshot {
                position: [x + 0.08, y - 0.08, 0.0],
                quaternion: [0.0, 0.0, 0.0, 1.0],
                ..initial
            };
            let conflicting = [VfxBinderTargetSnapshot {
                id: 0,
                matrix: VfxBinderMatrix {
                    position: [100.0; 3],
                    ..VfxBinderMatrix::IDENTITY
                },
            }];
            let mut playback = VfxPlayback::new(
                VfxRuntime::new(&file)
                    .with_binder_target_snapshot(&conflicting)
                    .unwrap()
                    .with_binder_object_snapshot(initial)
                    .unwrap(),
            );
            assert_eq!(playback.fallback_reason(), None);
            // Ordinary Scheduler owns a real Timeline now. Its first Prepare
            // creates the Item Binder; establish the initial birth explicitly
            // before changing the object snapshot for the following Time.
            playback
                .advance_with_binder_sources(0.0, Some(&conflicting), Some(initial))
                .unwrap();
            playback
                .advance_with_binder_sources(1.0 / 30.0, Some(&conflicting), Some(moved))
                .unwrap();
            assert_eq!(playback.fallback_reason(), None);
            let mut actual = Vec::new();
            playback.sample(&mut actual, &mut Vec::new());
            assert_eq!(actual.len(), 1);
            let inverse = 1.0 / 5.0f32.sqrt();
            let birth_basis = [
                [-sign * inverse, sign * 2.0 * inverse, 0.0],
                [-2.0 * inverse, -inverse, 0.0],
                [0.0, 0.0, 1.0],
            ];
            let current_basis = [[sign, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
            let mut reference = actual[0].clone();
            reference.position = if mode == 2 {
                moved.position
            } else {
                initial.position
            };
            reference.parent_basis = match mode {
                2 => current_basis,
                3 => birth_basis,
                _ => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            };
            reference.size = [0.08; 2];
            reference.orientation = [0.0, 0.0, 0.0, 1.0];
            for (a, b) in actual[0]
                .position
                .into_iter()
                .chain(actual[0].parent_basis.into_iter().flatten())
                .chain(actual[0].size)
                .chain(actual[0].orientation)
                .zip(
                    reference
                        .position
                        .into_iter()
                        .chain(reference.parent_basis.into_iter().flatten())
                        .chain(reference.size)
                        .chain(reference.orientation),
                )
            {
                assert!(
                    (a - b).abs() < 1e-6,
                    "mode {mode}, mirror {mirrored}: {a} != {b}"
                );
            }
            let mut wrong = reference.clone();
            wrong.position = if mode == 2 {
                initial.position
            } else {
                moved.position
            };
            wrong.parent_basis = if mode == 2 {
                birth_basis
            } else {
                current_basis
            };
            packets.extend(actual);
            references.push(reference);
            controls.push(wrong);
        }
    }
    (packets, references, controls)
}

#[test]
fn object_binder_packets_preserve_raw_quaternion_and_independent_history() {
    object_binder_history_packets();
}

#[test]
#[ignore = "native GPU: independent BTPT=0, raw quaternion, mirror and birth/current history"]
fn object_binder_raw_quaternion_and_history_reach_actual_gpu() {
    let (packets, references, controls) = object_binder_history_packets();
    for samples in [1, 4] {
        let draw = |label, quads: &Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-object-target-{label}-{samples}"),
                quads.clone(),
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )
        };
        let reference = draw("reference", &references);
        assert_image(&draw("actual", &packets), &reference);
        let wrong = draw("wrong-history", &controls);
        assert!(
            reference
                .iter()
                .zip(wrong)
                .filter(|(a, b)| a.iter().zip(b).any(|(a, b)| (a - b).abs() > 0.01))
                .count()
                > 10
        );
    }
}

#[test]
#[ignore = "native GPU, TRG/RTRG same-Prepare target Binder delay and birth history"]
fn shared_trigger_and_target_binder_history_reach_actual_gpu() {
    let (packets, references, controls) = shared_trigger_history_packets();
    for samples in [1, 4] {
        let draw = |label, quads: &Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-timeline-event-{label}-{samples}"),
                quads.clone(),
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )
        };
        let reference = draw("reference", &references);
        assert_image(&draw("actual", &packets), &reference);
        let wrong = draw("wrong-history", &controls);
        assert!(
            reference
                .iter()
                .zip(wrong)
                .filter(|(a, b)| a.iter().zip(b).any(|(a, b)| (a - b).abs() > 0.01))
                .count()
                > 10
        );
    }
}

fn shared_trigger_history_packets() -> (Vec<VfxQuad>, Vec<VfxQuad>, Vec<VfxQuad>) {
    use xiv_companion_data::{
        VfxBinderMatrix, VfxBinderTargetSnapshot, VfxPlayback, VfxRuntime, avfx::*,
    };
    let curve = |value| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z: value,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let mut packets = Vec::new();
    let mut references = Vec::new();
    let mut controls = Vec::new();
    for random in [false, true] {
        for mode in 0..=3 {
            let x = -0.75 + mode as f32 * 0.45;
            let y = if random { -0.25 } else { 0.3 };
            let point = |id, life, delay| AvfxBinder {
                bind_point_id: id,
                life,
                transform_scale: 255,
                following_target_orientation: true,
                properties_start: Some(AvfxBinderProperties {
                    generate_delay: delay,
                    bind_target_point_type: 3,
                    bind_point_id: id,
                    coord_update_frame: -1,
                    ..Default::default()
                }),
                data: Some(AvfxBinderData {
                    spring_strength: Some(curve(1.0)),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let item = AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: 1,
                emitter_index: 0,
                effector_index: -1,
                clip_index: -1,
                platform: 0,
            };
            let mut triggers = vec![
                AvfxSchedulerItem {
                    enabled: false,
                    start_time: 0,
                    timeline_index: -1,
                };
                12
            ];
            for slot in 8..=10 {
                triggers[slot] = AvfxSchedulerItem {
                    enabled: false,
                    start_time: 99,
                    timeline_index: 1,
                };
            }
            let file = AvfxFile {
                schedulers: vec![AvfxScheduler {
                    item_count: None,
                    items: vec![AvfxSchedulerItem {
                        enabled: true,
                        start_time: 0,
                        timeline_index: 0,
                    }],
                    triggers,
                }],
                timelines: vec![
                    AvfxTimeline {
                        binder_index: 0,
                        items: vec![AvfxTimelineItem {
                            start_time: 1,
                            end_time: 2,
                            binder_index: -1,
                            emitter_index: -1,
                            clip_index: 0,
                            ..item
                        }],
                        clips: vec![AvfxTimelineClip {
                            clip_type: Some(if random {
                                AvfxTimelineClipType::RandomTrigger
                            } else {
                                AvfxTimelineClipType::Trigger
                            }),
                            raw_ints: Some([8, 10, 0, 0]),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    AvfxTimeline {
                        binder_index: -1,
                        items: vec![
                            item,
                            AvfxTimelineItem {
                                start_time: 2,
                                ..item
                            },
                        ],
                        ..Default::default()
                    },
                ],
                binders: vec![point(4, 20, 2), point(7, 2, 1)],
                emitters: vec![AvfxEmitter {
                    emitter_type: Some(EmitterType::Point),
                    effector_index: -1,
                    create_count: curve(1.0),
                    create_interval: curve(1000.0),
                    particle_items: vec![AvfxEmitterItem {
                        enabled: true,
                        target_index: 0,
                        create_time: 1,
                        create_count: 1,
                        create_probability: 100,
                        parameter_link: -1,
                        parent_influence_coord: mode,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                particles: vec![AvfxParticle {
                    particle_type: Some(ParticleType::Quad),
                    collision_type: -1,
                    rotation_direction_base: rotation_direction_base::NONE,
                    life: AvfxLife {
                        enabled: true,
                        value: 30.0,
                        ..Default::default()
                    },
                    scale: AvfxCurve3Axis {
                        x: Some(curve(0.1)),
                        y: Some(curve(0.1)),
                        z: Some(curve(0.1)),
                        ..Default::default()
                    },
                    ..Default::default()
                }],
                ..Default::default()
            };
            let targets = |phase| {
                vec![
                    VfxBinderTargetSnapshot {
                        id: 4,
                        matrix: VfxBinderMatrix {
                            position: [100.0, 0.0, 0.0],
                            ..VfxBinderMatrix::IDENTITY
                        },
                    },
                    VfxBinderTargetSnapshot {
                        id: 7,
                        matrix: VfxBinderMatrix {
                            position: [x + phase as f32 * 0.1, y, 0.0],
                            ..VfxBinderMatrix::IDENTITY
                        },
                    },
                ]
            };
            let mut value = VfxPlayback::new(
                VfxRuntime::new(&file)
                    .with_binder_target_snapshot(&targets(0))
                    .unwrap(),
            );
            assert_eq!(value.fallback_reason(), None);
            for (phase, delta) in [(0, 0.0), (1, 3.0), (2, 2.0), (3, 1.0), (4, 1.0)] {
                value
                    .advance_with_binder_target_snapshot(delta / 30.0, &targets(phase))
                    .unwrap();
                assert_eq!(value.fallback_reason(), None);
            }
            let mut actual = Vec::new();
            value.sample(&mut actual, &mut Vec::new());
            assert_eq!(actual.len(), 2);
            for (index, packet) in actual.into_iter().enumerate() {
                let mut reference = packet;
                reference.position = [
                    x + if mode == 2 {
                        [0.3, 0.4][index]
                    } else {
                        [0.2, 0.3][index]
                    },
                    y,
                    0.0,
                ];
                reference.parent_basis = xiv_companion_data::VFX_IDENTITY_BASIS;
                reference.size = [0.05; 2];
                reference.orientation = [0.0, 0.0, 0.0, 1.0];
                for (a, b) in packet
                    .position
                    .into_iter()
                    .zip(reference.position)
                    .chain(
                        packet
                            .parent_basis
                            .into_iter()
                            .flatten()
                            .zip(reference.parent_basis.into_iter().flatten()),
                    )
                    .chain(packet.size.into_iter().zip(reference.size))
                {
                    assert!(
                        (a - b).abs() < 1e-5,
                        "TRG random={random}, PICd={mode}, birth={index}: {a} != {b}"
                    );
                }
                packets.push(packet);
                references.push(reference);
                let mut wrong = reference;
                wrong.position[0] += 0.07;
                wrong.position[1] += 0.02;
                controls.push(wrong);
            }
        }
    }
    assert_eq!(packets.len(), 16);
    (packets, references, controls)
}

fn shared_timeline_loop_history_packets() -> (Vec<VfxQuad>, Vec<VfxQuad>, Vec<VfxQuad>) {
    use xiv_companion_data::{
        VfxBinderMatrix, VfxBinderTargetSnapshot, VfxPlayback, VfxRuntime, avfx::*,
    };
    let curve = |value| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z: value,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let mut packets = Vec::new();
    let mut references = Vec::new();
    let mut controls = Vec::new();
    for inner in [false, true] {
        for mode in 0..=3 {
            let x = -0.7 + mode as f32 * 0.4;
            let y = if inner { -0.25 } else { 0.3 };
            let point = |id, life| AvfxBinder {
                bind_point_id: id,
                life,
                transform_scale: 255,
                following_target_orientation: true,
                properties_start: Some(AvfxBinderProperties {
                    bind_target_point_type: 3,
                    bind_point_id: id,
                    coord_update_frame: -1,
                    ..Default::default()
                }),
                data: Some(AvfxBinderData {
                    spring_strength: Some(curve(1.0)),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let item = AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: if inner { 1 } else { -1 },
                emitter_index: 0,
                effector_index: -1,
                clip_index: -1,
                platform: 0,
            };
            let file = AvfxFile {
                timelines: vec![AvfxTimeline {
                    binder_index: 0,
                    loop_start: if inner { 0 } else { 2 },
                    loop_end: if inner { 2 } else { 5 },
                    items: if inner {
                        vec![item]
                    } else {
                        vec![
                            item,
                            AvfxTimelineItem {
                                start_time: 3,
                                ..item
                            },
                        ]
                    },
                    ..Default::default()
                }],
                binders: if inner {
                    vec![point(4, -1), point(7, 1)]
                } else {
                    vec![point(4, -1)]
                },
                emitters: vec![AvfxEmitter {
                    emitter_type: Some(EmitterType::Point),
                    effector_index: -1,
                    create_count: curve(1.0),
                    create_interval: curve(1000.0),
                    particle_items: vec![AvfxEmitterItem {
                        enabled: true,
                        target_index: 0,
                        create_time: 1,
                        create_count: 1,
                        create_probability: 100,
                        parameter_link: -1,
                        parent_influence_coord: mode,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                particles: vec![AvfxParticle {
                    particle_type: Some(ParticleType::Quad),
                    collision_type: -1,
                    rotation_direction_base: rotation_direction_base::NONE,
                    life: AvfxLife {
                        enabled: true,
                        value: 30.0,
                        ..Default::default()
                    },
                    scale: AvfxCurve3Axis {
                        x: Some(curve(0.12)),
                        y: Some(curve(0.12)),
                        z: Some(curve(0.12)),
                        ..Default::default()
                    },
                    ..Default::default()
                }],
                ..Default::default()
            };
            let targets = |phase| {
                let mut values = vec![VfxBinderTargetSnapshot {
                    id: 4,
                    matrix: VfxBinderMatrix {
                        position: [
                            if inner {
                                100.0
                            } else {
                                x + phase as f32 * 0.06
                            },
                            y,
                            0.0,
                        ],
                        ..VfxBinderMatrix::IDENTITY
                    },
                }];
                if inner {
                    values.push(VfxBinderTargetSnapshot {
                        id: 7,
                        matrix: VfxBinderMatrix {
                            position: [x + phase as f32 * 0.12, y, 0.0],
                            ..VfxBinderMatrix::IDENTITY
                        },
                    });
                }
                values
            };
            let mut value = VfxPlayback::new(
                VfxRuntime::new(&file)
                    .with_binder_target_snapshot(&targets(0))
                    .unwrap(),
            );
            assert_eq!(value.fallback_reason(), None);
            for (phase, delta) in if inner {
                vec![(0, 0.0), (1, 2.0), (2, 1.0), (3, 1.0)]
            } else {
                vec![(0, 0.0), (1, 8.0), (2, 1.0), (3, 2.0)]
            } {
                value
                    .advance_with_binder_target_snapshot(delta / 30.0, &targets(phase))
                    .unwrap();
                assert_eq!(value.fallback_reason(), None);
            }
            let mut actual = Vec::new();
            value.sample(&mut actual, &mut Vec::new());
            assert_eq!(actual.len(), if inner { 3 } else { 4 });
            for (index, packet) in actual.into_iter().enumerate() {
                let mut reference = packet;
                reference.position = [
                    x + if inner {
                        if mode == 2 {
                            [0.0, 0.24, 0.36][index]
                        } else {
                            [0.0, 0.12, 0.36][index]
                        }
                    } else if mode == 2 {
                        0.18
                    } else {
                        index as f32 * 0.06
                    },
                    y,
                    0.0,
                ];
                reference.parent_basis = xiv_companion_data::VFX_IDENTITY_BASIS;
                reference.size = [0.06; 2];
                reference.orientation = [0.0, 0.0, 0.0, 1.0];
                for (a, b) in packet
                    .position
                    .into_iter()
                    .zip(reference.position)
                    .chain(
                        packet
                            .parent_basis
                            .into_iter()
                            .flatten()
                            .zip(reference.parent_basis.into_iter().flatten()),
                    )
                    .chain(packet.size.into_iter().zip(reference.size))
                {
                    assert!(
                        (a - b).abs() < 1e-5,
                        "loop inner={inner}, PICd={mode}, birth={index}: {a} != {b}"
                    );
                }
                packets.push(packet);
                references.push(reference);
                let mut wrong = reference;
                wrong.position[0] += 0.07;
                wrong.position[1] += 0.02;
                controls.push(wrong);
            }
        }
    }
    assert_eq!(packets.len(), 28);
    (packets, references, controls)
}

fn shared_timeline_history_packets() -> (Vec<VfxQuad>, Vec<VfxQuad>, Vec<VfxQuad>) {
    use xiv_companion_data::{
        VfxBinderMatrix, VfxBinderTargetSnapshot, VfxPlayback, VfxRuntime, avfx::*,
    };
    let curve = |value| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z: value,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let mut packets = Vec::new();
    let mut references = Vec::new();
    let mut controls = Vec::new();
    for inner in [false, true] {
        for mode in 0..=3 {
            let x = if inner {
                -0.3 + mode as f32 * 0.2
            } else {
                -0.6 + mode as f32 * 0.4
            };
            let y = if inner { -0.25 } else { 0.3 };
            let point = |id, spring| AvfxBinder {
                bind_point_id: id,
                life: -1,
                transform_scale: 255,
                following_target_orientation: true,
                properties_start: Some(AvfxBinderProperties {
                    bind_target_point_type: 3,
                    bind_point_id: id,
                    coord_update_frame: -1,
                    ..Default::default()
                }),
                data: Some(AvfxBinderData {
                    spring_strength: Some(curve(spring)),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let item = AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: if inner { 1 } else { -1 },
                emitter_index: 0,
                effector_index: -1,
                clip_index: -1,
                platform: 0,
            };
            let mut items = vec![item];
            if !inner {
                items.push(AvfxTimelineItem {
                    start_time: 2,
                    ..item
                });
            }
            let file = AvfxFile {
                timelines: vec![AvfxTimeline {
                    binder_index: 0,
                    items,
                    ..Default::default()
                }],
                binders: if inner {
                    vec![point(4, 1.0), point(7, 0.5)]
                } else {
                    vec![point(4, 1.0)]
                },
                emitters: vec![AvfxEmitter {
                    emitter_type: Some(EmitterType::Point),
                    effector_index: -1,
                    create_count: curve(1.0),
                    create_interval: curve(1000.0),
                    particle_items: vec![AvfxEmitterItem {
                        enabled: true,
                        target_index: 0,
                        create_time: 1,
                        create_count: 1,
                        create_probability: 100,
                        parameter_link: -1,
                        parent_influence_coord: mode,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                particles: vec![AvfxParticle {
                    particle_type: Some(ParticleType::Quad),
                    collision_type: -1,
                    rotation_direction_base: rotation_direction_base::NONE,
                    scale: AvfxCurve3Axis {
                        x: Some(curve(0.2)),
                        y: Some(curve(0.2)),
                        z: Some(curve(0.2)),
                        ..Default::default()
                    },
                    ..Default::default()
                }],
                ..Default::default()
            };
            let targets = |phase| {
                let mut values = vec![VfxBinderTargetSnapshot {
                    id: 4,
                    matrix: VfxBinderMatrix {
                        position: [
                            if inner {
                                100.0
                            } else {
                                x + phase as f32 * 0.125
                            },
                            y,
                            0.0,
                        ],
                        ..VfxBinderMatrix::IDENTITY
                    },
                }];
                if inner {
                    values.push(VfxBinderTargetSnapshot {
                        id: 7,
                        matrix: VfxBinderMatrix {
                            position: [2.0 * x, 2.0 * y, 0.0],
                            ..VfxBinderMatrix::IDENTITY
                        },
                    });
                }
                values
            };
            let mut value = VfxPlayback::new(
                VfxRuntime::new(&file)
                    .with_binder_target_snapshot(&targets(0))
                    .unwrap(),
            );
            assert_eq!(value.fallback_reason(), None);
            let mut actual = Vec::new();
            value.sample(&mut actual, &mut Vec::new());
            assert!(actual.is_empty());
            value
                .advance_with_binder_target_snapshot(0.0, &targets(0))
                .unwrap();
            if inner {
                value
                    .advance_with_binder_target_snapshot(1.0 / 30.0, &targets(0))
                    .unwrap();
            } else {
                value
                    .advance_with_binder_target_snapshot(2.0 / 30.0, &targets(1))
                    .unwrap();
                value
                    .advance_with_binder_target_snapshot(1.0 / 30.0, &targets(2))
                    .unwrap();
            }
            assert_eq!(value.fallback_reason(), None);
            value.sample(&mut actual, &mut Vec::new());
            assert_eq!(actual.len(), if inner { 1 } else { 2 });
            for (index, packet) in actual.into_iter().enumerate() {
                let mut reference = packet;
                reference.position = if inner {
                    if mode == 2 {
                        [1.75 * x, 1.75 * y, 0.0]
                    } else {
                        [x, y, 0.0]
                    }
                } else {
                    [
                        if mode == 2 {
                            x + 0.25
                        } else {
                            x + index as f32 * 0.125
                        },
                        y,
                        0.0,
                    ]
                };
                reference.parent_basis = xiv_companion_data::VFX_IDENTITY_BASIS;
                reference.size = [0.1; 2];
                reference.orientation = [0.0, 0.0, 0.0, 1.0];
                for (a, b) in packet
                    .position
                    .into_iter()
                    .zip(reference.position)
                    .chain(
                        packet
                            .parent_basis
                            .into_iter()
                            .flatten()
                            .zip(reference.parent_basis.into_iter().flatten()),
                    )
                    .chain(packet.size.into_iter().zip(reference.size))
                {
                    assert!(
                        (a - b).abs() < 1e-5,
                        "shared inner={inner}, PICd={mode}, Item={index}: {a} != {b}"
                    );
                }
                packets.push(packet);
                references.push(reference);
                // Incorrectly reuse the outer/birth cache, or overwrite the
                // inner constructor cache with ordinary Prepare in this input.
                reference.position = if inner {
                    [1.5 * x, 1.5 * y, 0.0]
                } else {
                    [x, y, 0.0]
                };
                controls.push(reference);
            }
        }
    }
    (packets, references, controls)
}

#[test]
#[ignore = "native GPU, scheduled direct emitter age and Document PICd histories"]
fn scheduled_direct_emitter_birth_reaches_actual_playback_gpu() {
    let (packets, references, controls) = scheduled_direct_emitter_history_packets();
    for samples in [1, 4] {
        let draw = |label, quads: &Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-direct-scheduled-history-{label}-{samples}"),
                quads.clone(),
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )
        };
        let reference = draw("reference", &references);
        assert_image(&draw("actual", &packets), &reference);
        let wrong = draw("zero-birth-age", &controls);
        assert!(
            reference
                .iter()
                .zip(wrong)
                .filter(|(a, b)| a.iter().zip(b).any(|(a, b)| (a - b).abs() > 0.01))
                .count()
                > 10
        );
    }
}

fn scheduled_direct_emitter_history_packets() -> (Vec<VfxQuad>, Vec<VfxQuad>, Vec<VfxQuad>) {
    use xiv_companion_data::{VfxBinderMatrix, VfxPlayback, VfxRuntime, avfx::*};
    let curve = |values: &[(i16, f32)]| AvfxCurve {
        keys: values
            .iter()
            .map(|&(time, z)| AvfxCurveKey {
                time,
                z,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
            })
            .collect(),
        ..Default::default()
    };
    let mut packets = Vec::new();
    let mut references = Vec::new();
    let mut controls = Vec::new();
    for mode in 0..=3 {
        let x = -0.75 + mode as f32 * 0.5;
        let file = AvfxFile {
            timelines: vec![AvfxTimeline {
                binder_index: -1,
                items: vec![AvfxTimelineItem {
                    enabled: true,
                    start_time: 2,
                    end_time: -1,
                    emitter_index: 0,
                    binder_index: -1,
                    effector_index: -1,
                    clip_index: -1,
                    platform: 0,
                }],
                ..Default::default()
            }],
            emitters: vec![AvfxEmitter {
                emitter_type: Some(EmitterType::Point),
                effector_index: -1,
                create_count: curve(&[(0, 1.0)]),
                create_interval: curve(&[(0, 1000.0)]),
                position: AvfxCurve3Axis {
                    x: Some(curve(&[(0, 0.125), (1, 0.25)])),
                    ..Default::default()
                },
                particle_items: vec![AvfxEmitterItem {
                    enabled: true,
                    target_index: 0,
                    create_time: 1,
                    create_count: 1,
                    create_probability: 100,
                    parameter_link: -1,
                    parent_influence_coord: mode,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            particles: vec![AvfxParticle {
                particle_type: Some(ParticleType::Quad),
                collision_type: -1,
                rotation_direction_base: rotation_direction_base::NONE,
                position: AvfxCurve3Axis {
                    x: Some(curve(&[(0, 0.25)])),
                    ..Default::default()
                },
                scale: AvfxCurve3Axis {
                    x: Some(curve(&[(0, 0.2)])),
                    y: Some(curve(&[(0, 0.2)])),
                    z: Some(curve(&[(0, 0.2)])),
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let birth = VfxBinderMatrix {
            basis: [[2.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]],
            position: [x - 0.75, 0.0, 0.0],
        };
        let current = VfxBinderMatrix {
            basis: [[0.0, 2.0, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]],
            position: [x + 0.25, -0.25, 0.0],
        };
        let mut playback = VfxPlayback::new(
            VfxRuntime::new(&file)
                .with_document_matrix(VfxBinderMatrix::IDENTITY)
                .unwrap(),
        );
        assert_eq!(playback.fallback_reason(), None);
        let mut actual = Vec::new();
        playback.sample(&mut actual, &mut Vec::new());
        assert!(actual.is_empty());
        playback
            .advance_with_document_matrix(1.0 / 30.0, current)
            .unwrap();
        playback.sample(&mut actual, &mut Vec::new());
        assert!(actual.is_empty());
        playback
            .advance_with_document_matrix(2.0 / 30.0, birth)
            .unwrap();
        playback
            .advance_with_document_matrix(1.0 / 30.0, current)
            .unwrap();
        assert_eq!(playback.fallback_reason(), None);
        playback.sample(&mut actual, &mut Vec::new());
        assert_eq!(actual.len(), 1);
        let mut reference = actual[0];
        reference.position = match mode {
            0 | 1 => [x - 0.25, 0.5, 0.0],
            2 => [x + 0.25, 0.75, 0.0],
            3 => [x + 0.25, 0.0, 0.0],
            _ => unreachable!(),
        };
        reference.parent_basis = if mode == 3 {
            birth.basis
        } else {
            current.basis
        };
        reference.size = [0.1; 2];
        reference.orientation = [0.0, 0.0, 0.0, 1.0];
        for (a, b) in actual[0]
            .position
            .into_iter()
            .zip(reference.position)
            .chain(
                actual[0]
                    .parent_basis
                    .into_iter()
                    .flatten()
                    .zip(reference.parent_basis.into_iter().flatten()),
            )
            .chain(actual[0].size.into_iter().zip(reference.size))
        {
            assert!((a - b).abs() < 1e-5, "direct PICd {mode}: {a} != {b}");
        }
        packets.push(actual[0]);
        references.push(reference);
        reference.position = match mode {
            0 | 1 => [x - 0.5, 0.5, 0.0],
            2 => [x + 0.25, 0.5, 0.0],
            3 => [x, 0.0, 0.0],
            _ => unreachable!(),
        };
        controls.push(reference);
    }
    (packets, references, controls)
}

enum AuxiliaryScenario {
    Point,
    Linear,
    LinearGlobal,
    LinearRootRevision,
    LinearMissingGoal,
    NestedPoint,
    RootRevision,
    Document,
    DynamicDocument,
}

fn render_point_auxiliary_playback(scenario: AuxiliaryScenario) {
    use xiv_companion_data::{AvfxFile, VfxPlayback, VfxRuntime, avfx::*};
    let nested = matches!(scenario, AuxiliaryScenario::NestedPoint);
    let linear_global = matches!(scenario, AuxiliaryScenario::LinearGlobal);
    let missing_goal = matches!(scenario, AuxiliaryScenario::LinearMissingGoal);
    let linear = matches!(
        scenario,
        AuxiliaryScenario::Linear
            | AuxiliaryScenario::LinearGlobal
            | AuxiliaryScenario::LinearRootRevision
            | AuxiliaryScenario::LinearMissingGoal
    );
    let root_revision = matches!(
        scenario,
        AuxiliaryScenario::RootRevision | AuxiliaryScenario::LinearRootRevision
    );
    let dynamic_document = matches!(scenario, AuxiliaryScenario::DynamicDocument);
    let document = matches!(
        scenario,
        AuxiliaryScenario::Document | AuxiliaryScenario::DynamicDocument
    );
    let constant = |value| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z: value,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let mut continuous_packets = Vec::new();
    let mut staged_packets = Vec::new();
    let mut references = Vec::new();
    let mut controls = Vec::new();
    for coord in 0..=3 {
        let x = -0.75 + coord as f32 * 0.5;
        let mut file = AvfxFile {
            binders: vec![AvfxBinder {
                bind_point_id: 3,
                life: -1,
                ..Default::default()
            }],
            timelines: vec![AvfxTimeline {
                binder_index: -1,
                items: vec![AvfxTimelineItem {
                    enabled: true,
                    start_time: 0,
                    end_time: -1,
                    binder_index: 0,
                    effector_index: -1,
                    emitter_index: 0,
                    platform: 0,
                    clip_index: -1,
                }],
                ..Default::default()
            }],
            emitters: vec![AvfxEmitter {
                emitter_type: Some(EmitterType::Point),
                effector_index: -1,
                create_count: constant(1.0),
                create_interval: constant(1000.0),
                position: AvfxCurve3Axis {
                    x: Some(constant(0.125)),
                    ..Default::default()
                },
                particle_items: vec![AvfxEmitterItem {
                    enabled: true,
                    target_index: 0,
                    parameter_link: -1,
                    create_probability: 100,
                    create_count: 1,
                    parent_influence_coord: coord,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            particles: vec![AvfxParticle {
                particle_type: Some(ParticleType::Quad),
                collision_type: -1,
                rotation_direction_base: rotation_direction_base::NONE,
                position: AvfxCurve3Axis {
                    x: Some(constant(0.25)),
                    ..Default::default()
                },
                scale: AvfxCurve3Axis {
                    x: Some(constant(0.2)),
                    y: Some(constant(0.2)),
                    z: Some(constant(0.2)),
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        if root_revision {
            file.global.ags_enabled = true;
            file.global.revised_rotation = [0.0, 0.0, std::f32::consts::FRAC_PI_2];
        }
        if document {
            file.binders.clear();
            file.timelines[0].items[0].binder_index = -1;
            file.global.revised_rotation = [0.0, 0.0, std::f32::consts::FRAC_PI_2];
            file.global.revised_scale = [-2.0, 0.5, 1.0];
            file.global.revised_position = [0.125, -0.125, 0.0];
        }
        if dynamic_document {
            file.global.revised_rotation = [0.0; 3];
            file.global.revised_scale = [1.0; 3];
            file.global.revised_position = [0.0; 3];
        }
        if nested {
            let mut child = file.emitters[0].clone();
            child.scale = AvfxCurve3Axis {
                x: Some(constant(0.5)),
                y: Some(constant(2.0)),
                z: Some(constant(1.0)),
                ..Default::default()
            };
            child.particle_items[0].parent_influence_coord = 2;
            file.emitters[0].scale = AvfxCurve3Axis {
                x: Some(constant(2.0)),
                y: Some(constant(3.0)),
                z: Some(constant(1.0)),
                ..Default::default()
            };
            file.emitters[0].particle_items.clear();
            file.emitters[0].emitter_items.push(AvfxEmitterItem {
                enabled: true,
                target_index: 1,
                parameter_link: -1,
                create_count: 1,
                create_probability: 100,
                parent_influence_coord: coord,
                ..Default::default()
            });
            file.emitters.push(child);
        }
        if linear {
            let properties = |id| xiv_companion_data::avfx::AvfxBinderProperties {
                bind_point_id: id,
                bind_target_point_type: 3,
                binder_name: "null".into(),
                coord_update_frame: -1,
                ..Default::default()
            };
            file.binders[0].binder_type = 1;
            file.binders[0].transform_scale = 255;
            file.binders[0].start_to_global_direction = linear_global;
            file.binders[0].properties_start = Some(properties(3));
            file.binders[0].properties_goal = Some(properties(4));
            file.binders[0].data = Some(xiv_companion_data::avfx::AvfxBinderData {
                carry_over_factor: Some(constant(0.5)),
                ..Default::default()
            });
        }
        let mut points = vec![VfxBindPoint {
            id: 3,
            parent_bone: None,
            translate: if root_revision {
                // A sends local +X to +2Y. Cancel the root and particle
                // local offsets to place the quad at the reference origin.
                [x, -0.75, 0.0]
            } else {
                [x + if nested && coord >= 2 { 1.25 } else { 0.75 }, 0.0, 0.0]
            },
            rotate: [0.0; 3],
        }];
        if linear {
            if linear_global {
                // Look-at toward -Z gives diag(-1,1,-1). PICd 0/1
                // consume A directly for particle Pos; 2/3 inherit M*A.
                points[0].translate = [x + if coord < 2 { 0.25 } else { -0.75 }, 0.0, 0.5];
            } else {
                points[0].translate[0] -= 0.25;
            }
            let mut goal = points[0].clone();
            goal.id = 4;
            if linear_global {
                goal.translate[2] = -0.5;
            } else {
                goal.translate[0] += 0.5;
            }
            points.push(goal);
        }
        if missing_goal {
            points.pop();
        }
        let runtime = if dynamic_document {
            VfxRuntime::new(&file)
                .with_document_matrix(xiv_companion_data::VfxBinderMatrix {
                    basis: [[2.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]],
                    position: [x - 0.75, 0.0, 0.0],
                })
                .unwrap()
        } else if document {
            VfxRuntime::new(&file)
                .with_document_matrix(xiv_companion_data::VfxBinderMatrix {
                    basis: [[0.0, 2.0, 0.0], [-3.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
                    position: [x - 2.625, -0.25, 0.0],
                })
                .unwrap()
        } else {
            VfxRuntime::with_bind_points(&file, &points)
                .with_document_scale([-2.0, 0.5, 1.0])
                .unwrap()
        };
        let mut staged = VfxPlayback::new(runtime.clone());
        assert_eq!(
            staged.fallback_reason().is_some(),
            linear_global,
            "{:?}",
            staged.fallback_reason()
        );
        if dynamic_document {
            staged
                .advance_with_document_matrix(
                    1.0 / 30.0,
                    xiv_companion_data::VfxBinderMatrix {
                        basis: [[0.0, 2.0, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]],
                        position: [x + 0.25, -0.25, 0.0],
                    },
                )
                .unwrap();
        } else {
            staged.advance(1.0 / 30.0).unwrap();
        }
        assert_eq!(
            staged.fallback_reason().is_some(),
            linear_global,
            "{:?}",
            staged.fallback_reason()
        );
        let mut sampled = Vec::new();
        let mut meshes = Vec::new();
        staged.sample(&mut sampled, &mut meshes);
        assert!(meshes.is_empty());
        if missing_goal {
            assert!(
                sampled.is_empty(),
                "failed first query cannot create staged children"
            );
            runtime.sample(1.0 / 30.0, &mut sampled);
            assert!(
                sampled.is_empty(),
                "failed first query cannot create continuous children"
            );
            let mut phantom = quad([1.0; 4], false);
            // Previous generic fallback used the start target with identity
            // auxiliary, then added emitter .125 and particle .25 local X.
            phantom.position = [x + 0.875, 0.0, 0.0];
            phantom.size = [0.1; 2];
            controls.push(phantom);
            continue;
        }
        assert_eq!(sampled.len(), 1);
        staged_packets.push(sampled[0]);
        runtime.sample(1.0 / 30.0, &mut sampled);
        assert_eq!(sampled.len(), 1);
        continuous_packets.push(sampled[0]);
        // Independent native-order geometry: target + Q*emitter Pos +
        // Q*particle Pos; local half-size remains 0.1, Q is consumed once.
        let mut reference = sampled[0];
        reference.position = if dynamic_document {
            match coord {
                0 | 1 => [x - 0.5, 0.5, 0.0],
                2 => [x + 0.25, 0.5, 0.0],
                3 => [x, 0.0, 0.0],
                _ => unreachable!(),
            }
        } else {
            [x, 0.0, 0.0]
        };
        reference.parent_basis = if linear_global {
            if coord < 2 {
                [[-2.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]]
            } else {
                [[2.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, -1.0]]
            }
        } else if dynamic_document {
            if coord == 3 {
                [[2.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]]
            } else {
                [[0.0, 2.0, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]]
            }
        } else if document {
            // Literal host * R_native(-RvR) * S. The independent main
            // getter contributes only world translation, auxiliary Pos=0.
            [[6.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]]
        } else if root_revision {
            // Native root Z rotation is transposed relative to ordinary
            // Euler. Literal R_native * diag(-2, .5, 1), not the helper.
            [[0.0, 2.0, 0.0], [0.5, 0.0, 0.0], [0.0, 0.0, 1.0]]
        } else if !nested {
            [[-2.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]]
        } else if coord < 2 {
            // A * child S: parent's own S does not enter PICd 0/1.
            [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        } else {
            // Parent M * child S: A * parent S * child S.
            [[-2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 1.0]]
        };
        reference.size = [0.1; 2];
        reference.orientation = [0.0, 0.0, 0.0, 1.0];
        references.push(reference);
        reference.position = if linear_global {
            // Keep A and the interpolated endpoint position, but drop M's
            // look-at basis. This isolates the missing bStG consumer.
            [x - if coord < 2 { 0.5 } else { 1.5 }, 0.0, 0.0]
        } else if dynamic_document {
            // Incorrectly rebuilding every birth with the current root.
            [x + 0.25, 0.5, 0.0]
        } else if document {
            // A merged main/identity auxiliary loses the particle's
            // independent A for PICd 0/1, while 2/3 still inherit A.
            [if coord < 2 { x - 1.25 } else { x }, 0.0, 0.0]
        } else if root_revision {
            // Previous generic Euler sent mirrored local X to -Y, leaving
            // the particle displaced by -1.5 after target translation.
            [x, -1.5, 0.0]
        } else {
            [
                if nested {
                    points[0].translate[0] + 1.75
                } else {
                    x + 1.125
                },
                0.0,
                0.0,
            ]
        };
        reference.parent_basis = if linear_global {
            [[-2.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.0]]
        } else if dynamic_document {
            [[0.0, 2.0, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]]
        } else if document {
            if coord < 2 {
                xiv_companion::VFX_IDENTITY_BASIS
            } else {
                reference.parent_basis
            }
        } else if root_revision {
            [[0.0, -2.0, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]]
        } else if nested {
            // Old root reconstruction consumed A twice and discarded PICd.
            [[4.0, 0.0, 0.0], [0.0, 1.5, 0.0], [0.0, 0.0, 1.0]]
        } else {
            xiv_companion::VFX_IDENTITY_BASIS
        };
        controls.push(reference);
    }
    for samples in [1, 4] {
        let draw = |label: &str, packets: &Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!(
                    "vfx-point-auxiliary-{}-{label}-{samples}",
                    if missing_goal {
                        "linear-missing-goal"
                    } else if linear && root_revision {
                        "linear-root-revision"
                    } else if linear_global {
                        "linear-global"
                    } else if linear {
                        "linear"
                    } else if dynamic_document {
                        "document-history"
                    } else if document {
                        "document"
                    } else if root_revision {
                        "root-revision"
                    } else if nested {
                        "nested"
                    } else {
                        "playback"
                    }
                ),
                packets.clone(),
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )
        };
        let reference = draw("reference", &references);
        if !dynamic_document {
            assert_image(&draw("continuous", &continuous_packets), &reference);
        }
        assert_image(&draw("staged", &staged_packets), &reference);
        let wrong = draw(
            if missing_goal {
                "phantom-child"
            } else if linear_global {
                "missing-direction"
            } else {
                "missing-auxiliary"
            },
            &controls,
        );
        assert!(
            reference
                .iter()
                .zip(wrong)
                .filter(|(a, b)| a.iter().zip(b).any(|(a, b)| (a - b).abs() > 0.01))
                .count()
                > 10
        );
    }
}

// Feed authored byte leaves through the parser before testing GPU depth states.
fn parsed_depth_flags(depth_test: bool, depth_write: bool, soft: bool) -> (bool, bool, bool) {
    fn block(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = name.to_vec();
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes.resize(bytes.len().next_multiple_of(4), 0);
        bytes
    }
    let leaves = [
        block(b"tDsD", &[u8::from(depth_test)]),
        block(b"wDsD", &[u8::from(depth_write)]),
        block(b"pSsD", &[u8::from(soft)]),
    ]
    .concat();
    let file =
        xiv_companion_data::AvfxFile::parse(&block(b"XFVA", &block(b"lctP", &leaves))).unwrap();
    let p = &file.particles[0];
    (p.depth_test, p.depth_write, p.is_soft_particle)
}

fn mesh(quad: VfxQuad) -> VfxMeshInstance {
    VfxMeshInstance {
        soft_particle: quad.soft_particle,
        soft_particle_fade_range: quad.soft_particle_fade_range,
        depth_offset_type: quad.depth_offset_type,
        depth_offset: quad.depth_offset,
        position: quad.position,
        orientation: quad.orientation,
        parent_basis: quad.parent_basis,
        movement_direction: quad.movement_direction,
        facing_parent_basis: quad.facing_parent_basis,
        rotation_direction_base: quad.rotation_direction_base,
        scale: [quad.size[0], quad.size[1], 1.0],
        color: quad.color,
        fresnel: None,
        draw_layer: quad.draw_layer,
        soft_key_offset: quad.soft_key_offset,
        draw_priority: quad.draw_priority,
        draw_order: quad.draw_order,
        texture_indexes: quad.texture_indexes,
        texture_uv_sets: quad.texture_uv_sets,
        combine_mode_tc1: quad.combine_mode_tc1,
        combine_modes: quad.combine_modes,
        color_to_alpha: quad.color_to_alpha,
        uv_origins: quad.uv_origins,
        uv_scales: quad.uv_scales,
        uv_by_pixel_position: quad.uv_by_pixel_position,
        uv_rotations: quad.uv_rotations,
        texture_borders: quad.texture_borders,
        texture_filters: quad.texture_filters,
        texture_normal_index: -1,
        normal_uv_set: 0,
        normal_uv_origin: [0.0; 2],
        normal_uv_scale: [1.0; 2],
        normal_uv_rotation: 0.0,
        normal_uv_by_pixel_position: false,
        normal_texture_borders: [0; 2],
        normal_texture_filter: 1,
        normal_power: 0.0,
        reflection_enabled: false,
        reflection_use_screen_copy: false,
        reflection_texture_index: -1,
        reflection_texture_filter: 1,
        reflection_calculate_color: 0,
        reflection_rate: 0.0,
        reflection_power: 0.0,
        texture1_is_shape_mask: quad.texture1_is_shape_mask,
        texture1_enabled: quad.texture1_enabled,
        texture1_use_screen_copy: quad.texture1_use_screen_copy,
        draw_mode: quad.draw_mode,
        depth_test: quad.depth_test,
        depth_write: quad.depth_write,
        texture_distortion_index: quad.texture_distortion_index,
        distortion_power: quad.distortion_power,
        distortion_targets: quad.distortion_targets,
        uvd_origin: quad.uvd_origin,
        uvd_scale: quad.uvd_scale,
        uvd_rotation: quad.uvd_rotation,
        uvd_by_pixel_position: quad.uvd_by_pixel_position,
        distortion_uv_set: quad.distortion_uv_set,
        distortion_borders: quad.distortion_borders,
        distortion_filter: quad.distortion_filter,
        texture_palette_index: quad.texture_palette_index,
        palette_offset: quad.palette_offset,
        palette_border: quad.palette_border,
        palette_filter: quad.palette_filter,
        cull_mode: quad.cull_mode,
        model_index: 1,
    }
}

fn square() -> VfxDrawModel {
    VfxDrawModel {
        vertices: [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]
            .map(|[x, y]| VfxDrawVertex {
                position: [x, y, 0.0],
                position_w: 1.0,
                normal: [128, 128, 255, 128],
                tangent: [255, 128, 128, 255],
                uvs: [[x * 0.5, y * 0.5]; 4],
                color: [255; 4],
            })
            .to_vec(),
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

fn reference_quad_geometry() -> VfxDrawModel {
    // AVFXTools ParticleItem.InitQuad: explicit position/UV pairs, facing -Z.
    // Encode the normalized reference UVs in VDrw's centered representation.
    VfxDrawModel {
        vertices: [
            ([-1.0, -1.0, 0.0], [1.0, 0.0]),
            ([-1.0, 1.0, 0.0], [1.0, 1.0]),
            ([1.0, -1.0, 0.0], [0.0, 0.0]),
            ([1.0, 1.0, 0.0], [0.0, 1.0]),
        ]
        .map(|(position, uv)| VfxDrawVertex {
            position,
            position_w: 1.0,
            normal: [128, 128, 0, 128],
            tangent: [255, 128, 128, 255],
            uvs: [uv.map(|value| value - 0.5); 4],
            color: [255; 4],
        })
        .to_vec(),
        indices: vec![2, 0, 1, 3, 2, 1],
    }
}

fn solid(rgba: [u8; 4]) -> Option<VfxTextureInput> {
    Some(VfxTextureInput {
        rgba: rgba.to_vec(),
        width: 1,
        height: 1,
        authored_mips: None,
        rgba16f_mips: None,
        cube_mips: None,
        cube_format: VfxTextureCubeFormat::Rgba8Unorm,
    })
}

fn solid_cube(rgba: [u8; 4]) -> Option<VfxTextureInput> {
    Some(VfxTextureInput {
        rgba: rgba.repeat(4),
        width: 2,
        height: 2,
        authored_mips: Some(vec![
            VfxTextureMipInput {
                rgba: rgba.repeat(4),
                width: 2,
                height: 2,
            },
            VfxTextureMipInput {
                rgba: rgba.to_vec(),
                width: 1,
                height: 1,
            },
        ]),
        rgba16f_mips: None,
        cube_mips: Some(vec![
            VfxTextureCubeMipInput {
                width: 2,
                height: 2,
                faces: std::array::from_fn(|_| rgba.repeat(4)),
            },
            VfxTextureCubeMipInput {
                width: 1,
                height: 1,
                faces: std::array::from_fn(|_| rgba.to_vec()),
            },
        ]),
        cube_format: VfxTextureCubeFormat::Rgba8Unorm,
    })
}

fn hdr_red_pixel(red_half: u16) -> Vec<u8> {
    [red_half, 0, 0, 0x3c00_u16]
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect()
}

fn solid_hdr_texture(red_half: u16) -> Option<VfxTextureInput> {
    let pixel = hdr_red_pixel(red_half);
    Some(VfxTextureInput {
        rgba: [128, 0, 0, 255].repeat(4),
        width: 2,
        height: 2,
        authored_mips: Some(vec![
            VfxTextureMipInput {
                rgba: [128, 0, 0, 255].repeat(4),
                width: 2,
                height: 2,
            },
            VfxTextureMipInput {
                rgba: vec![128, 0, 0, 255],
                width: 1,
                height: 1,
            },
        ]),
        rgba16f_mips: Some(vec![
            VfxTextureMipRgba16fInput {
                rgba16f: pixel.repeat(4),
                width: 2,
                height: 2,
            },
            VfxTextureMipRgba16fInput {
                rgba16f: pixel,
                width: 1,
                height: 1,
            },
        ]),
        cube_mips: None,
        cube_format: VfxTextureCubeFormat::Rgba8Unorm,
    })
}

fn solid_hdr_cube(red_half: u16) -> Option<VfxTextureInput> {
    let pixel = hdr_red_pixel(red_half);
    Some(VfxTextureInput {
        rgba: [128, 0, 0, 255].repeat(4),
        width: 2,
        height: 2,
        authored_mips: Some(vec![
            VfxTextureMipInput {
                rgba: [128, 0, 0, 255].repeat(4),
                width: 2,
                height: 2,
            },
            VfxTextureMipInput {
                rgba: vec![128, 0, 0, 255],
                width: 1,
                height: 1,
            },
        ]),
        rgba16f_mips: Some(vec![
            VfxTextureMipRgba16fInput {
                rgba16f: pixel.repeat(4),
                width: 2,
                height: 2,
            },
            VfxTextureMipRgba16fInput {
                rgba16f: pixel.clone(),
                width: 1,
                height: 1,
            },
        ]),
        cube_mips: Some(vec![
            VfxTextureCubeMipInput {
                width: 2,
                height: 2,
                faces: std::array::from_fn(|_| pixel.repeat(4)),
            },
            VfxTextureCubeMipInput {
                width: 1,
                height: 1,
                faces: std::array::from_fn(|_| pixel.clone()),
            },
        ]),
        cube_format: VfxTextureCubeFormat::Rgba16Float,
    })
}

fn authored_solid_mips(rgba: [u8; 4]) -> Option<VfxTextureInput> {
    let level = |size: u32| xiv_companion::renderer::VfxTextureMipInput {
        rgba: rgba.repeat((size * size) as usize),
        width: size,
        height: size,
    };
    Some(VfxTextureInput {
        rgba: rgba.repeat(16),
        width: 4,
        height: 4,
        authored_mips: Some(vec![level(4), level(2), level(1)]),
        rgba16f_mips: None,
        cube_mips: None,
        cube_format: VfxTextureCubeFormat::Rgba8Unorm,
    })
}

fn black_white_texture() -> Option<VfxTextureInput> {
    Some(VfxTextureInput {
        rgba: vec![0, 0, 0, 255, 255, 255, 255, 255],
        width: 2,
        height: 1,
        authored_mips: None,
        rgba16f_mips: None,
        cube_mips: None,
        cube_format: VfxTextureCubeFormat::Rgba8Unorm,
    })
}

fn render(
    name: &str,
    quads: Vec<VfxQuad>,
    meshes: Vec<VfxMeshInstance>,
    textures: Vec<Option<VfxTextureInput>>,
) -> Vec<[f32; 4]> {
    render_with_geometry(name, quads, meshes, textures, square())
}

fn render_with_geometry(
    name: &str,
    quads: Vec<VfxQuad>,
    meshes: Vec<VfxMeshInstance>,
    textures: Vec<Option<VfxTextureInput>>,
    geometry: VfxDrawModel,
) -> Vec<[f32; 4]> {
    render_with_geometry_and_samples(name, quads, meshes, textures, geometry, 1)
}

fn vfx_semantics_model() -> ModelData {
    ModelData {
        bounds: ModelBounds {
            min: [-1.0; 3],
            max: [1.0; 3],
            center: [0.0; 3],
            radius: 1.0,
        },
        materials: vec![],
        textures: vec![],
        meshes: vec![ModelMesh {
            path: "synthetic".into(),
            part_index: 0,
            mesh_category: None,
            submesh: None,
            shape_influences: vec![],
            shape_targets: vec![],
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic".into(),
            color: [1.0; 3],
            bone_table: None,
            vertices: [-1.0, 0.0, 1.0]
                .map(|v| ModelVertex {
                    // A degenerate line with actual radius 1 keeps the camera at (0,0,3).
                    position: [v, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    blend_weights: None,
                    blend_indices: None,
                    uv0: [0.0; 2],
                    uv1: [0.0; 2],
                    uv2: [0.0; 2],
                    uv3: [0.0; 2],
                    bitangent: [0.0, 1.0, 0.0, 1.0],
                    normal1: None,
                    bitangent1: None,
                    color: [1.0; 4],
                    color1: None,
                    flow0: None,
                    flow1: None,
                })
                .to_vec(),
            indices: vec![0, 1, 2],
        }],
    }
}

fn render_with_geometry_and_samples(
    name: &str,
    quads: Vec<VfxQuad>,
    meshes: Vec<VfxMeshInstance>,
    textures: Vec<Option<VfxTextureInput>>,
    geometry: VfxDrawModel,
    msaa_samples: u32,
) -> Vec<[f32; 4]> {
    render_with_geometry_and_camera(
        name,
        quads,
        meshes,
        textures,
        geometry,
        msaa_samples,
        [0.0; 2],
    )
}

fn render_with_geometry_and_camera(
    name: &str,
    quads: Vec<VfxQuad>,
    meshes: Vec<VfxMeshInstance>,
    textures: Vec<Option<VfxTextureInput>>,
    geometry: VfxDrawModel,
    msaa_samples: u32,
    camera: [f32; 2],
) -> Vec<[f32; 4]> {
    render_with_geometry_camera_and_frames(
        name,
        quads,
        meshes,
        textures,
        geometry,
        msaa_samples,
        camera,
        1,
    )
}

fn render_with_geometry_camera_and_frames(
    name: &str,
    quads: Vec<VfxQuad>,
    meshes: Vec<VfxMeshInstance>,
    textures: Vec<Option<VfxTextureInput>>,
    geometry: VfxDrawModel,
    msaa_samples: u32,
    camera: [f32; 2],
    frames: usize,
) -> Vec<[f32; 4]> {
    render_with_geometry_camera_frames_and_providers(
        name,
        quads,
        meshes,
        textures,
        geometry,
        msaa_samples,
        camera,
        frames,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_with_geometry_camera_frames_and_providers(
    name: &str,
    quads: Vec<VfxQuad>,
    meshes: Vec<VfxMeshInstance>,
    textures: Vec<Option<VfxTextureInput>>,
    geometry: VfxDrawModel,
    msaa_samples: u32,
    camera: [f32; 2],
    frames: usize,
    reflection_cube_color: Option<[u8; 4]>,
    portrait_color: Option<[u8; 4]>,
) -> Vec<[f32; 4]> {
    render_with_geometry_camera_frames_providers_and_roll(
        name,
        quads,
        meshes,
        textures,
        geometry,
        msaa_samples,
        camera,
        frames,
        reflection_cube_color,
        portrait_color,
        0.0,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_with_geometry_camera_frames_providers_and_roll(
    name: &str,
    quads: Vec<VfxQuad>,
    meshes: Vec<VfxMeshInstance>,
    textures: Vec<Option<VfxTextureInput>>,
    geometry: VfxDrawModel,
    msaa_samples: u32,
    camera: [f32; 2],
    frames: usize,
    reflection_cube_color: Option<[u8; 4]>,
    portrait_color: Option<[u8; 4]>,
    camera_roll: f32,
    scene_receiver: Option<VfxQuad>,
) -> Vec<[f32; 4]> {
    let mut model = vfx_semantics_model();
    if let Some(receiver) = scene_receiver {
        let template = model.meshes[0].vertices[0];
        let surface = &mut model.meshes[0];
        surface.color = receiver.color[..3].try_into().unwrap();
        surface.vertices = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]
            .map(|[x, y]| ModelVertex {
                position: [
                    receiver.position[0] + x * receiver.size[0],
                    receiver.position[1] + y * receiver.size[1],
                    receiver.position[2],
                ],
                ..template
            })
            .to_vec();
        surface.indices = vec![0, 1, 2, 0, 2, 3];
    }
    let mut options = WeaponModelSnapshotOptions::new(name)
        .with_viewport(64, 64)
        .with_camera(camera[0], camera[1], 3.0, [0.0; 2])
        .with_render_options(ModelRenderOptions {
            msaa_samples,
            camera_roll,
            ..Default::default()
        })
        .with_vfx_quads(quads)
        .with_vfx_mesh_instances(meshes)
        .with_vfx_meshes([VfxDrawModel::default(), geometry])
        .with_vfx_textures(textures)
        .with_render_repetitions(frames)
        .with_hdr_scene_capture();
    if let Some(rgba) = reflection_cube_color {
        options = options.with_vfx_reflection_cube_color(rgba);
    }
    if let Some(rgba) = portrait_color {
        options = options.with_vfx_portrait_color(rgba);
    }
    let snapshot = render_weapon_model_snapshot_with_options(options, &model)
        .expect("render VFX semantics fixture");
    snapshot.hdr_scene_rgba.expect("capture HDR")
}

// Decal receives model scene depth before the ordinary VFX stages. The first
// quad describes an opaque scene surface; it is not submitted as a VFX particle.
fn render_decal_scene(
    name: &str,
    mut quads: Vec<VfxQuad>,
    meshes: Vec<VfxMeshInstance>,
    textures: Vec<Option<VfxTextureInput>>,
    geometry: VfxDrawModel,
    msaa_samples: u32,
) -> Vec<[f32; 4]> {
    let receiver = quads.remove(0);
    render_with_geometry_camera_frames_providers_and_roll(
        name,
        quads,
        meshes,
        textures,
        geometry,
        msaa_samples,
        [0.0; 2],
        1,
        None,
        None,
        0.0,
        Some(receiver),
    )
}

fn render_center(name: &str, quads: Vec<VfxQuad>, meshes: Vec<VfxMeshInstance>) -> [f32; 4] {
    render(name, quads, meshes, vec![solid([255; 4]), solid([255; 4])])[32 * 64 + 32]
}

fn assert_color(actual: [f32; 4], expected: [f32; 4]) {
    for channel in 0..4 {
        assert!(
            (actual[channel] - expected[channel]).abs() < 0.003,
            "channel {channel}: actual {actual:?}, expected {expected:?}"
        );
    }
}

fn assert_image(actual: &[[f32; 4]], expected: &[[f32; 4]]) {
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            actual.iter().all(|channel| channel.is_finite()),
            "pixel {index}"
        );
        for channel in 0..4 {
            assert!(
                (actual[channel] - expected[channel]).abs() < 0.003,
                "pixel {index}: actual {actual:?}, expected {expected:?}"
            );
        }
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn decal_forward_ddtt_respects_available_scene_receivers() {
    for msaa_samples in [1, 4] {
        let mut receiver = quad([0.1, 0.2, 0.8, 1.0], false);
        receiver.size = [0.8, 0.8];
        receiver.depth_test = true;
        receiver.depth_write = true;

        let mut decal = quad([1.0; 4], false);
        decal.particle_type = Some(xiv_companion_data::avfx::ParticleType::Decal);
        decal.decal = Some(VfxDecal {
            ring: false,
            scaling_scale: 1.0,
            depth_type: 0,
            ring_width: 0.0,
            ring_fan: 0.0,
            scale_z: 2.0,
        });
        decal.size = [1.0, 0.5];
        let half_sqrt = 0.5_f32.sqrt();
        decal.orientation = [half_sqrt, 0.0, 0.0, half_sqrt];
        decal.texture_indexes[0] = 1;

        let textures = vec![solid([255; 4]), solid([255, 0, 0, 255])];
        let render_ddtt = |depth_type| {
            let mut decal = decal;
            decal.decal.as_mut().unwrap().depth_type = depth_type;
            render_decal_scene(
                &format!("vfx-decal-ddtt-{depth_type}-{msaa_samples}x"),
                vec![receiver, decal],
                vec![],
                textures.clone(),
                square(),
                msaa_samples,
            )
        };
        let baseline = render_decal_scene(
            &format!("vfx-decal-ddtt-baseline-{msaa_samples}x"),
            vec![receiver],
            vec![],
            textures.clone(),
            square(),
            msaa_samples,
        );
        let ordinary = render_ddtt(0);
        let stencil_only = render_ddtt(1);
        let both = render_ddtt(2);
        let center = 32 * 64 + 32;
        assert!(
            ordinary[center][0] > baseline[center][0] + 0.25,
            "MSAA={msaa_samples}: ordinary {:?}, baseline {:?}",
            ordinary[center],
            baseline[center]
        );
        // This preview has no water-surface 0x40 stencil producer. DDTT=1
        // selects only that pass, while DDTT=2 also selects the ordinary pass.
        assert_image(&stencil_only, &baseline);
        assert_image(&both, &ordinary);
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn decal_projects_only_onto_scene_depth_for_single_and_multisample_targets() {
    for msaa_samples in [1, 4] {
        let mut receiver = quad([0.1, 0.2, 0.8, 1.0], false);
        receiver.size = [0.8, 0.8];
        receiver.depth_test = true;
        receiver.depth_write = true;

        let mut decal = quad([1.0; 4], false);
        decal.particle_type = Some(xiv_companion_data::avfx::ParticleType::Decal);
        decal.decal = Some(VfxDecal {
            ring: false,
            scaling_scale: 1.0,
            depth_type: 0,
            ring_width: 0.0,
            ring_fan: 0.0,
            scale_z: 2.0,
        });
        decal.size = [1.0, 0.5];
        let half_sqrt = 0.5_f32.sqrt();
        decal.orientation = [half_sqrt, 0.0, 0.0, half_sqrt];
        decal.texture_indexes[0] = 1;
        decal.draw_priority = 1;

        let textures = vec![solid([255; 4]), solid([255, 0, 0, 255])];
        let baseline = render_decal_scene(
            &format!("vfx-decal-depth-baseline-{msaa_samples}"),
            vec![receiver],
            vec![],
            textures.clone(),
            square(),
            msaa_samples,
        );
        let actual = render_decal_scene(
            &format!("vfx-decal-depth-{msaa_samples}"),
            vec![receiver, decal],
            vec![],
            textures,
            square(),
            msaa_samples,
        );
        let center = 32 * 64 + 32;
        assert!(
            actual[center][0] > baseline[center][0] + 0.25,
            "MSAA={msaa_samples}: {:?} vs {:?}",
            actual[center],
            baseline[center]
        );
        for corner in [0, 63, 63 * 64, 64 * 64 - 1] {
            for channel in 0..4 {
                assert!(
                    (actual[corner][channel] - baseline[corner][channel]).abs() < 0.003,
                    "MSAA={msaa_samples}, corner={corner}: {:?} vs {:?}",
                    actual[corner],
                    baseline[corner]
                );
            }
        }

        let mut ring = decal;
        ring.particle_type = Some(xiv_companion_data::avfx::ParticleType::DecalRing);
        ring.decal = Some(VfxDecal {
            ring: true,
            scaling_scale: 1.0,
            depth_type: 0,
            ring_width: 0.1,
            ring_fan: 1.0,
            scale_z: 2.0,
        });
        let ring_image = render_decal_scene(
            &format!("vfx-decal-ring-depth-{msaa_samples}"),
            vec![receiver, ring],
            vec![],
            vec![solid([255; 4]), solid([255, 0, 0, 255])],
            square(),
            msaa_samples,
        );
        for channel in 0..4 {
            assert!(
                (ring_image[center][channel] - baseline[center][channel]).abs() < 0.003,
                "ring center, MSAA={msaa_samples}: {:?} vs {:?}",
                ring_image[center],
                baseline[center]
            );
        }
        let changed = ring_image
            .iter()
            .zip(&baseline)
            .filter(|(actual, expected)| actual[0] > expected[0] + 0.1)
            .count();
        assert!(changed > 20, "MSAA={msaa_samples}: {changed} ring pixels");
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn decal_does_not_sample_later_soft_particle_depth() {
    for msaa_samples in [1, 4] {
        let mut regular = quad([0.0, 0.0, 1.0, 0.5], false);
        regular.draw_priority = -2;

        let mut soft = quad([0.0, 1.0, 0.0, 1.0], false);
        soft.draw_priority = -1;
        soft.soft_particle = true;
        soft.soft_particle_fade_range = 0.01;
        soft.depth_test = true;
        soft.depth_write = true;

        let mut decal = quad([1.0; 4], false);
        decal.particle_type = Some(xiv_companion_data::avfx::ParticleType::Decal);
        decal.decal = Some(VfxDecal {
            ring: false,
            scaling_scale: 1.0,
            depth_type: 0,
            ring_width: 0.0,
            ring_fan: 0.0,
            scale_z: 2.0,
        });
        decal.size = [1.0, 0.5];
        let half_sqrt = 0.5_f32.sqrt();
        decal.orientation = [half_sqrt, 0.0, 0.0, half_sqrt];
        decal.texture_indexes[0] = 1;
        decal.draw_priority = 1;

        let textures = vec![solid([255; 4]), solid([255, 0, 0, 255])];
        let baseline = render_with_geometry_and_samples(
            &format!("vfx-decal-soft-baseline-{msaa_samples}"),
            vec![regular, soft],
            vec![],
            textures.clone(),
            square(),
            msaa_samples,
        );
        let actual = render_with_geometry_and_samples(
            &format!("vfx-decal-soft-{msaa_samples}"),
            vec![regular, soft, decal],
            vec![],
            textures,
            square(),
            msaa_samples,
        );
        let center = 32 * 64 + 32;
        assert!(
            baseline[center][1] > 0.9,
            "MSAA={msaa_samples}: {:?}",
            baseline[center]
        );
        for (actual, baseline) in actual.iter().zip(&baseline) {
            assert_color(*actual, *baseline);
        }
        for corner in [0, 63, 63 * 64, 64 * 64 - 1] {
            assert_color(actual[corner], baseline[corner]);
        }
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn decal_scene_stage_precedes_regular_and_soft_particles() {
    for msaa_samples in [1, 4] {
        let mut receiver = quad([0.0, 0.0, 1.0, 1.0], false);
        receiver.draw_priority = -2;
        receiver.depth_test = true;
        receiver.depth_write = true;

        let mut decal = quad([1.0; 4], false);
        decal.particle_type = Some(xiv_companion_data::avfx::ParticleType::Decal);
        decal.decal = Some(VfxDecal {
            ring: false,
            scaling_scale: 1.0,
            depth_type: 0,
            ring_width: 0.0,
            ring_fan: 0.0,
            scale_z: 2.0,
        });
        decal.size = [1.0, 0.5];
        let half_sqrt = 0.5_f32.sqrt();
        decal.orientation = [half_sqrt, 0.0, 0.0, half_sqrt];
        decal.texture_indexes[0] = 1;
        decal.draw_priority = -1;

        let mut regular = quad([0.0, 1.0, 0.0, 1.0], false);
        regular.draw_priority = 1;
        let textures = vec![solid([255; 4]), solid([255, 0, 0, 255])];
        let projected = render_decal_scene(
            &format!("vfx-decal-priority-control-{msaa_samples}"),
            vec![receiver, decal],
            vec![],
            textures.clone(),
            square(),
            msaa_samples,
        );
        assert!(
            projected[32 * 64 + 32][0] > 0.9,
            "MSAA={msaa_samples}: {:?}",
            projected[32 * 64 + 32]
        );

        let actual = render_decal_scene(
            &format!("vfx-decal-priority-regular-last-{msaa_samples}"),
            vec![receiver, decal, regular],
            vec![],
            textures.clone(),
            square(),
            msaa_samples,
        );
        assert_color(actual[32 * 64 + 32], [0.0, 1.0, 0.0, 1.0]);

        let mut soft = regular;
        soft.position[2] = 0.1;
        soft.soft_particle = true;
        soft.soft_particle_fade_range = 0.01;
        soft.depth_test = true;
        soft.depth_write = true;
        let actual = render_decal_scene(
            &format!("vfx-decal-priority-soft-last-{msaa_samples}"),
            vec![receiver, decal, soft],
            vec![],
            textures.clone(),
            square(),
            msaa_samples,
        );
        assert_color(actual[32 * 64 + 32], [0.0, 1.0, 0.0, 1.0]);

        regular.draw_priority = -1;
        decal.draw_priority = 1;
        let actual = render_decal_scene(
            &format!("vfx-decal-priority-decal-last-{msaa_samples}"),
            vec![receiver, regular, decal],
            vec![],
            textures.clone(),
            square(),
            msaa_samples,
        );
        assert_color(actual[32 * 64 + 32], [0.0, 1.0, 0.0, 1.0]);

        regular.draw_priority = 0;
        decal.draw_priority = -1;
        let mut later_decal = decal;
        later_decal.draw_priority = 1;
        later_decal.color[3] = 0.5;
        later_decal.texture_indexes[0] = 2;
        let actual = render_decal_scene(
            &format!("vfx-decal-priority-alternating-{msaa_samples}"),
            vec![receiver, decal, regular, later_decal],
            vec![],
            [textures, vec![solid([0, 0, 255, 255])]].concat(),
            square(),
            msaa_samples,
        );
        let center = actual[32 * 64 + 32];
        assert_color(center, [0.0, 1.0, 0.0, 1.0]);
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn decal_and_ring_half_alpha_compose_once() {
    for msaa_samples in [1, 4] {
        let mut receiver = quad([0.0, 0.0, 1.0, 1.0], false);
        receiver.depth_test = true;
        receiver.depth_write = true;
        let textures = vec![solid([255; 4]), solid([255, 0, 0, 255])];
        let baseline = render_decal_scene(
            &format!("vfx-decal-alpha-baseline-{msaa_samples}"),
            vec![receiver],
            vec![],
            textures.clone(),
            square(),
            msaa_samples,
        );

        for ring in [false, true] {
            let mut decal = quad([1.0; 4], false);
            decal.particle_type = Some(if ring {
                xiv_companion_data::avfx::ParticleType::DecalRing
            } else {
                xiv_companion_data::avfx::ParticleType::Decal
            });
            decal.decal = Some(VfxDecal {
                ring,
                scaling_scale: 1.0,
                depth_type: 0,
                ring_width: 0.1,
                ring_fan: 1.0,
                scale_z: 2.0,
            });
            decal.size = [1.0, 0.5];
            let half_sqrt = 0.5_f32.sqrt();
            decal.orientation = [half_sqrt, 0.0, 0.0, half_sqrt];
            decal.texture_indexes[0] = 1;
            decal.draw_priority = 1;
            let label = if ring { "ring" } else { "decal" };

            let opaque = render_decal_scene(
                &format!("vfx-{label}-alpha-opaque-{msaa_samples}"),
                vec![receiver, decal],
                vec![],
                textures.clone(),
                square(),
                msaa_samples,
            );
            decal.color[3] = 0.5;
            let half = render_decal_scene(
                &format!("vfx-{label}-alpha-half-{msaa_samples}"),
                vec![receiver, decal],
                vec![],
                textures.clone(),
                square(),
                msaa_samples,
            );
            let index = opaque
                .iter()
                .zip(&baseline)
                .position(|(projected, background)| projected[0] > background[0] + 0.25)
                .unwrap_or_else(|| {
                    panic!("{label}, MSAA={msaa_samples}: no fully projected pixel")
                });
            assert_color(
                half[index],
                std::array::from_fn(|c| (opaque[index][c] + baseline[index][c]) * 0.5),
            );
        }
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn same_priority_quad_mesh_quad_preserves_sampled_order() {
    for samples in [1, 4] {
        let background = render_with_geometry_and_samples(
            &format!("vfx-same-priority-background-{samples}"),
            vec![],
            vec![],
            vec![solid([255; 4])],
            square(),
            samples,
        )[32 * 64 + 32];
        let mut red = quad([1.0, 0.0, 0.0, 0.5], false);
        red.draw_order = Some(1);
        let mut green = quad([0.0, 1.0, 0.0, 0.5], false);
        green.draw_order = Some(2);
        let mut blue = quad([0.0, 0.0, 1.0, 0.5], false);
        blue.draw_order = Some(3);

        for screen_copy in [false, true] {
            let mut quads = vec![red, blue];
            if screen_copy {
                let mut offscreen = quad([0.0; 4], false);
                offscreen.position = [100.0; 3];
                offscreen.texture_indexes[0] = -2;
                offscreen.draw_order = Some(4);
                quads.push(offscreen);
            }
            let actual = render_with_geometry_and_samples(
                &format!("vfx-same-priority-quad-mesh-quad-{samples}-{screen_copy}"),
                quads,
                vec![mesh(green)],
                vec![solid([255; 4])],
                square(),
                samples,
            )[32 * 64 + 32];
            assert_color(
                actual,
                [
                    0.125 + 0.125 * background[0],
                    0.25 + 0.125 * background[1],
                    0.5 + 0.125 * background[2],
                    1.0,
                ],
            );
        }
    }
}

fn draw_order_render_fixture(mode: i32) -> xiv_companion_data::avfx::AvfxFile {
    use xiv_companion_data::avfx::*;
    let mut file = cone_model_render_fixture();
    file.global.draw_order = mode;
    file.timelines[0].binder_index = -1;
    file.emitters[0] = AvfxEmitter {
        emitter_type: Some(EmitterType::Point),
        effector_index: -1,
        particle_items: (0..3)
            .map(|target_index| AvfxEmitterItem {
                enabled: true,
                target_index,
                create_time: 1,
                create_count: 1,
                create_probability: 100,
                parameter_link: -1,
                parent_influence_coord: 2,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let curve = |x, y, z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            interpolation: 1,
            x,
            y,
            z,
        }],
        ..Default::default()
    };
    let original = file.particles[0].clone();
    file.particles = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        .into_iter()
        .enumerate()
        .map(|(index, rgb)| {
            let mut particle = original.clone();
            particle.collision_type = -1;
            particle.particle_type = Some(if index == 1 {
                ParticleType::LightModel
            } else {
                ParticleType::Quad
            });
            particle.data = if index == 1 {
                AvfxParticleData::LightModel { model_index: 1 }
            } else {
                AvfxParticleData::None
            };
            particle.scale = AvfxCurve3Axis {
                x: Some(curve(0.0, 0.0, 1.0)),
                y: Some(curve(0.0, 0.0, 1.0)),
                z: Some(curve(0.0, 0.0, 1.0)),
                ..Default::default()
            };
            particle.color = AvfxColorCurve {
                rgb: Some(curve(rgb[0], rgb[1], rgb[2])),
                alpha: Some(curve(0.0, 0.0, 0.5)),
                ..Default::default()
            };
            particle
        })
        .collect();
    file
}

#[test]
#[ignore = "requires native wgpu"]
fn reverse_draw_order_reaches_continuous_and_staged_pixels() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime};
    let file = draw_order_render_fixture(1);
    for staged in [false, true] {
        let runtime = VfxRuntime::new(&file);
        let (mut quads, mut meshes) = (Vec::new(), Vec::new());
        if staged {
            let playback = VfxPlayback::new(runtime);
            assert_eq!(playback.fallback_reason(), None);
            playback.sample(&mut quads, &mut meshes);
        } else {
            runtime.sample(0.0, &mut quads);
            runtime.sample_mesh(0.0, &mut meshes);
        }
        assert_eq!((quads.len(), meshes.len()), (2, 1));
        for samples in [1, 4] {
            let draw = |name: &str, quads, meshes| {
                render_with_geometry_and_samples(
                    &format!("vfx-reverse-{name}-{staged}-{samples}"),
                    quads,
                    meshes,
                    vec![solid([255; 4])],
                    square(),
                    samples,
                )[32 * 64 + 32]
            };
            let actual = draw("actual", quads.clone(), meshes.clone());
            let mut expected_quads = quads.clone();
            for quad in &mut expected_quads {
                quad.draw_order = Some(if quad.color[0] > quad.color[2] { 2 } else { 0 });
            }
            expected_quads.sort_by_key(|quad| quad.draw_order);
            let mut expected_meshes = meshes.clone();
            expected_meshes[0].draw_order = Some(1);
            assert_color(
                actual,
                draw(
                    "explicit-reverse",
                    expected_quads.clone(),
                    expected_meshes.clone(),
                ),
            );
            for quad in &mut expected_quads {
                quad.draw_order = quad.draw_order.map(|order| 2 - order);
            }
            expected_quads.sort_by_key(|quad| quad.draw_order);
            let forward = draw("forward-control", expected_quads, expected_meshes);
            assert!(
                (actual[0] - forward[0]).abs() > 0.1,
                "staged={staged}, samples={samples}: {actual:?} vs {forward:?}"
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn registered_depth_keys_reach_staged_pixels() {
    use xiv_companion_data::avfx_sim::depth_sort_key;
    use xiv_companion_data::{VfxPlayback, VfxRuntime};
    let file = draw_order_render_fixture(2);
    let playback = VfxPlayback::new(VfxRuntime::new(&file));
    assert_eq!(playback.fallback_reason(), None);
    let (mut quads, mut meshes) = (Vec::new(), Vec::new());
    // Synthetic registration snapshots deliberately differ from draw positions.
    // Blue, red, green differs from both default and Reverse order.
    playback
        .sample_with_depth_keys(
            |_, definition| {
                Some(depth_sort_key(
                    [0.0, 0.0, [0.0, 1.0, -1.0][definition]],
                    [0.0, 0.0, 1.0, 0.0],
                    definition as u32,
                ))
            },
            &mut quads,
            &mut meshes,
        )
        .unwrap();
    assert_eq!((quads.len(), meshes.len()), (2, 1));
    assert_eq!(meshes[0].draw_order, Some(2));
    for quad in &quads {
        assert_eq!(
            quad.draw_order,
            Some(if quad.color[0] > quad.color[2] { 1 } else { 0 })
        );
    }
    for samples in [1, 4] {
        let draw = |name: &str, quads, meshes| {
            render_with_geometry_and_samples(
                &format!("vfx-registered-depth-{name}-{samples}"),
                quads,
                meshes,
                vec![solid([255; 4])],
                square(),
                samples,
            )[32 * 64 + 32]
        };
        let actual = draw("actual", quads.clone(), meshes.clone());
        let mut expected_quads = quads.clone();
        for quad in &mut expected_quads {
            quad.draw_order = Some(if quad.color[0] > quad.color[2] { 1 } else { 0 });
        }
        expected_quads.sort_by_key(|quad| quad.draw_order);
        let mut expected_meshes = meshes.clone();
        expected_meshes[0].draw_order = Some(2);
        assert_color(
            actual,
            draw("explicit", expected_quads.clone(), expected_meshes.clone()),
        );
        for quad in &mut expected_quads {
            quad.draw_order = Some(if quad.color[0] > quad.color[2] { 0 } else { 2 });
        }
        expected_quads.sort_by_key(|quad| quad.draw_order);
        expected_meshes[0].draw_order = Some(1);
        let forward = draw("default-control", expected_quads, expected_meshes);
        assert!(
            (actual[2] - forward[2]).abs() > 0.1,
            "samples={samples}: {actual:?} vs {forward:?}"
        );
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn same_priority_regular_special_regular_preserves_sampled_order() {
    for samples in [1, 4] {
        let mut receiver = quad([0.0, 0.0, 0.0, 1.0], false);
        receiver.draw_priority = -1;
        receiver.depth_test = true;
        receiver.depth_write = true;

        let mut green = quad([0.0, 1.0, 0.0, 0.5], false);
        green.draw_order = Some(1);
        let mut blue = quad([0.0, 0.0, 1.0, 0.5], false);
        blue.draw_order = Some(3);

        for decal in [false, true] {
            let mut red = quad([1.0, 0.0, 0.0, 0.5], false);
            red.draw_order = Some(2);
            if decal {
                red.particle_type = Some(xiv_companion_data::avfx::ParticleType::Decal);
                red.decal = Some(VfxDecal {
                    ring: false,
                    scaling_scale: 1.0,
                    depth_type: 0,
                    ring_width: 0.0,
                    ring_fan: 0.0,
                    scale_z: 2.0,
                });
                red.size = [1.0, 0.5];
                let half_sqrt = 0.5_f32.sqrt();
                red.orientation = [half_sqrt, 0.0, 0.0, half_sqrt];
            } else {
                red.position[2] = 0.1;
                red.soft_particle = true;
                red.soft_particle_fade_range = 0.01;
                red.depth_test = true;
                red.depth_write = true;
            }
            let render = if decal {
                render_decal_scene
            } else {
                render_with_geometry_and_samples
            };
            let actual = render(
                &format!("vfx-same-priority-regular-special-regular-{samples}-{decal}"),
                vec![receiver, green, blue, red],
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )[32 * 64 + 32];
            assert_color(
                actual,
                if decal {
                    [0.125, 0.25, 0.5, 1.0]
                } else {
                    [0.25, 0.125, 0.5, 1.0]
                },
            );
        }
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn decal_applies_palette_and_distortion_for_single_and_multisample_targets() {
    let palette = Some(VfxTextureInput {
        rgba: [0_u8, 85, 170, 255]
            .into_iter()
            .flat_map(|red| [red, 0, 0, 255])
            .collect(),
        width: 4,
        height: 1,
        authored_mips: None,
        rgba16f_mips: None,
        cube_mips: None,
        cube_format: VfxTextureCubeFormat::Rgba8Unorm,
    });
    let mut gradient = Vec::new();
    for x in 0..16u8 {
        gradient.extend_from_slice(&[x * 17, 255 - x * 17, 32, 255]);
    }
    let gradient = Some(VfxTextureInput {
        rgba: gradient,
        width: 16,
        height: 1,
        authored_mips: None,
        rgba16f_mips: None,
        cube_mips: None,
        cube_format: VfxTextureCubeFormat::Rgba8Unorm,
    });

    for msaa_samples in [1, 4] {
        let mut receiver = quad([0.1, 0.2, 0.8, 1.0], false);
        receiver.size = [0.8, 0.8];
        receiver.depth_test = true;
        receiver.depth_write = true;

        let mut decal = quad([1.0; 4], false);
        decal.particle_type = Some(xiv_companion_data::avfx::ParticleType::Decal);
        decal.decal = Some(VfxDecal {
            ring: false,
            scaling_scale: 1.0,
            depth_type: 0,
            ring_width: 0.0,
            ring_fan: 0.0,
            scale_z: 2.0,
        });
        decal.size = [1.0, 0.5];
        let half_sqrt = 0.5_f32.sqrt();
        decal.orientation = [half_sqrt, 0.0, 0.0, half_sqrt];
        decal.draw_priority = 1;
        decal.texture_indexes[0] = 1;

        let palette_control = render_decal_scene(
            &format!("vfx-decal-palette-control-{msaa_samples}"),
            vec![receiver, decal],
            vec![],
            vec![solid([255; 4]), solid([26, 89, 153, 217]), palette.clone()],
            square(),
            msaa_samples,
        );
        decal.texture_palette_index = 2;
        decal.palette_offset = 26.0 / 255.0;
        decal.palette_border = 1;
        decal.palette_filter = 0;
        let palette_actual = render_decal_scene(
            &format!("vfx-decal-palette-{msaa_samples}"),
            vec![receiver, decal],
            vec![],
            vec![solid([255; 4]), solid([26, 89, 153, 217]), palette.clone()],
            square(),
            msaa_samples,
        );
        let center = 32 * 64 + 32;
        assert!(
            palette_actual[center]
                .iter()
                .zip(palette_control[center])
                .any(|(actual, control)| (actual - control).abs() > 0.1),
            "palette, MSAA={msaa_samples}: {:?} vs {:?}",
            palette_actual[center],
            palette_control[center]
        );

        decal.texture_palette_index = -1;
        decal.texture_indexes[0] = 1;
        decal.texture_filters[0] = 0;
        decal.texture_distortion_index = 2;
        decal.distortion_power = 0.5;
        decal.distortion_targets = 0;
        let distortion_control = render_decal_scene(
            &format!("vfx-decal-distortion-control-{msaa_samples}"),
            vec![receiver, decal],
            vec![],
            vec![solid([255; 4]), gradient.clone(), solid([255, 128, 0, 255])],
            square(),
            msaa_samples,
        );
        decal.distortion_targets = 1;
        let distortion_actual = render_decal_scene(
            &format!("vfx-decal-distortion-{msaa_samples}"),
            vec![receiver, decal],
            vec![],
            vec![solid([255; 4]), gradient.clone(), solid([255, 128, 0, 255])],
            square(),
            msaa_samples,
        );
        assert!(
            distortion_actual[center]
                .iter()
                .zip(distortion_control[center])
                .any(|(actual, control)| (actual - control).abs() > 0.1),
            "distortion, MSAA={msaa_samples}: {:?} vs {:?}",
            distortion_actual[center],
            distortion_control[center]
        );
    }
}

fn cosine3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dot: f32 = a.into_iter().zip(b).map(|(x, y)| x * y).sum();
    let aa: f32 = a.into_iter().map(|v| v * v).sum();
    let bb: f32 = b.into_iter().map(|v| v * v).sum();
    dot / (aa * bb).sqrt()
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn client_model_homogeneous_positions_reach_projection() {
    for w in [0.5, 2.0] {
        let mut geometry = square();
        for vertex in &mut geometry.vertices {
            vertex.position_w = w;
        }
        let mut instance = mesh(quad([0.4, 0.6, 0.8, 1.0], false));
        instance.position = [0.2, -0.1, 0.25];
        let actual = render_with_geometry(
            "vfx-homogeneous",
            vec![],
            vec![instance],
            vec![solid([255; 4])],
            geometry,
        );
        let mut reference = square();
        for vertex in &mut reference.vertices {
            vertex.position = vertex.position.map(|value| value / w);
        }
        let expected = render_with_geometry(
            "vfx-homogeneous-reference",
            vec![],
            vec![instance],
            vec![solid([255; 4])],
            reference,
        );
        assert_image(&actual, &expected);
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn client_model_fresnel_normalizes_interpolated_normals_and_view() {
    let particle_color = [0.5, 0.75, 0.6, 1.0];
    let mut instance = mesh(quad(particle_color, false));
    let begin = [1.0, 0.2, 0.1, 1.0];
    let end = [0.1, 0.3, 1.0, 1.0];
    instance.fresnel = Some(xiv_companion::VfxFresnel {
        kind: 1,
        direction: [100.0, 0.0, 0.0], // Camera mode must ignore the fixed axis.
        exponent: 2.0,
        color_begin: begin,
        color_end: end,
    });
    for reverse in [false, true] {
        let mut geometry = square();
        for vertex in &mut geometry.vertices {
            vertex.normal = [
                if vertex.position[0] < 0.0 { 0 } else { 255 },
                128,
                255,
                128,
            ];
            if reverse {
                vertex.normal = vertex.normal.map(|value| 255 - value);
            }
            vertex.color = [160, 200, 240, 255];
        }
        let image = render_with_geometry(
            "vfx-fresnel-camera",
            vec![],
            vec![instance],
            vec![solid([255; 4])],
            geometry,
        );
        for y in [20, 32, 43] {
            for x in [20, 32, 43] {
                let half_span = 3.0 * (22.5_f32.to_radians()).tan();
                let wx = ((x as f32 + 0.5) / 32.0 - 1.0) * half_span;
                let wy = (1.0 - (y as f32 + 0.5) / 32.0) * half_span;
                let normal = [wx * 0.5, 128.0 / 255.0 - 0.5, 0.5];
                let factor = cosine3(normal, [-wx, -wy, 3.0]).abs().max(1e-5).powi(2);
                let expected = std::array::from_fn(|i| {
                    (begin[i] + factor * (end[i] - begin[i]))
                        * particle_color[i]
                        * [160.0, 200.0, 240.0, 255.0][i]
                        / 255.0
                });
                assert_color(image[y * 64 + x], expected);
            }
        }
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn client_model_fresnel_axes_use_linear_normal_transform() {
    let mut instance = mesh(quad([0.4, 0.6, 0.8, 1.0], false));
    instance.scale = [1.2, 0.8, -0.6];
    instance.parent_basis = [[1.0, 0.0, 0.0], [0.5, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let transform = |[x, y, z]: [f32; 3]| [1.2 * x + 0.4 * y, 0.8 * y, -0.6 * z];
    let normal_bytes = [224, 160, 255, 128];
    let mut geometry = square();
    for vertex in &mut geometry.vertices {
        vertex.normal = normal_bytes;
    }
    let normal = transform([224.0, 160.0, 255.0].map(|v| v / 255.0 - 0.5));
    for kind in [2, 3] {
        let mut direction = [0.2, -0.8, 0.5];
        instance.fresnel = Some(xiv_companion::VfxFresnel {
            kind,
            direction,
            exponent: 3.0,
            color_begin: [0.0, 0.2, 0.8, 0.25],
            color_end: [1.0, 0.8, 0.1, 0.75],
        });
        if kind == 3 {
            direction = transform(direction);
        }
        let factor = cosine3(normal, direction).abs().max(1e-5).powi(3);
        let f = instance.fresnel.unwrap();
        let mut reference = instance;
        reference.fresnel = None;
        reference.color = std::array::from_fn(|i| {
            instance.color[i] * (f.color_begin[i] + factor * (f.color_end[i] - f.color_begin[i]))
        });
        let actual = render_with_geometry(
            "vfx-fresnel-axis",
            vec![],
            vec![instance],
            vec![solid([255; 4])],
            geometry.clone(),
        );
        let expected = render_with_geometry(
            "vfx-fresnel-axis-reference",
            vec![],
            vec![reference],
            vec![solid([255; 4])],
            geometry.clone(),
        );
        assert_image(&actual, &expected);
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn model_normal_texture_uses_client_tangent_frame_and_npow() {
    for msaa_samples in [1, 4] {
        let mut reference = mesh(quad([0.8, 0.6, 0.4, 1.0], false));
        reference.fresnel = Some(xiv_companion::VfxFresnel {
            kind: 2,
            direction: [0.0, 0.0, 1.0],
            exponent: 1.0,
            color_begin: [0.0, 0.0, 0.0, 1.0],
            color_end: [1.0; 4],
        });
        let mut mapped = reference;
        mapped.texture_normal_index = 1;
        mapped.normal_texture_filter = 0;
        mapped.normal_power = 1.0;
        let mut zero_power = mapped;
        zero_power.normal_power = 0.0;
        let textures = vec![solid([255; 4]), solid([255, 128, 128, 255])];

        let baseline = render_with_geometry_and_samples(
            "vfx-normal-map-reference",
            vec![],
            vec![reference],
            textures.clone(),
            square(),
            msaa_samples,
        );
        let zero = render_with_geometry_and_samples(
            "vfx-normal-map-zero-power",
            vec![],
            vec![zero_power],
            textures.clone(),
            square(),
            msaa_samples,
        );
        assert_image(&zero, &baseline);

        let actual = render_with_geometry_and_samples(
            "vfx-normal-map-tangent",
            vec![],
            vec![mapped],
            textures,
            square(),
            msaa_samples,
        );
        let center = actual[32 * 64 + 32];
        let baseline_center = baseline[32 * 64 + 32];
        assert!(baseline_center[0] > 0.75, "{baseline_center:?}");
        assert!(center[0] < 0.05, "{center:?}");
        assert_eq!(center[3], 1.0);
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn model_reflection_uses_rate_power_and_calculate_color_mode() {
    let base = mesh(quad([0.8, 0.6, 0.4, 1.0], false));
    let mut reflected = base;
    reflected.reflection_enabled = true;
    reflected.reflection_rate = 1.0;
    reflected.reflection_power = 1.0;
    reflected.reflection_calculate_color = 0;
    let textures = vec![solid([255; 4])];
    for msaa_samples in [1, 4] {
        let baseline = render_with_geometry_and_samples(
            "vfx-reflection-reference",
            vec![],
            vec![base],
            textures.clone(),
            square(),
            msaa_samples,
        );
        let actual = render_with_geometry_and_samples(
            "vfx-reflection-multiply",
            vec![],
            vec![reflected],
            textures.clone(),
            square(),
            msaa_samples,
        );
        let a = actual[32 * 64 + 32];
        let b = baseline[32 * 64 + 32];
        assert!(a[0] < b[0] && a[1] < b[1] && a[2] < b[2], "{a:?} vs {b:?}");

        reflected.reflection_calculate_color = 1;
        let add = render_with_geometry_and_samples(
            "vfx-reflection-add",
            vec![],
            vec![reflected],
            textures.clone(),
            square(),
            msaa_samples,
        );
        let add_center = add[32 * 64 + 32];
        assert!(
            add_center[0] > b[0] && add_center[1] > b[1],
            "{add_center:?} vs {b:?}"
        );

        reflected.reflection_calculate_color = 4;
        let min = render_with_geometry_and_samples(
            "vfx-reflection-min",
            vec![],
            vec![reflected],
            textures.clone(),
            square(),
            msaa_samples,
        );
        let min_center = min[32 * 64 + 32];
        assert!(
            min_center[0] < b[0] && min_center[1] < b[1],
            "{min_center:?} vs {b:?}"
        );

        reflected.reflection_calculate_color = 5;
        let unknown = render_with_geometry_and_samples(
            "vfx-reflection-unknown",
            vec![],
            vec![reflected],
            textures.clone(),
            square(),
            msaa_samples,
        );
        assert_eq!(unknown[32 * 64 + 32], b);

        // Client packs TCCT with the low three bits; 13 (0b1101) therefore
        // follows the Unknown/no-op branch just like the raw value 5.
        reflected.reflection_calculate_color = 13;
        let unknown_high_bits = render_with_geometry_and_samples(
            "vfx-reflection-unknown-high-bits",
            vec![],
            vec![reflected],
            textures.clone(),
            square(),
            msaa_samples,
        );
        assert_eq!(unknown_high_bits[32 * 64 + 32], b);
        reflected.reflection_calculate_color = 0;

        let red_provider = render_with_geometry_camera_frames_and_providers(
            "vfx-reflection-provider-red",
            vec![],
            vec![reflected],
            textures.clone(),
            square(),
            msaa_samples,
            [0.0; 2],
            1,
            Some([255, 0, 0, 255]),
            None,
        );
        let blue_provider = render_with_geometry_camera_frames_and_providers(
            "vfx-reflection-provider-blue",
            vec![],
            vec![reflected],
            textures.clone(),
            square(),
            msaa_samples,
            [0.0; 2],
            1,
            Some([0, 0, 255, 255]),
            None,
        );
        let red = red_provider[32 * 64 + 32];
        let blue = blue_provider[32 * 64 + 32];
        assert!(red[0] > blue[0] && red[2] < blue[2], "{red:?} vs {blue:?}");
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn model_reflection_selects_authored_cube_by_txno_without_group_aliasing() {
    let mut red = mesh(quad([0.2, 0.2, 0.2, 1.0], false));
    red.texture_indexes = [1, -1, -1, -1];
    red.reflection_enabled = true;
    red.reflection_rate = 0.5;
    red.reflection_power = 1.0;
    red.reflection_calculate_color = 1;
    red.reflection_texture_index = 0;
    let mut blue = red;
    blue.reflection_texture_index = 2;
    let textures = vec![
        solid_cube([255, 0, 0, 255]),
        solid([255; 4]),
        solid_cube([0, 0, 255, 255]),
    ];

    for msaa_samples in [1, 4] {
        let render_meshes = |name, meshes| {
            render_with_geometry_and_samples(
                name,
                vec![],
                meshes,
                textures.clone(),
                square(),
                msaa_samples,
            )
        };
        let red_image = render_meshes("vfx-file-cube-red", vec![red]);
        let blue_image = render_meshes("vfx-file-cube-blue", vec![blue]);
        let both_image = render_meshes("vfx-file-cube-adjacent", vec![red, blue]);
        let r = red_image[32 * 64 + 32];
        let b = blue_image[32 * 64 + 32];
        assert!(r[0] > b[0] && r[2] < b[2], "{r:?} vs {b:?}");
        assert_color(both_image[32 * 64 + 32], b);

        let mut non_cube = red;
        non_cube.reflection_texture_index = 1;
        let mut missing = red;
        missing.reflection_texture_index = -1;
        assert_image(
            &render_meshes("vfx-file-cube-noncube-fallback", vec![non_cube]),
            &render_meshes("vfx-file-cube-missing-fallback", vec![missing]),
        );

        let mut screen_red = red;
        let mut screen_blue = blue;
        screen_red.reflection_use_screen_copy = true;
        screen_blue.reflection_use_screen_copy = true;
        assert_image(
            &render_meshes("vfx-file-cube-screen-red", vec![screen_red]),
            &render_meshes("vfx-file-cube-screen-blue", vec![screen_blue]),
        );
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn model_reflection_preserves_hdr_half_float_cube_values() {
    let mut model = mesh(quad([0.08, 0.08, 0.08, 1.0], false));
    model.texture_indexes = [1, -1, -1, -1];
    model.reflection_enabled = true;
    model.reflection_rate = 0.2;
    model.reflection_power = 1.0;
    model.reflection_calculate_color = 1;
    model.reflection_texture_index = 0;

    for msaa_samples in [1, 4] {
        let render = |name, red_half| {
            render_with_geometry_and_samples(
                name,
                vec![],
                vec![model],
                vec![solid_hdr_cube(red_half), solid([255; 4])],
                square(),
                msaa_samples,
            )
        };
        let dim = render("vfx-hdr-cube-half", 0x3800)[32 * 64 + 32];
        let bright = render("vfx-hdr-cube-two", 0x4000)[32 * 64 + 32];
        assert!(
            bright[0] > dim[0] + 0.02,
            "MSAA {msaa_samples} lost half-float HDR reflection: {dim:?} vs {bright:?}"
        );
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn tc1_bc6h_2d_and_cube_face_zero_preserve_hdr_values() {
    for msaa_samples in [1, 4] {
        for cube in [false, true] {
            let render = |name, red_half| {
                render_with_geometry_and_samples(
                    name,
                    vec![quad([1.0; 4], false)],
                    vec![],
                    vec![if cube {
                        solid_hdr_cube(red_half)
                    } else {
                        solid_hdr_texture(red_half)
                    }],
                    square(),
                    msaa_samples,
                )
            };
            let dim = render("vfx-hdr-2d-half", 0x3800)[32 * 64 + 32];
            let bright = render("vfx-hdr-2d-two", 0x4000)[32 * 64 + 32];
            assert!(
                bright[0] > dim[0] + 0.25,
                "MSAA {msaa_samples}, cube {cube} lost HDR color sampling: {dim:?} vs {bright:?}"
            );
        }
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn character_portrait_builtin_source_uses_injected_provider() {
    let mut portrait = quad([1.0; 4], false);
    portrait.texture_indexes[0] = -5;
    for msaa_samples in [1, 4] {
        let render = |name, rgba| {
            render_with_geometry_camera_frames_and_providers(
                name,
                vec![portrait],
                vec![],
                vec![],
                square(),
                msaa_samples,
                [0.0; 2],
                1,
                None,
                Some(rgba),
            )
        };
        let red = render("vfx-portrait-provider-red", [255, 0, 0, 255])[32 * 64 + 32];
        let blue = render("vfx-portrait-provider-blue", [0, 0, 255, 255])[32 * 64 + 32];
        assert!(red[0] > blue[0] && red[2] < blue[2], "{red:?} vs {blue:?}");
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn tc1_builtin_sources_convert_sampled_rgb_to_alpha() {
    let background = render_center("vfx-builtin-c2a-background", vec![], vec![]);
    for source in [-2, -5] {
        let sampled_rgb = if source == -5 {
            [0.0, 1.0, 0.0]
        } else {
            [background[0], background[1], background[2]]
        };
        let luminance =
            sampled_rgb[0] * 0.298912 + sampled_rgb[1] * 0.586611 + sampled_rgb[2] * 0.114478;
        for mesh_mode in [false, true] {
            for samples in [1, 4] {
                let mut particle = quad([1.0; 4], false);
                particle.texture_indexes[0] = source;
                particle.color_to_alpha[0] = true;
                // A signed-byte TLst source does not set bUSC, so TCAT remains active.
                particle.texture1_use_screen_copy = false;
                let actual = render_with_geometry_camera_frames_and_providers(
                    &format!("vfx-builtin-c2a-{source}-{mesh_mode}-{samples}"),
                    if mesh_mode { vec![] } else { vec![particle] },
                    if mesh_mode {
                        vec![mesh(particle)]
                    } else {
                        vec![]
                    },
                    vec![],
                    square(),
                    samples,
                    [0.0; 2],
                    1,
                    None,
                    (source == -5).then_some([0, 255, 0, 0]),
                )[32 * 64 + 32];
                assert_color(
                    actual,
                    [
                        luminance + background[0] * (1.0 - luminance),
                        luminance + background[1] * (1.0 - luminance),
                        luminance + background[2] * (1.0 - luminance),
                        1.0,
                    ],
                );
            }
        }
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn client_model_fresnel_follows_sorted_instances_and_disabled_state() {
    let mut instances = Vec::new();
    let mut references = Vec::new();
    for (priority, x, enabled) in [(2, 0.3, true), (0, -0.3, true), (1, 0.0, false)] {
        let mut instance = mesh(quad([0.3, 0.5, 0.8, 0.6], false));
        instance.draw_priority = priority;
        instance.position[0] = x;
        instance.scale = [0.65, 0.65, 1.0];
        let mut reference = instance;
        if enabled {
            let end = if priority == 2 {
                [1.0, 0.2, 0.4, 0.8]
            } else {
                [0.3, 1.0, 0.2, 0.4]
            };
            instance.fresnel = Some(xiv_companion::VfxFresnel {
                kind: 2,
                direction: [0.0, 0.0, 1.0],
                exponent: 0.0,
                color_begin: [0.0; 4],
                color_end: end,
            });
            reference.color = std::array::from_fn(|i| instance.color[i] * end[i]);
        }
        instances.push(instance);
        references.push(reference);
    }
    let mut marker = quad([0.8, 0.3, 0.1, 0.4], false);
    marker.draw_priority = 1;
    marker.size = [0.5, 0.2];
    let actual = render(
        "vfx-fresnel-sorted",
        vec![marker],
        instances,
        vec![solid([255; 4])],
    );
    let expected = render(
        "vfx-fresnel-sorted-reference",
        vec![marker],
        references,
        vec![solid([255; 4])],
    );
    assert_image(&actual, &expected);
}

fn render_alpha_depth_scene(
    name: &str,
    mut foreground: VfxQuad,
    geometry: Option<VfxDrawModel>,
    mut textures: Vec<Option<VfxTextureInput>>,
    msaa_samples: u32,
) -> [f32; 4] {
    // Draw a gray background, the nearer test particle, then a red particle
    // behind it. Only fragments surviving alpha testing may hide the red.
    let white = textures.len() as i32;
    textures.push(solid([255; 4]));
    let mut background = quad([0.2, 0.3, 0.4, 1.0], false);
    background.texture_indexes[0] = white;
    background.draw_priority = -2;
    let mut behind = quad([1.0, 0.0, 0.0, 1.0], false);
    behind.texture_indexes[0] = white;
    behind.depth_test = true;
    foreground.position[2] = 0.5;
    foreground.depth_test = true;
    foreground.depth_write = true;
    foreground.draw_priority = -1;
    let mut quads = vec![background, behind];
    let mut meshes = Vec::new();
    if geometry.is_some() {
        meshes.push(mesh(foreground));
    } else {
        quads.push(foreground);
    }
    render_with_geometry_and_samples(
        name,
        quads,
        meshes,
        textures,
        geometry.unwrap_or_else(square),
        msaa_samples,
    )[32 * 64 + 32]
}

#[test]
#[ignore = "requires native wgpu"]
fn depth_offset_modes_change_depth_comparison_for_supported_particles() {
    use xiv_companion_data::avfx::ParticleType;

    let red = [1.0, 0.0, 0.0, 1.0];
    let green = [0.0, 1.0, 0.0, 1.0];
    for msaa_samples in [1, 4] {
        for particle_type in [
            ParticleType::Quad,
            ParticleType::Powder,
            ParticleType::Model,
            ParticleType::LightModel,
        ] {
            let mut quads = Vec::new();
            let mut meshes = Vec::new();
            for (column, (occluder_z, offset, offset_type)) in [
                (0.5, 0.0, 0),
                (0.5, -0.2, 0),
                (0.0, 0.2, 0),
                (0.5, -0.002, 0),
                (0.5, -0.002, 1),
                (0.5, -0.2, 2),
            ]
            .into_iter()
            .enumerate()
            {
                let at_depth = |color, z| {
                    let perspective = (3.0 - z) / 3.0;
                    let mut particle = quad(color, false);
                    particle.position = [(-0.9 + column as f32 * 0.36) * perspective, 0.0, z];
                    particle.size = [0.13 * perspective; 2];
                    particle.depth_test = true;
                    particle
                };
                let mut occluder = at_depth(red, occluder_z);
                occluder.draw_priority = -2;
                occluder.depth_write = true;
                quads.push(occluder);

                let mut candidate = at_depth(green, 0.0);
                candidate.particle_type = Some(particle_type);
                candidate.depth_offset_type = offset_type;
                candidate.depth_offset = offset;
                candidate.draw_priority = -1;
                if matches!(
                    particle_type,
                    ParticleType::Model | ParticleType::LightModel
                ) {
                    meshes.push(mesh(candidate));
                } else {
                    quads.push(candidate);
                }
            }
            let pixels = render_with_geometry_and_samples(
                &format!("vfx-legacy-depth-offset-{msaa_samples}-{particle_type:?}"),
                quads,
                meshes,
                vec![solid([255; 4])],
                square(),
                msaa_samples,
            );
            for (column, x) in [9, 18, 27, 37, 46, 55].into_iter().enumerate() {
                assert_color(
                    pixels[32 * 64 + x],
                    if matches!(column, 1 | 4) { green } else { red },
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn point_initialization_queries_and_partial_depth_reach_gpu() {
    use xiv_companion_data::{
        VFX_IDENTITY_BASIS, VfxBinderMatrix, VfxBinderQueryStatus, VfxBinderTarget,
        VfxDepthOffsetParameters, VfxPointBinderFrame, VfxPointBinderState, avfx::AvfxBinder,
    };
    let matrix = VfxBinderMatrix {
        basis: VFX_IDENTITY_BASIS,
        position: [0.0; 3],
    };
    let binder = AvfxBinder {
        vfx_scale_depth_offset: true,
        transform_scale_depth_offset: true,
        ..Default::default()
    };
    let params = VfxDepthOffsetParameters {
        document_override: 0.0,
        bias_z_max_scale: 1.0,
        bias_z_max_distance: 0.0,
    };
    let mut actual = Vec::new();
    let mut expected = Vec::new();
    let mut control = Vec::new();
    for step in 0..4 {
        let mut state = VfxPointBinderState::new(matrix);
        let mut targets = 0;
        let mut scalars = 0;
        let outcome = state.initialize_frame(
            &binder,
            VfxPointBinderFrame {
                age: 1.0,
                query_deadline: if step == 0 { 0.0 } else { -1.0 },
                camera_position: [0.0, 0.0, 3.0],
                camera: None,
                document_scale: [9.0; 3],
                root_revision: None,
                self_target: true,
            },
            || {
                let call = targets;
                targets += 1;
                if step == 2 && call == 1 {
                    return None;
                }
                Some(VfxBinderTarget {
                    basis: VFX_IDENTITY_BASIS,
                    position: [0.0; 3],
                    scale: [
                        1.0,
                        1.0,
                        if (step == 1 && call == 1) || (step == 3 && call == 0) {
                            1.0
                        } else {
                            200.0
                        },
                    ],
                })
            },
            |scale| {
                let call = scalars;
                scalars += 1;
                *scale = if step == 3 {
                    if call == 0 { 0.0 } else { 3.0 }
                } else {
                    2.0
                };
                !(step == 3 && call == 1)
            },
            1.0,
            [0.0; 3],
        );
        assert_eq!(outcome.query, VfxBinderQueryStatus::Refreshed);
        assert_eq!(
            outcome.update,
            Some(
                [
                    VfxBinderQueryStatus::Skipped,
                    VfxBinderQueryStatus::Refreshed,
                    VfxBinderQueryStatus::TargetUnavailable,
                    VfxBinderQueryStatus::ScaleUnavailable,
                ][step]
            )
        );
        assert_eq!(targets, if step == 0 { 1 } else { 2 });
        assert_eq!(scalars, if step == 3 { 2 } else { 1 });
        let offset = params.evaluate(
            -0.00001,
            state.depth_offset_multiplier(&binder),
            state.matrix.position,
            [0.0, 0.0, 3.0],
        );
        let x = -0.75 + step as f32 * 0.5;
        let mut occluder = quad([1.0, 0.0, 0.0, 1.0], false);
        occluder.position = [x * 2.5 / 3.0, 0.0, 0.5];
        occluder.size = [0.2 * 2.5 / 3.0; 2];
        occluder.depth_test = true;
        occluder.depth_write = true;
        occluder.draw_priority = -2;
        actual.push(occluder);
        expected.push(occluder);
        control.push(occluder);
        let mut particle = quad([0.0, 1.0, 0.0, 1.0], false);
        particle.position = [x, 0.0, 0.0];
        particle.size = [0.13; 2];
        particle.depth_test = true;
        particle.draw_priority = -1;
        particle.depth_offset_type = 1;
        particle.depth_offset = offset;
        actual.push(particle);
        // Independent constants: the first query bypasses CoUF; a second
        // query can replace depth, reset it on target failure, or retain a
        // late failing callback's scalar and target-depth writes. All four
        // initial queries succeeded, so native initialization creates a child.
        particle.depth_offset = [-0.004, -0.00002, -0.00002, -0.006][step];
        expected.push(particle);
        // Missing first query, missing second query, rolled-back target
        // failure or rolled-back late scalar failure, respectively.
        particle.depth_offset = [0.0, -0.004, -0.004, 0.0][step];
        control.push(particle);
    }
    for samples in [1, 4] {
        let draw = |label: &str, packets: &Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-point-initialization-{label}-{samples}"),
                packets.clone(),
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )
        };
        let pixels = draw("actual", &actual);
        assert_image(&pixels, &draw("reference", &expected));
        let wrong = draw("control", &control);
        for (step, x) in [13, 26, 38, 51].into_iter().enumerate() {
            assert_color(
                pixels[32 * 64 + x],
                if step == 1 || step == 2 {
                    [1.0, 0.0, 0.0, 1.0]
                } else {
                    [0.0, 1.0, 0.0, 1.0]
                },
            );
            assert_color(
                wrong[32 * 64 + x],
                if step == 1 || step == 2 {
                    [0.0, 1.0, 0.0, 1.0]
                } else {
                    [1.0, 0.0, 0.0, 1.0]
                },
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn point_query_cache_failure_and_deadline_depth_reach_gpu() {
    use xiv_companion_data::{
        VFX_IDENTITY_BASIS, VfxBinderMatrix, VfxBinderQueryStatus, VfxBinderTarget,
        VfxDepthOffsetParameters, VfxPointBinderFrame, VfxPointBinderState, avfx::AvfxBinder,
    };
    let matrix = VfxBinderMatrix {
        basis: VFX_IDENTITY_BASIS,
        position: [0.0; 3],
    };
    let mut state = VfxPointBinderState {
        matrix,
        auxiliary_matrix: VfxBinderMatrix::IDENTITY,
        target: matrix,
        scale: [1.0; 3],
        vfx_scale: 0.0,
        transform_depth_scale: 1.0,
    };
    let binder = AvfxBinder {
        vfx_scale_depth_offset: true,
        transform_scale_depth_offset: true,
        ..Default::default()
    };
    let target = VfxBinderTarget {
        basis: VFX_IDENTITY_BASIS,
        position: [0.0; 3],
        scale: [1.0, 1.0, 200.0],
    };
    let params = VfxDepthOffsetParameters {
        document_override: 0.0,
        bias_z_max_scale: 1.0,
        bias_z_max_distance: 0.0,
    };
    let mut actual = Vec::new();
    let mut expected = Vec::new();
    let mut control = Vec::new();
    for step in 0..4 {
        if step == 3 {
            state.vfx_scale = 0.0;
        } // explicit upstream cache reset
        let status = state.update_frame(
            &binder,
            VfxPointBinderFrame {
                age: step as f32,
                query_deadline: if step == 1 { 0.0 } else { -1.0 },
                camera_position: if step == 1 {
                    [3.0, 0.0, 0.0]
                } else {
                    [0.0, 0.0, 3.0]
                },
                camera: None,
                document_scale: [9.0; 3],
                root_revision: None,
                self_target: true,
            },
            || (step != 2).then_some(target),
            |scale| {
                *scale = 2.0;
                step != 3
            },
            1.0,
            [0.0; 3],
        );
        assert_eq!(
            status,
            [
                VfxBinderQueryStatus::Refreshed,
                VfxBinderQueryStatus::Skipped,
                VfxBinderQueryStatus::TargetUnavailable,
                VfxBinderQueryStatus::ScaleUnavailable
            ][step]
        );
        let offset = params.evaluate(
            -0.00001,
            state.depth_offset_multiplier(&binder),
            state.matrix.position,
            [0.0, 0.0, 3.0],
        );
        let x = -0.75 + step as f32 * 0.5;
        let mut occluder = quad([1.0, 0.0, 0.0, 1.0], false);
        occluder.position = [x * 2.5 / 3.0, 0.0, 0.5];
        occluder.size = [0.2 * 2.5 / 3.0; 2];
        occluder.depth_test = true;
        occluder.depth_write = true;
        occluder.draw_priority = -2;
        actual.push(occluder);
        expected.push(occluder);
        control.push(occluder);
        let mut particle = quad([0.0, 1.0, 0.0, 1.0], false);
        particle.position = [x, 0.0, 0.0];
        particle.size = [0.13; 2];
        particle.depth_test = true;
        particle.draw_priority = -1;
        particle.depth_offset_type = 1;
        particle.depth_offset = offset;
        actual.push(particle);
        // Independent constants: successful Z query caches depth 200 and VFX
        // 2; a skipped query retains both; target failure resets depth to 1;
        // late scale failure still commits depth and callback scalar writes.
        particle.depth_offset = [-0.004, -0.004, -0.00002, -0.004][step];
        expected.push(particle);
        // Wrong alternatives: re-query camera after deadline, roll back every
        // failed query, or roll back writes made by the failing scale callback.
        particle.depth_offset = [-0.004, -0.00002, -0.004, 0.0][step];
        control.push(particle);
    }
    for samples in [1, 4] {
        let draw = |label: &str, packets: &Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-point-depth-cache-{label}-{samples}"),
                packets.clone(),
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )
        };
        let pixels = draw("actual", &actual);
        assert_image(&pixels, &draw("reference", &expected));
        let wrong = draw("control", &control);
        for (step, x) in [13, 26, 38, 51].into_iter().enumerate() {
            assert_color(
                pixels[32 * 64 + x],
                if step == 2 {
                    [1.0, 0.0, 0.0, 1.0]
                } else {
                    [0.0, 1.0, 0.0, 1.0]
                },
            );
            assert_color(
                wrong[32 * 64 + x],
                if step == 0 || step == 2 {
                    [0.0, 1.0, 0.0, 1.0]
                } else {
                    [1.0, 0.0, 0.0, 1.0]
                },
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn sampled_document_binder_and_distance_depth_reach_gpu() {
    use xiv_companion_data::{VfxRuntime, avfx::*};
    let curve = |z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let mut file = cone_model_render_fixture();
    file.timelines[0].binder_index = -1;
    file.timelines[0].items[0].binder_index = 0;
    file.binders = vec![AvfxBinder {
        bind_point_id: 3,
        vfx_scale_depth_offset: true,
        transform_scale_depth_offset: true,
        vfx_scale_bias: 9.0,
        ..Default::default()
    }];
    file.global.bias_z_max_scale = 4.0;
    file.global.bias_z_max_distance = 6.0;
    file.emitters[0] = AvfxEmitter {
        emitter_type: Some(EmitterType::Point),
        effector_index: -1,
        particle_items: vec![AvfxEmitterItem {
            enabled: true,
            target_index: 0,
            create_time: 1,
            create_count: 1,
            create_probability: 100,
            parameter_link: -1,
            parent_influence_coord: 2,
            ..Default::default()
        }],
        ..Default::default()
    };
    file.particles[0] = AvfxParticle {
        depth_test: true,
        depth_offset: 0.0,
        draw_priority: -1,
        rotation_direction_base: rotation_direction_base::NONE,
        scale: AvfxCurve3Axis {
            x: Some(curve(0.26)),
            y: Some(curve(0.26)),
            ..Default::default()
        },
        texture_color1: file.particles[0].texture_color1.clone(),
        ..Default::default()
    };
    // TC1 uses TLst, not TxNo. An empty list selects the preview's translucent
    // fallback dot, which cannot serve as an opaque depth-occlusion marker.
    file.particles[0]
        .texture_color1
        .as_mut()
        .unwrap()
        .texture_list = vec![0];
    let mut actual_quads = Vec::new();
    let mut actual_meshes = Vec::new();
    let mut expected_quads = Vec::new();
    let mut expected_meshes = Vec::new();
    let mut control_quads = Vec::new();
    let mut control_meshes = Vec::new();
    for (column, kind) in [
        ParticleType::Quad,
        ParticleType::Powder,
        ParticleType::LightModel,
    ]
    .into_iter()
    .enumerate()
    {
        let x = -0.6 + column as f32 * 0.6;
        let points = [VfxBindPoint {
            id: 3,
            parent_bone: None,
            translate: [x, 0.0, 0.0],
            rotate: [0.0; 3],
        }];
        file.particles[0].particle_type = Some(kind);
        file.particles[0].data = if kind == ParticleType::LightModel {
            AvfxParticleData::LightModel { model_index: 1 }
        } else {
            AvfxParticleData::None
        };
        file.particles[0].simple_anim_enable = kind == ParticleType::Powder;
        file.particles[0].simple = (kind == ParticleType::Powder).then_some(AvfxParticleSimple {
            create_count: 1,
            create_interval_life: 100,
            injection_model_index: -1,
            injection_vertex_bind_model_index: -1,
            scale_start: [0.13; 2],
            scale_end: [0.13; 2],
            scale_rand_x: [1.0; 2],
            scale_rand_y: [1.0; 2],
            uv_cell: [1; 2],
            colors: [[255; 4]; 4],
            ..Default::default()
        });
        let runtime = VfxRuntime::with_bind_points(&file, &points)
            .with_vfx_scale(2.0)
            .unwrap()
            .with_document_depth_offset(-0.05)
            .unwrap()
            .with_camera_position([0.0, 0.0, 3.0])
            .unwrap();
        let mut quads = Vec::new();
        let mut meshes = Vec::new();
        runtime.sample(0.0, &mut quads);
        runtime.sample_mesh(0.0, &mut meshes);
        assert_eq!(quads.len() + meshes.len(), 1, "{kind:?}");
        // Independent reference for these axis-aligned unit targets: bTSd=1,
        // Document -0.05, cached VFX=2, distance ramp 4*sqrt(x*x+9)/6.
        let expected_offset = (-0.05 * 2.0) * ((x * x + 9.0).sqrt() / 6.0 * 4.0);
        let mut occluder = quad([1.0, 0.0, 0.0, 1.0], false);
        occluder.position = [x * 2.5 / 3.0, 0.0, 0.5];
        occluder.size = [0.22 * 2.5 / 3.0; 2];
        occluder.depth_test = true;
        occluder.depth_write = true;
        occluder.draw_priority = -2;
        actual_quads.push(occluder);
        expected_quads.push(occluder);
        control_quads.push(occluder);
        for mut q in quads {
            q.color = [0.0, 1.0, 0.0, 1.0];
            assert!((q.depth_offset - expected_offset).abs() < 1e-6);
            actual_quads.push(q);
            q.depth_offset = expected_offset;
            expected_quads.push(q);
            q.depth_offset = 0.0;
            control_quads.push(q);
        }
        for mut m in meshes {
            m.color = [0.0, 1.0, 0.0, 1.0];
            m.scale = [0.13; 3];
            assert!((m.depth_offset - expected_offset).abs() < 1e-6);
            actual_meshes.push(m);
            m.depth_offset = expected_offset;
            expected_meshes.push(m);
            m.depth_offset = 0.0;
            control_meshes.push(m);
        }
    }
    for samples in [1, 4] {
        let draw = |label: &str, quads: &Vec<VfxQuad>, meshes: &Vec<VfxMeshInstance>| {
            render_with_geometry_and_samples(
                &format!("vfx-depth-inputs-{label}-{samples}"),
                quads.clone(),
                meshes.clone(),
                vec![solid([255; 4])],
                square(),
                samples,
            )
        };
        let actual = draw("actual", &actual_quads, &actual_meshes);
        assert_image(
            &actual,
            &draw("reference", &expected_quads, &expected_meshes),
        );
        let control = draw("control", &control_quads, &control_meshes);
        for x in [17, 32, 47] {
            assert_color(actual[32 * 64 + x], [0.0, 1.0, 0.0, 1.0]);
            assert_color(control[32 * 64 + x], [1.0, 0.0, 0.0, 1.0]);
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn depth_flags_match_client_depth_enable_and_write_mask() {
    let red = [1.0, 0.0, 0.0, 1.0];
    let green = [0.0, 1.0, 0.0, 1.0];
    let flags = [(false, false), (false, true), (true, false), (true, true)];
    for msaa_samples in [1, 4] {
        for mesh_mode in [false, true] {
            let mut quads = Vec::new();
            let mut meshes = Vec::new();
            for row in 0..3 {
                for (column, (depth_test, depth_write)) in flags.into_iter().enumerate() {
                    let at_depth = |color, z| {
                        // Keep each cell's projected position and size fixed at all depths.
                        let perspective = (3.0 - z) / 3.0;
                        let mut particle = quad(color, false);
                        particle.position = [
                            (-0.75 + column as f32 * 0.5) * perspective,
                            (0.5 - row as f32 * 0.5) * perspective,
                            z,
                        ];
                        particle.size = [0.2 * perspective; 2];
                        particle
                    };
                    let mut occluder = at_depth(red, if row == 0 { 0.5 } else { 0.0 });
                    occluder.depth_test = true;
                    occluder.depth_write = true;
                    occluder.draw_priority = -2;
                    quads.push(occluder);

                    let mut candidate = at_depth(green, if row == 1 { 0.5 } else { 0.0 });
                    (
                        candidate.depth_test,
                        candidate.depth_write,
                        candidate.soft_particle,
                    ) = parsed_depth_flags(depth_test, depth_write, false);
                    candidate.draw_priority = -1;
                    if mesh_mode {
                        meshes.push(mesh(candidate));
                    } else {
                        quads.push(candidate);
                    }
                    if row == 1 {
                        let mut probe = at_depth(red, 0.0);
                        probe.depth_test = true;
                        quads.push(probe);
                    }
                }
            }
            let pixels = render_with_geometry_and_samples(
                &format!("vfx-depth-flags-{msaa_samples}-{mesh_mode}"),
                quads,
                meshes,
                vec![solid([255; 4])],
                square(),
                msaa_samples,
            );
            for (row, y) in [19, 32, 45].into_iter().enumerate() {
                for (column, (depth_test, depth_write)) in flags.into_iter().enumerate() {
                    let visible = match row {
                        0 => !depth_test,
                        1 => depth_test && depth_write,
                        2 => true, // LessEqual must accept a particle at the stored depth.
                        _ => unreachable!(),
                    };
                    let x = [13, 26, 38, 51][column];
                    eprintln!(
                        "row={row}, test={depth_test}, write={depth_write}, mesh={mesh_mode}, MSAA={msaa_samples}"
                    );
                    assert_color(pixels[y * 64 + x], if visible { green } else { red });
                }
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn soft_particles_respect_depth_test_against_scene_depth() {
    let red = [1.0, 0.0, 0.0, 1.0];
    let green = [0.0, 1.0, 0.0, 1.0];
    for msaa_samples in [1, 4] {
        for mesh_mode in [false, true] {
            let mut quads = Vec::new();
            let mut meshes = Vec::new();
            for column in 0..3 {
                let at_depth = |color, z| {
                    let perspective = (3.0 - z) / 3.0;
                    let mut particle = quad(color, false);
                    particle.position = [(-0.5 + column as f32 * 0.5) * perspective, 0.0, z];
                    particle.size = [0.2 * perspective; 2];
                    particle
                };
                let mut receiver = at_depth(red, if column == 2 { 0.0 } else { 0.5 });
                receiver.depth_test = true;
                receiver.depth_write = true;
                receiver.draw_priority = -2;
                quads.push(receiver);

                let mut particle = at_depth(green, if column == 2 { 0.5 } else { 0.0 });
                (
                    particle.depth_test,
                    particle.depth_write,
                    particle.soft_particle,
                ) = parsed_depth_flags(column != 0, false, true);
                particle.soft_particle_fade_range = 0.01;
                particle.draw_priority = -1;
                if mesh_mode {
                    meshes.push(mesh(particle));
                } else {
                    quads.push(particle);
                }
            }
            let pixels = render_with_geometry_and_samples(
                &format!("vfx-soft-depth-test-{msaa_samples}-{mesh_mode}"),
                quads,
                meshes,
                vec![solid([255; 4])],
                square(),
                msaa_samples,
            );
            for (column, x) in [19, 32, 45].into_iter().enumerate() {
                assert_color(pixels[32 * 64 + x], if column == 1 { red } else { green });
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn soft_particles_write_depth_for_later_soft_draws() {
    let red = [1.0, 0.0, 0.0, 1.0];
    let green = [0.0, 1.0, 0.0, 1.0];
    let flags = [(true, true), (true, false), (false, true)];
    for msaa_samples in [1, 4] {
        for mesh_mode in [false, true] {
            let mut quads = Vec::new();
            let mut meshes = Vec::new();
            for (column, (depth_test, depth_write)) in flags.into_iter().enumerate() {
                let at_depth = |color, z| {
                    let perspective = (3.0 - z) / 3.0;
                    let mut particle = quad(color, false);
                    particle.position = [(-0.5 + column as f32 * 0.5) * perspective, 0.0, z];
                    particle.size = [0.2 * perspective; 2];
                    particle.soft_particle = true;
                    particle.soft_particle_fade_range = 0.01;
                    particle.depth_test = true;
                    particle
                };
                let mut foreground = at_depth(green, 0.5);
                (
                    foreground.depth_test,
                    foreground.depth_write,
                    foreground.soft_particle,
                ) = parsed_depth_flags(depth_test, depth_write, true);
                foreground.draw_priority = -1;
                if mesh_mode {
                    meshes.push(mesh(foreground));
                } else {
                    quads.push(foreground);
                }
                let mut behind = at_depth(red, 0.0);
                behind.draw_priority = 0;
                quads.push(behind);
            }
            let pixels = render_with_geometry_and_samples(
                &format!("vfx-soft-depth-write-{msaa_samples}-{mesh_mode}"),
                quads,
                meshes,
                vec![solid([255; 4])],
                square(),
                msaa_samples,
            );
            for (column, x) in [19, 32, 45].into_iter().enumerate() {
                assert_color(pixels[32 * 64 + x], if column == 0 { green } else { red });
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn soft_and_regular_particles_obey_priority_across_passes() {
    for samples in [1, 4] {
        let background = render_with_geometry_and_samples(
            &format!("vfx-soft-priority-background-{samples}"),
            vec![],
            vec![],
            vec![solid([255; 4])],
            square(),
            samples,
        )[32 * 64 + 32];
        for (soft_first, first_mesh, second_mesh) in [
            (true, false, false),
            (true, false, true),
            (true, true, false),
            (false, false, true),
        ] {
            let mut first = quad([1.0, 0.0, 0.0, 0.5], false);
            first.draw_priority = -1;
            first.soft_particle = soft_first;
            first.soft_particle_fade_range = 0.01;
            let mut second = quad([0.0, 0.0, 1.0, 0.5], false);
            second.draw_priority = 1;
            second.soft_particle = !soft_first;
            second.soft_particle_fade_range = 0.01;
            let actual = render_with_geometry_and_samples(
                &format!("vfx-soft-priority-{samples}-{soft_first}-{first_mesh}-{second_mesh}"),
                [
                    (!first_mesh).then_some(first),
                    (!second_mesh).then_some(second),
                ]
                .into_iter()
                .flatten()
                .collect(),
                [
                    first_mesh.then_some(mesh(first)),
                    second_mesh.then_some(mesh(second)),
                ]
                .into_iter()
                .flatten()
                .collect(),
                vec![solid([255; 4])],
                square(),
                samples,
            )[32 * 64 + 32];
            assert_color(
                actual,
                [
                    0.25 + 0.25 * background[0],
                    0.25 * background[1],
                    0.5 + 0.25 * background[2],
                    1.0,
                ],
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn soft_depth_snapshot_refreshes_after_an_intervening_regular_draw() {
    for samples in [1, 4] {
        let mut first = quad([1.0, 0.0, 0.0, 0.5], false);
        first.draw_priority = -2;
        first.soft_particle = true;
        first.soft_particle_fade_range = 0.01;
        first.depth_test = false;

        let mut receiver = quad([0.0, 1.0, 0.0, 1.0], false);
        receiver.draw_priority = 0;
        receiver.depth_test = true;
        receiver.depth_write = true;

        let mut last = quad([0.0, 0.0, 1.0, 1.0], true);
        last.draw_priority = 2;
        last.position[2] = 0.08;
        last.soft_particle = true;
        last.soft_particle_fade_range = 0.8;
        last.depth_test = true;

        let actual = render_with_geometry_and_samples(
            &format!("vfx-soft-priority-depth-refresh-{samples}"),
            vec![first, receiver, last],
            vec![],
            vec![solid([255; 4])],
            square(),
            samples,
        )[32 * 64 + 32];
        assert!(actual[1] > 0.9, "receiver {samples}x: {actual:?}");
        assert!(actual[2] < 0.1, "soft fade {samples}x: {actual:?}");
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn soft_particles_sample_depth_per_msaa_sample_at_receiver_edge() {
    // At z=0 the 45-degree camera projects this world X to pixel X=32.5.
    let edge_x = 3.0 * 22.5_f32.to_radians().tan() / 64.0;
    let mut receiver = quad([0.0, 1.0, 0.0, 1.0], false);
    receiver.position = [edge_x - 1.0, 0.0, 0.0];
    receiver.size = [1.0, 2.0];
    receiver.depth_test = true;
    receiver.depth_write = true;

    let mut particle = quad([1.0, 0.0, 0.0, 1.0], true);
    particle.position[2] = 0.08;
    particle.size = [2.0; 2];
    particle.draw_priority = 1;
    particle.soft_particle = true;
    particle.soft_particle_fade_range = 0.8;
    particle.depth_test = true;

    let pixels = render_with_geometry_and_samples(
        "vfx-soft-depth-4x-receiver-edge",
        vec![receiver, particle],
        vec![],
        vec![solid([255; 4])],
        square(),
        4,
    );
    let left = pixels[32 * 64 + 31];
    let edge = pixels[32 * 64 + 32];
    let right = pixels[32 * 64 + 33];
    assert!(
        left[1] > right[1] + 0.8,
        "receiver sides: {left:?} {right:?}"
    );
    assert!(right[0] > left[0] + 0.8, "soft sides: {left:?} {right:?}");
    let receiver_coverage = (edge[1] - right[1]) / (left[1] - right[1]);
    let soft_uncovered = (edge[0] - left[0]) / (right[0] - left[0]);
    assert!(
        (0.2..0.8).contains(&receiver_coverage),
        "receiver edge: {left:?} {edge:?} {right:?}"
    );
    assert!(
        (soft_uncovered - (1.0 - receiver_coverage)).abs() < 0.06,
        "soft edge: {left:?} {edge:?} {right:?}"
    );
}

#[test]
#[ignore = "requires native wgpu"]
fn soft_particle_negative_fade_range_preserves_signed_division() {
    let mut receiver = quad([0.0, 0.0, 0.0, 1.0], false);
    receiver.size = [10.0; 2];
    receiver.depth_test = true;
    receiver.depth_write = true;

    let mut particle = quad([1.0, 0.0, 0.0, 1.0], true);
    particle.position[2] = 0.08;
    particle.size = [0.4; 2];
    particle.draw_priority = 1;
    particle.soft_particle = true;
    particle.depth_test = true;

    for samples in [1, 4] {
        let render = |mut candidate: VfxQuad, range: f32| {
            candidate.soft_particle_fade_range = range;
            render_with_geometry_and_samples(
                &format!("vfx-soft-signed-range-{samples}-{range}"),
                vec![receiver, candidate],
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )[32 * 64 + 32]
        };
        let positive = render(particle, 0.8);
        let negative = render(particle, -0.8);
        let zero = render(particle, 0.0);
        // At the center, the world-space separation is .08 and SPFR is .8:
        // squared distance fading must be about .01. A mistaken [-1, 1]
        // depth reconstruction also fades, but produces the wrong magnitude.
        assert!(
            (positive[0] - 0.01).abs() < 0.001,
            "positive {samples}x: {positive:?}"
        );
        assert!(zero[0] > 0.9, "zero {samples}x: {zero:?}");
        assert!(
            (positive[0] - negative[0]).abs() < 0.01,
            "signed range {samples}x: {positive:?} {negative:?}"
        );

        let mut distant = particle;
        distant.position[2] = 0.3;
        distant.color[3] = 0.25;
        let positive_far = render(distant, 0.1);
        let negative_far = render(distant, -0.1);
        assert!(
            negative_far[0] > positive_far[0] + 0.5,
            "negative over-range {samples}x: {positive_far:?} {negative_far:?}"
        );
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn transparent_fragments_do_not_occlude_later_particles() {
    let draw_modes = [0, 1, 2, 4];
    // Client Apricot shaders use a strict < 1/255 cutoff.
    let alphas = [0.0, 0.0038, 0.0039, 0.00391, 1.0 / 255.0];
    let background_color = [0.2, 0.3, 0.4, 1.0];
    for msaa_samples in [1, 4] {
        for mesh_mode in [false, true] {
            let mut quads = Vec::new();
            let mut meshes = Vec::new();
            for (row, alpha) in alphas.into_iter().enumerate() {
                for (column, draw_mode) in draw_modes.into_iter().enumerate() {
                    let at_depth = |color, z| {
                        // All three layers cover the same projected cell.
                        let perspective = (3.0 - z) / 3.0;
                        let mut particle = quad(color, false);
                        particle.position = [
                            (-0.75 + column as f32 * 0.5) * perspective,
                            (0.8 - row as f32 * 0.4) * perspective,
                            z,
                        ];
                        particle.size = [0.16 * perspective; 2];
                        particle
                    };
                    let mut background = at_depth(background_color, 0.0);
                    background.draw_priority = -2;
                    quads.push(background);

                    let mut foreground = at_depth([0.0, 1.0, 0.0, alpha], 0.5);
                    foreground.draw_mode = draw_mode;
                    foreground.depth_test = true;
                    foreground.depth_write = true;
                    foreground.draw_priority = -1;
                    if mesh_mode {
                        meshes.push(mesh(foreground));
                    } else {
                        quads.push(foreground);
                    }

                    let mut behind = at_depth([1.0, 0.0, 0.0, 1.0], 0.0);
                    behind.depth_test = true;
                    quads.push(behind);
                }
            }
            let pixels = render_with_geometry_and_samples(
                &format!("vfx-alpha-depth-{msaa_samples}-{mesh_mode}"),
                quads,
                meshes,
                vec![solid([255; 4])],
                square(),
                msaa_samples,
            );
            for (row, alpha) in alphas.into_iter().enumerate() {
                for (column, draw_mode) in draw_modes.into_iter().enumerate() {
                    let expected = if alpha < 1.0 / 255.0 {
                        [1.0, 0.0, 0.0, 1.0]
                    } else {
                        std::array::from_fn(|i| {
                            if i == 3 {
                                return 1.0;
                            }
                            let source = if i == 1 { alpha } else { 0.0 };
                            match draw_mode {
                                0 => source + background_color[i] * (1.0 - alpha),
                                1 => background_color[i] * (source + 1.0 - alpha),
                                2 => source + background_color[i],
                                4 => source + background_color[i] * (1.0 - source),
                                _ => unreachable!(),
                            }
                        })
                    };
                    let x = [13, 26, 38, 51][column];
                    let y = [11, 22, 32, 42, 53][row];
                    eprintln!(
                        "alpha={alpha}, mode={draw_mode}, mesh={mesh_mode}, MSAA={msaa_samples}"
                    );
                    assert_color(pixels[y * 64 + x], expected);
                }
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn alpha_cutoff_uses_composed_texture_particle_and_vertex_alpha() {
    for msaa_samples in [1, 4] {
        for mesh_mode in [false, true] {
            for case in [
                "texture",
                "secondary",
                "mask",
                "vertex",
                "max",
                "disabled",
                "c2a",
            ] {
                if case == "vertex" && !mesh_mode {
                    continue;
                }
                let mut foreground = quad([0.0, 1.0, 0.0, 1.0], false);
                let mut geometry = square();
                let mut textures = vec![solid([255; 4]), solid([255; 4])];
                match case {
                    "texture" => {
                        foreground.color[3] = 0.5;
                        textures[0] = solid([255, 255, 255, 1]);
                    }
                    "secondary" => {
                        foreground.color[3] = 0.5;
                        foreground.texture_indexes[1] = 1;
                        textures[1] = solid([255, 255, 255, 1]);
                    }
                    "mask" => {
                        foreground.texture1_is_shape_mask = true;
                        foreground.color_to_alpha[0] = true;
                        textures[0] = solid([0, 0, 0, 255]);
                    }
                    "vertex" => {
                        foreground.color[3] = 0.5;
                        for vertex in &mut geometry.vertices {
                            vertex.color[3] = 1;
                        }
                    }
                    "max" => {
                        textures[0] = solid([255, 255, 255, 0]);
                        foreground.texture_indexes[1] = 1;
                        foreground.combine_modes[0][1] = 1;
                    }
                    "disabled" => {
                        textures[0] = solid([255, 255, 255, 0]);
                        foreground.texture1_enabled = false;
                    }
                    "c2a" => {
                        foreground.texture_indexes[1] = 1;
                        foreground.color_to_alpha[1] = true;
                        textures[1] = solid([0, 0, 0, 255]);
                    }
                    _ => unreachable!(),
                }
                let name = format!("vfx-alpha-composed-{msaa_samples}-{mesh_mode}-{case}");
                let actual = render_alpha_depth_scene(
                    &name,
                    foreground,
                    mesh_mode.then_some(geometry),
                    textures,
                    msaa_samples,
                );
                eprintln!("{name}: {actual:?}");
                assert_color(
                    actual,
                    if matches!(case, "max" | "disabled") {
                        [0.0, 1.0, 0.0, 1.0]
                    } else {
                        [1.0, 0.0, 0.0, 1.0]
                    },
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn client_color_to_alpha_uses_luminance_and_white_rgb() {
    let background = render_center("vfx-c2a-background", vec![], vec![]);
    // Independent client DXBC coefficients, with texture alpha deliberately zero.
    for (rgb, luminance) in [
        ([255, 0, 0], 0.298912),
        ([0, 255, 0], 0.586611),
        ([0, 0, 255], 0.114478),
    ] {
        for mesh_mode in [false, true] {
            for layer in 0..4 {
                let mut particle = quad([0.4, 0.6, 0.8, 0.5], false);
                particle.texture_indexes[layer] = 1;
                particle.color_to_alpha[layer] = true;
                let actual = render(
                    &format!("vfx-client-c2a-{rgb:?}-{mesh_mode}-{layer}"),
                    if mesh_mode { vec![] } else { vec![particle] },
                    if mesh_mode {
                        vec![mesh(particle)]
                    } else {
                        vec![]
                    },
                    vec![solid([255; 4]), solid([rgb[0], rgb[1], rgb[2], 0])],
                )[32 * 64 + 32];
                let alpha = 0.5 * luminance;
                assert_color(
                    actual,
                    std::array::from_fn(|channel| {
                        if channel == 3 {
                            1.0
                        } else {
                            particle.color[channel] * alpha + background[channel] * (1.0 - alpha)
                        }
                    }),
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn client_tc1_modes_gate_channels_independently_of_list_source() {
    let background = render_center("vfx-tc1-background", vec![], vec![]);
    let texture = [64, 128, 192, 128];
    for mesh_mode in [false, true] {
        for from_list in [false, true] {
            for color_to_alpha in [false, true] {
                // Client TC1 uses nonzero TCCT/TCAT as channel enables.
                for mode in [0, 1, 2, 3, 4, 5] {
                    let mut particle = quad([0.4, 0.6, 0.8, 0.5], false);
                    particle.texture1_is_shape_mask = from_list;
                    particle.color_to_alpha[0] = color_to_alpha;
                    particle.combine_mode_tc1 = [mode, mode % 4];
                    let actual = render(
                        &format!("vfx-client-tc1-{mesh_mode}-{from_list}-{color_to_alpha}-{mode}"),
                        if mesh_mode { vec![] } else { vec![particle] },
                        if mesh_mode {
                            vec![mesh(particle)]
                        } else {
                            vec![]
                        },
                        vec![solid(texture)],
                    )[32 * 64 + 32];
                    let tex = texture.map(|value| value as f32 / 255.0);
                    let alpha = 0.5
                        * if mode % 4 == 0 {
                            1.0
                        } else if color_to_alpha {
                            0.298912 * tex[0] + 0.586611 * tex[1] + 0.114478 * tex[2]
                        } else {
                            tex[3]
                        };
                    assert_color(
                        actual,
                        std::array::from_fn(|channel| {
                            if channel == 3 {
                                1.0
                            } else {
                                let rgb = particle.color[channel]
                                    * if mode == 0 || color_to_alpha {
                                        1.0
                                    } else {
                                        tex[channel]
                                    };
                                rgb * alpha + background[channel] * (1.0 - alpha)
                            }
                        }),
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn texture_palette_remaps_each_tc1_channel_before_later_composition() {
    let palette = Some(VfxTextureInput {
        rgba: [0_u8, 85, 170, 255]
            .into_iter()
            .flat_map(|red| [red, 0, 0, 255])
            .collect(),
        width: 4,
        height: 1,
        authored_mips: None,
        rgba16f_mips: None,
        cube_mips: None,
        cube_format: VfxTextureCubeFormat::Rgba8Unorm,
    });
    for mesh_mode in [false, true] {
        let mut particle = quad([1.0; 4], false);
        particle.draw_mode = 8;
        particle.texture_palette_index = 1;
        particle.palette_offset = 26.0 / 255.0;
        particle.palette_border = 1;
        particle.palette_filter = 0;
        let actual = render(
            &format!("vfx-texture-palette-{mesh_mode}"),
            if mesh_mode { vec![] } else { vec![particle] },
            if mesh_mode {
                vec![mesh(particle)]
            } else {
                vec![]
            },
            vec![solid([26, 89, 153, 217]), palette.clone()],
        )[32 * 64 + 32];
        assert_color(actual, [0.0, 85.0 / 255.0, 170.0 / 255.0, 1.0]);
    }
    // Model's TP callback writes a float uniform. Negative values and a
    // coordinate just below a texel boundary distinguish it from UNORM8.
    for samples in [1, 4] {
        for (case, offset, expected) in [
            ("negative", -0.25, [0.0, 0.0, 85.0 / 255.0, 170.0 / 255.0]),
            (
                "sub-byte-boundary",
                0.25 - 26.0 / 255.0 - 0.0001,
                [0.0, 85.0 / 255.0, 170.0 / 255.0, 1.0],
            ),
        ] {
            let mut particle = quad([1.0; 4], false);
            particle.draw_mode = 8;
            particle.texture_palette_index = 1;
            particle.palette_offset = offset;
            particle.palette_border = 1;
            particle.palette_filter = 0;
            let actual = render_with_geometry_and_samples(
                &format!("vfx-model-palette-float-{case}-{samples}"),
                vec![],
                vec![mesh(particle)],
                vec![solid([26, 89, 153, 217]), palette.clone()],
                square(),
                samples,
            )[32 * 64 + 32];
            assert_color(actual, expected);
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn tc1_channel_controls_sample_current_screen_copy_and_ignore_its_alpha() {
    let background = render_center("vfx-tc1-groups-background", vec![], vec![]);
    let texture = [64, 128, 192, 64];
    let tex = texture.map(|value| value as f32 / 255.0);
    for mesh_mode in [false, true] {
        for samples in [1, 4] {
            let mut base = quad([0.4, 0.6, 0.8, 0.5], false);
            base.combine_mode_tc1 = [0, 0];
            let mut overlay = base;
            overlay.combine_mode_tc1 = [1, 1];
            for soft_particle in [false, true] {
                for screen_copy in [false, true] {
                    overlay.soft_particle = soft_particle;
                    overlay.soft_particle_fade_range = 0.01;
                    overlay.texture1_use_screen_copy = screen_copy;
                    overlay.texture_indexes[0] = if screen_copy { -2 } else { 0 };
                    let actual = render_with_geometry_and_samples(
                        &format!(
                            "vfx-tc1-groups-{mesh_mode}-{samples}-{screen_copy}-{soft_particle}"
                        ),
                        if mesh_mode {
                            vec![]
                        } else {
                            vec![base, overlay]
                        },
                        if mesh_mode {
                            vec![mesh(base), mesh(overlay)]
                        } else {
                            vec![]
                        },
                        vec![solid(texture)],
                        square(),
                        samples,
                    )[32 * 64 + 32];
                    let sampled = if screen_copy { background } else { tex };
                    let alpha = 0.5 * if screen_copy { 1.0 } else { tex[3] };
                    assert_color(
                        actual,
                        std::array::from_fn(|channel| {
                            if channel == 3 {
                                1.0
                            } else {
                                let below = base.color[channel] * 0.5 + background[channel] * 0.5;
                                overlay.color[channel] * sampled[channel] * alpha
                                    + below * (1.0 - alpha)
                            }
                        }),
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn tc1_previous_frame_copy_reads_the_prior_completed_hdr_scene() {
    let background = render_center("vfx-previous-frame-background", vec![], vec![]);
    for samples in [1, 4] {
        let mut particle = quad([0.5, 0.75, 1.0, 0.5], false);
        particle.texture_indexes[0] = -3;
        particle.texture1_use_screen_copy = true;
        let draw = |frames| {
            render_with_geometry_camera_and_frames(
                &format!("vfx-previous-frame-{samples}-{frames}"),
                vec![particle],
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
                [0.0; 2],
                frames,
            )[32 * 64 + 32]
        };
        let first = draw(1);
        let second = draw(2);
        assert_color(
            second,
            std::array::from_fn(|channel| {
                if channel == 3 {
                    1.0
                } else {
                    particle.color[channel] * first[channel] * 0.5 + background[channel] * 0.5
                }
            }),
        );
        assert!(
            second[..3]
                .iter()
                .zip(&first[..3])
                .any(|(second, first)| (second - first).abs() > 0.002),
            "previous-frame source did not advance: first={first:?}, second={second:?}"
        );
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn client_final_color_clamps_after_all_layers() {
    let background = render_center("vfx-final-clamp-background", vec![], vec![]);
    for mesh_mode in [false, true] {
        for draw_mode in [0, 1, 2, 4] {
            for case in [
                "negative-rgb",
                "alpha-high",
                "alpha-product",
                "subtract-recover",
            ] {
                let mut particle = quad([0.4, 0.6, 0.8, 1.0], false);
                particle.draw_mode = draw_mode;
                let mut textures = vec![solid([255; 4])];
                let (rgb, alpha): ([f32; 3], f32) = match case {
                    "negative-rgb" => {
                        particle.color = [-0.4, 2.0, 0.8, 0.5];
                        ([0.0, 2.0, 0.8], 0.5)
                    }
                    "alpha-high" => {
                        particle.color[3] = 2.0;
                        ([0.4, 0.6, 0.8], 2.0)
                    }
                    "alpha-product" => {
                        particle.color[3] = 2.0;
                        textures[0] = solid([255, 255, 255, 64]);
                        ([0.4, 0.6, 0.8], 128.0 / 255.0)
                    }
                    "subtract-recover" => {
                        // Intermediate -0.25 recovers to +0.25 before final clamp.
                        textures = vec![
                            solid([64, 128, 192, 255]),
                            solid([128, 64, 64, 255]),
                            solid([128; 4]),
                        ];
                        particle.texture_indexes = [0, 1, 2, -1];
                        particle.combine_modes[0] = [2, 0];
                        particle.combine_modes[1] = [1, 3];
                        (
                            [0.4 * 64.0 / 255.0, 0.6 * 192.0 / 255.0, 0.8 * 256.0 / 255.0],
                            1.0,
                        )
                    }
                    _ => unreachable!(),
                };
                let actual = render(
                    &format!("vfx-client-clamp-{mesh_mode}-{draw_mode}-{case}"),
                    if mesh_mode { vec![] } else { vec![particle] },
                    if mesh_mode {
                        vec![mesh(particle)]
                    } else {
                        vec![]
                    },
                    textures,
                )[32 * 64 + 32];
                assert_color(
                    actual,
                    std::array::from_fn(|channel| {
                        if channel == 3 {
                            return 1.0;
                        }
                        let coverage = alpha.min(1.0);
                        let src = rgb[channel] * coverage;
                        let dst = background[channel];
                        match draw_mode {
                            0 => src + dst * (1.0 - coverage),
                            1 => dst * (rgb[channel].min(1.0) * coverage + 1.0 - coverage),
                            2 => src + dst,
                            4 => {
                                let screen = (rgb[channel] * alpha).min(1.0);
                                screen + dst * (1.0 - screen)
                            }
                            _ => unreachable!(),
                        }
                    }),
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn disabled_tc1_uses_neutral_base_for_quads_and_meshes() {
    let background = render_center("vfx-disabled-tc1-background", vec![], vec![]);
    let color = [0.3, 0.6, 0.9, 0.5];
    let textures = vec![
        solid([255, 0, 0, 0]),
        solid([64, 128, 192, 128]),
        solid([255; 4]),
    ];
    for mesh_mode in [false, true] {
        for second_layer in [false, true] {
            let mut reference = quad(color, false);
            reference.texture_indexes[0] = 2;
            if second_layer {
                reference.texture_indexes[1] = 1;
            }
            let render_particle = |name: &str, particle: VfxQuad| {
                render(
                    name,
                    if mesh_mode { vec![] } else { vec![particle] },
                    if mesh_mode {
                        vec![mesh(particle)]
                    } else {
                        vec![]
                    },
                    textures.clone(),
                )
            };
            let expected = render_particle(
                &format!("vfx-disabled-tc1-reference-{mesh_mode}-{second_layer}"),
                reference,
            );
            let layer = if second_layer {
                [64.0 / 255.0, 128.0 / 255.0, 192.0 / 255.0, 128.0 / 255.0]
            } else {
                [1.0; 4]
            };
            let alpha = color[3] * layer[3];
            assert_color(
                expected[32 * 64 + 32],
                [
                    color[0] * layer[0] * alpha + background[0] * (1.0 - alpha),
                    color[1] * layer[1] * alpha + background[1] * (1.0 - alpha),
                    color[2] * layer[2] * alpha + background[2] * (1.0 - alpha),
                    alpha + background[3] * (1.0 - alpha),
                ],
            );
            for texture_index in [-1, 0] {
                let mut particle = reference;
                particle.texture1_enabled = false;
                particle.texture_indexes[0] = texture_index;
                particle.texture1_is_shape_mask = true;
                particle.color_to_alpha[0] = true;
                particle.combine_mode_tc1 = [1, 1];
                assert_image(
                    &render_particle(
                        &format!("vfx-disabled-tc1-{mesh_mode}-{second_layer}-{texture_index}"),
                        particle,
                    ),
                    &expected,
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn disc_uses_client_ring_grid_width_compensation_and_packed_vertex_attributes() {
    use xiv_companion_data::avfx::ParticleType;
    use xiv_companion_data::avfx_sim::VfxDisc;
    let diagonal = std::f32::consts::FRAC_1_SQRT_2;
    for (case, ss, factor, sign, cull) in [
        (0, 1.0, 0.0, 1.0, 0),
        (1, 0.0, 1.0, -1.0, 0),
        (2, 0.5, 1.0, 1.0, 3),
        (3, 1.0, 0.0, 1.0, 2),
    ] {
        let mut particle = quad([1.0; 4], false);
        particle.particle_type = Some(ParticleType::Disc);
        particle.size = [0.65 * sign, 0.35];
        particle.orientation = [diagonal, 0.0, 0.0, diagonal];
        particle.draw_mode = 8;
        particle.cull_mode = cull;
        particle.uv_origins = [[-0.0123, 0.0209]; 4];
        particle.uv_scales = [[0.827, 0.711]; 4];
        particle.disc = Some(VfxDisc {
            counts: [2, 3, 3],
            angle: std::f32::consts::FRAC_PI_2,
            radius: [0.5, 0.7],
            width: [0.1, 0.15],
            height_inner: [0.0, 0.1],
            height_outer: [-0.1, 0.2],
            color_inner: [0.0, 0.0, 0.0, 1.0],
            color_outer: [1.0; 4],
            point_interval_factor: factor,
            scaling_scale: ss,
            scale_z: 0.8,
        });
        // Explicit quarter-circle directions and radial weights, independent of
        // GPU tessellation and sin/cos. The average X/Z scale is (1.3 + 0.8) / 2.
        let width_factor = if ss < 1.0 {
            -(1.0 / 1.05 + (1.0 - 1.0 / 1.05) * ss)
        } else {
            1.0
        };
        let weights = if factor == 0.0 {
            [0.0, 0.5, 1.0]
        } else {
            [0.0, 0.0625, 1.0]
        };
        let mut geometry = VfxDrawModel::default();
        for part in [1.0, -1.0] {
            for (row, [x, z]) in [[1.0, 0.0], [diagonal, -diagonal], [0.0, -1.0]]
                .into_iter()
                .enumerate()
            {
                let radius = [0.5, 0.6, 0.7][row];
                let width = [0.1, 0.125, 0.15][row] * width_factor;
                let inner_height = [0.0, 0.05, 0.1][row];
                let outer_height = [-0.1, 0.05, 0.2][row];
                for (col, weight) in weights.into_iter().enumerate() {
                    let distance = radius - width + 2.0 * width * weight;
                    let uv = [col as f32 / 2.0, row as f32 / 2.0];
                    let uv = std::array::from_fn(|axis| {
                        let moved = particle.uv_origins[0][axis]
                            + 0.5
                            + particle.uv_scales[0][axis] * (uv[axis] - 0.5);
                        ((moved * 1000.0) as i32 as i16) as f32 * 0.001 - 0.5
                    });
                    let gray = [0, 128, 255][col];
                    geometry.vertices.push(VfxDrawVertex {
                        position: [
                            x * part * distance,
                            inner_height + (outer_height - inner_height) * weight,
                            z * part * distance,
                        ],
                        position_w: 1.0,
                        uvs: [uv; 4],
                        color: [gray, gray, gray, 255],
                        normal: [128, 255, 128, 128],
                        tangent: [255, 128, 128, 255],
                    });
                }
            }
        }
        geometry.indices = [
            0, 1, 3, 3, 1, 4, 1, 2, 4, 4, 2, 5, 3, 4, 6, 6, 4, 7, 4, 5, 7, 7, 5, 8, 9, 10, 12, 12,
            10, 13, 10, 11, 13, 13, 11, 14, 12, 13, 15, 15, 13, 16, 13, 14, 16, 16, 14, 17,
        ]
        .to_vec();
        let mut reference = mesh(particle);
        reference.scale = [1.3 * sign, 0.7, 0.8];
        reference.uv_origins = [[0.0; 2]; 4];
        reference.uv_scales = [[1.0; 2]; 4];
        reference.cull_mode = if cull == 3 { 1 } else { cull };
        for samples in [1, 4] {
            let actual = render_with_geometry_and_samples(
                &format!("vfx-disc-{case}-actual-{samples}"),
                vec![particle],
                vec![],
                gradient_textures(),
                geometry.clone(),
                samples,
            );
            let expected = render_with_geometry_and_samples(
                &format!("vfx-disc-{case}-expected-{samples}"),
                vec![],
                vec![reference],
                gradient_textures(),
                geometry.clone(),
                samples,
            );
            assert_image(&actual, &expected);
            if cull == 0 {
                assert!(actual.iter().filter(|p| p[0] + p[1] + p[2] > 0.1).count() > 15);
                let mut control = particle;
                control.disc = None;
                control.particle_type = Some(ParticleType::Quad);
                let control = render_with_geometry_and_samples(
                    &format!("vfx-disc-{case}-quad-control-{samples}"),
                    vec![control],
                    vec![],
                    gradient_textures(),
                    geometry.clone(),
                    samples,
                );
                assert!(
                    actual
                        .iter()
                        .zip(control)
                        .any(|(a, b)| (a[0] - b[0]).abs() > 0.1)
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn polygon_matches_client_triangle_fan_for_axes_uv_and_msaa() {
    use xiv_companion_data::avfx::ParticleType;
    use xiv_companion_data::avfx_sim::VfxPolygon;

    let mul = |axis: [f32; 3], scale: f32| axis.map(|value| value * scale);
    let sub = |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|i| a[i] - b[i]);
    for mode in 0..=2 {
        let mut particle = quad([0.8, 0.6, 0.4, 1.0], false);
        particle.particle_type = Some(ParticleType::Polygon);
        particle.rotation_direction_base = mode;
        particle.size = [0.7, 0.45];
        particle.parent_basis = [[0.8, 0.1, 0.2], [-0.2, 0.7, 0.1], [0.3, -0.4, 0.6]];
        particle.uv_origins = [[-0.0123, 0.0209]; 4];
        particle.uv_scales = [[0.827, 0.711]; 4];
        particle.draw_mode = 8;
        particle.polygon = Some(VfxPolygon {
            count: 5,
            scale_z: 1.6,
        });

        let axes = [
            mul(particle.parent_basis[0], particle.size[0]),
            mul(particle.parent_basis[1], particle.size[1]),
            mul(particle.parent_basis[2], 0.8),
        ];
        let plane = match mode {
            0 => [axes[2], axes[1]],
            1 => [axes[0], axes[2]],
            _ => [axes[0], axes[1]],
        };
        let mut geometry = VfxDrawModel::default();
        geometry.vertices.push(VfxDrawVertex {
            position: [0.0; 3],
            position_w: 1.0,
            uvs: [[0.0; 2]; 4],
            color: [255; 4],
            normal: [128, 128, 255, 128],
            tangent: [255, 128, 128, 255],
        });
        for index in 0..5 {
            let angle = index as f32 * std::f32::consts::TAU / 5.0 + std::f32::consts::FRAC_PI_2;
            let radial = [angle.sin(), angle.cos()];
            let position = sub(mul(plane[0], radial[0]), mul(plane[1], radial[1]));
            let base_uv = [0.5 + radial[0] * 0.5, 0.5 + radial[1] * 0.5];
            let uv = std::array::from_fn(|axis| {
                let moved = particle.uv_origins[0][axis]
                    + 0.5
                    + particle.uv_scales[0][axis] * (base_uv[axis] - 0.5);
                ((moved * 1000.0) as i32 as i16) as f32 * 0.001 - 0.5
            });
            geometry.vertices.push(VfxDrawVertex {
                position,
                position_w: 1.0,
                uvs: [uv; 4],
                color: [255; 4],
                normal: [128, 128, 255, 128],
                tangent: [255, 128, 128, 255],
            });
        }
        for index in 0..5_u32 {
            geometry.indices.extend([(index + 1) % 5 + 1, index + 1, 0]);
        }
        let center_uv = std::array::from_fn(|axis| {
            let moved = particle.uv_origins[0][axis] + 0.5;
            ((moved * 1000.0) as i32 as i16) as f32 * 0.001 - 0.5
        });
        geometry.vertices[0].uvs = [center_uv; 4];

        let mut reference = mesh(particle);
        reference.position = [0.0; 3];
        reference.orientation = [0.0, 0.0, 0.0, 1.0];
        reference.parent_basis = xiv_companion::VFX_IDENTITY_BASIS;
        reference.scale = [1.0; 3];
        reference.uv_origins = [[0.0; 2]; 4];
        reference.uv_scales = [[1.0; 2]; 4];
        for samples in [1, 4] {
            let actual = render_with_geometry_and_samples(
                &format!("vfx-polygon-axis-{mode}-actual-{samples}"),
                vec![particle],
                vec![],
                gradient_textures(),
                geometry.clone(),
                samples,
            );
            let expected = render_with_geometry_and_samples(
                &format!("vfx-polygon-axis-{mode}-expected-{samples}"),
                vec![],
                vec![reference],
                gradient_textures(),
                geometry.clone(),
                samples,
            );
            assert_image(&actual, &expected);
            assert!(actual.iter().filter(|pixel| pixel[3] > 0.1).count() > 20);
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn laser_matches_explicit_client_strip_for_axes_mirror_and_msaa() {
    use xiv_companion_data::avfx::ParticleType;
    use xiv_companion_data::avfx_sim::VfxLaser;

    fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }
    fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
    }
    fn mul(v: [f32; 3], scale: f32) -> [f32; 3] {
        [v[0] * scale, v[1] * scale, v[2] * scale]
    }
    fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }
    fn normalize(v: [f32; 3]) -> [f32; 3] {
        let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if length > 1.0e-6 {
            mul(v, length.recip())
        } else {
            [0.0; 3]
        }
    }

    let camera_position = [0.0, 0.0, 3.0];
    let view_direction = [0.0, 0.0, 1.0];
    for (mode, mirror) in [(0, false), (1, false), (2, false), (1, true)] {
        let mirror = if mirror { -1.0 } else { 1.0 };
        let axes: [[f32; 3]; 3] = [
            [0.7 * mirror, 0.1 * mirror, 0.0],
            [0.1 * mirror, 0.7 * mirror, 0.0],
            [-0.6 * mirror, 0.3 * mirror, 0.0],
        ];
        let perpendicular = match mode {
            0 => [axes[1], axes[2]],
            1 => [axes[0], axes[2]],
            _ => [axes[0], axes[1]],
        };
        let width_scale = perpendicular
            .map(|axis| (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt())
            .into_iter()
            .sum::<f32>()
            * 0.5;
        let width = 0.13 * width_scale;
        let start = [-0.15, -0.25, 0.0];
        let beam = axes[mode as usize];
        let end = add(start, beam);
        let cap = |point: [f32; 3]| {
            normalize(cross(
                view_direction,
                cross(sub(point, camera_position), beam),
            ))
        };
        let points = [
            sub(start, mul(cap(start), width)),
            start,
            end,
            add(end, mul(cap(end), width)),
        ];
        let mut geometry = VfxDrawModel::default();
        for point_index in 0_usize..4 {
            let previous = points[point_index.saturating_sub(1)];
            let next = points[(point_index + 1).min(3)];
            let offset = mul(
                normalize(cross(
                    sub(previous, next),
                    sub(points[point_index], camera_position),
                )),
                width,
            );
            for side in 0..2 {
                let world = if side == 0 {
                    sub(points[point_index], offset)
                } else {
                    add(points[point_index], offset)
                };
                let uv = [side as f32 - 0.5, [0.0, 0.5, 0.5, 0.0][point_index] - 0.5];
                geometry.vertices.push(VfxDrawVertex {
                    position: sub(world, start),
                    position_w: 1.0,
                    uvs: [uv; 4],
                    color: [255; 4],
                    normal: [128, 128, 255, 128],
                    tangent: [255, 128, 128, 255],
                });
            }
        }
        geometry.indices = vec![0, 1, 3, 3, 2, 0, 2, 3, 5, 5, 4, 2, 4, 5, 7, 7, 6, 4];

        let mut particle = quad([1.0; 4], false);
        particle.particle_type = Some(ParticleType::Laser);
        particle.position = start;
        particle.size = [1.0; 2];
        particle.parent_basis = axes;
        particle.rotation_direction_base = mode;
        particle.laser = Some(VfxLaser {
            length: 1.0,
            width: 0.13,
            scale: [1.0; 3],
        });
        let mut reference = mesh(particle);
        reference.parent_basis = xiv_companion::VFX_IDENTITY_BASIS;
        reference.scale = [1.0; 3];
        for samples in [1, 4] {
            let actual = render_with_geometry_and_samples(
                &format!("vfx-laser-{mode}-{mirror}-actual-{samples}"),
                vec![particle],
                vec![],
                vec![solid([255; 4])],
                geometry.clone(),
                samples,
            );
            let expected = render_with_geometry_and_samples(
                &format!("vfx-laser-{mode}-{mirror}-expected-{samples}"),
                vec![],
                vec![reference],
                vec![solid([255; 4])],
                geometry.clone(),
                samples,
            );
            assert_image(&actual, &expected);
            assert!(actual.iter().filter(|pixel| pixel[0] > 0.1).count() > 15);
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn non_simple_line_uses_line_list_and_interpolates_endpoint_colors() {
    use xiv_companion_data::avfx::ParticleType;
    use xiv_companion_data::avfx_sim::VfxLine;

    for samples in [1, 4] {
        let mut particle = quad([1.0; 4], false);
        particle.particle_type = Some(ParticleType::Line);
        particle.position = [-0.65, 0.0, 0.0];
        particle.size = [1.0; 2];
        particle.rotation_direction_base = 0;
        particle.line = Some(VfxLine {
            scale: [1.0; 3],
            length: 1.3,
            endpoint_offset: None,
            color_begin: [1.0, 0.0, 0.0, 1.0],
            color_end: [0.0, 0.0, 1.0, 1.0],
        });
        let pixels = render_with_geometry_and_samples(
            &format!("vfx-line-list-{samples}"),
            vec![particle],
            vec![],
            vec![solid([255; 4])],
            square(),
            samples,
        );
        let lit: Vec<_> = pixels
            .iter()
            .filter(|pixel| pixel[0] + pixel[2] > 0.1)
            .collect();
        assert!(lit.len() > 10, "MSAA={samples}: {} lit pixels", lit.len());
        assert!(lit.iter().any(|pixel| pixel[0] > pixel[2] * 2.0));
        assert!(lit.iter().any(|pixel| pixel[2] > pixel[0] * 2.0));
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn ordinary_line_local_scale_rotation_and_parent_reach_pixels() {
    use xiv_companion_data::avfx::ParticleType;
    use xiv_companion_data::avfx_sim::VfxLine;
    // Parent * Rz(pi) * local scale * (axis * length), evaluated explicitly.
    let offsets = [
        [0.25, 0.125, 0.0],
        [-0.375, -0.75, 0.0],
        [-0.125, -0.0625, -0.125],
    ];
    for (mode, offset) in offsets.into_iter().enumerate() {
        let mut actual = quad([1.0; 4], false);
        actual.particle_type = Some(ParticleType::Line);
        actual.rotation_direction_base = mode as i32;
        actual.orientation = [0.0, 0.0, 1.0, 0.0];
        actual.parent_basis = [[1.0, 0.5, 0.0], [0.5, 1.0, 0.0], [1.0, 0.5, 1.0]];
        actual.line = Some(VfxLine {
            scale: [-0.5, 1.5, -0.25],
            length: 0.5,
            endpoint_offset: None,
            color_begin: [1.0, 0.0, 0.0, 1.0],
            color_end: [0.0, 0.0, 1.0, 1.0],
        });
        let mut expected = actual;
        expected.line.as_mut().unwrap().endpoint_offset = Some(offset);
        let mut unscaled = actual;
        unscaled.line.as_mut().unwrap().scale = [1.0; 3];
        for samples in [1, 4] {
            let draw = |name: &str, particle| {
                render_with_geometry_and_samples(
                    &format!("vfx-line-scale-{mode}-{name}-{samples}"),
                    vec![particle],
                    vec![],
                    vec![solid([255; 4])],
                    square(),
                    samples,
                )
            };
            let pixels = draw("actual", actual);
            assert_image(&pixels, &draw("expected", expected));
            assert!(
                pixels
                    .iter()
                    .zip(draw("unscaled", unscaled))
                    .filter(|(a, b)| (a[0] - b[0]).abs() + (a[2] - b[2]).abs() > 0.1)
                    .count()
                    > 5
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn line_world_endpoint_offset_uses_arbitrary_direction() {
    use xiv_companion_data::avfx::ParticleType;
    use xiv_companion_data::avfx_sim::VfxLine;

    for samples in [1, 4] {
        let mut particle = quad([1.0; 4], false);
        particle.particle_type = Some(ParticleType::Line);
        particle.position = [-0.55, -0.45, 0.0];
        particle.rotation_direction_base = 2;
        particle.line = Some(VfxLine {
            scale: [1.0; 3],
            length: 100.0,
            endpoint_offset: Some([1.1, 0.9, 0.0]),
            color_begin: [1.0, 0.0, 0.0, 1.0],
            color_end: [0.0, 1.0, 0.0, 1.0],
        });
        let pixels = render_with_geometry_and_samples(
            &format!("vfx-line-world-offset-{samples}"),
            vec![particle],
            vec![],
            vec![solid([255; 4])],
            square(),
            samples,
        );
        let lit: Vec<_> = pixels
            .iter()
            .filter(|pixel| pixel[0] + pixel[1] > 0.1)
            .collect();
        assert!(lit.len() > 10, "MSAA={samples}: {} lit pixels", lit.len());
        assert!(lit.iter().any(|pixel| pixel[0] > pixel[1] * 2.0));
        assert!(lit.iter().any(|pixel| pixel[1] > pixel[0] * 2.0));
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn line_laser_and_disc_soft_particles_fade_against_scene_depth() {
    use xiv_companion_data::avfx::ParticleType;
    use xiv_companion_data::avfx_sim::{VfxDisc, VfxLaser, VfxLine};

    let mut receiver = quad([0.0, 0.0, 0.0, 1.0], false);
    receiver.size = [10.0; 2];
    receiver.depth_test = true;
    receiver.depth_write = true;

    let mut line = quad([1.0, 0.0, 0.0, 1.0], true);
    line.particle_type = Some(ParticleType::Line);
    line.rotation_direction_base = 0;
    line.position = [-0.7, 0.5, 0.08];
    line.draw_priority = 1;
    line.cull_mode = 1;
    line.line = Some(VfxLine {
        scale: [1.0; 3],
        length: 1.4,
        endpoint_offset: None,
        color_begin: [1.0, 0.0, 0.0, 1.0],
        color_end: [1.0, 0.0, 0.0, 1.0],
    });

    let mut laser = quad([0.0, 1.0, 0.0, 1.0], true);
    laser.particle_type = Some(ParticleType::Laser);
    laser.rotation_direction_base = 0;
    laser.position = [-0.6, 0.0, 0.08];
    laser.draw_priority = 1;
    laser.laser = Some(VfxLaser {
        length: 1.2,
        width: 0.08,
        scale: [1.0; 3],
    });

    let mut disc = quad([0.0, 0.0, 1.0, 1.0], true);
    disc.particle_type = Some(ParticleType::Disc);
    disc.rotation_direction_base = 0;
    disc.position = [0.0, -0.5, 0.08];
    disc.size = [0.3; 2];
    disc.draw_priority = 1;
    disc.orientation = [
        std::f32::consts::FRAC_1_SQRT_2,
        0.0,
        0.0,
        std::f32::consts::FRAC_1_SQRT_2,
    ];
    disc.disc = Some(VfxDisc {
        counts: [1, 3, 16],
        angle: std::f32::consts::TAU,
        radius: [0.8; 2],
        width: [0.2; 2],
        height_inner: [0.0; 2],
        height_outer: [0.0; 2],
        color_inner: [1.0; 4],
        color_outer: [1.0; 4],
        point_interval_factor: 0.0,
        scaling_scale: 1.0,
        scale_z: 1.0,
    });

    let particles = [line, laser, disc];
    let textures = vec![solid([255; 4])];
    for samples in [1, 4] {
        let baseline = render_with_geometry_and_samples(
            &format!("vfx-soft-special-baseline-{samples}"),
            vec![receiver],
            vec![],
            textures.clone(),
            square(),
            samples,
        );
        let mut hard_particles = vec![receiver];
        hard_particles.extend(particles);
        let hard = render_with_geometry_and_samples(
            &format!("vfx-soft-special-hard-{samples}"),
            hard_particles,
            vec![],
            textures.clone(),
            square(),
            samples,
        );
        let mut soft_particles = vec![receiver];
        soft_particles.extend(particles.map(|mut particle| {
            particle.soft_particle = true;
            particle.soft_particle_fade_range = 0.8;
            particle
        }));
        let soft = render_with_geometry_and_samples(
            &format!("vfx-soft-special-fade-{samples}"),
            soft_particles,
            vec![],
            textures.clone(),
            square(),
            samples,
        );
        for (channel, name) in ["Line", "Laser", "Disc"].into_iter().enumerate() {
            let contribution = |pixels: &[[f32; 4]]| {
                pixels
                    .iter()
                    .zip(&baseline)
                    .map(|(pixel, base)| (pixel[channel] - base[channel]).max(0.0))
                    .sum::<f32>()
            };
            let hard_energy = contribution(&hard);
            let soft_energy = contribution(&soft);
            assert!(
                hard_energy > 1.0,
                "MSAA={samples} {name}: hard energy {hard_energy}"
            );
            assert!(
                soft_energy < hard_energy * 0.25,
                "MSAA={samples} {name}: soft {soft_energy}, hard {hard_energy}"
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn linear_edge_polyline_uses_camera_facing_three_column_geometry() {
    use xiv_companion_data::avfx::ParticleType;
    use xiv_companion_data::avfx_sim::VfxPolyline;

    for samples in [1, 4] {
        let mut particle = quad([1.0; 4], false);
        particle.particle_type = Some(ParticleType::Polyline);
        particle.soft_particle = true;
        particle.soft_particle_fade_range = 0.75;
        particle.position = [-0.6, 0.0, 0.0];
        particle.polyline = Some(VfxPolyline {
            point_count: 4,
            point_count_center: 2,
            uv_precision: 0,
            use_edge: true,
            not_billboard_axis: None,
            reverse_points: false,
            uv_span: 1.0,
            positions: {
                let mut points = [[0.0; 3]; 64];
                points[..4].copy_from_slice(&[
                    [-0.6, 0.0, 0.0],
                    [-0.2, 0.0, 0.0],
                    [0.2, 0.0, 0.0],
                    [0.6, 0.0, 0.0],
                ]);
                points
            },
            end_distortion: [u8::MAX; 64],
            widths: [0.12, 0.18, 0.12],
            colors: [[1.0, 0.0, 0.0, 1.0]; 3],
            edge_colors: [[0.0, 0.0, 1.0, 1.0]; 3],
        });
        let pixels = render_with_geometry_and_samples(
            &format!("vfx-polyline-soft-linear-edge-{samples}"),
            vec![particle],
            vec![],
            vec![solid([255; 4])],
            square(),
            samples,
        );
        let lit: Vec<_> = pixels
            .iter()
            .filter(|pixel| pixel[0] + pixel[2] > 0.1)
            .collect();
        assert!(lit.len() > 100, "MSAA={samples}: {} lit pixels", lit.len());
        assert!(lit.iter().any(|pixel| pixel[0] > pixel[2] * 2.0));
        assert!(lit.iter().any(|pixel| pixel[2] > pixel[0] * 2.0));
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn non_edge_polyline_uses_not_billboard_two_column_geometry() {
    use xiv_companion_data::avfx::ParticleType;
    use xiv_companion_data::avfx_sim::VfxPolyline;

    for samples in [1, 4] {
        let background = render_with_geometry_and_samples(
            &format!("vfx-polyline-non-edge-background-{samples}"),
            vec![],
            vec![],
            vec![solid([255; 4])],
            square(),
            samples,
        );
        let mut particle = quad([1.0; 4], false);
        particle.particle_type = Some(ParticleType::Polyline);
        particle.polyline = Some(VfxPolyline {
            point_count: 4,
            point_count_center: 0,
            uv_precision: 0,
            use_edge: false,
            not_billboard_axis: Some([0.0, 1.0, 0.0]),
            reverse_points: false,
            uv_span: 1.0,
            positions: {
                let mut points = [[0.0; 3]; 64];
                points[..4].copy_from_slice(&[
                    [-0.6, 0.0, 0.0],
                    [-0.2, 0.0, 0.0],
                    [0.2, 0.0, 0.0],
                    [0.6, 0.0, 0.0],
                ]);
                points
            },
            end_distortion: [u8::MAX; 64],
            widths: [0.14; 3],
            colors: [[1.0, 0.0, 0.0, 1.0]; 3],
            edge_colors: [[0.0, 0.0, 1.0, 1.0]; 3],
        });
        let pixels = render_with_geometry_and_samples(
            &format!("vfx-polyline-non-edge-{samples}"),
            vec![particle],
            vec![],
            vec![solid([255; 4])],
            square(),
            samples,
        );
        let lit = pixels
            .iter()
            .zip(&background)
            .filter(|(pixel, base)| pixel[0] - base[0] > 0.1)
            .count();
        assert!(lit > 100, "MSAA={samples}: {lit} red Polyline pixels");
        let blue_pixel = pixels
            .iter()
            .zip(&background)
            .enumerate()
            .find(|(_, (pixel, base))| pixel[2] > base[2] + 0.02);
        assert!(
            blue_pixel.is_none(),
            "MSAA={samples}: non-edge writer must not use blue edge colors: {blue_pixel:?}"
        );
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn disc_batches_preserve_different_grid_sizes_and_mixed_particle_order() {
    use xiv_companion_data::avfx::ParticleType;
    use xiv_companion_data::avfx_sim::VfxDisc;
    let mut particles = Vec::new();
    for (index, (kind, counts)) in [
        (ParticleType::Disc, [1, 2, 2]),
        (ParticleType::Quad, [0; 3]),
        (ParticleType::Disc, [2, 2, 3]),
        (ParticleType::Windmill, [0; 3]),
        (ParticleType::Disc, [1, 2, 64]),
        (ParticleType::Disc, [2, 3, 5]),
    ]
    .into_iter()
    .enumerate()
    {
        let mut particle = quad([0.15 + index as f32 * 0.13, 0.5, 0.3, 0.6], false);
        particle.particle_type = Some(kind);
        particle.size = [0.55; 2];
        particle.position[0] = index as f32 * 0.13 - 0.3;
        if kind == ParticleType::Disc {
            let diagonal = std::f32::consts::FRAC_1_SQRT_2;
            particle.orientation = [diagonal, 0.0, 0.0, diagonal];
            particle.disc = Some(VfxDisc {
                counts,
                angle: std::f32::consts::FRAC_PI_2,
                radius: [0.65; 2],
                width: [0.2; 2],
                height_inner: [0.0; 2],
                height_outer: [0.0; 2],
                color_inner: [1.0; 4],
                color_outer: [1.0; 4],
                point_interval_factor: 0.0,
                scaling_scale: 1.0,
                scale_z: 1.1,
            });
        }
        particles.push(particle);
    }
    for samples in [1, 4] {
        for reverse in [false, true] {
            let mut actual = particles.clone();
            if reverse {
                actual.reverse();
            }
            let mut separate_draws = actual.clone();
            // Increasing priorities keep input order but force an independent
            // draw for every particle, including shapes with equal vertex counts.
            for (index, particle) in separate_draws.iter_mut().enumerate() {
                particle.draw_priority = index as i32;
            }
            let actual = render_with_geometry_and_samples(
                "vfx-disc-batches",
                actual,
                vec![],
                vec![solid([200, 160, 80, 255])],
                square(),
                samples,
            );
            let expected = render_with_geometry_and_samples(
                "vfx-disc-separate-draws",
                separate_draws,
                vec![],
                vec![solid([200, 160, 80, 255])],
                square(),
                samples,
            );
            assert_image(&actual, &expected);
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn windmill_uses_client_quadrants_uvs_and_screen_basis() {
    use xiv_companion_data::avfx::ParticleType;
    // Explicit client 0x1403ff320 vertex/UV table, independent of the shader.
    let geometry = VfxDrawModel {
        vertices: [
            ([-1.0, 1.0, 0.0], [0.0, 0.0]),
            ([0.0, 1.0, 0.0], [1.0, 0.0]),
            ([-1.0, 0.0, 0.0], [0.0, 1.0]),
            ([0.0, 0.0, 0.0], [1.0, 1.0]),
            ([0.0, 1.0, 0.0], [0.0, 1.0]),
            ([1.0, 1.0, 0.0], [0.0, 0.0]),
            ([0.0, 0.0, 0.0], [1.0, 1.0]),
            ([1.0, 0.0, 0.0], [1.0, 0.0]),
            ([-1.0, 0.0, 0.0], [1.0, 0.0]),
            ([0.0, 0.0, 0.0], [1.0, 1.0]),
            ([-1.0, -1.0, 0.0], [0.0, 0.0]),
            ([0.0, -1.0, 0.0], [0.0, 1.0]),
            ([0.0, 0.0, 0.0], [1.0, 1.0]),
            ([1.0, 0.0, 0.0], [0.0, 1.0]),
            ([0.0, -1.0, 0.0], [1.0, 0.0]),
            ([1.0, -1.0, 0.0], [0.0, 0.0]),
        ]
        .map(|(position, uv)| VfxDrawVertex {
            position,
            position_w: 1.0,
            normal: [128, 128, 255, 128],
            tangent: [255, 128, 128, 255],
            uvs: [uv.map(|value| value - 0.5); 4],
            color: [255; 4],
        })
        .to_vec(),
        indices: (0..4)
            .flat_map(|q| [3, 1, 0, 0, 2, 3].map(|i| i + q * 4))
            .collect(),
    };
    for uv_type in [0, 1] {
        let mut geometry = geometry.clone();
        if uv_type == 1 {
            for (vertex, uv) in geometry.vertices[4..12].iter_mut().zip([
                [1.0, 0.0],
                [0.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
                [0.0, 1.0],
                [1.0, 1.0],
                [0.0, 0.0],
                [1.0, 0.0],
            ]) {
                vertex.uvs = [uv.map(|value| value - 0.5); 4];
            }
        }
        for (samples, camera) in [
            (1, [0.0, 0.0]),
            (4, [0.0, 0.0]),
            (1, [0.65_f32, -0.3]),
            (4, [0.65_f32, -0.3]),
        ] {
            let mut quads = Vec::new();
            let mut references = Vec::new();
            for (row, sign) in [1.0, -1.0].into_iter().enumerate() {
                for cull in 0..4 {
                    let mut particle = quad([1.0; 4], false);
                    particle.particle_type = Some(ParticleType::Windmill);
                    particle.windmill_uv_type = uv_type;
                    particle.position = [-0.75 + cull as f32 * 0.5, -0.35 + row as f32 * 0.7, 0.0];
                    particle.size = [0.18 * sign, 0.24];
                    particle.orientation = [0.0, 0.2_f32.sin(), 0.0, 0.2_f32.cos()];
                    particle.rotation_direction_base = [10, 6, 9, 5][cull];
                    particle.draw_mode = 8;
                    particle.cull_mode = cull as i32;
                    let mut reference = mesh(particle);
                    reference.rotation_direction_base = 10;
                    let (sy, cy) = camera[0].sin_cos();
                    let (sp, cp) = camera[1].sin_cos();
                    reference.parent_basis = [
                        [cy, 0.0, -sy],
                        [-sy * sp, cp, -cy * sp],
                        [sy * cp, sp, cy * cp],
                    ];
                    reference.cull_mode = if cull == 3 { 1 } else { cull as i32 };
                    references.push(reference);
                    particle.uv_origins = [[0.23, 0.37]; 4];
                    particle.uv_scales = [[-0.4, 0.3]; 4];
                    particle.uv_rotations = [0.9; 4];
                    quads.push(particle);
                }
            }
            let actual = render_with_geometry_and_camera(
                "vfx-windmill",
                quads,
                vec![],
                gradient_textures(),
                geometry.clone(),
                samples,
                camera,
            );
            let expected = render_with_geometry_and_camera(
                "vfx-windmill-reference",
                vec![],
                references,
                gradient_textures(),
                geometry.clone(),
                samples,
                camera,
            );
            assert!(expected.iter().filter(|p| p[0] + p[1] + p[2] > 0.1).count() > 50);
            assert_image(&actual, &expected);
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn windmill_batches_preserve_vertex_counts_texture_rules_and_input_order() {
    use xiv_companion_data::avfx::ParticleType;
    for samples in [1, 4] {
        for reverse in [false, true] {
            let mut actual = Vec::new();
            let mut expected = Vec::new();
            for (index, kind) in [
                ParticleType::Windmill,
                ParticleType::Powder,
                ParticleType::Quad,
                ParticleType::Windmill,
            ]
            .into_iter()
            .enumerate()
            {
                let mut particle = quad([0.2 + index as f32 * 0.2, 0.5, 0.3, 0.5], false);
                particle.particle_type = Some(kind);
                particle.powder_single = kind == ParticleType::Powder;
                particle.size = [0.6; 2];
                particle.position[0] = index as f32 * 0.2 - 0.3;
                // Shared texture settings but distinct shader rules and vertex counts.
                particle.texture_indexes = [0, 1, 1, 1];
                particle.texture_distortion_index = 1;
                particle.distortion_power = 0.4;
                particle.distortion_targets = 1;
                particle.combine_modes = [[1, 1]; 3];
                actual.push(particle);
                let mut reference = particle;
                reference.particle_type = Some(ParticleType::Quad);
                reference.powder_single = false;
                if kind != ParticleType::Quad {
                    reference.texture_indexes[1..].fill(-1);
                    reference.texture_distortion_index = -1;
                }
                expected.push(reference);
            }
            if reverse {
                actual.reverse();
                expected.reverse();
            }
            let textures = vec![solid([128, 200, 64, 255]), solid([80, 40, 90, 128])];
            let rendered = render_with_geometry_and_samples(
                "vfx-windmill-batches",
                actual,
                vec![],
                textures.clone(),
                square(),
                samples,
            );
            let reference = render_with_geometry_and_samples(
                "vfx-windmill-batches-reference",
                expected,
                vec![],
                textures,
                square(),
                samples,
            );
            assert_image(&rendered, &reference);
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn powder_single_uses_client_corners_uvs_and_screen_basis() {
    use xiv_companion_data::avfx::ParticleType;
    let geometry = VfxDrawModel {
        vertices: [
            ([-1.0, 1.0, 0.0], [0.0, 0.0]),
            ([1.0, 1.0, 0.0], [1.0, 0.0]),
            ([-1.0, -1.0, 0.0], [0.0, 1.0]),
            ([1.0, -1.0, 0.0], [1.0, 1.0]),
        ]
        .map(|(position, uv)| VfxDrawVertex {
            position,
            position_w: 1.0,
            normal: [128, 128, 255, 128],
            tangent: [255, 128, 128, 255],
            uvs: [uv.map(|value| value - 0.5); 4],
            color: [255; 4],
        })
        .to_vec(),
        indices: vec![3, 1, 0, 0, 2, 3],
    };
    for (samples, camera) in [
        (1, [0.0, 0.0]),
        (4, [0.0, 0.0]),
        (1, [0.65_f32, -0.3]),
        (4, [0.65_f32, -0.3]),
    ] {
        let mut quads = Vec::new();
        let mut references = Vec::new();
        for (row, sign) in [1.0, -1.0].into_iter().enumerate() {
            for cull in 0..4 {
                let mut particle = quad([1.0; 4], false);
                particle.particle_type = Some(ParticleType::Powder);
                particle.powder_single = true;
                particle.position = [-0.75 + cull as f32 * 0.5, -0.35 + row as f32 * 0.7, 0.0];
                particle.size = [0.18 * sign, 0.24];
                particle.orientation = [0.0, 0.2_f32.sin(), 0.0, 0.2_f32.cos()];
                particle.rotation_direction_base = [10, 6, 9, 5][cull];
                particle.draw_mode = 8;
                particle.cull_mode = cull as i32;
                let mut reference = mesh(particle);
                reference.rotation_direction_base = 10;
                let (sy, cy) = camera[0].sin_cos();
                let (sp, cp) = camera[1].sin_cos();
                reference.parent_basis = [
                    [cy, 0.0, -sy],
                    [-sy * sp, cp, -cy * sp],
                    [sy * cp, sp, cy * cp],
                ];
                // Powder Double directly selects CullBack, without a second draw.
                reference.cull_mode = if cull == 3 { 1 } else { cull as i32 };
                references.push(reference);
                // UvSt is not part of this client vertex path.
                particle.uv_origins = [[0.23, 0.37]; 4];
                particle.uv_scales = [[-0.4, 0.3]; 4];
                particle.uv_rotations = [0.9; 4];
                quads.push(particle);
            }
        }
        let actual = render_with_geometry_and_camera(
            "vfx-powder-single",
            quads,
            vec![],
            gradient_textures(),
            geometry.clone(),
            samples,
            camera,
        );
        let expected = render_with_geometry_and_camera(
            "vfx-powder-single-reference",
            vec![],
            references,
            gradient_textures(),
            geometry.clone(),
            samples,
            camera,
        );
        assert!(expected.iter().filter(|p| p[0] + p[1] + p[2] > 0.1).count() > 50);
        assert_image(&actual, &expected);
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn staged_polyline_history_uses_submitted_draws() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime, avfx::*};
    let mut file = cone_model_render_fixture();
    file.timelines[0].binder_index = -1;
    file.emitters[0] = AvfxEmitter {
        emitter_type: Some(EmitterType::Point),
        effector_index: -1,
        particle_items: vec![AvfxEmitterItem {
            enabled: true,
            target_index: 0,
            create_time: 1,
            create_count: 1,
            create_probability: 100,
            parameter_link: -1,
            parent_influence_coord: 2,
            ..Default::default()
        }],
        ..Default::default()
    };
    let curve = |values: &[(i16, f32)]| AvfxCurve {
        keys: values
            .iter()
            .map(|&(time, z)| AvfxCurveKey {
                time,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
                z,
            })
            .collect(),
        ..Default::default()
    };
    file.particles[0] = AvfxParticle {
        particle_type: Some(ParticleType::Polyline),
        collision_type: -1,
        rotation_direction_base: 2,
        position: AvfxCurve3Axis {
            x: Some(curve(&[(0, -0.5), (4, 0.5)])),
            y: Some(curve(&[(0, -0.5), (1, 0.5), (2, -0.5), (4, 0.5)])),
            ..Default::default()
        },
        data: AvfxParticleData::Polyline(AvfxParticleDataPolyline {
            create_line_type: 1,
            point_count: 4,
            point_count_center: 2,
            width: curve(&[(0, 0.1)]),
            width_begin: curve(&[(0, 1.0)]),
            width_center: curve(&[(0, 1.0)]),
            width_end: curve(&[(0, 1.0)]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut playback = VfxPlayback::new(VfxRuntime::new(&file));
    assert_eq!(playback.fallback_reason(), None);
    let sample = |value: &VfxPlayback| {
        let mut quads = Vec::new();
        value.sample(&mut quads, &mut Vec::new());
        assert_eq!(quads.len(), 1);
        quads[0]
    };
    let _ = sample(&playback);
    for _ in 0..3 {
        playback.advance(1.0 / 30.0).unwrap();
    }
    let actual = sample(&playback);
    let mut expected = actual;
    expected.polyline.as_mut().unwrap().positions[..4].copy_from_slice(&[
        [0.0, -0.5, 0.0],
        [-0.5, -0.5, 0.0],
        [-0.5, -0.5, 0.0],
        [-0.5, -0.5, 0.0],
    ]);
    let mut old = expected;
    old.polyline.as_mut().unwrap().positions[1] = [-0.25, 0.5, 0.0];
    for samples in [1, 4] {
        let draw = |name: &str, particle| {
            render_with_geometry_and_samples(
                &format!("vfx-polyline-draw-history-{name}-{samples}"),
                vec![particle],
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )
        };
        let pixels = draw("actual", actual);
        assert_image(&pixels, &draw("expected", expected));
        assert!(
            pixels
                .iter()
                .zip(draw("update-history", old))
                .filter(|(a, b)| (a[0] - b[0]).abs() > 0.1)
                .count()
                > 20
        );
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn staged_simple_line_substeps_reach_rendering() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime, avfx::*};
    let mut file = cone_model_render_fixture();
    file.timelines[0].binder_index = -1;
    file.emitters[0] = AvfxEmitter {
        emitter_type: Some(EmitterType::Point),
        effector_index: -1,
        particle_items: vec![AvfxEmitterItem {
            enabled: true,
            target_index: 0,
            create_time: 1,
            create_count: 1,
            create_probability: 100,
            parameter_link: -1,
            parent_influence_coord: 2,
            ..Default::default()
        }],
        ..Default::default()
    };
    file.particles[0] = AvfxParticle {
        particle_type: Some(ParticleType::Line),
        collision_type: -1,
        simple_anim_enable: true,
        simple: Some(AvfxParticleSimple {
            create_count: 1,
            create_interval_life: 100,
            create_new_after_delete: true,
            injection_model_index: -1,
            injection_vertex_bind_model_index: -1,
            injection_direction_type: 0,
            line_length_max: 10.0,
            velocity_min: 0.4,
            velocity_max: 0.4,
            coord_accuracy: [1.0; 3],
            scale_start: [0.25; 2],
            scale_end: [0.25; 2],
            scale_rand_x: [1.0; 2],
            scale_rand_y: [1.0; 2],
            uv_cell: [1; 2],
            colors: [[255; 4]; 4],
            ..Default::default()
        }),
        data: AvfxParticleData::Line(AvfxParticleDataLine::default()),
        texture_color1: file.particles[0].texture_color1.clone(),
        ..Default::default()
    };
    let mut playback = VfxPlayback::new(VfxRuntime::new(&file));
    assert_eq!(playback.fallback_reason(), None);
    let sample = |playback: &VfxPlayback| {
        let mut values = Vec::new();
        playback.sample(&mut values, &mut Vec::new());
        values
    };
    assert!(sample(&playback).is_empty());
    playback.advance(0.1 / 30.0).unwrap();
    assert_eq!(sample(&playback)[0].position, [0.0; 3]);
    playback.advance(0.1 / 30.0).unwrap();
    let first = sample(&playback)[0];
    playback.advance(0.5 / 30.0).unwrap();
    let actual = sample(&playback)[0];
    let mut expected = actual;
    expected.position = first.position.map(|value| value * 7.0);
    expected.line.as_mut().unwrap().endpoint_offset = Some(first.position.map(|value| value * 6.0));
    for axis in 0..3 {
        assert!((actual.position[axis] - expected.position[axis]).abs() < 1e-5);
    }
    let mut collapsed = expected;
    collapsed.position = first.position.map(|value| value * 6.0);
    collapsed.line.as_mut().unwrap().endpoint_offset =
        Some(first.position.map(|value| value * 5.0));
    for samples in [1, 4] {
        let draw = |name: &str, particle| {
            render_with_geometry_and_samples(
                &format!("vfx-simple-line-substeps-{name}-{samples}"),
                vec![particle],
                vec![],
                vec![solid([255; 4])],
                square(),
                samples,
            )
        };
        let pixels = draw("actual", actual);
        assert_image(&pixels, &draw("expected", expected));
        assert!(
            pixels
                .iter()
                .zip(draw("collapsed", collapsed))
                .filter(|(a, b)| (a[0] - b[0]).abs() > 0.1)
                .count()
                > 2
        );
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn point_binder_base_scale_and_powder_compensation_reach_rendering() {
    use xiv_companion_data::{VfxRuntime, avfx::*};
    let curve = |z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            x: 0.0,
            y: 0.0,
            z,
            interpolation: 1,
        }],
        ..Default::default()
    };
    let mut file = cone_model_render_fixture();
    file.timelines[0].binder_index = -1;
    file.timelines[0].items[0].binder_index = 0;
    file.binders = vec![AvfxBinder {
        bind_point_id: 3,
        vfx_scale_enabled: true,
        vfx_scale_bias: 1.0,
        ..Default::default()
    }];
    file.emitters[0] = AvfxEmitter {
        emitter_type: Some(EmitterType::Point),
        effector_index: -1,
        scale: AvfxCurve3Axis {
            x: Some(curve(1.0)),
            y: Some(curve(2.0)),
            z: Some(curve(3.0)),
            ..Default::default()
        },
        particle_items: vec![AvfxEmitterItem {
            enabled: true,
            target_index: 0,
            create_time: 1,
            create_count: 1,
            create_probability: 100,
            parameter_link: -1,
            parent_influence_coord: 2,
            ..Default::default()
        }],
        ..Default::default()
    };
    file.particles[0] = AvfxParticle {
        particle_type: Some(ParticleType::Powder),
        collision_type: -1,
        simple_anim_enable: true,
        simple: Some(AvfxParticleSimple {
            create_count: 1,
            create_interval_life: 100,
            injection_model_index: -1,
            injection_vertex_bind_model_index: -1,
            scale_start: [0.25; 2],
            scale_end: [0.25; 2],
            scale_rand_x: [1.0; 2],
            scale_rand_y: [1.0; 2],
            uv_cell: [1; 2],
            colors: [[255; 4]; 4],
            ..Default::default()
        }),
        texture_color1: file.particles[0].texture_color1.clone(),
        ..Default::default()
    };
    let points = [VfxBindPoint {
        id: 3,
        parent_bone: None,
        translate: [0.0; 3],
        rotate: [0.0; 3],
    }];
    for (mode, by_parent, document) in [(2, false, false), (4, true, false), (2, false, true)] {
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.base_direction_type = mode;
        simple.scale_by_parent = by_parent;
        let runtime = VfxRuntime::with_bind_points(&file, &points)
            .with_vfx_scale(2.0)
            .unwrap();
        let runtime = if document {
            // Point applies Document +0x38 even with authored bDSE=false.
            runtime.with_document_scale([2.0, 0.5, 1.0]).unwrap()
        } else {
            runtime
        };
        let mut actual = Vec::new();
        runtime.sample(0.5, &mut actual);
        assert_eq!(actual.len(), 1);
        let mut expected = actual.clone();
        expected[0].position = [0.0; 3];
        expected[0].orientation = [0.0, 0.0, 0.0, 1.0];
        expected[0].size = if document {
            [1.0, 0.25]
        } else if by_parent {
            [0.5, 1.0]
        } else {
            [0.5; 2]
        };
        expected[0].parent_basis = if mode == 4 {
            // Root auxiliary contributes Q first; SBDT 4 cancels Q.
            [[1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 3.0]]
        } else if document {
            [[4.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 6.0]]
        } else {
            [[2.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 6.0]]
        };
        assert_eq!(actual[0].size, expected[0].size);
        assert_eq!(actual[0].parent_basis, expected[0].parent_basis);
        let mut control = expected.clone();
        if mode == 4 {
            control[0].parent_basis = [[2.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 6.0]];
        } else if document {
            control[0].size = [0.5; 2];
        } else {
            control[0].size = [0.25; 2];
        }
        for samples in [1, 4] {
            let draw = |name, packets: &Vec<VfxQuad>| {
                let channel = if document { "document" } else { "query" };
                render_with_geometry_and_samples(
                    &format!("vfx-binder-powder-scale-{channel}-{mode}-{name}-{samples}"),
                    packets.clone(),
                    vec![],
                    vec![solid([80, 220, 160, 255])],
                    square(),
                    samples,
                )
            };
            let pixels = draw("actual", &actual);
            assert_image(&pixels, &draw("reference", &expected));
            assert!(
                pixels
                    .iter()
                    .zip(draw("control", &control))
                    .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                    .count()
                    > 20,
                "SBDT={mode}, bSnP={by_parent}, Document={document}, MSAA={samples}"
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn staged_powder_incremental_motion_and_rebirth_reach_rendering() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime, avfx::*};
    let mut file = cone_model_render_fixture();
    file.timelines[0].binder_index = -1;
    file.emitters[0] = AvfxEmitter {
        emitter_type: Some(EmitterType::Point),
        effector_index: -1,
        particle_items: vec![AvfxEmitterItem {
            enabled: true,
            target_index: 0,
            create_time: 1,
            create_count: 1,
            create_probability: 100,
            parameter_link: -1,
            parent_influence_coord: 2,
            ..Default::default()
        }],
        ..Default::default()
    };
    file.particles[0] = AvfxParticle {
        particle_type: Some(ParticleType::Powder),
        collision_type: -1,
        simple_anim_enable: true,
        simple: Some(AvfxParticleSimple {
            create_count: 1,
            create_interval_life: 1,
            create_new_after_delete: true,
            injection_model_index: -1,
            injection_vertex_bind_model_index: -1,
            injection_direction_type: 2,
            velocity_min: 0.4,
            velocity_max: 0.4,
            coord_accuracy: [1.0; 3],
            scale_start: [0.25; 2],
            scale_end: [0.25; 2],
            scale_rand_x: [1.0; 2],
            scale_rand_y: [1.0; 2],
            uv_cell: [1; 2],
            colors: [[255; 4]; 4],
            ..Default::default()
        }),
        data: AvfxParticleData::Powder {
            use_character_movement: false,
            use_character_location: false,
            is_lightning: false,
            directional_light_type: 0,
            center_offset: 0.0,
        },
        texture_color1: file.particles[0].texture_color1.clone(),
        ..Default::default()
    };
    let mut playback = VfxPlayback::new(VfxRuntime::new(&file));
    assert_eq!(playback.fallback_reason(), None);
    let sample = |playback: &VfxPlayback| {
        let (mut quads, mut meshes) = (Vec::new(), Vec::new());
        playback.sample(&mut quads, &mut meshes);
        assert!(meshes.is_empty());
        quads
    };
    assert!(sample(&playback).is_empty());
    playback.advance(0.1 / 30.0).unwrap();
    let birth = sample(&playback);
    assert_eq!(birth.len(), 1);
    assert_eq!(birth[0].position, [0.0; 3]);
    playback.advance(0.5 / 30.0).unwrap();
    let moved = sample(&playback);
    assert_eq!(sample(&playback), moved);
    assert!((moved[0].position[0] - 0.24).abs() < 1e-5);
    // .5 runs six f32 substeps; four separate .1 inputs finish this life.
    for _ in 0..4 {
        playback.advance(0.1 / 30.0).unwrap();
    }
    let reborn = sample(&playback);
    assert_eq!(reborn, birth);
    let mut reference = birth.clone();
    reference[0].position = [0.24, 0.0, 0.0];
    for samples in [1, 4] {
        let render = |name: &str, particles: Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-powder-incremental-{name}-{samples}"),
                particles,
                vec![],
                vec![solid([80, 220, 160, 255])],
                square(),
                samples,
            )
        };
        let initial = render("birth", birth.clone());
        let actual = render("moved", moved.clone());
        assert_image(&actual, &render("reference", reference.clone()));
        assert_image(&initial, &render("reborn", reborn.clone()));
        assert!(
            initial
                .iter()
                .zip(&actual)
                .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                .count()
                > 20
        );
    }
    // Twenty +e8 calls each execute ten substeps. They must not collapse
    // into a single capped ten-frame call. The first substep only activates.
    file.emitters[0].particle_items[0].start_frame = 20;
    file.emitters[0].particle_items[0].start_frame_null_update = true;
    let simple = file.particles[0].simple.as_mut().unwrap();
    simple.create_interval_life = 0;
    simple.velocity_min = 0.02;
    simple.velocity_max = 0.02;
    let warmed = VfxPlayback::new(VfxRuntime::new(&file));
    assert_eq!(warmed.fallback_reason(), None);
    let warmed = sample(&warmed);
    assert_eq!(warmed.len(), 1);
    assert!((warmed[0].position[0] - 0.398).abs() < 1e-5);
    let mut expected = birth.clone();
    expected[0].position = [0.398, 0.0, 0.0];
    let mut collapsed = expected.clone();
    collapsed[0].position[0] = 0.198;
    for samples in [1, 4] {
        let draw = |name: &str, particles: Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-powder-prewarm-{name}-{samples}"),
                particles,
                vec![],
                vec![solid([80, 220, 160, 255])],
                square(),
                samples,
            )
        };
        let actual = draw("actual", warmed.clone());
        assert_image(&actual, &draw("reference", expected.clone()));
        let control = draw("collapsed-time", collapsed.clone());
        assert!(
            actual
                .iter()
                .zip(control)
                .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                .count()
                > 20
        );
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn initial_particle_space_stays_fixed_beneath_a_following_emitter() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime, avfx::*};
    let constant = |value| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
            z: value,
        }],
        ..Default::default()
    };
    let item = AvfxEmitterItem {
        enabled: true,
        target_index: 0,
        create_time: 1,
        create_count: 1,
        create_probability: 100,
        parameter_link: -1,
        ..Default::default()
    };
    let mut file = cone_model_render_fixture();
    file.timelines[0].binder_index = -1;
    file.emitters = vec![
        AvfxEmitter {
            emitter_type: Some(EmitterType::Point),
            effector_index: -1,
            position: AvfxCurve3Axis {
                x: Some(AvfxCurve {
                    keys: vec![
                        AvfxCurveKey {
                            time: 0,
                            interpolation: 1,
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                        AvfxCurveKey {
                            time: 30,
                            interpolation: 1,
                            x: 0.0,
                            y: 0.0,
                            z: 3.0,
                        },
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
            emitter_items: vec![AvfxEmitterItem {
                target_index: 1,
                parent_influence_coord: 2,
                ..item
            }],
            ..Default::default()
        },
        AvfxEmitter {
            emitter_type: Some(EmitterType::Point),
            effector_index: -1,
            particle_items: vec![AvfxEmitterItem {
                parent_influence_coord: 3,
                ..item
            }],
            ..Default::default()
        },
    ];
    file.particles = vec![AvfxParticle {
        particle_type: Some(ParticleType::Quad),
        collision_type: -1,
        rotation_direction_base: rotation_direction_base::NONE,
        position: AvfxCurve3Axis {
            x: Some(constant(0.5)),
            ..Default::default()
        },
        texture_color1: file.particles[0].texture_color1.clone(),
        ..Default::default()
    }];
    let mut playback = VfxPlayback::new(VfxRuntime::new(&file));
    assert_eq!(playback.fallback_reason(), None);
    let (mut birth, mut meshes) = (Vec::new(), Vec::new());
    playback.sample(&mut birth, &mut meshes);
    assert_eq!(birth.len(), 1);
    assert_eq!(birth[0].position, [0.5, 0.0, 0.0]);
    for _ in 0..20 {
        playback.advance(1.0 / 30.0).unwrap();
    }
    let mut actual = Vec::new();
    playback.sample(&mut actual, &mut meshes);
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].position, birth[0].position);
    let mut drifting = birth.clone();
    drifting[0].position[0] += 1.9;
    for samples in [1, 4] {
        let draw = |name: &str, particles: Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-initial-ancestor-{name}-{samples}"),
                particles,
                vec![],
                vec![solid([80, 220, 160, 255])],
                square(),
                samples,
            )
        };
        let pixels = draw("actual", actual.clone());
        assert_image(&pixels, &draw("birth-reference", birth.clone()));
        let control = draw("drifting-control", drifting.clone());
        assert!(
            pixels
                .iter()
                .zip(control)
                .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                .count()
                > 20
        );
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn powder_simple_uses_client_corners_uvs_pivot_and_facing_bases() {
    use xiv_companion_data::avfx::ParticleType;
    let pivot = [0.3, -0.4];
    let geometry = VfxDrawModel {
        vertices: [
            ([-1.0, 1.0, 0.0], [0.0, 0.0]),
            ([1.0, 1.0, 0.0], [1.0, 0.0]),
            ([-1.0, -1.0, 0.0], [0.0, 1.0]),
            ([1.0, -1.0, 0.0], [1.0, 1.0]),
        ]
        .map(|(position, uv)| VfxDrawVertex {
            position: [position[0] + pivot[0], position[1] + pivot[1], position[2]],
            position_w: 1.0,
            normal: [128, 128, 255, 128],
            tangent: [255, 128, 128, 255],
            uvs: [uv.map(|value| value - 0.5); 4],
            color: [255; 4],
        })
        .to_vec(),
        indices: vec![3, 1, 0, 0, 2, 3],
    };
    for (facing, basis) in [
        (5, xiv_companion_data::VFX_IDENTITY_BASIS),
        (10, [[1.0, 0.1, 0.2], [0.3, 0.7, 0.0], [0.2, 0.3, 1.0]]),
        (10, [[1.0, 0.1, 0.2], [-0.2, -0.3, -1.0], [0.3, 0.7, 0.0]]),
    ] {
        for (samples, camera) in [(1, [0.0, 0.0]), (4, [0.65_f32, -0.3])] {
            let mut quads = Vec::new();
            let mut references = Vec::new();
            for (row, sign) in [1.0, -1.0].into_iter().enumerate() {
                for cull in 0..4 {
                    let mut particle = quad([1.0; 4], false);
                    particle.particle_type = Some(ParticleType::Powder);
                    particle.rotation_direction_base = facing;
                    particle.parent_basis = basis;
                    particle.position = [-0.75 + cull as f32 * 0.5, -0.35 + row as f32 * 0.7, 0.0];
                    particle.size = [0.18 * sign, 0.24];
                    particle.orientation = [0.0, 0.2_f32.sin(), 0.0, 0.2_f32.cos()];
                    particle.pivot = pivot;
                    particle.uv_origins[0] = [-0.125, 0.25];
                    particle.uv_scales[0] = [if cull % 2 == 0 { 0.25 } else { -0.25 }, 0.5];
                    particle.draw_mode = 8;
                    particle.cull_mode = cull as i32;
                    let mut reference = mesh(particle);
                    reference.rotation_direction_base = 10;
                    let (sy, cy) = camera[0].sin_cos();
                    let (sp, cp) = camera[1].sin_cos();
                    reference.parent_basis = if facing == 5 {
                        [
                            [cy, 0.0, -sy],
                            [-sy * sp, cp, -cy * sp],
                            [sy * cp, sp, cy * cp],
                        ]
                    } else {
                        basis
                    };
                    reference.cull_mode = if cull == 3 { 1 } else { cull as i32 };
                    references.push(reference);
                    quads.push(particle);
                }
            }
            let actual = render_with_geometry_and_camera(
                "vfx-powder-simple",
                quads,
                vec![],
                gradient_textures(),
                geometry.clone(),
                samples,
                camera,
            );
            let expected = render_with_geometry_and_camera(
                "vfx-powder-simple-reference",
                vec![],
                references,
                gradient_textures(),
                geometry.clone(),
                samples,
                camera,
            );
            assert!(expected.iter().filter(|p| p[0] + p[1] + p[2] > 0.1).count() > 50);
            assert_image(&actual, &expected);
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn powder_ignores_secondary_color_layers_and_distortion() {
    use xiv_companion_data::avfx::ParticleType;
    let textures = vec![
        black_white_texture(),
        solid([64, 128, 192, 128]),
        solid([255; 4]),
    ];
    for msaa_samples in [1, 4] {
        for distortion in [false, true] {
            let mut reference = quad([0.6, 0.7, 0.8, 0.75], false);
            reference.texture_filters[0] = 0;
            let mut changed = reference;
            if distortion {
                changed.texture_distortion_index = 2;
                changed.distortion_power = 1.0;
                changed.distortion_targets = 1;
            } else {
                changed.texture_indexes = [0, 1, 1, 1];
                changed.combine_modes = [[1, 1], [2, 2], [3, 0]];
            }
            let render_particle = |name: &str, particle| {
                render_with_geometry_and_samples(
                    name,
                    vec![particle],
                    vec![],
                    textures.clone(),
                    square(),
                    msaa_samples,
                )
            };
            let expected = render_particle("vfx-powder-texture-reference", reference);
            let ordinary = render_particle("vfx-powder-texture-ordinary-control", changed);
            assert!(
                ordinary
                    .iter()
                    .zip(&expected)
                    .any(|(a, b)| (a[0] - b[0]).abs() > 0.05)
            );
            changed.particle_type = Some(ParticleType::Powder);
            // Powder's packed corner UVs reverse both axes relative to Quad.
            reference.uv_scales[0] = [-1.0; 2];
            let expected = render_particle("vfx-powder-packed-uv-reference", reference);
            assert_image(
                &render_particle("vfx-powder-texture-filtered", changed),
                &expected,
            );

            // Alternating types with identical texture settings must not reuse
            // the other type's bind group, regardless of insertion order.
            let mut left = changed;
            left.size = [0.45, 0.8];
            left.position[0] = -0.5;
            let mut right = left;
            right.position[0] = 0.5;
            right.particle_type = Some(ParticleType::Quad);
            let mut clean_left = left;
            clean_left.texture_indexes[1..].fill(-1);
            clean_left.texture_distortion_index = -1;
            clean_left.particle_type = Some(ParticleType::Quad);
            clean_left.uv_scales[0] = [-1.0; 2];
            let expected = render_with_geometry_and_samples(
                "vfx-powder-mixed-reference",
                vec![clean_left, right],
                vec![],
                textures.clone(),
                square(),
                msaa_samples,
            );
            for instances in [vec![left, right], vec![right, left]] {
                assert_image(
                    &render_with_geometry_and_samples(
                        "vfx-powder-mixed",
                        instances,
                        vec![],
                        textures.clone(),
                        square(),
                        msaa_samples,
                    ),
                    &expected,
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn powder_and_windmill_final_color_preserves_signed_rgb_and_tc1_controls() {
    use xiv_companion_data::avfx::ParticleType;
    let background = [0.2, 0.3, 0.4, 1.0];
    let source = [-0.4, 0.4, 2.0, 2.0];
    let texel = [64.0 / 255.0, 128.0 / 255.0, 192.0 / 255.0, 128.0 / 255.0];
    for msaa_samples in [1, 4] {
        for draw_mode in [0, 1, 2, 4, 8] {
            let mut backdrop = quad(background, false);
            backdrop.texture1_enabled = false;
            backdrop.draw_priority = -1;
            let mut instances = vec![backdrop];
            let mut expected = Vec::new();
            for kind in [
                ParticleType::Quad,
                ParticleType::Powder,
                ParticleType::Windmill,
            ] {
                let powder = kind != ParticleType::Quad;
                for (enabled, channels, color_to_alpha) in [
                    (true, [1, 1], false),
                    (true, [0, 0], false),
                    (true, [1, 1], true),
                    (false, [1, 1], true),
                ] {
                    let index = expected.len();
                    let mut particle = quad(source, false);
                    particle.particle_type = Some(kind);
                    particle.draw_mode = draw_mode;
                    particle.size = [0.18; 2];
                    particle.position = [
                        -0.75 + (index % 4) as f32 * 0.5,
                        0.6 - (index / 4) as f32 * 0.6,
                        0.0,
                    ];
                    particle.texture1_enabled = enabled;
                    particle.combine_mode_tc1 = channels;
                    particle.color_to_alpha[0] = color_to_alpha;
                    instances.push(particle);
                    let layer = if !enabled || channels == [0, 0] {
                        [1.0; 4]
                    } else if color_to_alpha {
                        [
                            1.0,
                            1.0,
                            1.0,
                            texel[0] * 0.298912 + texel[1] * 0.586611 + texel[2] * 0.114478,
                        ]
                    } else {
                        texel
                    };
                    let raw_alpha = source[3] * layer[3];
                    let alpha = raw_alpha.min(1.0);
                    expected.push(std::array::from_fn(|channel| {
                        if channel == 3 {
                            return if draw_mode == 8 { alpha } else { 1.0 };
                        }
                        let mut rgb = source[channel] * layer[channel];
                        if !powder {
                            rgb = rgb.max(0.0);
                        }
                        let dst = background[channel];
                        match draw_mode {
                            0 => rgb * alpha + dst * (1.0 - alpha),
                            1 => dst * (1.0 + alpha * (rgb.clamp(0.0, 1.0) - 1.0)),
                            2 => rgb * alpha + dst,
                            4 => {
                                let src = (rgb * raw_alpha).clamp(0.0, 1.0);
                                src + dst * (1.0 - src)
                            }
                            8 => rgb,
                            _ => unreachable!(),
                        }
                    }));
                }
            }
            let pixels = render_with_geometry_and_samples(
                "vfx-powder-final-color",
                instances,
                vec![],
                vec![solid([64, 128, 192, 128])],
                square(),
                msaa_samples,
            );
            for (index, expected) in expected.into_iter().enumerate() {
                eprintln!(
                    "type_row={}, tc1_case={}, RMT={draw_mode}, MSAA={msaa_samples}",
                    index / 4,
                    index % 4
                );
                assert_color(
                    pixels[[17, 32, 47][index / 4] * 64 + [13, 26, 38, 51][index % 4]],
                    expected,
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn powder_double_culling_retains_only_front_faces() {
    use xiv_companion_data::avfx::ParticleType;
    for msaa_samples in [1, 4] {
        let mut backdrop = quad([0.0, 0.0, 0.0, 1.0], false);
        backdrop.draw_priority = -1;
        let mut instances = vec![backdrop];
        for mirror in [false, true] {
            for cull_mode in 0..=3 {
                let mut particle = quad([0.8, 0.4, 0.2, 1.0], false);
                particle.particle_type = Some(ParticleType::Powder);
                particle.cull_mode = cull_mode;
                particle.size = [if mirror { -0.2 } else { 0.2 }, 0.2];
                particle.position = [
                    -0.75 + cull_mode as f32 * 0.5,
                    if mirror { -0.5 } else { 0.5 },
                    0.0,
                ];
                instances.push(particle);
            }
        }
        let pixels = render_with_geometry_and_samples(
            "vfx-powder-cull",
            instances,
            vec![],
            vec![solid([255; 4])],
            square(),
            msaa_samples,
        );
        for (row, is_front) in [true, false].into_iter().enumerate() {
            for cull_mode in 0..=3 {
                let visible =
                    cull_mode == 0 || (is_front && cull_mode != 2) || (!is_front && cull_mode == 2);
                assert_color(
                    pixels[[19, 45][row] * 64 + [13, 26, 38, 51][cull_mode]],
                    if visible {
                        [0.8, 0.4, 0.2, 1.0]
                    } else {
                        [0.0, 0.0, 0.0, 1.0]
                    },
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn quad_culling_respects_winding_facing_and_mirrored_transforms() {
    use xiv_companion_data::avfx::rotation_direction_base as facing;
    let background = render_center("vfx-cull-background", vec![], vec![]);
    let source = [0.8, 0.4, 0.2, 1.0];
    for (label, mode, size_x, parent_x, rotation, is_front) in [
        ("fixed", facing::NONE, 1.0, 1.0, [0.0, 0.0, 0.0, 1.0], false),
        (
            "screen",
            facing::SCREEN_BILLBOARD,
            1.0,
            1.0,
            [0.0, 0.0, 0.0, 1.0],
            true,
        ),
        (
            "camera",
            facing::CAMERA_BILLBOARD,
            1.0,
            1.0,
            [0.0, 0.0, 0.0, 1.0],
            false,
        ),
        (
            "tree",
            facing::TREE_BILLBOARD,
            1.0,
            1.0,
            [0.0, 0.0, 0.0, 1.0],
            false,
        ),
        (
            "local-mirror",
            facing::CAMERA_BILLBOARD,
            -1.0,
            1.0,
            [0.0, 0.0, 0.0, 1.0],
            true,
        ),
        (
            "parent-mirror",
            facing::CAMERA_BILLBOARD,
            1.0,
            -1.0,
            [0.0, 0.0, 0.0, 1.0],
            true,
        ),
        (
            "half-turn",
            facing::CAMERA_BILLBOARD,
            1.0,
            1.0,
            [0.0, 1.0, 0.0, 0.0],
            true,
        ),
    ] {
        // AVFXTools InitQuad's (2,0,1)/(3,2,1) indices face local -Z.
        // From the +Z camera, the fixed and camera-facing quads retain local -Z winding.
        for cull_mode in [0, 1, 2] {
            let mut instance = quad(source, false);
            instance.rotation_direction_base = mode;
            instance.size[0] = size_x;
            instance.parent_basis[0][0] = parent_x;
            instance.orientation = rotation;
            instance.cull_mode = cull_mode;
            let actual = render_center(
                &format!("vfx-cull-{label}-{cull_mode}"),
                vec![instance],
                vec![],
            );
            // Client CulT selects the retained face: 1 -> D3D CULL_BACK,
            // 2 -> D3D CULL_FRONT, both with FrontCounterClockwise=true.
            let hidden = (cull_mode == 1 && !is_front) || (cull_mode == 2 && is_front);
            assert_color(actual, if hidden { background } else { source });
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn quad_batches_keep_culling_changes_in_input_order() {
    let mut first = quad([1.0, 0.0, 0.0, 1.0], false);
    first.rotation_direction_base =
        xiv_companion_data::avfx::rotation_direction_base::SCREEN_BILLBOARD;
    first.cull_mode = 1;
    let mut hidden = first;
    hidden.cull_mode = 2;
    hidden.color = [0.0, 1.0, 0.0, 1.0];
    let mut last = first;
    last.color = [0.0, 0.0, 1.0, 0.5];
    assert_color(
        render_center("vfx-cull-batch-order", vec![first, hidden, last], vec![]),
        [0.5, 0.0, 0.5, 1.0],
    );
}

#[test]
#[ignore = "requires native wgpu"]
fn quad_double_culling_draws_back_then_front_per_definition_batch() {
    use xiv_companion_data::avfx::ParticleType;
    // Front red is closer than back blue. Within one definition the client
    // streams both quads, then draws all back faces before all front faces.
    let cases = [
        (false, false, false, false, false, [0.5, 0.0, 0.25, 1.0]),
        (true, false, false, false, false, [0.5, 0.0, 0.25, 1.0]),
        (false, true, false, false, false, [0.25, 0.0, 0.5, 1.0]),
        (false, false, true, false, false, [0.5, 0.0, 0.25, 1.0]),
        (false, true, true, false, false, [0.5, 0.0, 0.0, 1.0]),
        (false, false, false, true, false, [0.25, 0.0, 0.5, 1.0]),
        (false, false, true, true, false, [0.5, 0.0, 0.0, 1.0]),
        (false, false, false, false, true, [0.25, 0.0, 0.5, 1.0]),
    ];
    for msaa_samples in [1, 4] {
        let mut background = quad([0.0, 0.0, 0.0, 1.0], false);
        background.draw_priority = -1;
        let mut instances = vec![background];
        for (index, (reverse, split, write, mirror, fallback, _)) in cases.into_iter().enumerate() {
            let mut red = quad([1.0, 0.0, 0.0, 0.5], false);
            red.particle_index = index * 2;
            if fallback {
                // Line still uses the unsupported-geometry quad fallback.
                red.particle_type = Some(ParticleType::Line);
            }
            red.cull_mode = 3;
            red.depth_test = true;
            red.depth_write = write;
            red.size = [-0.2, 0.2];
            red.parent_basis[0][0] = if mirror { -1.0 } else { 1.0 };
            red.position = [
                -0.75 + (index % 4) as f32 * 0.5,
                0.5 - (index / 4) as f32,
                0.1,
            ];
            let mut blue = red;
            blue.color = [0.0, 0.0, 1.0, 0.5];
            blue.position[2] = -0.1;
            blue.size[0] = 0.2;
            if split {
                blue.particle_index += 1;
            }
            instances.extend(if reverse { [blue, red] } else { [red, blue] });
        }
        let pixels = render_with_geometry_and_samples(
            &format!("vfx-quad-double-cull-{msaa_samples}"),
            instances,
            vec![],
            vec![solid([255; 4])],
            square(),
            msaa_samples,
        );
        for (index, (reverse, split, write, mirror, fallback, expected)) in
            cases.into_iter().enumerate()
        {
            let x = [13, 26, 38, 51][index % 4];
            let y = [19, 45][index / 4];
            eprintln!(
                "reverse={reverse}, split={split}, write={write}, mirror={mirror}, \
                 fallback={fallback}, MSAA={msaa_samples}"
            );
            assert_color(pixels[y * 64 + x], expected);
        }

        // 4095 quads fill the client's streamed vertex window. The next quad
        // starts a new Double batch even when its definition and TC1 match.
        let mut red = quad([1.0, 0.0, 0.0, 0.5], false);
        red.cull_mode = 3;
        red.size[0] = -1.0;
        let mut invisible = red;
        invisible.color[3] = 0.0;
        let mut blue = red;
        blue.size[0] = 1.0;
        blue.color = [0.0, 0.0, 1.0, 0.5];
        let mut quads = vec![invisible; 4096];
        quads[0] = red;
        quads[4095] = blue;
        let pixels = render_with_geometry_and_samples(
            &format!("vfx-quad-double-capacity-{msaa_samples}"),
            quads,
            vec![mesh(background)],
            vec![solid([255; 4])],
            square(),
            msaa_samples,
        );
        assert_color(pixels[32 * 64 + 32], [0.25, 0.0, 0.5, 1.0]);
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn mesh_double_culling_draws_back_then_front_for_each_instance() {
    // Put front-facing red triangles first in the index buffer. The client
    // draws back-facing blue triangles first, regardless of index order.
    let mut geometry = square();
    for vertex in &mut geometry.vertices {
        vertex.position[2] = 0.25;
        vertex.color = [255, 0, 0, 255];
    }
    let back = square().vertices.into_iter().map(|mut vertex| {
        vertex.position[2] = -0.25;
        vertex.color = [0, 0, 255, 255];
        vertex
    });
    geometry.vertices.extend(back);
    geometry.indices.extend([6, 5, 4, 7, 6, 4]);
    let cases = [
        (false, false, 1, [0.5, 0.0, 0.25, 1.0]),
        (false, false, 2, [0.625, 0.0, 0.3125, 1.0]),
        (false, true, 1, [0.5, 0.0, 0.25, 1.0]),
        (false, true, 2, [0.75, 0.0, 0.125, 1.0]),
        (true, false, 1, [0.25, 0.0, 0.5, 1.0]),
        (true, false, 2, [0.3125, 0.0, 0.625, 1.0]),
        (true, true, 1, [0.5, 0.0, 0.0, 1.0]),
        (true, true, 2, [0.75, 0.0, 0.0, 1.0]),
    ];
    for msaa_samples in [1, 4] {
        for soft_particle in [false, true] {
            let mut background = quad([0.0, 0.0, 0.0, 1.0], false);
            background.draw_priority = -1;
            let mut instances = Vec::new();
            for (index, (mirrored, depth_write, copies, _)) in cases.into_iter().enumerate() {
                let mut instance = mesh(quad([1.0, 1.0, 1.0, 0.5], false));
                instance.cull_mode = 3;
                instance.depth_test = true;
                instance.depth_write = depth_write;
                instance.soft_particle = soft_particle;
                instance.soft_particle_fade_range = 0.01;
                instance.scale = [0.2, 0.2, 1.0];
                instance.parent_basis[0][0] = if mirrored { -1.0 } else { 1.0 };
                instance.position = [
                    -0.75 + (index % 4) as f32 * 0.5,
                    0.5 - (index / 4) as f32,
                    0.0,
                ];
                instances.extend(std::iter::repeat_n(instance, copies));
            }
            let pixels = render_with_geometry_and_samples(
                &format!("vfx-mesh-double-cull-{msaa_samples}-{soft_particle}"),
                vec![background],
                instances,
                vec![solid([255; 4])],
                geometry.clone(),
                msaa_samples,
            );
            for (index, (mirrored, depth_write, copies, expected)) in cases.into_iter().enumerate()
            {
                let x = [13, 26, 38, 51][index % 4];
                let y = [19, 45][index / 4];
                eprintln!(
                    "mirror={mirrored}, write={depth_write}, copies={copies}, soft={soft_particle}, MSAA={msaa_samples}"
                );
                assert_color(pixels[y * 64 + x], expected);
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn blend_and_add_use_their_declared_equations() {
    let background = render_center("vfx-equation-background", vec![], vec![]);
    let source = [0.8, 0.4, 0.2, 0.25];
    for mesh_mode in [false, true] {
        for add in [false, true] {
            let instance = quad(source, add);
            let actual = render_center(
                &format!("vfx-equation-{mesh_mode}-{add}"),
                if mesh_mode { vec![] } else { vec![instance] },
                if mesh_mode {
                    vec![mesh(instance)]
                } else {
                    vec![]
                },
            );
            assert_color(
                actual,
                std::array::from_fn(|i| {
                    if add {
                        if i == 3 {
                            background[3]
                        } else {
                            source[i] * source[3] + background[i]
                        }
                    } else {
                        if i == 3 {
                            source[3] + background[3] * (1.0 - source[3])
                        } else {
                            source[i] * source[3] + background[i] * (1.0 - source[3])
                        }
                    }
                }),
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn multiply_and_screen_use_alpha_weighted_equations() {
    let mut destination = quad([0.6, 0.3, 0.9, 1.0], false);
    destination.draw_priority = -1;
    let background = render_center("vfx-mode-destination", vec![destination], vec![]);
    for draw_mode in [1, 4] {
        for alpha in [0.0, 0.25, 1.0] {
            let mut particle = quad([0.8, 0.4, 0.2, alpha], false);
            particle.draw_mode = draw_mode;
            let expected = std::array::from_fn(|i| {
                if i == 3 {
                    background[i]
                } else if draw_mode == 1 {
                    background[i] * (1.0 - alpha + particle.color[i] * alpha)
                } else {
                    particle.color[i] * alpha + background[i] * (1.0 - particle.color[i] * alpha)
                }
            });
            for mesh_mode in [false, true] {
                assert_color(
                    render_center(
                        &format!("vfx-mode-{draw_mode}-{alpha}-{mesh_mode}"),
                        if mesh_mode {
                            vec![destination]
                        } else {
                            vec![destination, particle]
                        },
                        if mesh_mode {
                            vec![mesh(particle)]
                        } else {
                            vec![]
                        },
                    ),
                    expected,
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn client_draw_modes_use_shader_and_hardware_blend_equations() {
    // Client RMT table plus NoneControl / LerpWhite / ModulateAlpha DXBC.
    // The preview keeps coverage alpha; the client's scene alpha has another role.
    let modes = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, -1, 13, 1000];
    let source = [1.6_f32, 0.4, 0.2];
    let destination = [0.6_f32, 0.3, 0.9, 1.0];
    for msaa_samples in [1, 4] {
        for mesh_mode in [false, true] {
            for alpha in [0.0_f32, 0.25, 1.0, 2.0] {
                let mut background = quad(destination, false);
                background.draw_priority = -1;
                let mut quads = vec![background];
                let mut meshes = Vec::new();
                for (index, draw_mode) in modes.into_iter().enumerate() {
                    let mut particle = quad([source[0], source[1], source[2], alpha], false);
                    particle.draw_mode = draw_mode;
                    particle.size = [0.2; 2];
                    particle.position = [
                        -0.75 + (index % 4) as f32 * 0.5,
                        0.75 - (index / 4) as f32 * 0.5,
                        0.0,
                    ];
                    if mesh_mode {
                        meshes.push(mesh(particle));
                    } else {
                        quads.push(particle);
                    }
                }
                let pixels = render_with_geometry_and_samples(
                    &format!("vfx-client-rmt-{msaa_samples}-{mesh_mode}-{alpha}"),
                    quads,
                    meshes,
                    vec![solid([255; 4])],
                    square(),
                    msaa_samples,
                );
                for (index, draw_mode) in modes.into_iter().enumerate() {
                    let expected = std::array::from_fn(|channel| {
                        let dst = destination[channel];
                        if alpha == 0.0 || channel == 3 {
                            return if draw_mode == 8 && alpha != 0.0 && channel == 3 {
                                alpha.min(1.0)
                            } else {
                                dst
                            };
                        }
                        let src = source[channel];
                        let coverage = alpha.min(1.0);
                        match draw_mode {
                            0 => src * coverage + dst * (1.0 - coverage),
                            1 | 9 => dst * (1.0 - coverage + src.min(1.0) * coverage),
                            3 | 11 => dst - src * coverage,
                            4 | 12 => {
                                let screen = (src * alpha).min(1.0);
                                screen + dst * (1.0 - screen)
                            }
                            5 => src * (1.0 - dst),
                            6 => src.min(dst),
                            7 => src.max(dst),
                            8 => src,
                            _ => dst + src * coverage,
                        }
                    });
                    // At z=0 the 45-degree camera projects these cell centers here.
                    let centers = [13, 26, 38, 51];
                    let actual = pixels[centers[index / 4] * 64 + centers[index % 4]];
                    eprintln!(
                        "RMT={draw_mode}, alpha={alpha}, mesh={mesh_mode}, MSAA={msaa_samples}"
                    );
                    assert_color(actual, expected);
                }
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn curve_add_and_rgb_repeat_reach_quad_and_mesh_rendering() {
    use xiv_companion_data::{VfxRuntime, avfx::*};
    let curve = |values: &[(i16, f32)]| AvfxCurve {
        post_behavior: BEHAVIOR_ADD,
        keys: values
            .iter()
            .map(|&(time, z)| AvfxCurveKey {
                time,
                z,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
            })
            .collect(),
        ..Default::default()
    };
    let mut file = AvfxFile {
        timelines: vec![AvfxTimeline {
            items: vec![AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: -1,
                effector_index: -1,
                emitter_index: 0,
                platform: 0,
                clip_index: -1,
            }],
            ..Default::default()
        }],
        emitters: vec![AvfxEmitter {
            emitter_type: Some(EmitterType::Point),
            particle_items: vec![AvfxEmitterItem {
                enabled: true,
                target_index: 0,
                create_time: 1,
                create_count: 1,
                create_probability: 100,
                parameter_link: -1,
                parent_influence_coord: 0,
                ..Default::default()
            }],
            ..Default::default()
        }],
        particles: vec![AvfxParticle {
            collision_type: -1,
            rotation_direction_base: rotation_direction_base::NONE,
            position: AvfxCurve3Axis {
                x: Some(curve(&[(0, -0.5), (4, 0.0), (8, -0.25)])),
                ..Default::default()
            },
            rotation: AvfxCurve3Axis {
                z: Some(curve(&[(0, 0.0), (8, std::f32::consts::FRAC_PI_4)])),
                ..Default::default()
            },
            scale: AvfxCurve3Axis {
                x: Some(curve(&[(0, 0.5), (8, 1.0)])),
                y: Some(curve(&[(0, 0.5)])),
                ..Default::default()
            },
            gravity: curve(&[(0, 0.0), (8, 1.0 / 128.0)]),
            color: AvfxColorCurve {
                rgb: Some(AvfxCurve {
                    post_behavior: BEHAVIOR_ADD,
                    keys: vec![
                        AvfxCurveKey {
                            time: 0,
                            interpolation: 1,
                            x: 0.25,
                            y: 0.5,
                            z: 0.125,
                        },
                        AvfxCurveKey {
                            time: 8,
                            interpolation: 1,
                            x: 0.75,
                            y: 0.25,
                            z: 0.5,
                        },
                    ],
                    ..Default::default()
                }),
                alpha: Some(curve(&[(0, 0.25), (8, 0.5)])),
                ..Default::default()
            },
            texture_color1: Some(AvfxParticleTexture {
                enabled: true,
                texture_index: 0,
                calculate_color: 1,
                calculate_alpha: 1,
                ..Default::default()
            }),
            ..Default::default()
        }],
        models: vec![
            VfxModelGeometry::default(),
            VfxModelGeometry {
                draw: Some(square()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    for kind in [ParticleType::Quad, ParticleType::LightModel] {
        file.particles[0].particle_type = Some(kind);
        file.particles[0].data = if kind == ParticleType::Quad {
            AvfxParticleData::None
        } else {
            AvfxParticleData::LightModel { model_index: 1 }
        };
        for frame in [8.0_f32, 12.0] {
            let mut reference = file.clone();
            let particle = &mut reference.particles[0];
            particle.position.x = Some(curve(&[(0, if frame == 8.0 { -0.25 } else { 0.25 })]));
            particle.position.y = Some(curve(&[(0, frame.powi(3) / 6144.0)]));
            particle.rotation.z = Some(curve(&[(0, frame / 8.0 * std::f32::consts::FRAC_PI_4)]));
            particle.scale.x = Some(curve(&[(0, 0.5 + frame / 16.0)]));
            particle.gravity = AvfxCurve::default();
            let rgb = particle.color.rgb.as_mut().unwrap();
            rgb.keys.truncate(1);
            if frame == 12.0 {
                (rgb.keys[0].x, rgb.keys[0].y, rgb.keys[0].z) = (0.5, 0.375, 0.3125);
            }
            particle.color.alpha = Some(curve(&[(0, 0.25 + frame / 32.0)]));

            let mut clamped = file.clone();
            let particle = &mut clamped.particles[0];
            for curve in [
                particle.position.x.as_mut().unwrap(),
                particle.rotation.z.as_mut().unwrap(),
                particle.scale.x.as_mut().unwrap(),
                &mut particle.gravity,
                particle.color.rgb.as_mut().unwrap(),
                particle.color.alpha.as_mut().unwrap(),
            ] {
                curve.post_behavior = BEHAVIOR_CONST;
            }
            let sample = |file: &AvfxFile| {
                let runtime = VfxRuntime::new(file);
                let (mut quads, mut meshes) = (Vec::new(), Vec::new());
                runtime.sample(frame / 30.0, &mut quads);
                runtime.sample_mesh(frame / 30.0, &mut meshes);
                assert_eq!(quads.len() + meshes.len(), 1);
                (quads, meshes)
            };
            let actual = sample(&file);
            let expected = sample(&reference);
            let control = sample(&clamped);
            for samples in [1, 4] {
                let draw = |name: &str, (quads, meshes): &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                    render_with_geometry_and_samples(
                        &format!("vfx-curve-add-{kind:?}-{frame}-{name}-{samples}"),
                        quads.clone(),
                        meshes.clone(),
                        vec![solid([255; 4])],
                        square(),
                        samples,
                    )
                };
                let pixels = draw("actual", &actual);
                assert_image(&pixels, &draw("expected", &expected));
                assert!(pixels.iter().filter(|p| p[0] > 0.01).count() > 20);
                assert!(
                    pixels
                        .iter()
                        .zip(draw("clamped", &control))
                        .filter(|(a, b)| (a[0] - b[0]).abs() > 0.01)
                        .count()
                        > 20
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn staged_laser_draw_age_and_prewarm_reach_rendering() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime, avfx::*};
    let curve = |start, end| AvfxCurve {
        keys: vec![
            AvfxCurveKey {
                time: 0,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
                z: start,
            },
            AvfxCurveKey {
                time: 4,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
                z: end,
            },
        ],
        ..Default::default()
    };
    let file = AvfxFile {
        timelines: vec![AvfxTimeline {
            binder_index: -1,
            items: vec![AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: -1,
                effector_index: -1,
                emitter_index: 0,
                platform: 0,
                clip_index: -1,
            }],
            ..Default::default()
        }],
        emitters: vec![AvfxEmitter {
            emitter_type: Some(EmitterType::Point),
            effector_index: -1,
            position: AvfxCurve3Axis {
                x: Some(curve(-0.6, -0.6)),
                ..Default::default()
            },
            particle_items: vec![AvfxEmitterItem {
                enabled: true,
                target_index: 0,
                create_time: 1,
                start_frame: 4,
                start_frame_null_update: true,
                create_count: 1,
                create_probability: 100,
                parameter_link: -1,
                parent_influence_coord: 1,
                influence_coord_pos: true,
                ..Default::default()
            }],
            ..Default::default()
        }],
        particles: vec![AvfxParticle {
            particle_type: Some(ParticleType::Laser),
            collision_type: -1,
            rotation_direction_base: 0,
            position: AvfxCurve3Axis {
                y: Some(curve(-0.3, 0.1)),
                ..Default::default()
            },
            scale: AvfxCurve3Axis {
                x: Some(curve(1.0, 1.0)),
                y: Some(curve(1.0, 1.0)),
                ..Default::default()
            },
            data: AvfxParticleData::Laser(AvfxParticleDataLaser {
                length: curve(0.3, 1.0),
                width: curve(0.05, 0.18),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    };
    let playback = VfxPlayback::new(VfxRuntime::new(&file));
    assert_eq!(playback.fallback_reason(), None);
    let mut actual = Vec::new();
    playback.sample(&mut actual, &mut Vec::new());
    assert_eq!(actual.len(), 1);
    let mut expected = actual.clone();
    expected[0].position = [-0.6, 0.0, 0.0];
    expected[0].laser.as_mut().unwrap().length = 1.0;
    expected[0].laser.as_mut().unwrap().width = 0.18;
    let mut old_geometry = expected.clone();
    old_geometry[0].laser.as_mut().unwrap().length = 0.825;
    old_geometry[0].laser.as_mut().unwrap().width = 0.1475;
    let mut old_prewarm = expected.clone();
    old_prewarm[0].position[1] = -0.3;
    for samples in [1, 4] {
        let draw = |name: &str, quads: &Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-staged-laser-age-{name}-{samples}"),
                quads.clone(),
                vec![],
                vec![],
                square(),
                samples,
            )
        };
        let pixels = draw("actual", &actual);
        assert_image(&pixels, &draw("expected", &expected));
        for (name, control) in [
            ("cached-geometry", &old_geometry),
            ("empty-prewarm", &old_prewarm),
        ] {
            assert!(
                pixels
                    .iter()
                    .zip(draw(name, control))
                    .filter(|(a, b)| (a[0] - b[0]).abs() > 0.1)
                    .count()
                    > 20,
                "{name}, MSAA {samples}"
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn staged_disc_draw_age_cached_color_and_prewarm_reach_rendering() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime, avfx::*};
    let curve = |start, end| AvfxCurve {
        keys: vec![
            AvfxCurveKey {
                time: 0,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
                z: start,
            },
            AvfxCurveKey {
                time: 4,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
                z: end,
            },
        ],
        ..Default::default()
    };
    let file = AvfxFile {
        timelines: vec![AvfxTimeline {
            binder_index: -1,
            items: vec![AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: -1,
                effector_index: -1,
                emitter_index: 0,
                platform: 0,
                clip_index: -1,
            }],
            ..Default::default()
        }],
        emitters: vec![AvfxEmitter {
            emitter_type: Some(EmitterType::Point),
            effector_index: -1,
            position: AvfxCurve3Axis {
                x: Some(curve(0.0, 0.0)),
                ..Default::default()
            },
            particle_items: vec![AvfxEmitterItem {
                enabled: true,
                target_index: 0,
                create_time: 1,
                start_frame: 4,
                start_frame_null_update: true,
                create_count: 1,
                create_probability: 100,
                parameter_link: -1,
                parent_influence_coord: 1,
                influence_coord_pos: true,
                ..Default::default()
            }],
            ..Default::default()
        }],
        particles: vec![AvfxParticle {
            particle_type: Some(ParticleType::Disc),
            collision_type: -1,
            rotation_direction_base: 2,
            rotation: AvfxCurve3Axis {
                x: Some(curve(
                    std::f32::consts::FRAC_PI_2,
                    std::f32::consts::FRAC_PI_2,
                )),
                ..Default::default()
            },
            position: AvfxCurve3Axis {
                y: Some(curve(-0.3, 0.1)),
                ..Default::default()
            },
            scale: AvfxCurve3Axis {
                x: Some(curve(1.0, 1.0)),
                y: Some(curve(1.0, 1.0)),
                ..Default::default()
            },
            data: AvfxParticleData::Disc(AvfxParticleDataDisc {
                parts_count: 1,
                parts_count_u: 2,
                parts_count_v: 32,
                scaling_scale: 100,
                angle: curve(std::f32::consts::TAU, std::f32::consts::TAU),
                radius_begin: curve(0.2, 0.5),
                radius_end: curve(0.2, 0.5),
                width_begin: curve(0.1, 0.1),
                width_end: curve(0.1, 0.1),
                color_edge_inner: AvfxColorCurve {
                    alpha: Some(curve(0.8, 0.2)),
                    ..Default::default()
                },
                color_edge_outer: AvfxColorCurve {
                    alpha: Some(curve(0.8, 0.2)),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    };
    let playback = VfxPlayback::new(VfxRuntime::new(&file));
    assert_eq!(playback.fallback_reason(), None);
    let mut actual = Vec::new();
    playback.sample(&mut actual, &mut Vec::new());
    assert_eq!(actual.len(), 1);
    let mut expected = actual.clone();
    expected[0].position = [0.0, 0.0, 0.0];
    expected[0].disc.as_mut().unwrap().radius = [0.5; 2];
    expected[0].disc.as_mut().unwrap().color_inner[3] = 0.8;
    expected[0].disc.as_mut().unwrap().color_outer[3] = 0.8;
    let mut old_geometry = expected.clone();
    old_geometry[0].disc.as_mut().unwrap().radius = [0.425; 2];
    let mut old_prewarm = expected.clone();
    old_prewarm[0].position[1] = -0.3;
    let mut current_colors = expected.clone();
    current_colors[0].disc.as_mut().unwrap().color_inner[3] = 0.2;
    current_colors[0].disc.as_mut().unwrap().color_outer[3] = 0.2;
    for samples in [1, 4] {
        let draw = |name: &str, quads: &Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-staged-disc-age-{name}-{samples}"),
                quads.clone(),
                vec![],
                vec![],
                square(),
                samples,
            )
        };
        let pixels = draw("actual", &actual);
        assert_image(&pixels, &draw("expected", &expected));
        for (name, control) in [
            ("cached-geometry", &old_geometry),
            ("empty-prewarm", &old_prewarm),
            ("uncached-colors", &current_colors),
        ] {
            assert!(
                pixels
                    .iter()
                    .zip(draw(name, control))
                    .filter(|(a, b)| (a[0] - b[0]).abs() > 0.1)
                    .count()
                    > 20,
                "{name}, MSAA {samples}"
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn staged_playback_curve_caches_reach_quad_rendering() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime, avfx::*};
    let curve = |start, end| AvfxCurve {
        keys: vec![
            AvfxCurveKey {
                time: 0,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
                z: start,
            },
            AvfxCurveKey {
                time: 4,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
                z: end,
            },
        ],
        ..Default::default()
    };
    let file = AvfxFile {
        timelines: vec![AvfxTimeline {
            binder_index: -1,
            items: vec![AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: -1,
                effector_index: -1,
                emitter_index: 0,
                platform: 0,
                clip_index: -1,
            }],
            ..Default::default()
        }],
        emitters: vec![AvfxEmitter {
            emitter_type: Some(EmitterType::Point),
            effector_index: -1,
            position: AvfxCurve3Axis {
                x: Some(curve(-0.6, 0.6)),
                ..Default::default()
            },
            particle_items: vec![AvfxEmitterItem {
                enabled: true,
                target_index: 0,
                create_time: 1,
                create_count: 1,
                create_probability: 100,
                parameter_link: -1,
                parent_influence_coord: 1,
                influence_coord_pos: true,
                ..Default::default()
            }],
            ..Default::default()
        }],
        particles: vec![AvfxParticle {
            particle_type: Some(ParticleType::Quad),
            collision_type: -1,
            rotation_direction_base: rotation_direction_base::NONE,
            rotation: AvfxCurve3Axis {
                z: Some(curve(0.0, std::f32::consts::FRAC_PI_2)),
                ..Default::default()
            },
            scale: AvfxCurve3Axis {
                x: Some(curve(1.4, 1.4)),
                y: Some(curve(1.0, 0.4)),
                ..Default::default()
            },
            ..Default::default()
        }],
        ..Default::default()
    };
    let runtime = VfxRuntime::new(&file);
    let mut playback = VfxPlayback::new(runtime.clone());
    assert_eq!(playback.fallback_reason(), None);
    let mut expected = Vec::new();
    playback.sample(&mut expected, &mut Vec::new());
    assert_eq!(expected.len(), 1);
    expected[0].position[0] = 0.0;
    expected[0].size[1] = 0.35;
    let (s, c) = (std::f32::consts::PI / 8.0).sin_cos();
    expected[0].orientation = [0.0, 0.0, s, c];
    playback.advance(2.0 / 30.0).unwrap();
    playback.advance(2.0 / 30.0).unwrap();
    let mut actual = Vec::new();
    playback.sample(&mut actual, &mut Vec::new());
    let mut continuous = Vec::new();
    runtime.sample(4.0 / 30.0, &mut continuous);
    for samples in [1, 4] {
        let draw = |name: &str, quads: &Vec<VfxQuad>| {
            render_with_geometry_and_samples(
                &format!("vfx-staged-cache-{name}-{samples}"),
                quads.clone(),
                vec![],
                vec![],
                square(),
                samples,
            )
        };
        let pixels = draw("actual", &actual);
        assert_image(&pixels, &draw("expected", &expected));
        let control = draw("continuous", &continuous);
        assert!(
            pixels
                .iter()
                .zip(control)
                .filter(|(a, b)| (a[0] - b[0]).abs() > 0.1)
                .count()
                > 20
        );
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn staged_mesh_prewarm_and_fresnel_reach_rendering() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime, avfx::*};
    let curve = |start, end| AvfxCurve {
        keys: [(0, start), (8, end)]
            .into_iter()
            .map(|(time, z)| AvfxCurveKey {
                time,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
                z,
            })
            .collect(),
        ..Default::default()
    };
    let mut file = AvfxFile {
        timelines: vec![AvfxTimeline {
            binder_index: -1,
            items: vec![AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: -1,
                effector_index: -1,
                emitter_index: 0,
                platform: 0,
                clip_index: -1,
            }],
            ..Default::default()
        }],
        emitters: vec![AvfxEmitter {
            emitter_type: Some(EmitterType::Point),
            effector_index: -1,
            particle_items: vec![AvfxEmitterItem {
                enabled: true,
                target_index: 0,
                create_time: 1,
                create_count: 1,
                create_probability: 100,
                parameter_link: -1,
                start_frame: 4,
                start_frame_null_update: true,
                ..Default::default()
            }],
            ..Default::default()
        }],
        particles: vec![AvfxParticle {
            particle_type: Some(ParticleType::Model),
            collision_type: -1,
            rotation_direction_base: rotation_direction_base::NONE,
            rotation: AvfxCurve3Axis {
                z: Some(curve(0.0, std::f32::consts::FRAC_PI_2)),
                ..Default::default()
            },
            scale: AvfxCurve3Axis {
                x: Some(curve(1.4, 1.4)),
                y: Some(curve(1.0, 0.2)),
                ..Default::default()
            },
            data: AvfxParticleData::Model {
                model_number_random_value: 0,
                model_number_random_type: 0,
                model_number_random_interval: 0,
                fresnel_type: 1,
                directional_light_type: 0,
                point_light_type: 0,
                is_lightning: false,
                is_morph: false,
                model_indexes: vec![1],
                animation_number: None,
                morph: None,
                fresnel_curve: Some(curve(1.0, 5.0)),
                fresnel_curve_random: None,
                fresnel_rotation: None,
                color_begin: AvfxColorCurve {
                    alpha: Some(curve(0.2, 0.6)),
                    ..Default::default()
                },
                color_end: AvfxColorCurve {
                    alpha: Some(curve(0.8, 0.4)),
                    ..Default::default()
                },
            },
            ..Default::default()
        }],
        models: vec![
            VfxModelGeometry::default(),
            VfxModelGeometry {
                draw: Some(square()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let runtime = VfxRuntime::new(&file);
    let playback = VfxPlayback::new(runtime.clone());
    assert_eq!(playback.fallback_reason(), None);
    let mut actual = Vec::new();
    playback.sample(&mut Vec::new(), &mut actual);
    assert_eq!(actual.len(), 1);
    file.emitters[0].particle_items[0].start_frame = 0;
    let mut expected = Vec::new();
    VfxRuntime::new(&file).sample_mesh(0.0, &mut expected);
    assert_eq!(expected.len(), 1);
    // StFr=4 refreshes properties at ages 0,1,2,3, leaves UV/Fresnel
    // colors at zero, and draws FrC at age 4.
    expected[0].scale[1] = 0.7;
    let (s, c) = (3.0 * std::f32::consts::PI / 32.0).sin_cos();
    expected[0].orientation = [0.0, 0.0, s, c];
    expected[0].fresnel.as_mut().unwrap().exponent = 3.0;
    let mut continuous = Vec::new();
    runtime.sample_mesh(0.0, &mut continuous);
    for samples in [1, 4] {
        let draw = |name: &str, meshes: &Vec<VfxMeshInstance>| {
            render_with_geometry_and_samples(
                &format!("vfx-staged-mesh-{name}-{samples}"),
                vec![],
                meshes.clone(),
                vec![],
                square(),
                samples,
            )
        };
        let pixels = draw("actual", &actual);
        assert_image(&pixels, &draw("expected", &expected));
        let control = draw("continuous", &continuous);
        assert!(
            pixels
                .iter()
                .zip(control)
                .filter(|(a, b)| (a[0] - b[0]).abs() > 0.05)
                .count()
                > 20
        );
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn scalar_random_constant_amplitudes_keep_rendered_attributes() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime, avfx::*};
    let curve = |z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let make = |multiple: bool, amplitude: f32| {
        let random = |z| {
            let mut value = AvfxCurve {
                random_type: 1,
                ..curve(z * amplitude)
            };
            if multiple {
                value.keys.push(AvfxCurveKey {
                    time: 10,
                    ..value.keys[0]
                });
            }
            value
        };
        AvfxFile {
            timelines: vec![AvfxTimeline {
                binder_index: -1,
                items: vec![AvfxTimelineItem {
                    enabled: true,
                    start_time: 0,
                    end_time: -1,
                    binder_index: -1,
                    effector_index: -1,
                    emitter_index: 0,
                    platform: 0,
                    clip_index: -1,
                }],
                ..Default::default()
            }],
            emitters: vec![AvfxEmitter {
                emitter_type: Some(EmitterType::Point),
                position: AvfxCurve3Axis {
                    x: Some(curve(-0.3)),
                    random_x: Some(random(0.2)),
                    ..Default::default()
                },
                effector_index: -1,
                particle_items: vec![AvfxEmitterItem {
                    enabled: true,
                    target_index: 0,
                    create_time: 1,
                    create_count: 1,
                    create_probability: 100,
                    parameter_link: -1,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            particles: vec![AvfxParticle {
                collision_type: -1,
                rotation_direction_base: rotation_direction_base::NONE,
                position: AvfxCurve3Axis {
                    random_x: Some(random(0.6)),
                    ..Default::default()
                },
                rotation: AvfxCurve3Axis {
                    random_z: Some(random(1.0)),
                    ..Default::default()
                },
                scale: AvfxCurve3Axis {
                    x: Some(curve(0.4)),
                    y: Some(curve(0.4)),
                    random_x: Some(random(0.3)),
                    random_y: Some(random(0.2)),
                    ..Default::default()
                },
                color: AvfxColorCurve {
                    random: [0.5, -0.5, 0.8, -0.2, 0.2].map(|v| Some(random(v))),
                    ..Default::default()
                },
                uv_sets: vec![AvfxUvSet {
                    scroll: AvfxCurve2Axis {
                        random_x: Some(random(0.6)),
                        ..Default::default()
                    },
                    scale: AvfxCurve2Axis {
                        random_x: Some(random(0.2)),
                        ..Default::default()
                    },
                    rotation_random: random(0.4),
                    ..Default::default()
                }],
                texture_color1: Some(AvfxParticleTexture {
                    enabled: true,
                    texture_index: 0,
                    calculate_color: 1,
                    calculate_alpha: 1,
                    ..Default::default()
                }),
                ..Default::default()
            }],
            models: vec![
                VfxModelGeometry::default(),
                VfxModelGeometry {
                    draw: Some(square()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    };
    for kind in [ParticleType::Quad, ParticleType::LightModel] {
        for staged in [false, true] {
            let sample = |mut file: AvfxFile| {
                file.particles[0].particle_type = Some(kind);
                if kind == ParticleType::LightModel {
                    file.particles[0].data = AvfxParticleData::LightModel { model_index: 1 };
                }
                let runtime = VfxRuntime::new(&file);
                let mut packets = (Vec::new(), Vec::new());
                if staged {
                    let mut playback = VfxPlayback::new(runtime);
                    assert!(
                        playback.fallback_reason().is_none(),
                        "{:?}",
                        playback.fallback_reason()
                    );
                    playback.advance(2.0 / 30.0).unwrap();
                    playback.advance(3.0 / 30.0).unwrap();
                    playback.sample(&mut packets.0, &mut packets.1);
                } else {
                    runtime.sample(5.0 / 30.0, &mut packets.0);
                    runtime.sample_mesh(5.0 / 30.0, &mut packets.1);
                }
                assert_eq!(packets.0.len() + packets.1.len(), 1);
                packets
            };
            let actual = sample(make(true, 1.0));
            let reference = sample(make(false, 1.0));
            let control = sample(make(true, 0.0));
            assert_eq!(actual, reference);
            for samples in [1, 4] {
                let draw = |name, packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                    render_with_geometry_and_samples(
                        &format!("vfx-scalar-random-{kind:?}-{staged}-{name}-{samples}"),
                        packets.0.clone(),
                        packets.1.clone(),
                        vec![black_white_texture()],
                        square(),
                        samples,
                    )
                };
                let pixels = draw("actual", &actual);
                assert_image(&pixels, &draw("reference", &reference));
                assert!(
                    pixels
                        .iter()
                        .zip(draw("control", &control))
                        .filter(|(a, b)| (a[0] - b[0]).abs() > 0.05)
                        .count()
                        > 20
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn cone_shape_birth_rotation_and_signed_motion_reach_rendering() {
    use xiv_companion_data::{VfxRuntime, avfx::*};
    let curve = |z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let mut file = AvfxFile {
        timelines: vec![AvfxTimeline {
            items: vec![AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: -1,
                effector_index: -1,
                emitter_index: 0,
                platform: 0,
                clip_index: -1,
            }],
            ..Default::default()
        }],
        emitters: vec![AvfxEmitter {
            emitter_type: Some(EmitterType::Cone),
            position: AvfxCurve3Axis {
                x: Some(curve(0.1)),
                y: Some(curve(-0.1)),
                ..Default::default()
            },
            rotation: AvfxCurve3Axis {
                z: Some(curve(std::f32::consts::FRAC_PI_2)),
                ..Default::default()
            },
            scale: AvfxCurve3Axis {
                x: Some(curve(-1.0)),
                y: Some(curve(2.0)),
                ..Default::default()
            },
            data: Some(AvfxEmitterData::Cone(ConeEmitterData {
                rotation: EmitterDataRotation {
                    angles: [
                        curve(-std::f32::consts::FRAC_PI_2),
                        Default::default(),
                        Default::default(),
                    ],
                    ..Default::default()
                },
                inner_size: curve(0.15),
                outer_size: curve(0.15),
                injection_speed: curve(0.05),
                ..Default::default()
            })),
            particle_items: vec![AvfxEmitterItem {
                enabled: true,
                target_index: 0,
                create_time: 1,
                create_count: 1,
                create_probability: 100,
                parent_influence_coord: 0,
                ..Default::default()
            }],
            ..Default::default()
        }],
        particles: vec![AvfxParticle {
            rotation_direction_base: rotation_direction_base::NONE,
            scale: AvfxCurve3Axis {
                x: Some(curve(0.25)),
                y: Some(curve(0.25)),
                ..Default::default()
            },
            texture_color1: Some(AvfxParticleTexture {
                enabled: true,
                texture_index: 0,
                calculate_color: 1,
                calculate_alpha: 1,
                ..Default::default()
            }),
            ..Default::default()
        }],
        models: vec![
            VfxModelGeometry::default(),
            VfxModelGeometry {
                draw: Some(square()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    for speed in [0.05, -0.05] {
        let Some(AvfxEmitterData::Cone(data)) = &mut file.emitters[0].data else {
            unreachable!()
        };
        data.injection_speed = curve(speed);
        for kind in [ParticleType::Quad, ParticleType::LightModel] {
            file.particles[0].particle_type = Some(kind);
            file.particles[0].data = if kind == ParticleType::Quad {
                AvfxParticleData::None
            } else {
                AvfxParticleData::LightModel { model_index: 1 }
            };
            let runtime = VfxRuntime::new(&file);
            let mut actual = (Vec::new(), Vec::new());
            runtime.sample(2.0 / 30.0, &mut actual.0);
            runtime.sample_mesh(2.0 / 30.0, &mut actual.1);
            assert_eq!(actual.0.len() + actual.1.len(), 1);
            // Shape +Z -> +Y, parent Rz(90) -> -X. Birth distance scales by 2,
            // whereas injection speed uses the normalized world direction.
            let position = [-0.2 - 2.0 * speed, -0.1, 0.0];
            let mut expected = actual.clone();
            for quad in &mut expected.0 {
                quad.position = position;
            }
            for mesh in &mut expected.1 {
                mesh.position = position;
            }
            let mut control = expected.clone();
            for quad in &mut control.0 {
                quad.position[0] += 0.5;
            }
            for mesh in &mut control.1 {
                mesh.position[0] += 0.5;
            }
            for samples in [1, 4] {
                let draw = |name, packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                    render_with_geometry_and_samples(
                        &format!("vfx-cone-shape-{speed}-{kind:?}-{name}-{samples}"),
                        packets.0.clone(),
                        packets.1.clone(),
                        vec![solid([80, 220, 160, 255])],
                        square(),
                        samples,
                    )
                };
                let pixels = draw("actual", &actual);
                assert_image(&pixels, &draw("reference", &expected));
                assert!(
                    pixels
                        .iter()
                        .zip(draw("control", &control))
                        .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                        .count()
                        > 20
                );
            }
        }
    }
}

fn cone_model_render_fixture() -> xiv_companion_data::avfx::AvfxFile {
    use xiv_companion_data::avfx::*;
    let curve = |z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    AvfxFile {
        timelines: vec![AvfxTimeline {
            items: vec![AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: -1,
                effector_index: -1,
                emitter_index: 0,
                clip_index: -1,
                platform: 0,
            }],
            ..Default::default()
        }],
        emitters: vec![AvfxEmitter {
            emitter_type: Some(EmitterType::ConeModel),
            position: AvfxCurve3Axis {
                x: Some(curve(0.1)),
                ..Default::default()
            },
            scale: AvfxCurve3Axis {
                x: Some(curve(-1.1)),
                y: Some(curve(0.8)),
                ..Default::default()
            },
            data: Some(AvfxEmitterData::ConeModel(ConeModelEmitterData {
                generate_method: 3,
                divide_x: 4,
                divide_y: 1,
                radius: curve(0.35),
                injection_angle: curve(std::f32::consts::FRAC_PI_2),
                injection_speed: curve(-0.08),
                ..Default::default()
            })),
            particle_items: vec![AvfxEmitterItem {
                enabled: true,
                target_index: 0,
                create_time: 1,
                create_count: 5,
                create_probability: 100,
                parameter_link: -1,
                ..Default::default()
            }],
            ..Default::default()
        }],
        particles: vec![AvfxParticle {
            rotation_direction_base: rotation_direction_base::NONE,
            scale: AvfxCurve3Axis {
                x: Some(curve(0.12)),
                y: Some(curve(0.12)),
                z: Some(curve(0.12)),
                ..Default::default()
            },
            texture_color1: Some(AvfxParticleTexture {
                enabled: true,
                texture_index: 0,
                calculate_color: 1,
                calculate_alpha: 1,
                ..Default::default()
            }),
            ..Default::default()
        }],
        models: vec![
            VfxModelGeometry::default(),
            VfxModelGeometry {
                draw: Some(square()),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn cone_model_discrete_births_and_signed_motion_reach_rendering() {
    use xiv_companion_data::{VfxRuntime, avfx::*};
    let mut file = cone_model_render_fixture();
    for method in [1, 3] {
        let Some(AvfxEmitterData::ConeModel(data)) = &mut file.emitters[0].data else {
            unreachable!()
        };
        data.generate_method = method;
        for kind in [ParticleType::Quad, ParticleType::LightModel] {
            file.particles[0].particle_type = Some(kind);
            file.particles[0].data = if kind == ParticleType::Quad {
                AvfxParticleData::None
            } else {
                AvfxParticleData::LightModel { model_index: 1 }
            };
            let runtime = VfxRuntime::new(&file);
            let mut actual = (Vec::new(), Vec::new());
            runtime.sample(2.0 / 30.0, &mut actual.0);
            runtime.sample_mesh(2.0 / 30.0, &mut actual.1);
            assert_eq!(actual.0.len() + actual.1.len(), 5);
            let radius = if method == 3 { 0.35 } else { 0.0 };
            let positions = [
                [0.1, 0.0, radius - 0.16],
                [0.1, -0.8 * radius + 0.16, 0.0],
                [0.1 - 1.1 * radius + 0.16, 0.0, 0.0],
                [0.1, 0.8 * radius - 0.16, 0.0],
                [0.1 + 1.1 * radius - 0.16, 0.0, 0.0],
            ];
            let mut expected = actual.clone();
            for (q, position) in expected.0.iter_mut().zip(positions) {
                q.position = position;
            }
            for (m, position) in expected.1.iter_mut().zip(positions) {
                m.position = position;
            }
            let mut control = expected.clone();
            for q in &mut control.0 {
                q.position[0] += 0.5;
            }
            for m in &mut control.1 {
                m.position[0] += 0.5;
            }
            for samples in [1, 4] {
                let draw = |name, packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                    render_with_geometry_and_samples(
                        &format!("vfx-cone-model-{method}-{kind:?}-{name}-{samples}"),
                        packets.0.clone(),
                        packets.1.clone(),
                        vec![solid([80, 220, 160, 255])],
                        square(),
                        samples,
                    )
                };
                let pixels = draw("actual", &actual);
                assert_image(&pixels, &draw("reference", &expected));
                assert!(
                    pixels
                        .iter()
                        .zip(draw("control", &control))
                        .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                        .count()
                        > 20
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn cone_model_animated_binding_reaches_rendering() {
    use xiv_companion_data::{VfxRuntime, avfx::*};
    let ramp = |start, end| AvfxCurve {
        keys: [(0, start), (4, end)]
            .into_iter()
            .map(|(time, z)| AvfxCurveKey {
                time,
                z,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
            })
            .collect(),
        ..Default::default()
    };
    for (method, divide_x) in [(3, 4), (1, 4), (1, 1)] {
        let mut file = cone_model_render_fixture();
        // ToVertex's children overlap. Keep that single silhouette large
        // enough for the control comparison at 64x64, including MSAA edges.
        file.particles[0].scale = AvfxCurve3Axis {
            x: Some(ramp(0.2, 0.2)),
            y: Some(ramp(0.2, 0.2)),
            z: Some(ramp(0.2, 0.2)),
            ..Default::default()
        };
        let emitter = &mut file.emitters[0];
        emitter.position.x = Some(ramp(0.1, 0.25));
        emitter.position.y = Some(ramp(0.0, -0.15));
        emitter.scale.y = Some(ramp(0.8, 1.2));
        let item = &mut emitter.particle_items[0];
        item.create_count = divide_x + 1;
        item.parent_influence_coord = 1;
        item.influence_coord_pos = true;
        let Some(AvfxEmitterData::ConeModel(data)) = &mut emitter.data else {
            unreachable!()
        };
        data.generate_method = method;
        data.divide_x = divide_x;
        data.radius = ramp(0.2, 0.45);
        data.injection_angle = ramp(std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_2);
        data.injection_speed = Default::default();
        data.rotation.angles[1] = ramp(0.0, std::f32::consts::FRAC_PI_2);
        for kind in [ParticleType::Quad, ParticleType::LightModel] {
            file.particles[0].particle_type = Some(kind);
            file.particles[0].data = if kind == ParticleType::Quad {
                AvfxParticleData::None
            } else {
                AvfxParticleData::LightModel { model_index: 1 }
            };
            let sample = |file: &AvfxFile| {
                let runtime = VfxRuntime::new(file);
                let mut packets = (Vec::new(), Vec::new());
                runtime.sample(4.0 / 30.0, &mut packets.0);
                runtime.sample_mesh(4.0 / 30.0, &mut packets.1);
                packets
            };
            let actual = sample(&file);
            assert_eq!(actual.0.len() + actual.1.len(), (divide_x + 1) as usize);
            // At frame 4, Ry(90) precedes parent scale (-1.1, 1.2, 1).
            // ToVertex binds index -1: x=4 gives the pole, x=1 angle=-pi/2.
            let positions = if method == 1 {
                vec![
                    if divide_x == 1 {
                        [0.25, 0.39, 0.0]
                    } else {
                        [-0.245, -0.15, 0.0]
                    };
                    (divide_x + 1) as usize
                ]
            } else {
                vec![
                    [-0.245, -0.15, 0.0],
                    [0.25, -0.69, 0.0],
                    [0.25, -0.15, -0.45],
                    [0.25, 0.39, 0.0],
                    [0.25, -0.15, 0.45],
                ]
            };
            for (position, expected) in actual
                .0
                .iter()
                .map(|quad| quad.position)
                .chain(actual.1.iter().map(|mesh| mesh.position))
                .zip(&positions)
            {
                for axis in 0..3 {
                    assert!(
                        (position[axis] - expected[axis]).abs() < 1e-6,
                        "method={method}, divide_x={divide_x}, {kind:?}: {position:?} != {expected:?}"
                    );
                }
            }
            let mut expected = actual.clone();
            for (quad, position) in expected.0.iter_mut().zip(&positions) {
                quad.position = *position;
            }
            for (mesh, position) in expected.1.iter_mut().zip(&positions) {
                mesh.position = *position;
            }
            let mut unbound = file.clone();
            unbound.emitters[0].particle_items[0].influence_coord_pos = false;
            let control = sample(&unbound);
            for samples in [1, 4] {
                let draw = |name, packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                    render_with_geometry_and_samples(
                        &format!(
                            "vfx-cone-model-binding-{method}-{divide_x}-{kind:?}-{name}-{samples}"
                        ),
                        packets.0.clone(),
                        packets.1.clone(),
                        vec![solid([80, 220, 160, 255])],
                        square(),
                        samples,
                    )
                };
                let pixels = draw("actual", &actual);
                assert_image(&pixels, &draw("reference", &expected));
                let changed = pixels
                    .iter()
                    .zip(draw("control", &control))
                    .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                    .count();
                assert!(
                    changed > 20,
                    "method={method}, divide_x={divide_x}, {kind:?}, samples={samples}: only {changed} changed pixels"
                );
            }
        }
    }
}

fn model_render_fixture() -> xiv_companion_data::avfx::AvfxFile {
    use xiv_companion_data::avfx::*;
    let curve = |z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let mut file = cone_model_render_fixture();
    let emitter = &mut file.emitters[0];
    emitter.emitter_type = Some(EmitterType::Model);
    emitter.particle_items[0].create_count = 1;
    emitter.injection_angle[0] = curve(std::f32::consts::FRAC_PI_2);
    emitter.data = Some(AvfxEmitterData::Model(ModelEmitterData {
        model_index: 0,
        generate_method: 3,
        injection_speed: curve(0.1),
        rotation: EmitterDataRotation {
            angles: [
                Default::default(),
                Default::default(),
                curve(std::f32::consts::FRAC_PI_2),
            ],
            ..Default::default()
        },
        ..Default::default()
    }));
    file.models[0].emit_vertices = vec![VfxEmitVertex {
        position: [0.0, 0.2, 0.0],
        normal: [0.0, 1.0, 0.0],
        color: [255; 4],
    }];
    file.models[0].emit_vertex_numbers = vec![0];
    file
}

#[test]
#[ignore = "requires native wgpu"]
fn model_direction_override_reaches_quad_and_mesh_rendering() {
    use xiv_companion_data::{VfxRuntime, avfx::*};
    let mut file = model_render_fixture();
    // A single child must cover enough pixels for the independent shifted
    // control to distinguish directions at 64x64, including MSAA edges.
    let scale = &mut file.particles[0].scale;
    for axis in [&mut scale.x, &mut scale.y, &mut scale.z] {
        axis.as_mut().unwrap().keys[0].z = 0.3;
    }
    for enabled in [false, true] {
        file.emitters[0].any_direction = enabled;
        for kind in [ParticleType::Quad, ParticleType::LightModel] {
            file.particles[0].particle_type = Some(kind);
            file.particles[0].data = if kind == ParticleType::Quad {
                AvfxParticleData::None
            } else {
                AvfxParticleData::LightModel { model_index: 1 }
            };
            let runtime = VfxRuntime::new(&file);
            let mut actual = (Vec::new(), Vec::new());
            runtime.sample(2.0 / 30.0, &mut actual.0);
            runtime.sample_mesh(2.0 / 30.0, &mut actual.1);
            assert_eq!(actual.0.len() + actual.1.len(), 1);
            // Shape Rz and mirrored parent place the vertex at x=0.32.
            // The normal gives +X; the bAD override gives -X at speed 0.1.
            let position = [if enabled { 0.12 } else { 0.52 }, 0.0, 0.0];
            let mut expected = actual.clone();
            for quad in &mut expected.0 {
                quad.position = position;
            }
            for mesh in &mut expected.1 {
                mesh.position = position;
            }
            let mut control = expected.clone();
            for quad in &mut control.0 {
                quad.position[0] = 0.64 - position[0];
            }
            for mesh in &mut control.1 {
                mesh.position[0] = 0.64 - position[0];
            }
            for samples in [1, 4] {
                let draw = |name, packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                    render_with_geometry_and_samples(
                        &format!("vfx-model-direction-{enabled}-{kind:?}-{name}-{samples}"),
                        packets.0.clone(),
                        packets.1.clone(),
                        vec![solid([80, 220, 160, 255])],
                        square(),
                        samples,
                    )
                };
                let pixels = draw("actual", &actual);
                assert_image(&pixels, &draw("reference", &expected));
                assert!(
                    pixels
                        .iter()
                        .zip(draw("control", &control))
                        .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                        .count()
                        > 20
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn model_random_shape_and_speed_reach_quad_and_mesh_rendering() {
    use xiv_companion_data::{VfxRuntime, avfx::*};
    for speed_mode in [4, 5] {
        let mut file = model_render_fixture();
        file.emitters[0].particle_items[0].create_count = 8;
        let Some(AvfxEmitterData::Model(data)) = &mut file.emitters[0].data else {
            unreachable!()
        };
        data.model_index = 256;
        data.generate_method = 259;
        data.rotation.order = 256;
        data.injection_speed_random = data.injection_speed.clone();
        data.injection_speed_random.random_type = speed_mode;
        data.injection_speed_random.keys[0].z = 0.25;
        data.injection_speed.keys[0].z = 0.0;
        data.rotation.angles_random[2] = data.rotation.angles[2].clone();
        data.rotation.angles_random[2].keys[0].z = 0.9;
        data.rotation.angles_random[2].random_type = 4;
        data.rotation.angles[2].keys[0].z = 0.0;
        for kind in [ParticleType::Quad, ParticleType::LightModel] {
            file.particles[0].particle_type = Some(kind);
            file.particles[0].data = if kind == ParticleType::Quad {
                AvfxParticleData::None
            } else {
                AvfxParticleData::LightModel { model_index: 1 }
            };
            let sample = |file: &AvfxFile| {
                let runtime = VfxRuntime::new(file);
                let mut packets = (Vec::new(), Vec::new());
                runtime.sample(2.0 / 30.0, &mut packets.0);
                runtime.sample_mesh(2.0 / 30.0, &mut packets.1);
                packets
            };
            let actual = sample(&file);
            assert_eq!(actual.0.len() + actual.1.len(), 8);
            // Zero amplitudes preserve Always draws and therefore the birth seeds.
            let mut stationary = file.clone();
            let Some(AvfxEmitterData::Model(data)) = &mut stationary.emitters[0].data else {
                unreachable!()
            };
            data.injection_speed_random.keys[0].z = 0.0;
            let stationary = sample(&stationary);
            let mut straight = file.clone();
            let Some(AvfxEmitterData::Model(data)) = &mut straight.emitters[0].data else {
                unreachable!()
            };
            data.rotation.angles_random[2].keys[0].z = 0.0;
            let straight = sample(&straight);
            let positions = |packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                packets
                    .0
                    .iter()
                    .map(|q| q.position)
                    .chain(packets.1.iter().map(|m| m.position))
                    .collect::<Vec<_>>()
            };
            let births = positions(&stationary);
            let speeds = positions(&straight);
            assert!(births.windows(2).any(|p| (p[0][0] - p[1][0]).abs() > 1e-4));
            assert!(speeds.windows(2).any(|p| (p[0][1] - p[1][1]).abs() > 1e-4));
            let positions: Vec<_> = births
                .iter()
                .zip(&speeds)
                .map(|(birth, straight)| {
                    let sin = (birth[0] - 0.1) / 0.22;
                    let cos = birth[1] / 0.16;
                    assert!((sin * sin + cos * cos - 1.0).abs() < 1e-5);
                    let (x, y) = (1.1 * sin, 0.8 * cos);
                    let length = x.hypot(y);
                    let distance = straight[1] - 0.16;
                    [
                        birth[0] + distance * x / length,
                        birth[1] + distance * y / length,
                        0.0,
                    ]
                })
                .collect();
            let mut expected = actual.clone();
            for (quad, position) in expected.0.iter_mut().zip(&positions) {
                quad.position = *position;
            }
            for (mesh, position) in expected.1.iter_mut().zip(&positions) {
                mesh.position = *position;
            }
            for samples in [1, 4] {
                let draw = |name, packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                    render_with_geometry_and_samples(
                        &format!("vfx-model-random-{speed_mode}-{kind:?}-{name}-{samples}"),
                        packets.0.clone(),
                        packets.1.clone(),
                        vec![solid([80, 220, 160, 255])],
                        square(),
                        samples,
                    )
                };
                let pixels = draw("actual", &actual);
                assert_image(&pixels, &draw("reference", &expected));
                assert!(
                    pixels
                        .iter()
                        .zip(draw("stationary", &stationary))
                        .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                        .count()
                        > 20
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn cylinder_model_discrete_births_and_animated_binding_reach_rendering() {
    use xiv_companion_data::{VfxRuntime, avfx::*};
    let ramp = |start, end| AvfxCurve {
        keys: [(0, start), (4, end)]
            .into_iter()
            .map(|(time, z)| AvfxCurveKey {
                time,
                z,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
            })
            .collect(),
        ..Default::default()
    };
    for method in [1, 3] {
        for follow in [false, true] {
            let mut file = cone_model_render_fixture();
            let emitter = &mut file.emitters[0];
            emitter.emitter_type = Some(EmitterType::CylinderModel);
            emitter.data = Some(AvfxEmitterData::CylinderModel(CylinderModelEmitterData {
                generate_method: method,
                divide_x: 4,
                divide_y: 2,
                radius: ramp(0.2, 0.4),
                length: ramp(0.4, 0.8),
                rotation: EmitterDataRotation {
                    angles: [
                        Default::default(),
                        Default::default(),
                        ramp(0.0, std::f32::consts::FRAC_PI_2),
                    ],
                    ..Default::default()
                },
                ..Default::default()
            }));
            emitter.particle_items[0].create_count = 12;
            emitter.particle_items[0].parent_influence_coord = 1;
            emitter.particle_items[0].influence_coord_pos = follow;
            for kind in [ParticleType::Quad, ParticleType::LightModel] {
                file.particles[0].particle_type = Some(kind);
                file.particles[0].data = if kind == ParticleType::Quad {
                    AvfxParticleData::None
                } else {
                    AvfxParticleData::LightModel { model_index: 1 }
                };
                let runtime = VfxRuntime::new(&file);
                let mut actual = (Vec::new(), Vec::new());
                runtime.sample(4.0 / 30.0, &mut actual.0);
                runtime.sample_mesh(4.0 / 30.0, &mut actual.1);
                assert_eq!(actual.0.len() + actual.1.len(), 12);
                let positions: Vec<_> = (0_i32..12)
                    .map(|index| {
                        if method == 1 && !follow {
                            return [0.1, 0.0, 0.0];
                        }
                        let index = if method == 1 { -1 } else { index };
                        let angle = (index % 4) as f32 * std::f32::consts::FRAC_PI_2;
                        let (s, c) = angle.sin_cos();
                        if follow {
                            let height = (index / 4) as f32 * 0.4 - 0.4;
                            [0.1 + 1.1 * height, 0.32 * s, 0.4 * c]
                        } else {
                            [
                                0.1 - 0.22 * s,
                                0.8 * ((index / 4) as f32 * 0.2 - 0.2),
                                0.2 * c,
                            ]
                        }
                    })
                    .collect();
                let mut reference = actual.clone();
                for (quad, position) in reference.0.iter_mut().zip(&positions) {
                    quad.position = *position;
                }
                for (mesh, position) in reference.1.iter_mut().zip(&positions) {
                    mesh.position = *position;
                }
                let mut flat = actual.clone();
                let flat_position = |index: usize| {
                    let angle = (index % 4) as f32 * std::f32::consts::FRAC_PI_2;
                    [0.1 - 0.22 * angle.cos(), 0.0, 0.2 * angle.sin()]
                };
                for (index, quad) in flat.0.iter_mut().enumerate() {
                    quad.position = flat_position(index);
                }
                for (index, mesh) in flat.1.iter_mut().enumerate() {
                    mesh.position = flat_position(index);
                }
                for samples in [1, 4] {
                    let draw = |name, packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                        render_with_geometry_and_samples(
                            &format!("vfx-cylinder-{method}-{follow}-{kind:?}-{name}-{samples}"),
                            packets.0.clone(),
                            packets.1.clone(),
                            vec![solid([80, 220, 160, 255])],
                            square(),
                            samples,
                        )
                    };
                    let pixels = draw("actual", &actual);
                    assert_image(&pixels, &draw("reference", &reference));
                    let difference = pixels
                        .iter()
                        .zip(draw("flat", &flat))
                        .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                        .count();
                    assert!(
                        difference > 20,
                        "{method}/{follow}/{kind:?}/{samples}: {difference}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn sphere_model_discrete_births_and_binding_reach_quad_and_mesh_rendering() {
    use xiv_companion_data::{VfxRuntime, avfx::*};
    let ramp = |start, end| AvfxCurve {
        keys: [(0, start), (4, end)]
            .into_iter()
            .map(|(time, z)| AvfxCurveKey {
                time,
                z,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
            })
            .collect(),
        ..Default::default()
    };
    let normals = [
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
        [-1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
    ];
    for method in [1, 3, 7] {
        for follow in [false, true] {
            let mut file = cone_model_render_fixture();
            let emitter = &mut file.emitters[0];
            emitter.emitter_type = Some(EmitterType::SphereModel);
            emitter.data = Some(AvfxEmitterData::SphereModel(SphereModelEmitterData {
                generate_method: method,
                divide_x: 4,
                divide_y: 2,
                radius: ramp(0.2, 0.4),
                injection_speed: ramp(0.025, 0.025),
                rotation: EmitterDataRotation {
                    angles: [
                        Default::default(),
                        Default::default(),
                        ramp(0.0, std::f32::consts::FRAC_PI_2),
                    ],
                    ..Default::default()
                },
                ..Default::default()
            }));
            emitter.particle_items[0].create_count = 6;
            emitter.particle_items[0].parent_influence_coord = 1;
            emitter.particle_items[0].influence_coord_pos = follow;
            for kind in [ParticleType::Quad, ParticleType::LightModel] {
                file.particles[0].particle_type = Some(kind);
                file.particles[0].data = if kind == ParticleType::Quad {
                    AvfxParticleData::None
                } else {
                    AvfxParticleData::LightModel { model_index: 1 }
                };
                let runtime = VfxRuntime::new(&file);
                let mut actual = (Vec::new(), Vec::new());
                runtime.sample(4.0 / 30.0, &mut actual.0);
                runtime.sample_mesh(4.0 / 30.0, &mut actual.1);
                assert_eq!(actual.0.len() + actual.1.len(), 6);
                let positions: Vec<_> = (0..6)
                    .map(|i| {
                        let n = normals[if method == 7 { 1 + i % 4 } else { i }];
                        let direction: [f32; 3] = [-1.1 * n[0], 0.8 * n[1], n[2]];
                        let norm = direction.iter().map(|v| v * v).sum::<f32>().sqrt();
                        let point = if method == 1 {
                            [0.0; 3]
                        } else if follow {
                            [0.44 * n[1], 0.32 * n[0], 0.4 * n[2]]
                        } else {
                            direction.map(|v| v * 0.2)
                        };
                        std::array::from_fn(|axis| {
                            [0.1, 0.0, 0.0][axis] + point[axis] + 0.1 * direction[axis] / norm
                        })
                    })
                    .collect();
                let mut reference = actual.clone();
                for (quad, position) in reference.0.iter_mut().zip(&positions) {
                    quad.position = *position;
                }
                for (mesh, position) in reference.1.iter_mut().zip(&positions) {
                    mesh.position = *position;
                }
                let mut collapsed = actual.clone();
                for quad in &mut collapsed.0 {
                    quad.position = [0.1, 0.0, 0.0];
                }
                for mesh in &mut collapsed.1 {
                    mesh.position = [0.1, 0.0, 0.0];
                }
                for samples in [1, 4] {
                    let draw = |name, packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                        render_with_geometry_and_samples(
                            &format!("vfx-sphere-{method}-{follow}-{kind:?}-{name}-{samples}"),
                            packets.0.clone(),
                            packets.1.clone(),
                            vec![solid([80, 220, 160, 255])],
                            square(),
                            samples,
                        )
                    };
                    let pixels = draw("actual", &actual);
                    assert_image(&pixels, &draw("reference", &reference));
                    let difference = pixels
                        .iter()
                        .zip(draw("collapsed", &collapsed))
                        .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                        .count();
                    assert!(
                        difference > 20,
                        "{method}/{follow}/{kind:?}/{samples}: {difference}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn unknown_euler_order_uses_client_identity_in_quad_and_mesh_rendering() {
    use xiv_companion_data::{VfxRuntime, avfx::*};
    let curve = |z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let angles = [0.43, -0.71, 1.09];
    let mut file = model_render_fixture();
    file.emitters[0].rotation = AvfxCurve3Axis {
        x: Some(curve(angles[0])),
        y: Some(curve(angles[1])),
        z: Some(curve(angles[2])),
        ..Default::default()
    };
    file.emitters[0].rotation_order = 6;
    let Some(AvfxEmitterData::Model(data)) = &mut file.emitters[0].data else {
        unreachable!()
    };
    data.injection_speed = Default::default();
    data.rotation.angles = angles.map(curve);
    data.rotation.order = 6;
    file.particles[0].rotation = file.emitters[0].rotation.clone();
    file.particles[0].rotation_order = 6;
    file.particles[0].scale = AvfxCurve3Axis {
        x: Some(curve(0.4)),
        y: Some(curve(0.4)),
        z: Some(curve(0.4)),
        ..Default::default()
    };
    for kind in [ParticleType::Quad, ParticleType::LightModel] {
        file.particles[0].particle_type = Some(kind);
        file.particles[0].data = if kind == ParticleType::Quad {
            AvfxParticleData::None
        } else {
            AvfxParticleData::LightModel { model_index: 1 }
        };
        let sample = |file: &AvfxFile| {
            let runtime = VfxRuntime::new(file);
            let mut packets = (Vec::new(), Vec::new());
            runtime.sample(0.0, &mut packets.0);
            runtime.sample_mesh(0.0, &mut packets.1);
            packets
        };
        let actual = sample(&file);
        assert_eq!(actual.0.len() + actual.1.len(), 1);
        let mut reference = actual.clone();
        // All three multi-axis rotations use the client identity fallback.
        // Parent scale (-1.1, 0.8, 1) and translation (0.1, 0, 0) remain.
        for quad in &mut reference.0 {
            quad.position = [0.1, 0.16, 0.0];
            quad.orientation = [0.0, 0.0, 0.0, 1.0];
        }
        for mesh in &mut reference.1 {
            mesh.position = [0.1, 0.16, 0.0];
            mesh.orientation = [0.0, 0.0, 0.0, 1.0];
        }
        let mut zyx = file.clone();
        zyx.emitters[0].rotation_order = 5;
        zyx.particles[0].rotation_order = 5;
        let Some(AvfxEmitterData::Model(data)) = &mut zyx.emitters[0].data else {
            unreachable!()
        };
        data.rotation.order = 5;
        let control = sample(&zyx);
        for samples in [1, 4] {
            let draw = |name, packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                render_with_geometry_and_samples(
                    &format!("vfx-euler-unknown-{kind:?}-{name}-{samples}"),
                    packets.0.clone(),
                    packets.1.clone(),
                    vec![solid([80, 220, 160, 255])],
                    square(),
                    samples,
                )
            };
            let pixels = draw("actual", &actual);
            assert_image(&pixels, &draw("reference", &reference));
            assert!(
                pixels
                    .iter()
                    .zip(draw("zyx", &control))
                    .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                    .count()
                    > 20
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn child_emitter_birth_motion_and_follow_reach_rendering() {
    use xiv_companion_data::{VfxRuntime, avfx::*};
    let curve = |start, end| AvfxCurve {
        keys: [(0, start), (4, end)]
            .into_iter()
            .map(|(time, z)| AvfxCurveKey {
                time,
                z,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
            })
            .collect(),
        ..Default::default()
    };
    let item = AvfxEmitterItem {
        enabled: true,
        target_index: 0,
        create_time: 1,
        create_count: 1,
        create_probability: 100,
        parent_influence_coord: 2,
        ..Default::default()
    };
    let mut file = AvfxFile {
        timelines: vec![AvfxTimeline {
            items: vec![AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: -1,
                effector_index: -1,
                emitter_index: 0,
                platform: 0,
                clip_index: -1,
            }],
            ..Default::default()
        }],
        emitters: vec![
            AvfxEmitter {
                emitter_type: Some(EmitterType::Model),
                position: AvfxCurve3Axis {
                    x: Some(curve(0.0, 0.3)),
                    y: Some(curve(0.0, 0.2)),
                    ..Default::default()
                },
                rotation: AvfxCurve3Axis {
                    z: Some(curve(0.0, std::f32::consts::FRAC_PI_2)),
                    ..Default::default()
                },
                data: Some(AvfxEmitterData::Model(ModelEmitterData {
                    model_index: 0,
                    generate_method: 3,
                    injection_speed: curve(0.1, 0.1),
                    ..Default::default()
                })),
                emitter_items: vec![AvfxEmitterItem {
                    target_index: 1,
                    create_count: 2,
                    local_direction: 1,
                    ..item.clone()
                }],
                ..Default::default()
            },
            AvfxEmitter {
                emitter_type: Some(EmitterType::Point),
                rotation: AvfxCurve3Axis {
                    x: Some(curve(
                        std::f32::consts::FRAC_PI_2,
                        std::f32::consts::FRAC_PI_2,
                    )),
                    ..Default::default()
                },
                scale: AvfxCurve3Axis {
                    x: Some(curve(0.3, 0.3)),
                    y: Some(curve(0.2, 0.2)),
                    z: Some(curve(0.3, 0.3)),
                    ..Default::default()
                },
                particle_items: vec![item],
                ..Default::default()
            },
        ],
        particles: vec![AvfxParticle {
            particle_type: Some(ParticleType::Quad),
            rotation_direction_base: rotation_direction_base::NONE,
            texture_color1: Some(AvfxParticleTexture {
                enabled: true,
                texture_index: 0,
                calculate_color: 1,
                calculate_alpha: 1,
                ..Default::default()
            }),
            ..Default::default()
        }],
        models: vec![
            VfxModelGeometry {
                emit_vertex_numbers: vec![0, 1],
                emit_vertices: [-0.6, 0.6]
                    .map(|x| VfxEmitVertex {
                        position: [x, 0.0, 0.0],
                        normal: [0.0, 1.0, 0.0],
                        color: [255; 4],
                    })
                    .to_vec(),
                ..Default::default()
            },
            VfxModelGeometry {
                draw: Some(square()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    for mode in [0, 2, 3] {
        file.emitters[0].emitter_items[0].parent_influence_coord = mode;
        for kind in [ParticleType::Quad, ParticleType::LightModel] {
            file.particles[0].particle_type = Some(kind);
            file.particles[0].data = if kind == ParticleType::Quad {
                AvfxParticleData::None
            } else {
                AvfxParticleData::LightModel { model_index: 1 }
            };
            let runtime = VfxRuntime::new(&file);
            let mut actual = (Vec::new(), Vec::new());
            runtime.sample(4.0 / 30.0, &mut actual.0);
            runtime.sample_mesh(4.0 / 30.0, &mut actual.1);
            assert_eq!(actual.0.len() + actual.1.len(), 2);
            let mut expected = actual.clone();
            for index in 0..2 {
                let x = [-0.6, 0.6][index];
                let (position, basis) = if mode == 2 {
                    (
                        [-0.1, 0.2 + x, 0.0],
                        [[0.0, -0.3, 0.0], [-0.2, 0.0, 0.0], [0.0, 0.0, -0.3]],
                    )
                } else {
                    (
                        [x, 0.4, 0.0],
                        [[-0.3, 0.0, 0.0], [0.0, 0.2, 0.0], [0.0, 0.0, -0.3]],
                    )
                };
                if kind == ParticleType::Quad {
                    expected.0[index].position = position;
                    expected.0[index].parent_basis = basis;
                } else {
                    expected.1[index].position = position;
                    expected.1[index].parent_basis = basis;
                }
            }
            let mut control = expected.clone();
            for q in &mut control.0 {
                q.position[1] += 0.5;
            }
            for m in &mut control.1 {
                m.position[1] += 0.5;
            }
            for samples in [1, 4] {
                let draw = |name, packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                    render_with_geometry_and_samples(
                        &format!("vfx-child-emitter-{mode}-{kind:?}-{name}-{samples}"),
                        packets.0.clone(),
                        packets.1.clone(),
                        vec![solid([230, 95, 140, 255])],
                        square(),
                        samples,
                    )
                };
                let reference = draw("reference", &expected);
                assert_image(&draw("actual", &actual), &reference);
                assert!(
                    reference
                        .iter()
                        .zip(draw("control", &control))
                        .filter(|(a, b)| (a[0] - b[0]).abs() > 0.05)
                        .count()
                        > 50
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn point_direction_reaches_quad_and_mesh_rendering() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime, avfx::*};
    let curve = |z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let axes = |[x, y, z]: [f32; 3]| AvfxCurve3Axis {
        x: Some(curve(x)),
        y: Some(curve(y)),
        z: Some(curve(z)),
        ..Default::default()
    };
    let item = AvfxEmitterItem {
        enabled: true,
        target_index: 0,
        create_time: 1,
        create_count: 1,
        create_probability: 100,
        parameter_link: -1,
        ..Default::default()
    };
    let mut file = AvfxFile {
        timelines: vec![AvfxTimeline {
            binder_index: -1,
            items: vec![AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: -1,
                effector_index: -1,
                emitter_index: 0,
                platform: 0,
                clip_index: -1,
            }],
            ..Default::default()
        }],
        emitters: vec![AvfxEmitter {
            emitter_type: Some(EmitterType::Point),
            effector_index: -1,
            rotation: axes([0.0, 0.0, std::f32::consts::FRAC_PI_2]),
            scale: axes([-1.5, 2.0, 0.75]),
            ..Default::default()
        }],
        models: vec![
            VfxModelGeometry::default(),
            VfxModelGeometry {
                draw: Some(square()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    for (index, kind) in [ParticleType::Quad, ParticleType::LightModel]
        .into_iter()
        .enumerate()
    {
        file.emitters[0].particle_items.push(AvfxEmitterItem {
            target_index: index as i32,
            local_direction: 1,
            ..item.clone()
        });
        file.particles.push(AvfxParticle {
            particle_type: Some(kind),
            collision_type: -1,
            rotation_direction_base: rotation_direction_base::NONE,
            position: axes([index as f32 * 0.8 - 0.4, 0.2, 0.1]),
            scale: axes([0.3, 0.2, 0.3]),
            texture_color1: Some(AvfxParticleTexture {
                enabled: true,
                texture_index: 0,
                calculate_color: 1,
                calculate_alpha: 1,
                ..Default::default()
            }),
            data: if kind == ParticleType::Quad {
                AvfxParticleData::None
            } else {
                AvfxParticleData::LightModel { model_index: 1 }
            },
            ..Default::default()
        });
    }
    for nested in [false, true] {
        if nested {
            let mut leaf = file.emitters[0].clone();
            leaf.rotation = Default::default();
            leaf.scale = Default::default();
            for item in &mut leaf.particle_items {
                item.parent_influence_coord = 2;
            }
            let point = AvfxEmitter {
                rotation: axes([std::f32::consts::FRAC_PI_2, 0.0, 0.0]),
                particle_items: vec![],
                emitter_items: vec![AvfxEmitterItem {
                    target_index: 2,
                    parent_influence_coord: 2,
                    ..item.clone()
                }],
                ..file.emitters[0].clone()
            };
            file.emitters = vec![
                AvfxEmitter {
                    emitter_type: Some(EmitterType::Cone),
                    data: Some(AvfxEmitterData::Cone(ConeEmitterData {
                        rotation: EmitterDataRotation {
                            angles: [
                                curve(-std::f32::consts::FRAC_PI_2),
                                Default::default(),
                                Default::default(),
                            ],
                            ..Default::default()
                        },
                        ..Default::default()
                    })),
                    emitter_items: vec![AvfxEmitterItem {
                        target_index: 1,
                        parent_influence_coord: 2,
                        ..item.clone()
                    }],
                    ..Default::default()
                },
                point,
                leaf,
            ];
        }
        let runtime = VfxRuntime::new(&file);
        let mut actual = (Vec::new(), Vec::new());
        runtime.sample(4.0 / 30.0, &mut actual.0);
        runtime.sample_mesh(4.0 / 30.0, &mut actual.1);
        assert_eq!((actual.0.len(), actual.1.len()), (1, 1));
        let mut expected = actual.clone();
        // No Binder: identity. Cone +Y through two Points: W * [-X,Z,Y].
        let (positions, basis) = if nested {
            (
                [[-0.6, -0.15, 0.2], [0.6, -0.15, 0.2]],
                [[1.5, 0.0, 0.0], [0.0, -0.75, 0.0], [0.0, 0.0, 2.0]],
            )
        } else {
            (
                [[-0.4, 0.2, 0.1], [0.4, 0.2, 0.1]],
                xiv_companion::VFX_IDENTITY_BASIS,
            )
        };
        expected.0[0].position = positions[0];
        expected.1[0].position = positions[1];
        expected.0[0].parent_basis = basis;
        expected.1[0].parent_basis = basis;
        let staged = if nested {
            None
        } else {
            let mut playback = VfxPlayback::new(runtime);
            assert_eq!(playback.fallback_reason(), None);
            playback.advance(4.0 / 30.0).unwrap();
            let mut packets = (Vec::new(), Vec::new());
            playback.sample(&mut packets.0, &mut packets.1);
            Some(packets)
        };
        let mut control = expected.clone();
        control.0[0].position[1] += 0.6;
        control.1[0].position[1] += 0.6;
        for samples in [1, 4] {
            let draw = |name, packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                render_with_geometry_and_samples(
                    &format!("vfx-point-direction-{nested}-{name}-{samples}"),
                    packets.0.clone(),
                    packets.1.clone(),
                    vec![solid([70, 220, 140, 255])],
                    square(),
                    samples,
                )
            };
            let reference = draw("reference", &expected);
            assert_image(&draw("continuous", &actual), &reference);
            if let Some(staged) = &staged {
                assert_image(&draw("staged", staged), &reference);
            }
            assert!(
                reference
                    .iter()
                    .zip(draw("control", &control))
                    .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                    .count()
                    > 50
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn emitter_vr_keeps_zero_velocity_particles_stationary_in_rendering() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime, avfx::*};
    let curve = |z| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            z,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
        }],
        ..Default::default()
    };
    let mut file = AvfxFile {
        timelines: vec![AvfxTimeline {
            binder_index: -1,
            items: vec![AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: -1,
                effector_index: -1,
                emitter_index: 0,
                platform: 0,
                clip_index: -1,
            }],
            ..Default::default()
        }],
        emitters: vec![AvfxEmitter {
            emitter_type: Some(EmitterType::Point),
            effector_index: -1,
            rotation_velocity: [curve(0.3), curve(0.6), curve(0.9)],
            rotation_velocity_random: [curve(1.0), curve(0.5), curve(0.25)],
            ..Default::default()
        }],
        models: vec![
            VfxModelGeometry::default(),
            VfxModelGeometry {
                draw: Some(square()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    for (index, kind) in [ParticleType::Quad, ParticleType::LightModel]
        .into_iter()
        .enumerate()
    {
        file.emitters[0].particle_items.push(AvfxEmitterItem {
            enabled: true,
            target_index: index as i32,
            create_time: 1,
            create_count: 1,
            create_probability: 100,
            parameter_link: -1,
            ..Default::default()
        });
        file.particles.push(AvfxParticle {
            particle_type: Some(kind),
            collision_type: -1,
            rotation_direction_base: rotation_direction_base::NONE,
            position: AvfxCurve3Axis {
                x: Some(curve(index as f32 - 0.5)),
                ..Default::default()
            },
            scale: AvfxCurve3Axis {
                x: Some(curve(0.35)),
                y: Some(curve(0.35)),
                z: Some(curve(0.35)),
                ..Default::default()
            },
            rotation_velocity: [curve(0.7), curve(0.2), curve(0.1)],
            texture_color1: Some(AvfxParticleTexture {
                enabled: true,
                texture_index: 0,
                calculate_color: 1,
                calculate_alpha: 1,
                ..Default::default()
            }),
            data: if kind == ParticleType::Quad {
                AvfxParticleData::None
            } else {
                AvfxParticleData::LightModel { model_index: 1 }
            },
            ..Default::default()
        });
    }
    let runtime = VfxRuntime::new(&file);
    let mut playback = VfxPlayback::new(runtime.clone());
    assert!(
        playback.fallback_reason().is_none(),
        "{:?}",
        playback.fallback_reason()
    );
    let mut reference = (Vec::new(), Vec::new());
    playback.sample(&mut reference.0, &mut reference.1);
    assert_eq!((reference.0.len(), reference.1.len()), (1, 1));
    assert_eq!(reference.0[0].position, [-0.5, 0.0, 0.0]);
    assert_eq!(reference.1[0].position, [0.5, 0.0, 0.0]);
    playback.advance(4.0 / 30.0).unwrap();
    let mut staged = (Vec::new(), Vec::new());
    playback.sample(&mut staged.0, &mut staged.1);
    let mut continuous = (Vec::new(), Vec::new());
    runtime.sample(4.0 / 30.0, &mut continuous.0);
    runtime.sample_mesh(4.0 / 30.0, &mut continuous.1);
    let mut displaced = reference.clone();
    displaced.0[0].position[1] = 0.7;
    displaced.1[0].position[1] = 0.7;
    for samples in [1, 4] {
        let draw = |name: &str, packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
            render_with_geometry_and_samples(
                &format!("vfx-emitter-vr-{name}-{samples}"),
                packets.0.clone(),
                packets.1.clone(),
                vec![solid([70, 220, 140, 255])],
                square(),
                samples,
            )
        };
        let expected = draw("stationary", &reference);
        assert_image(&draw("staged", &staged), &expected);
        assert_image(&draw("continuous", &continuous), &expected);
        let control = draw("displaced-control", &displaced);
        assert!(
            expected
                .iter()
                .zip(control)
                .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                .count()
                > 50
        );
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn staged_gravity_birth_prewarm_and_follow_reach_rendering() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime, avfx::*};
    let curve = |values: &[(i16, f32)]| AvfxCurve {
        keys: values
            .iter()
            .map(|&(time, z)| AvfxCurveKey {
                time,
                z,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
            })
            .collect(),
        ..Default::default()
    };
    let mut file = AvfxFile {
        timelines: vec![AvfxTimeline {
            binder_index: -1,
            items: vec![AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: -1,
                effector_index: -1,
                emitter_index: 0,
                platform: 0,
                clip_index: -1,
            }],
            ..Default::default()
        }],
        emitters: vec![AvfxEmitter {
            emitter_type: Some(EmitterType::Point),
            effector_index: -1,
            gravity: curve(&[(0, -0.02)]),
            ..Default::default()
        }],
        models: vec![
            VfxModelGeometry::default(),
            VfxModelGeometry {
                draw: Some(square()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    for (index, kind) in [
        ParticleType::Quad,
        ParticleType::Model,
        ParticleType::LightModel,
    ]
    .into_iter()
    .enumerate()
    {
        file.emitters[0].particle_items.push(AvfxEmitterItem {
            enabled: true,
            target_index: index as i32,
            create_time: 1,
            create_count: 1,
            create_probability: 100,
            parameter_link: -1,
            generate_delay: 1,
            start_frame: 2,
            start_frame_null_update: true,
            parent_influence_coord: 2,
            ..Default::default()
        });
        file.particles.push(AvfxParticle {
            particle_type: Some(kind),
            collision_type: -1,
            rotation_direction_base: rotation_direction_base::NONE,
            position: AvfxCurve3Axis {
                x: Some(curve(&[(0, (index as f32 - 1.0) * 0.65)])),
                ..Default::default()
            },
            scale: AvfxCurve3Axis {
                x: Some(curve(&[(0, 0.22)])),
                y: Some(curve(&[(0, 0.22)])),
                z: Some(curve(&[(0, 0.22)])),
                ..Default::default()
            },
            gravity: curve(&[(0, 0.0), (20, 0.8)]),
            texture_color1: Some(AvfxParticleTexture {
                enabled: true,
                texture_index: 0,
                calculate_color: 1,
                calculate_alpha: 1,
                ..Default::default()
            }),
            data: match kind {
                ParticleType::Quad => AvfxParticleData::None,
                ParticleType::LightModel => AvfxParticleData::LightModel { model_index: 1 },
                _ => AvfxParticleData::Model {
                    model_number_random_value: 0,
                    model_number_random_type: 0,
                    model_number_random_interval: 0,
                    fresnel_type: 0,
                    directional_light_type: 0,
                    point_light_type: 0,
                    is_lightning: false,
                    is_morph: false,
                    model_indexes: vec![1],
                    animation_number: None,
                    morph: None,
                    fresnel_curve: None,
                    fresnel_curve_random: None,
                    fresnel_rotation: None,
                    color_begin: AvfxColorCurve::default(),
                    color_end: AvfxColorCurve::default(),
                },
            },
            ..Default::default()
        });
    }
    let sample = |file: &AvfxFile| {
        let mut playback = VfxPlayback::new(VfxRuntime::new(file));
        assert_eq!(playback.fallback_reason(), None);
        for _ in 0..2 {
            playback.advance(2.0 / 30.0).unwrap();
        }
        assert_eq!(playback.fallback_reason(), None);
        let (mut quads, mut meshes) = (Vec::new(), Vec::new());
        playback.sample(&mut quads, &mut meshes);
        assert_eq!((quads.len(), meshes.len()), (1, 2));
        (quads, meshes)
    };
    let actual = sample(&file);
    file.emitters[0].gravity = AvfxCurve::default();
    for particle in &mut file.particles {
        particle.gravity = AvfxCurve::default();
    }
    let mut reference = sample(&file);
    // Root: -0.02*(2^2 + 2*2^2) = -0.24. Quad has no +e8 motion:
    // newborn +100: v=.08, y=.08; next delta2: v=.40, y=.88.
    // Mesh prewarm leaves v=.04,y=.04, yielding y=1.04 after those phases.
    reference.0[0].position[1] = 0.64;
    for mesh in &mut reference.1 {
        mesh.position[1] = 0.80;
    }
    let mut continuous = reference.clone();
    // Continuous integrals at age4: particle .04*4^3/6; root -.02*4^2/2.
    continuous.0[0].position[1] = 0.04 * 64.0 / 6.0 - 0.16;
    for mesh in &mut continuous.1 {
        mesh.position[1] = continuous.0[0].position[1];
    }
    for samples in [1, 4] {
        let draw = |name: &str, packets: &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
            render_with_geometry_and_samples(
                &format!("vfx-staged-gravity-{name}-{samples}"),
                packets.0.clone(),
                packets.1.clone(),
                vec![solid([70, 220, 140, 255])],
                square(),
                samples,
            )
        };
        let pixels = draw("actual", &actual);
        assert_image(&pixels, &draw("expected", &reference));
        let control = draw("continuous-integral", &continuous);
        assert!(
            pixels
                .iter()
                .zip(control)
                .filter(|(a, b)| (a[1] - b[1]).abs() > 0.05)
                .count()
                > 50
        );
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn staged_distortion_draw_age_and_quad_byte_packing_reach_rendering() {
    use xiv_companion_data::{VfxPlayback, VfxRuntime, avfx::*};
    let curve = |end| AvfxCurve {
        keys: [(0, 0.0), (8, end)]
            .into_iter()
            .map(|(time, z)| AvfxCurveKey {
                time,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
                z,
            })
            .collect(),
        ..Default::default()
    };
    for kind in [
        ParticleType::Quad,
        ParticleType::Model,
        ParticleType::LightModel,
    ] {
        let file = AvfxFile {
            timelines: vec![AvfxTimeline {
                binder_index: -1,
                items: vec![AvfxTimelineItem {
                    enabled: true,
                    start_time: 0,
                    end_time: -1,
                    binder_index: -1,
                    effector_index: -1,
                    emitter_index: 0,
                    platform: 0,
                    clip_index: -1,
                }],
                ..Default::default()
            }],
            emitters: vec![AvfxEmitter {
                emitter_type: Some(EmitterType::Point),
                effector_index: -1,
                particle_items: vec![AvfxEmitterItem {
                    enabled: true,
                    target_index: 0,
                    create_time: 1,
                    create_count: 1,
                    create_probability: 100,
                    parameter_link: -1,
                    start_frame: 4,
                    start_frame_null_update: true,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            particles: vec![AvfxParticle {
                particle_type: Some(kind),
                collision_type: -1,
                rotation_direction_base: rotation_direction_base::NONE,
                data: match kind {
                    ParticleType::Quad => AvfxParticleData::None,
                    ParticleType::LightModel => AvfxParticleData::LightModel { model_index: 1 },
                    _ => AvfxParticleData::Model {
                        model_number_random_value: 0,
                        model_number_random_type: 0,
                        model_number_random_interval: 0,
                        fresnel_type: 0,
                        directional_light_type: 0,
                        point_light_type: 0,
                        is_lightning: false,
                        is_morph: false,
                        model_indexes: vec![1],
                        animation_number: None,
                        morph: None,
                        fresnel_curve: None,
                        fresnel_curve_random: None,
                        fresnel_rotation: None,
                        color_begin: AvfxColorCurve::default(),
                        color_end: AvfxColorCurve::default(),
                    },
                },
                texture_color1: Some(AvfxParticleTexture {
                    enabled: true,
                    texture_index: 0,
                    calculate_color: 1,
                    calculate_alpha: 1,
                    texture_filter: 1,
                    texture_border_u: 1,
                    texture_border_v: 1,
                    ..Default::default()
                }),
                texture_distortion: Some(AvfxParticleDistortion {
                    enabled: true,
                    texture_index: 1,
                    target_uv: [true, false, false, false],
                    power: curve(-1.0),
                    ..Default::default()
                }),
                uv_sets: vec![AvfxUvSet {
                    scroll: AvfxCurve2Axis {
                        x: Some(curve(0.4)),
                        ..Default::default()
                    },
                    ..Default::default()
                }],
                ..Default::default()
            }],
            models: vec![
                VfxModelGeometry::default(),
                VfxModelGeometry {
                    draw: Some(square()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut playback = VfxPlayback::new(VfxRuntime::new(&file));
        assert_eq!(playback.fallback_reason(), None);
        playback.advance(2.0 / 30.0).unwrap();
        let mut quads = Vec::new();
        let mut meshes = Vec::new();
        playback.sample(&mut quads, &mut meshes);
        assert_eq!(quads.len() + meshes.len(), 1);
        // UV is cached at age 4; DPow is drawn at age 6 (-0.75).
        // CVTTSS2SI(-0.75 * 255) = -191, whose low byte is 65.
        // A -1.5 control differs by almost exactly one repeated UV tile
        // after the 0.5 distortion offset, hiding the byte-wrap effect.
        let (power, stale_power) = if kind == ParticleType::Quad {
            (65.0 / 255.0, 129.0 / 255.0)
        } else {
            (-0.75, -0.5)
        };
        for samples in [1, 4] {
            let draw = |name: &str, reference: bool, power: f32, uv_age: f32| {
                let mut quads = quads.clone();
                let mut meshes = meshes.clone();
                for quad in &mut quads {
                    quad.distortion_power = power;
                    if reference {
                        quad.texture_distortion_index = -1;
                        quad.uv_origins[0] = [uv_age * 0.05 + power * 0.5, -power * 0.5];
                    }
                }
                for mesh in &mut meshes {
                    mesh.distortion_power = power;
                    if reference {
                        mesh.texture_distortion_index = -1;
                        mesh.uv_origins[0] = [uv_age * 0.05 + power * 0.5, -power * 0.5];
                    }
                }
                render_with_geometry_and_samples(
                    &format!("vfx-staged-distortion-{kind:?}-{name}-{samples}"),
                    quads,
                    meshes,
                    gradient_textures(),
                    square(),
                    samples,
                )
            };
            let actual_power = quads
                .first()
                .map_or_else(|| meshes[0].distortion_power, |quad| quad.distortion_power);
            let pixels = draw("actual", false, actual_power, 0.0);
            assert_image(&pixels, &draw("expected", true, power, 4.0));
            for (name, control_power, uv_age) in [
                ("stale-power", stale_power, 4.0),
                ("current-uv", power, 6.0),
            ] {
                let control = draw(name, true, control_power, uv_age);
                assert!(
                    pixels
                        .iter()
                        .zip(control)
                        .filter(|(a, b)| { (a[0] - b[0]).abs().max((a[1] - b[1]).abs()) > 0.03 })
                        .count()
                        > 20,
                    "{kind:?}, {name}, MSAA={samples}"
                );
            }
            if kind == ParticleType::Quad {
                let control = draw("float-power", false, -0.75, 0.0);
                assert!(
                    pixels
                        .iter()
                        .zip(control)
                        .filter(|(a, b)| { (a[0] - b[0]).abs().max((a[1] - b[1]).abs()) > 0.1 })
                        .count()
                        > 20
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn sampled_creation_angles_rotate_quad_and_mesh_geometry() {
    use xiv_companion_data::{
        AVFX_FPS, VfxRuntime,
        avfx::{
            AvfxColorCurve, AvfxCurve, AvfxCurve3Axis, AvfxCurveKey, AvfxEmitter, AvfxEmitterItem,
            AvfxFile, AvfxParticle, AvfxParticleData, AvfxTimeline, AvfxTimelineItem, ParticleType,
            VfxModelGeometry, rotation_direction_base,
        },
    };
    let constant = |value| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
            z: value,
        }],
        ..Default::default()
    };
    let mut file = AvfxFile {
        timelines: vec![AvfxTimeline {
            items: vec![AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: -1,
                effector_index: -1,
                emitter_index: 0,
                platform: 0,
                clip_index: -1,
            }],
            ..Default::default()
        }],
        emitters: vec![AvfxEmitter {
            create_count: constant(1.0),
            create_interval: constant(10.0),
            particle_items: vec![AvfxEmitterItem {
                enabled: true,
                target_index: 0,
                create_probability: 100,
                by_injection_angle: [0.0, 0.0, std::f32::consts::FRAC_PI_2],
                ..Default::default()
            }],
            ..Default::default()
        }],
        particles: vec![AvfxParticle {
            particle_type: Some(ParticleType::Quad),
            rotation_direction_base: rotation_direction_base::NONE,
            scale: AvfxCurve3Axis {
                x: Some(constant(1.4)),
                y: Some(constant(0.2)),
                ..Default::default()
            },
            color: AvfxColorCurve {
                alpha: Some(constant(1.0)),
                ..Default::default()
            },
            ..Default::default()
        }],
        models: vec![
            VfxModelGeometry::default(),
            VfxModelGeometry {
                draw: Some(square()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    for mesh_mode in [false, true] {
        if mesh_mode {
            file.particles[0].particle_type = Some(ParticleType::LightModel);
            file.particles[0].data = AvfxParticleData::LightModel { model_index: 1 };
        }
        let mut neutral = file.clone();
        neutral.emitters[0].particle_items[0].by_injection_angle = [0.0; 3];
        let sample = |file: &AvfxFile| {
            let runtime = VfxRuntime::new(file);
            let mut quads = Vec::new();
            let mut meshes = Vec::new();
            runtime.sample(10.0 / AVFX_FPS, &mut quads);
            runtime.sample_mesh(10.0 / AVFX_FPS, &mut meshes);
            assert_eq!(quads.len() + meshes.len(), 2);
            (quads, meshes)
        };
        let actual = sample(&file);
        let baseline = sample(&neutral);
        let mut expected = baseline.clone();
        let (s, c) = std::f32::consts::FRAC_PI_4.sin_cos();
        if mesh_mode {
            expected.1[1].orientation = [0.0, 0.0, s, c];
        } else {
            expected.0[1].orientation = [0.0, 0.0, s, c];
        }
        for samples in [1, 4] {
            let draw = |name: &str, (quads, meshes): &(Vec<VfxQuad>, Vec<VfxMeshInstance>)| {
                render_with_geometry_and_samples(
                    &format!("vfx-creation-angle-{name}-{mesh_mode}-{samples}"),
                    quads.clone(),
                    meshes.clone(),
                    vec![],
                    square(),
                    samples,
                )
            };
            let pixels = draw("actual", &actual);
            assert_image(&pixels, &draw("expected", &expected));
            let unchanged = draw("neutral", &baseline);
            assert!(
                pixels
                    .iter()
                    .zip(unchanged)
                    .filter(|(a, b)| (a[0] - b[0]).abs() > 0.1)
                    .count()
                    > 20,
                "creation angle did not change visible geometry"
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn sampled_creation_events_preserve_alpha_composition_order() {
    use xiv_companion_data::{
        AVFX_FPS, VfxRuntime,
        avfx::{
            AvfxColorCurve, AvfxCurve, AvfxCurveKey, AvfxEmitter, AvfxEmitterItem, AvfxFile,
            AvfxParticle, AvfxScheduler, AvfxSchedulerItem, AvfxTimeline, AvfxTimelineItem,
            ParticleType, rotation_direction_base,
        },
    };

    let constant = |[x, y, z]: [f32; 3]| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            interpolation: AvfxCurveKey::INTERPOLATION_LINEAR,
            x,
            y,
            z,
        }],
        ..Default::default()
    };
    let file = AvfxFile {
        schedulers: vec![AvfxScheduler {
            items: vec![AvfxSchedulerItem {
                enabled: true,
                start_time: 0,
                timeline_index: 0,
            }],
            ..Default::default()
        }],
        timelines: vec![AvfxTimeline {
            items: vec![AvfxTimelineItem {
                enabled: true,
                start_time: 0,
                end_time: -1,
                binder_index: -1,
                effector_index: -1,
                emitter_index: 0,
                platform: 0,
                clip_index: -1,
            }],
            ..Default::default()
        }],
        emitters: vec![AvfxEmitter {
            create_count: constant([0.0, 0.0, 1.0]),
            create_interval: constant([0.0, 0.0, 10.0]),
            particle_items: (0..2)
                .map(|target_index| AvfxEmitterItem {
                    enabled: true,
                    target_index,
                    create_time: 0,
                    create_probability: 100,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }],
        particles: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
            .map(|rgb| AvfxParticle {
                particle_type: Some(ParticleType::Quad),
                rotation_direction_base: rotation_direction_base::NONE,
                color: AvfxColorCurve {
                    rgb: Some(constant(rgb)),
                    alpha: Some(constant([0.0, 0.0, 0.5])),
                    ..Default::default()
                },
                ..Default::default()
            })
            .to_vec(),
        ..Default::default()
    };
    let mut quads = Vec::new();
    VfxRuntime::new(&file).sample(20.0 / AVFX_FPS, &mut quads);
    assert_eq!(quads.len(), 6);
    let background = render_center("vfx-creation-order-background", vec![], vec![]);
    // Three red/green pairs at alpha 1/2 leave 1/64 of the background.
    let expected = [
        background[0] / 64.0 + 21.0 / 64.0,
        background[1] / 64.0 + 42.0 / 64.0,
        background[2] / 64.0,
        background[3] / 64.0 + 63.0 / 64.0,
    ];
    for msaa_samples in [1, 4] {
        let pixels = render_with_geometry_and_samples(
            &format!("vfx-creation-order-{msaa_samples}"),
            quads.clone(),
            vec![],
            vec![],
            square(),
            msaa_samples,
        );
        assert_color(pixels[32 * 64 + 32], expected);
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn texture_groups_preserve_equal_priority_input_order() {
    let mut red = quad([1.0, 0.0, 0.0, 1.0], false);
    red.texture_indexes[0] = 1;
    let green = quad([0.0, 1.0, 0.0, 1.0], false);
    assert_color(
        render_center("vfx-order-texture-quads", vec![red, green], vec![]),
        green.color,
    );
    assert_color(
        render_center(
            "vfx-order-texture-meshes",
            vec![],
            vec![mesh(red), mesh(green)],
        ),
        green.color,
    );
}

#[test]
#[ignore = "requires native wgpu"]
fn ordinary_layers_order_before_particle_priority() {
    // Different passes, same-pass bases, and a low-five-bit layer alias.
    for (earlier_layer, later_layer) in [(7, 0), (3, 1), (39, 5)] {
        let mut earlier = quad([1.0, 0.0, 0.0, 1.0], false);
        earlier.draw_layer = earlier_layer;
        earlier.draw_priority = 7;
        let mut later = quad([0.0, 1.0, 0.0, 1.0], false);
        later.draw_layer = later_layer;
        later.draw_priority = -7;
        for mixed in [false, true] {
            assert_color(
                render_center(
                    &format!("vfx-layer-order-{earlier_layer}-{later_layer}-{mixed}"),
                    if mixed {
                        vec![later]
                    } else {
                        vec![later, earlier]
                    },
                    if mixed { vec![mesh(earlier)] } else { vec![] },
                ),
                later.color,
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn priority_orders_quads_and_meshes_across_blend_modes() {
    let mut red = quad([1.0, 0.0, 0.0, 1.0], true);
    red.draw_priority = -1;
    let green = quad([0.0, 1.0, 0.0, 1.0], false);
    for mesh_mode in [false, true] {
        assert_color(
            render_center(
                &format!("vfx-order-blend-{mesh_mode}"),
                if mesh_mode { vec![] } else { vec![green, red] },
                if mesh_mode {
                    vec![mesh(green), mesh(red)]
                } else {
                    vec![]
                },
            ),
            green.color,
        );
    }
    assert_color(
        render_center("vfx-order-cross-type", vec![green], vec![mesh(red)]),
        green.color,
    );
    red.draw_priority = 1;
    assert_color(
        render_center(
            "vfx-order-cross-type-reversed",
            vec![green],
            vec![mesh(red)],
        ),
        [1.0, 1.0, 0.0, 1.0],
    );
    let mut first = quad([1.0, 0.0, 0.0, 1.0], false);
    first.draw_priority = -1;
    let mut last = quad([0.0, 0.0, 1.0, 1.0], false);
    last.draw_priority = 1;
    assert_color(
        render_center(
            "vfx-order-range-boundary",
            vec![first, last],
            vec![mesh(green)],
        ),
        last.color,
    );
}

#[test]
#[ignore = "requires native wgpu"]
fn empty_draw_model_is_skipped_without_shifting_model_indexes() {
    let mut particle = mesh(quad([1.0; 4], false));
    particle.model_index = 0;
    assert_color(
        render_center("vfx-empty-model", vec![], vec![particle]),
        render_center("vfx-empty-model-reference", vec![], vec![]),
    );
}

#[test]
#[ignore = "requires native wgpu"]
fn camera_billboard_applies_local_rotation() {
    let mut particle = quad([0.2, 0.8, 0.4, 1.0], false);
    particle.rotation_direction_base =
        xiv_companion_data::avfx::rotation_direction_base::CAMERA_BILLBOARD;
    particle.position[0] = 0.55;
    let (s, c) = (std::f32::consts::FRAC_PI_4 * 0.5).sin_cos();
    particle.orientation = [s, 0.0, 0.0, c];
    let mut reference = particle;
    reference.rotation_direction_base = xiv_companion_data::avfx::rotation_direction_base::NONE;
    let (sy, cy) = ((-particle.position[0]).atan2(3.0) * 0.5).sin_cos();
    reference.orientation = [cy * s, sy * c, -sy * s, cy * c];
    for mesh_mode in [false, true] {
        let draw = |name: &str, p| {
            render(
                name,
                if mesh_mode { vec![] } else { vec![p] },
                if mesh_mode { vec![mesh(p)] } else { vec![] },
                vec![solid([255; 4])],
            )
        };
        let actual = draw(&format!("vfx-billboard-local-{mesh_mode}"), particle);
        let expected = draw(
            &format!("vfx-billboard-local-reference-{mesh_mode}"),
            reference,
        );
        assert_image(&actual, &expected);
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn billboard_modes_have_distinct_screen_camera_and_vertical_bases() {
    use xiv_companion_data::avfx::rotation_direction_base as facing;
    for mesh_mode in [false, true] {
        let mut images = Vec::new();
        for mode in [
            facing::SCREEN_BILLBOARD,
            facing::CAMERA_BILLBOARD,
            facing::TREE_BILLBOARD,
        ] {
            let mut particle = quad([1.0; 4], false);
            particle.position = [0.55, 0.4, 0.0];
            particle.size = [0.45, 0.65];
            particle.rotation_direction_base = mode;
            let mut reference = particle;
            reference.rotation_direction_base = facing::NONE;
            // Independent yaw/pitch construction for the camera at (0,0,3).
            let yaw = if mode == facing::SCREEN_BILLBOARD {
                std::f32::consts::PI
            } else if mode == facing::CAMERA_BILLBOARD {
                (-particle.position[0]).atan2(3.0)
            } else {
                (-particle.position[0]).atan2(3.0)
            };
            let pitch = if mode == facing::CAMERA_BILLBOARD {
                particle.position[1].atan2(particle.position[0].hypot(3.0))
            } else {
                0.0
            };
            let (sy, cy) = (yaw * 0.5).sin_cos();
            let (sx, cx) = (pitch * 0.5).sin_cos();
            reference.orientation = [cy * sx, sy * cx, -sy * sx, cy * cx];
            let draw = |name: &str, p| {
                render(
                    name,
                    if mesh_mode { vec![] } else { vec![p] },
                    if mesh_mode { vec![mesh(p)] } else { vec![] },
                    gradient_textures(),
                )
            };
            let actual = draw(&format!("vfx-facing-{mesh_mode}-{mode}"), particle);
            let expected = draw(
                &format!("vfx-facing-reference-{mesh_mode}-{mode}"),
                reference,
            );
            assert_image(&actual, &expected);
            images.push(actual);
        }
        for first in 0..images.len() {
            for second in first + 1..images.len() {
                let changed = images[first]
                    .iter()
                    .zip(&images[second])
                    .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| (a - b).abs() > 0.01))
                    .count();
                assert!(
                    changed > 10,
                    "modes must produce different images: {first}, {second}"
                );
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn screen_and_camera_billboards_match_pitched_camera_bases_for_quad_and_model() {
    use xiv_companion_data::avfx::rotation_direction_base as facing;
    let camera = [0.3_f32, 0.4_f32];
    let view = [
        camera[0].sin() * camera[1].cos(),
        camera[1].sin(),
        camera[0].cos() * camera[1].cos(),
    ];
    let mut particle = quad([1.0; 4], false);
    particle.position = [0.35, 0.15, 0.0];
    particle.size = [0.45, 0.65];
    let normalize = |v: [f32; 3]| {
        let length = v.iter().map(|value| value * value).sum::<f32>().sqrt();
        v.map(|value| value / length)
    };
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let camera_right = normalize(cross([0.0, 1.0, 0.0], view));
    let camera_up = normalize(cross(view, camera_right));
    for mode in [facing::SCREEN_BILLBOARD, facing::CAMERA_BILLBOARD] {
        particle.rotation_direction_base = mode;
        let mut reference = particle;
        reference.rotation_direction_base = facing::NONE;
        reference.parent_basis = if mode == facing::SCREEN_BILLBOARD {
            [
                camera_right.map(|value| -value),
                camera_up,
                view.map(|value| -value),
            ]
        } else {
            let forward = normalize(std::array::from_fn(|axis| {
                view[axis] * 3.0 - particle.position[axis]
            }));
            let right = normalize(cross(camera_up, forward));
            [right, cross(forward, right), forward]
        };

        for msaa_samples in [1, 4] {
            for mesh_mode in [false, true] {
                let draw = |name: &str, particle| {
                    render_with_geometry_and_camera(
                        name,
                        if mesh_mode { vec![] } else { vec![particle] },
                        if mesh_mode {
                            vec![mesh(particle)]
                        } else {
                            vec![]
                        },
                        gradient_textures(),
                        square(),
                        msaa_samples,
                        camera,
                    )
                };
                let actual = draw(
                    &format!("vfx-billboard-pitched-{mode}-{msaa_samples}-{mesh_mode}"),
                    particle,
                );
                let expected = draw(
                    &format!("vfx-billboard-pitched-reference-{mode}-{msaa_samples}-{mesh_mode}"),
                    reference,
                );
                assert_image(&actual, &expected);
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn screen_and_camera_billboards_follow_rolled_view_axes_for_quad_and_model() {
    use xiv_companion_data::avfx::rotation_direction_base as facing;

    for mode in [facing::SCREEN_BILLBOARD, facing::CAMERA_BILLBOARD] {
        let mut particle = quad([1.0; 4], false);
        particle.rotation_direction_base = mode;
        particle.size = [0.45, 0.65];
        let mut reference = particle;
        reference.rotation_direction_base = facing::NONE;
        reference.parent_basis = if mode == facing::SCREEN_BILLBOARD {
            [[0.0, -1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, -1.0]]
        } else {
            [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]
        };
        for msaa_samples in [1, 4] {
            for mesh_mode in [false, true] {
                let draw = |name: &str, particle| {
                    render_with_geometry_camera_frames_providers_and_roll(
                        name,
                        if mesh_mode { vec![] } else { vec![particle] },
                        if mesh_mode {
                            vec![mesh(particle)]
                        } else {
                            vec![]
                        },
                        gradient_textures(),
                        square(),
                        msaa_samples,
                        [0.0; 2],
                        1,
                        None,
                        None,
                        std::f32::consts::FRAC_PI_2,
                        None,
                    )
                };
                let actual = draw(
                    &format!("vfx-billboard-roll-{mode}-{msaa_samples}-{mesh_mode}"),
                    particle,
                );
                let expected = draw(
                    &format!("vfx-billboard-roll-reference-{mode}-{msaa_samples}-{mesh_mode}"),
                    reference,
                );
                assert_image(&actual, &expected);
                assert!(actual.iter().any(|pixel| pixel[0] > 0.1 || pixel[1] > 0.1));
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn tree_billboard_matches_old_y_axis_and_column_scales_for_quad_and_model() {
    use xiv_companion_data::avfx::rotation_direction_base as facing;
    let camera = [0.3_f32, 0.4_f32];
    let view = [
        camera[0].sin() * camera[1].cos(),
        camera[1].sin(),
        camera[0].cos() * camera[1].cos(),
    ];
    let normalize = |v: [f32; 3]| {
        let length = v.iter().map(|value| value * value).sum::<f32>().sqrt();
        v.map(|value| value / length)
    };
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let mut particle = quad([1.0; 4], false);
    particle.position = [0.35, 0.15, 0.0];
    particle.size = [0.45, 0.65];
    particle.parent_basis = [[0.0, 1.5, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, -1.3]];
    particle.rotation_direction_base = facing::TREE_BILLBOARD;
    let old_y = particle.parent_basis[1];
    let ray: [f32; 3] = std::array::from_fn(|axis| particle.position[axis] - view[axis] * 3.0);
    let right = normalize(cross(ray, old_y));
    let up = normalize(old_y);
    let forward = normalize(cross(right, old_y));
    let mut reference = particle;
    reference.rotation_direction_base = facing::NONE;
    reference.parent_basis = [
        right.map(|value| value * 1.5),
        up.map(|value| value * 0.5),
        forward.map(|value| value * 1.3),
    ];
    for msaa_samples in [1, 4] {
        for mesh_mode in [false, true] {
            let draw = |name: &str, particle| {
                let mut model = mesh(particle);
                model.fresnel = Some(xiv_companion::VfxFresnel {
                    kind: 3,
                    direction: [0.3, -0.4, 0.7],
                    exponent: 2.0,
                    color_begin: [0.2, 0.4, 0.8, 1.0],
                    color_end: [1.0, 0.7, 0.2, 1.0],
                });
                render_with_geometry_and_camera(
                    name,
                    if mesh_mode { vec![] } else { vec![particle] },
                    if mesh_mode { vec![model] } else { vec![] },
                    gradient_textures(),
                    square(),
                    msaa_samples,
                    camera,
                )
            };
            let actual = draw(
                &format!("vfx-tree-billboard-{msaa_samples}-{mesh_mode}"),
                particle,
            );
            let expected = draw(
                &format!("vfx-tree-billboard-reference-{msaa_samples}-{mesh_mode}"),
                reference,
            );
            assert_image(&actual, &expected);
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn tree_billboard_disc_matches_replaced_matrix_at_1x_and_4x() {
    use xiv_companion_data::avfx::{ParticleType, rotation_direction_base as facing};
    use xiv_companion_data::avfx_sim::VfxDisc;
    let camera = [0.3_f32, 0.4_f32];
    let view = [
        camera[0].sin() * camera[1].cos(),
        camera[1].sin(),
        camera[0].cos() * camera[1].cos(),
    ];
    let normalize = |v: [f32; 3]| {
        let length = v.iter().map(|value| value * value).sum::<f32>().sqrt();
        v.map(|value| value / length)
    };
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let mut particle = quad([1.0; 4], false);
    particle.particle_type = Some(ParticleType::Disc);
    particle.position = [0.35, 0.15, 0.0];
    particle.size = [0.45, 0.65];
    particle.parent_basis = [[0.0, 1.5, 0.0], [-0.5, 0.0, 0.0], [0.0, 0.0, -1.3]];
    particle.rotation_direction_base = facing::TREE_BILLBOARD;
    particle.disc = Some(VfxDisc {
        counts: [2, 3, 3],
        angle: 0.0,
        radius: [0.5, 0.7],
        width: [0.1, 0.15],
        height_inner: [0.0, 0.1],
        height_outer: [-0.1, 0.2],
        color_inner: [1.0; 4],
        color_outer: [1.0; 4],
        point_interval_factor: 0.0,
        scaling_scale: 1.0,
        scale_z: 0.8,
    });
    let old_y = particle.parent_basis[1];
    let ray: [f32; 3] = std::array::from_fn(|axis| particle.position[axis] - view[axis] * 3.0);
    let right = normalize(cross(ray, old_y));
    let up = normalize(old_y);
    let forward = normalize(cross(right, old_y));
    let mut reference = particle;
    reference.rotation_direction_base = facing::NONE;
    reference.parent_basis = [
        right.map(|value| value * 1.5),
        up.map(|value| value * 0.5),
        forward.map(|value| value * 1.3),
    ];
    for msaa_samples in [1, 4] {
        let draw = |name: &str, particle| {
            render_with_geometry_and_camera(
                name,
                vec![particle],
                vec![],
                gradient_textures(),
                square(),
                msaa_samples,
                camera,
            )
        };
        let actual = draw(&format!("vfx-tree-disc-{msaa_samples}"), particle);
        let expected = draw(
            &format!("vfx-tree-disc-reference-{msaa_samples}"),
            reference,
        );
        assert_image(&actual, &expected);
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn axis_y_billboards_match_screen_and_per_particle_yaw_geometry() {
    use xiv_companion_data::avfx::rotation_direction_base as facing;
    for sample_count in [1, 4] {
        for mesh_mode in [false, true] {
            for mode in [facing::BILLBOARD_AXIS_Y, facing::CAMERA_BILLBOARD_AXIS_Y] {
                let mut particle = quad([1.0; 4], false);
                particle.position = [0.55, 0.4, 0.0];
                particle.size = [0.45, 0.65];
                particle.rotation_direction_base = mode;
                let mut reference = particle;
                reference.rotation_direction_base = facing::NONE;
                let yaw = if mode == facing::BILLBOARD_AXIS_Y {
                    std::f32::consts::PI
                } else {
                    (-particle.position[0]).atan2(3.0)
                };
                let (sy, cy) = (yaw * 0.5).sin_cos();
                reference.orientation = [0.0, sy, 0.0, cy];
                let draw = |name: &str, p| {
                    render_with_geometry_and_samples(
                        name,
                        if mesh_mode { vec![] } else { vec![p] },
                        if mesh_mode { vec![mesh(p)] } else { vec![] },
                        gradient_textures(),
                        square(),
                        sample_count,
                    )
                };
                let actual = draw(
                    &format!("vfx-facing-axis-y-{sample_count}-{mesh_mode}-{mode}"),
                    particle,
                );
                let expected = draw(
                    &format!("vfx-facing-axis-y-reference-{sample_count}-{mesh_mode}-{mode}"),
                    reference,
                );
                assert_image(&actual, &expected);
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn movement_direction_modes_match_explicit_bases_and_fallbacks() {
    use xiv_companion_data::avfx::rotation_direction_base as facing;
    let cases = [
        (
            facing::MOVE_DIRECTION,
            [1.0, 0.0, 0.0],
            xiv_companion::VFX_IDENTITY_BASIS,
            [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
        ),
        (
            facing::MOVE_DIRECTION,
            [0.0, 1.0, 0.0],
            xiv_companion::VFX_IDENTITY_BASIS,
            [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
        ),
        (
            facing::MOVE_DIRECTION_BILLBOARD,
            [1.0, 0.0, 0.0],
            xiv_companion::VFX_IDENTITY_BASIS,
            [[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]],
        ),
        (
            facing::MOVE_DIRECTION_BILLBOARD,
            [0.0, 0.0, 1.0],
            xiv_companion::VFX_IDENTITY_BASIS,
            xiv_companion::VFX_IDENTITY_BASIS,
        ),
    ];
    for sample_count in [1, 4] {
        for mesh_mode in [false, true] {
            for (case, (mode, direction, parent, expected_basis)) in cases.iter().enumerate() {
                let mut particle = quad([1.0; 4], false);
                particle.position = [0.0; 3];
                particle.size = [0.45, 0.65];
                particle.rotation_direction_base = *mode;
                particle.movement_direction = *direction;
                particle.facing_parent_basis = *parent;
                particle.orientation = if *mode == facing::MOVE_DIRECTION {
                    [0.0, 0.0, 0.25881904, 0.9659258]
                } else {
                    [0.25881904, 0.0, 0.0, 0.9659258]
                };
                let mut reference = particle;
                reference.rotation_direction_base = facing::NONE;
                reference.parent_basis = *expected_basis;
                let draw = |name: &str, p| {
                    render_with_geometry_and_samples(
                        name,
                        if mesh_mode { vec![] } else { vec![p] },
                        if mesh_mode { vec![mesh(p)] } else { vec![] },
                        gradient_textures(),
                        square(),
                        sample_count,
                    )
                };
                let actual = draw(
                    &format!("vfx-facing-movement-{sample_count}-{mesh_mode}-{case}"),
                    particle,
                );
                let expected = draw(
                    &format!("vfx-facing-movement-reference-{sample_count}-{mesh_mode}-{case}"),
                    reference,
                );
                assert_image(&actual, &expected);
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn ordinary_fixed_facing_modes_keep_the_preexisting_particle_matrix() {
    use xiv_companion_data::avfx::rotation_direction_base as facing;
    for sample_count in [1, 4] {
        for mesh_mode in [false, true] {
            let mut reference = quad([1.0; 4], false);
            reference.position = [0.25, -0.2, 0.0];
            reference.size = [0.45, 0.65];
            reference.orientation = [0.0, 0.0, 0.25881904, 0.9659258];
            reference.rotation_direction_base = facing::NONE;
            let draw = |name: &str, p| {
                render_with_geometry_and_samples(
                    name,
                    if mesh_mode { vec![] } else { vec![p] },
                    if mesh_mode { vec![mesh(p)] } else { vec![] },
                    gradient_textures(),
                    square(),
                    sample_count,
                )
            };
            let expected = draw(
                &format!("vfx-facing-fixed-reference-{sample_count}-{mesh_mode}"),
                reference,
            );
            for mode in [facing::X, facing::Y, facing::Z, facing::NONE] {
                let mut particle = reference;
                particle.rotation_direction_base = mode;
                let actual = draw(
                    &format!("vfx-facing-fixed-{sample_count}-{mesh_mode}-{mode}"),
                    particle,
                );
                assert_image(&actual, &expected);
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn billboard_nonuniform_scale_and_pivot_match_precomputed_geometry() {
    use xiv_companion_data::avfx::rotation_direction_base as facing;
    for sign in [1.0, -1.0] {
        let mut particle = quad([1.0; 4], false);
        particle.rotation_direction_base = facing::CAMERA_BILLBOARD;
        particle.size = [sign * 0.8, 0.3];
        particle.pivot = [0.5, -0.25];
        particle.orientation = [
            0.0,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            std::f32::consts::FRAC_1_SQRT_2,
        ];
        let mut geometry = reference_quad_geometry();
        let px = particle.pivot[0] * particle.size[0];
        let py = particle.pivot[1] * particle.size[1];
        for vertex in &mut geometry.vertices {
            let [x, y, _] = vertex.position;
            // S, then Rz(90) around the scaled pivot; the +Z camera basis is identity.
            vertex.position = [
                -y * particle.size[1] + px + py,
                x * particle.size[0] - px + py,
                0.0,
            ];
        }
        let actual = render(
            &format!("vfx-pivot-{sign}"),
            vec![particle],
            vec![],
            gradient_textures(),
        );
        let expected = render_with_geometry(
            &format!("vfx-pivot-reference-{sign}"),
            vec![],
            vec![mesh(quad([1.0; 4], false))],
            gradient_textures(),
            geometry,
        );
        assert_image(&actual, &expected);
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn affine_parent_transform_and_pivot_match_precomputed_quads() {
    for sign in [1.0, -1.0] {
        let mut particle = quad([1.0; 4], false);
        particle.position = [0.12, -0.17, 0.1];
        particle.size = [sign * 0.5, 0.3];
        particle.pivot = [0.2, -0.3];
        particle.orientation = [
            0.0,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            std::f32::consts::FRAC_1_SQRT_2,
        ];
        particle.parent_basis = [[1.0, 0.0, 0.2], [0.5, 1.5, 0.0], [0.25, 0.3, 1.0]];
        let mut geometry = reference_quad_geometry();
        let px = particle.pivot[0] * particle.size[0];
        let py = particle.pivot[1] * particle.size[1];
        for vertex in &mut geometry.vertices {
            let [x, y, _] = vertex.position;
            let rx = -y * particle.size[1] + px + py;
            let ry = x * particle.size[0] - px + py;
            vertex.position = [rx + 0.5 * ry, 1.5 * ry, 0.2 * rx];
        }
        let mut reference = mesh(quad([1.0; 4], false));
        reference.position = particle.position;
        let actual = render(
            &format!("vfx-affine-quad-{sign}"),
            vec![particle],
            vec![],
            gradient_textures(),
        );
        let expected = render_with_geometry(
            &format!("vfx-affine-quad-reference-{sign}"),
            vec![],
            vec![reference],
            gradient_textures(),
            geometry,
        );
        assert_image(&actual, &expected);
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn affine_mesh_transform_and_packed_uv_sets_match_precomputed_geometry() {
    let mut source = square();
    for (index, vertex) in source.vertices.iter_mut().enumerate() {
        vertex.position[2] = [0.1, -0.2, 0.15, 0.3][index];
        let [u, v] = vertex.uvs[0];
        vertex.uvs = [[u, v], [-u, v], [v, -u], [u * 0.5 - 0.25, v * 0.5]];
    }
    for uv_set in 0..4 {
        let mut particle = mesh(quad([1.0; 4], false));
        particle.scale = [-0.5, 0.3, 2.0];
        particle.orientation = [
            std::f32::consts::FRAC_1_SQRT_2,
            0.0,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
        ];
        particle.parent_basis = [[1.0, 0.0, 0.2], [0.5, 1.5, 0.0], [0.25, 0.3, 1.0]];
        particle.texture_uv_sets[0] = uv_set;
        let mut expected_geometry = source.clone();
        for vertex in &mut expected_geometry.vertices {
            let [x, y, z] = vertex.position;
            // S(-.5,.3,2), Rx(90), then the parent shear.
            vertex.position = [
                -0.5 * x - z + 0.075 * y,
                -3.0 * z + 0.09 * y,
                -0.1 * x + 0.3 * y,
            ];
            vertex.uvs[0] = vertex.uvs[uv_set as usize];
        }
        let actual = render_with_geometry(
            &format!("vfx-affine-mesh-uv-{uv_set}"),
            vec![],
            vec![particle],
            gradient_textures(),
            source.clone(),
        );
        let expected = render_with_geometry(
            &format!("vfx-affine-mesh-reference-{uv_set}"),
            vec![],
            vec![mesh(quad([1.0; 4], false))],
            gradient_textures(),
            expected_geometry,
        );
        assert_image(&actual, &expected);
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn camera_billboard_degenerate_view_directions_have_finite_visible_fallbacks() {
    use xiv_companion_data::avfx::rotation_direction_base as facing;
    let background = render_center("vfx-facing-degenerate-background", vec![], vec![]);
    for (mode, y) in [
        (facing::CAMERA_BILLBOARD, 0.0),
        (facing::CAMERA_BILLBOARD, 0.001),
    ] {
        let mut particle = quad([0.2, 0.8, 0.4, 1.0], false);
        particle.position = [0.0, y, 3.0];
        particle.size = [0.15; 2];
        particle.rotation_direction_base = mode;
        let mut geometry = square();
        let at_pole = y > 0.01;
        for vertex in &mut geometry.vertices {
            vertex.position[2] = if at_pole { 1.0 } else { -1.0 };
        }
        let actual = render_with_geometry(
            &format!("vfx-facing-degenerate-{mode}-{y}"),
            vec![],
            vec![mesh(particle)],
            vec![solid([255; 4])],
            geometry,
        );
        let mut reference = particle;
        reference.position[2] = 2.0;
        reference.rotation_direction_base = facing::NONE;
        if at_pole {
            reference.orientation = [0.0, 1.0, 0.0, 0.0];
        }
        let expected = render(
            &format!("vfx-facing-degenerate-reference-{mode}-{y}"),
            vec![],
            vec![mesh(reference)],
            vec![solid([255; 4])],
        );
        assert_image(&actual, &expected);
        assert!(
            actual
                .iter()
                .filter(|pixel| (pixel[1] - background[1]).abs() > 0.1)
                .count()
                > 20
        );
    }
    let mut particle = quad([0.2, 0.8, 0.4, 1.0], false);
    particle.position = [0.0, 0.2, 3.0];
    particle.size = [0.3, 2.0];
    particle.rotation_direction_base = facing::CAMERA_BILLBOARD;
    let mut reference = particle;
    reference.rotation_direction_base = facing::NONE;
    let mut geometry = square();
    for vertex in &mut geometry.vertices {
        vertex.position[2] = -1.0;
    }
    let actual = render_with_geometry(
        "vfx-facing-vertical",
        vec![],
        vec![mesh(particle)],
        vec![solid([255; 4])],
        geometry.clone(),
    );
    let expected = render_with_geometry(
        "vfx-facing-vertical-reference",
        vec![],
        vec![mesh(reference)],
        vec![solid([255; 4])],
        geometry,
    );
    assert_image(&actual, &expected);
    assert!(
        actual
            .iter()
            .filter(|pixel| (pixel[1] - background[1]).abs() > 0.1)
            .count()
            > 20
    );
}

#[test]
#[ignore = "requires native wgpu"]
fn tree_billboard_parallel_camera_ray_does_not_emit_nonfinite_geometry() {
    use xiv_companion_data::avfx::rotation_direction_base as facing;
    let background = render_center("vfx-tree-parallel-background", vec![], vec![]);
    let mut particle = quad([0.2, 0.8, 0.4, 1.0], false);
    particle.rotation_direction_base = facing::TREE_BILLBOARD;
    particle.parent_basis = [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]];
    assert_color(
        render_center("vfx-tree-parallel-quad", vec![particle], vec![]),
        background,
    );
    assert_color(
        render_center("vfx-tree-parallel-model", vec![], vec![mesh(particle)]),
        background,
    );
}

fn gradient_textures() -> Vec<Option<VfxTextureInput>> {
    let mut rgba = Vec::new();
    for y in 0..16u8 {
        for x in 0..16u8 {
            rgba.extend_from_slice(&[x * 17, y * 17, 0, 255]);
        }
    }
    vec![
        Some(VfxTextureInput {
            rgba,
            width: 16,
            height: 16,
            authored_mips: None,
            rgba16f_mips: None,
            cube_mips: None,
            cube_format: VfxTextureCubeFormat::Rgba8Unorm,
        }),
        solid([255, 0, 0, 255]),
    ]
}

#[test]
#[ignore = "requires native wgpu"]
fn model_centered_uvs_match_quad_sampling_for_color_and_distortion() {
    for samples in [1, 4] {
        for transformed in [false, true] {
            for (layer, uv_set) in [
                (0, 0),
                (1, 1),
                (2, 2),
                (3, 3),
                (4, 0),
                (4, 1),
                (4, 2),
                (4, 3),
            ] {
                let mut particle = quad([1.0; 4], false);
                particle.size = [0.8, 0.65];
                particle.texture_indexes = [-1; 4];
                particle.texture1_enabled = layer == 0 || layer == 4;
                particle.texture_borders = [[1, 1]; 4];
                if layer < 4 {
                    particle.texture_indexes[layer] = 0;
                    particle.texture_uv_sets[layer] = uv_set as i32;
                    if transformed {
                        particle.uv_origins[layer] = [0.08, -0.12];
                        particle.uv_scales[layer] = [-0.7, 0.45];
                        particle.uv_rotations[layer] = 0.8;
                    }
                } else {
                    particle.texture_indexes[0] = 0;
                    particle.uv_scales[0] = [0.0; 2];
                    particle.texture_distortion_index = 0;
                    particle.distortion_targets = 1;
                    particle.distortion_power = 0.65;
                    particle.distortion_uv_set = uv_set as i32;
                    particle.distortion_borders = [1, 1];
                    if transformed {
                        particle.uvd_origin = [0.08, -0.12];
                        particle.uvd_scale = [-0.7, 0.45];
                        particle.uvd_rotation = 0.8;
                    }
                }
                let mut geometry = reference_quad_geometry();
                // Raw VDrw coordinates. The independent Quad path spans [0,1].
                for (vertex, uv) in geometry.vertices.iter_mut().zip([
                    [0.5, -0.5],
                    [0.5, 0.5],
                    [-0.5, -0.5],
                    [-0.5, 0.5],
                ]) {
                    vertex.uvs = [[2.0, 3.0]; 4];
                    vertex.uvs[uv_set] = uv;
                }
                let actual = render_with_geometry_and_samples(
                    &format!("vfx-model-centered-uv-{samples}-{transformed}-{layer}-{uv_set}"),
                    vec![],
                    vec![mesh(particle)],
                    gradient_textures(),
                    geometry,
                    samples,
                );
                let expected = render_with_geometry_and_samples(
                    &format!(
                        "vfx-model-centered-uv-reference-{samples}-{transformed}-{layer}-{uv_set}"
                    ),
                    vec![particle],
                    vec![],
                    gradient_textures(),
                    square(),
                    samples,
                );
                assert_image(&actual, &expected);
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn quad_uses_reference_uvs_for_color_and_distortion_layers() {
    use xiv_companion_data::avfx::rotation_direction_base as facing;
    for transformed in [false, true] {
        for layer in 0..5 {
            let mut particle = quad([1.0; 4], false);
            particle.size = [0.8, 0.65];
            particle.texture_indexes = [-1; 4];
            particle.texture1_enabled = layer == 0 || layer == 4;
            particle.texture_uv_sets = [3, 2, 1, 0];
            particle.texture_borders = [[1, 1]; 4];
            if layer < 4 {
                particle.texture_indexes[layer] = 0;
            } else {
                // Hold TC1's base UV constant so only TD supplies spatial variation.
                particle.texture_indexes[0] = 0;
                particle.uv_scales[0] = [0.0; 2];
                particle.texture_distortion_index = 0;
                particle.distortion_targets = 1;
                particle.distortion_power = 0.65;
                particle.distortion_uv_set = 2;
                particle.distortion_borders = [1, 1];
            }
            if transformed {
                particle.rotation_direction_base = facing::CAMERA_BILLBOARD;
                particle.size[0] *= -1.0;
                if layer < 4 {
                    particle.uv_origins[layer] = [0.08, -0.12];
                    particle.uv_scales[layer] = [-0.7, 0.45];
                    particle.uv_rotations[layer] = 0.8;
                } else {
                    particle.uvd_origin = [0.08, -0.12];
                    particle.uvd_scale = [-0.7, 0.45];
                    particle.uvd_rotation = 0.8;
                }
            }
            let actual = render(
                &format!("vfx-quad-uv-{transformed}-{layer}"),
                vec![particle],
                vec![],
                gradient_textures(),
            );
            let expected = render_with_geometry(
                &format!("vfx-quad-uv-reference-{transformed}-{layer}"),
                vec![],
                vec![mesh(particle)],
                gradient_textures(),
                reference_quad_geometry(),
            );
            assert_image(&actual, &expected);
            if !transformed {
                assert!(actual[32 * 64 + 24][0] > actual[32 * 64 + 40][0] + 0.1);
                // Local +Y (V=1) projects toward the top of the image.
                assert!(actual[24 * 64 + 32][1] > actual[40 * 64 + 32][1] + 0.1);
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn pixel_position_uv_uses_fragment_coordinates_for_color_and_distortion_layers() {
    let draw = |name: &str, particle: VfxQuad, mesh_mode: bool| {
        render(
            name,
            if mesh_mode { vec![] } else { vec![particle] },
            if mesh_mode {
                vec![mesh(particle)]
            } else {
                vec![]
            },
            gradient_textures(),
        )
    };
    let transformed_pixel_uv =
        |position: [f32; 3], origin: [f32; 2], scale: [f32; 2], rotation: f32| {
            let quantized_rotation =
                (rotation.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * 65535.0)
                    .round()
                    / 65535.0
                    * std::f32::consts::TAU;
            let focal = 1.0 / 22.5_f32.to_radians().tan();
            let center = [
                0.5 + 0.5 * focal * position[0] / (3.0 - position[2]),
                0.5 - 0.5 * focal * position[1] / (3.0 - position[2]),
            ];
            let [x, y] = [32.5 / 64.0 - center[0], 32.5 / 64.0 - center[1]];
            let (sin, cos) = quantized_rotation.sin_cos();
            [
                center[0] + origin[0] + scale[0] * (cos * x + sin * y),
                center[1] + origin[1] + scale[1] * (cos * y - sin * x),
            ]
        };

    for mesh_mode in [false, true] {
        for layer in 0..4 {
            let mut expected = quad([1.0; 4], false);
            expected.position = [0.18, -0.12, 0.0];
            expected.size = [0.85, 0.7];
            expected.texture_indexes = [-1; 4];
            expected.texture_indexes[layer] = 0;
            expected.texture1_enabled = layer == 0;
            expected.texture_borders[layer] = [1, 1];
            expected.uv_by_pixel_position[layer] = true;

            let mut actual = expected;
            actual.uv_origins[layer] = [0.31, -0.27];
            actual.uv_scales[layer] = [-0.45, 1.8];
            actual.uv_rotations[layer] = 1.1;
            let mut parameter = actual;
            parameter.uv_by_pixel_position[layer] = false;

            let expected_pixels = draw(
                &format!("vfx-pixel-uv-reference-{mesh_mode}-{layer}"),
                expected,
                mesh_mode,
            );
            let actual_pixels = draw(
                &format!("vfx-pixel-uv-{mesh_mode}-{layer}"),
                actual,
                mesh_mode,
            );
            if mesh_mode {
                let uv = transformed_pixel_uv(
                    actual.position,
                    actual.uv_origins[layer],
                    actual.uv_scales[layer],
                    actual.uv_rotations[layer],
                );
                let mut formula_reference = actual;
                formula_reference.uv_by_pixel_position[layer] = false;
                formula_reference.uv_origins[layer] = [uv[0] - 0.5, uv[1] - 0.5];
                formula_reference.uv_scales[layer] = [0.0; 2];
                formula_reference.uv_rotations[layer] = 0.0;
                let formula_pixels = draw(
                    &format!("vfx-pixel-uv-model-formula-{layer}"),
                    formula_reference,
                    true,
                );
                assert_color(actual_pixels[32 * 64 + 32], formula_pixels[32 * 64 + 32]);
                if layer == 0 {
                    // Model ByPixelPosition applies the UvSet affine transform
                    // around the projected instance origin, unlike Shape.
                    assert!(
                        actual_pixels
                            .iter()
                            .zip(&expected_pixels)
                            .filter(|(a, b)| { (a[0] - b[0]).abs() + (a[1] - b[1]).abs() > 0.05 })
                            .count()
                            > 20,
                        "model pixel UV must consume UvSet transform"
                    );
                }
            } else {
                assert_image(&actual_pixels, &expected_pixels);
            }
            let parameter_pixels = draw(
                &format!("vfx-parameter-uv-control-{mesh_mode}-{layer}"),
                parameter,
                mesh_mode,
            );
            assert!(
                actual_pixels
                    .iter()
                    .zip(&parameter_pixels)
                    .filter(|(a, b)| (a[0] - b[0]).abs() + (a[1] - b[1]).abs() > 0.05)
                    .count()
                    > 20,
                "mesh={mesh_mode}, layer={layer}"
            );
        }

        let mut expected = quad([1.0; 4], false);
        expected.position = [0.18, -0.12, 0.0];
        expected.size = [0.85, 0.7];
        expected.uv_scales[0] = [0.0; 2];
        expected.texture_distortion_index = 0;
        expected.distortion_targets = 1;
        expected.distortion_power = 0.75;
        expected.distortion_borders = [1, 1];
        expected.uvd_by_pixel_position = true;
        let mut actual = expected;
        actual.uvd_origin = [0.31, -0.27];
        actual.uvd_scale = [-0.45, 1.8];
        actual.uvd_rotation = 1.1;
        let actual_pixels = draw(&format!("vfx-pixel-uv-td-{mesh_mode}"), actual, mesh_mode);
        let expected_pixels = draw(
            &format!("vfx-pixel-uv-td-reference-{mesh_mode}"),
            expected,
            mesh_mode,
        );
        if mesh_mode {
            let uv = transformed_pixel_uv(
                actual.position,
                actual.uvd_origin,
                actual.uvd_scale,
                actual.uvd_rotation,
            );
            let mut formula_reference = actual;
            formula_reference.uvd_by_pixel_position = false;
            formula_reference.uvd_origin = [uv[0] - 0.5, uv[1] - 0.5];
            formula_reference.uvd_scale = [0.0; 2];
            formula_reference.uvd_rotation = 0.0;
            let formula_pixels = draw("vfx-pixel-uv-td-model-formula", formula_reference, true);
            assert_color(actual_pixels[32 * 64 + 32], formula_pixels[32 * 64 + 32]);
            assert!(
                actual_pixels
                    .iter()
                    .zip(&expected_pixels)
                    .filter(|(a, b)| (a[0] - b[0]).abs() + (a[1] - b[1]).abs() > 0.05)
                    .count()
                    > 20,
                "model pixel TD must consume UvSet transform"
            );
        } else {
            assert_image(&actual_pixels, &expected_pixels);
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn distortion_targets_survive_gpu_interpolation() {
    let mut particle = quad([1.0; 4], false);
    particle.texture_borders[0] = [1, 1];
    particle.texture_distortion_index = 1;
    particle.distortion_power = 0.5;
    particle.distortion_targets = 1;
    let mut reference = particle;
    reference.texture_distortion_index = -1;
    reference.uv_origins[0] = [0.25, -0.25];
    for mesh_mode in [false, true] {
        let render_particle = |name: &str, p: VfxQuad| {
            render(
                name,
                if mesh_mode { vec![] } else { vec![p] },
                if mesh_mode { vec![mesh(p)] } else { vec![] },
                gradient_textures(),
            )[32 * 64 + 32]
        };
        assert_color(
            render_particle(&format!("vfx-distortion-{mesh_mode}"), particle),
            render_particle(&format!("vfx-distortion-reference-{mesh_mode}"), reference),
        );
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn distortion_texture_uses_its_uv_rotation() {
    let mut particle = quad([1.0; 4], false);
    particle.texture_distortion_index = 1;
    particle.distortion_power = 0.5;
    particle.distortion_targets = 1;
    particle.uvd_rotation = std::f32::consts::FRAC_PI_2;
    let mut reference = particle;
    reference.uvd_rotation = 0.0;
    let mut textures = gradient_textures();
    textures[1] = textures[0].clone();
    let mut reference_textures = textures.clone();
    let rotated = reference_textures[1].as_mut().unwrap();
    for y in 0..16 {
        for x in 0..16 {
            // MoveUV at pi/2 maps (u, v) to (v, 1-u).
            rotated.rgba[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4].copy_from_slice(&[
                (y * 17) as u8,
                ((15 - x) * 17) as u8,
                0,
                255,
            ]);
        }
    }
    for mesh_mode in [false, true] {
        let render_particle = |name: &str, p, textures| {
            render(
                name,
                if mesh_mode { vec![] } else { vec![p] },
                if mesh_mode { vec![mesh(p)] } else { vec![] },
                textures,
            )
        };
        let actual = render_particle(
            &format!("vfx-td-rotation-{mesh_mode}"),
            particle,
            textures.clone(),
        );
        let expected = render_particle(
            &format!("vfx-td-rotation-reference-{mesh_mode}"),
            reference,
            reference_textures.clone(),
        );
        for y in 26..38 {
            for x in 26..38 {
                assert_color(actual[y * 64 + x], expected[y * 64 + x]);
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn packed_rotation_of_unused_layer_does_not_change_visible_layer() {
    let mut reference = quad([1.0; 4], false);
    reference.uv_rotations[0] = 0.8;
    let mut particle = reference;
    // This pair encodes a NaN if the packed u32 is transported as Float32.
    particle.uv_rotations[1] = std::f32::consts::TAU * (0x7fc1 as f32 / 65535.0);
    for mesh_mode in [false, true] {
        let render_particle = |name: &str, p: VfxQuad| {
            render(
                name,
                if mesh_mode { vec![] } else { vec![p] },
                if mesh_mode { vec![mesh(p)] } else { vec![] },
                gradient_textures(),
            )
        };
        let actual = render_particle(&format!("vfx-rotation-packed-{mesh_mode}"), particle);
        let expected = render_particle(&format!("vfx-rotation-reference-{mesh_mode}"), reference);
        for y in 26..38 {
            for x in 26..38 {
                assert_color(actual[y * 64 + x], expected[y * 64 + x]);
            }
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn invalid_texture_index_uses_fallback_instead_of_last_texture() {
    let mut particle = quad([1.0; 4], false);
    particle.texture_indexes[0] = i32::MAX;
    let mut reference = particle;
    reference.texture_indexes[0] = -1;
    let render_particle =
        |name: &str, p| render(name, vec![p], vec![], vec![solid([0, 255, 0, 255])])[32 * 64 + 32];
    assert_color(
        render_particle("vfx-invalid-texture", particle),
        render_particle("vfx-invalid-texture-reference", reference),
    );
}

#[test]
#[ignore = "requires native wgpu"]
fn each_color_layer_uses_its_filter_without_reusing_incompatible_samplers() {
    let background = render_center("vfx-filter-background", vec![], vec![]);
    for mesh_mode in [false, true] {
        for layer in 0..4 {
            let mut nearest = quad([1.0, 0.0, 0.0, 1.0], true);
            nearest.texture_indexes[layer] = 1;
            // At u=0.375 a two-texel black/white texture is 0 with nearest
            // filtering and 0.25 with linear filtering.
            nearest.uv_origins[layer] = [-0.125, 0.0];
            nearest.uv_scales[layer] = [0.0; 2];
            nearest.texture_filters[layer] = 0;
            let mut linear = nearest;
            linear.color = [0.0, 1.0, 0.0, 1.0];
            linear.texture_filters[layer] = 1;
            let particles = vec![nearest, linear];
            let actual = render(
                &format!("vfx-filter-{mesh_mode}-{layer}"),
                if mesh_mode { vec![] } else { particles.clone() },
                if mesh_mode {
                    particles.into_iter().map(mesh).collect()
                } else {
                    vec![]
                },
                vec![solid([255; 4]), black_white_texture()],
            )[32 * 64 + 32];
            assert_color(
                actual,
                [
                    background[0],
                    background[1] + 0.25,
                    background[2],
                    background[3],
                ],
            );
        }
    }
}

#[test]
#[ignore = "requires native wgpu"]
fn advanced_texture_filters_create_all_layer_samplers_and_render() {
    let mut particle = quad([0.25, 0.5, 0.75, 1.0], false);
    particle.texture_filters = [2, 3, 4, 2];
    particle.distortion_filter = 3;
    particle.palette_filter = 4;
    let textures = vec![authored_solid_mips([255; 4]), solid([255; 4])];
    let actual = render(
        "vfx-advanced-filter",
        vec![particle],
        vec![],
        textures.clone(),
    )[32 * 64 + 32];

    let mut reference = particle;
    reference.texture_filters = [1; 4];
    reference.distortion_filter = 1;
    reference.palette_filter = 1;
    assert_color(
        actual,
        render(
            "vfx-advanced-filter-reference",
            vec![reference],
            vec![],
            textures,
        )[32 * 64 + 32],
    );
}

#[test]
#[ignore = "requires native wgpu"]
fn distortion_layer_uses_its_own_filter() {
    let background = render_center("vfx-td-filter-background", vec![], vec![]);
    for mesh_mode in [false, true] {
        let mut nearest = quad([1.0, 0.0, 0.0, 1.0], true);
        nearest.uv_scales[0] = [0.0; 2];
        nearest.texture_distortion_index = 0;
        nearest.distortion_power = 0.25;
        nearest.distortion_targets = 1;
        nearest.uvd_origin = [-0.125, 0.0];
        nearest.uvd_scale = [0.0; 2];
        nearest.distortion_filter = 0;
        let mut linear = nearest;
        linear.color = [0.0, 1.0, 0.0, 1.0];
        linear.distortion_filter = 1;
        let particles = vec![nearest, linear];
        let actual = render(
            &format!("vfx-td-filter-{mesh_mode}"),
            if mesh_mode { vec![] } else { particles.clone() },
            if mesh_mode {
                particles.into_iter().map(mesh).collect()
            } else {
                vec![]
            },
            vec![black_white_texture()],
        )[32 * 64 + 32];
        // TD samples 0 / 0.25, displacing TC1's u=0.5 by -0.125 / -0.0625.
        // Linear TC1 samples at u=0.375 / 0.4375 then yield 0.25 / 0.375.
        assert_color(
            actual,
            [
                background[0] + 0.25,
                background[1] + 0.375,
                background[2],
                background[3],
            ],
        );
    }
}

/// The scene path must keep every model's buffers and the VFX stage alive together.
/// Compare separate GPU instances with a combined mesh under identical bounds.
#[test]
#[ignore = "requires native wgpu"]
fn multi_instance_scene_keeps_models_and_vfx_at_1x_and_4x() {
    use xiv_companion::PreparedModelOptions;
    use xiv_companion::renderer::test_support::{SceneSnapshotEntry, render_scene_snapshot};

    let panel = |side: f32| {
        let mut model = vfx_semantics_model();
        let mesh = &mut model.meshes[0];
        mesh.path = format!("scene-panel-{side}");
        let template = mesh.vertices[0];
        mesh.vertices.extend(
            [[-0.2, -0.3], [0.2, -0.3], [0.0, 0.3]].map(|[x, y]| ModelVertex {
                position: [side + x, y, 0.0],
                ..template
            }),
        );
        mesh.indices = vec![3, 4, 5];
        model
    };
    let left = panel(-0.45);
    let right = panel(0.45);
    let mut combined = left.clone();
    combined.meshes.extend(right.meshes.clone());
    let prepared = PreparedModelOptions::default().with_component_preview_layout(false);
    let entries = [&left, &right]
        .map(|model| SceneSnapshotEntry::new(model).with_prepared_options(prepared.clone()));
    let pixels = |snapshot: xiv_companion::renderer::test_support::ModelSnapshot| {
        image::open(snapshot.png_path)
            .unwrap()
            .to_rgba8()
            .into_raw()
    };
    for msaa_samples in [1, 4] {
        let options = WeaponModelSnapshotOptions::new(format!("glamour-vfx-scene-{msaa_samples}"))
            .with_viewport(64, 64)
            .with_camera(0.0, 0.0, 3.0, [0.0; 2])
            .with_prepared_model_options(prepared.clone())
            .with_render_options(ModelRenderOptions {
                msaa_samples,
                ..Default::default()
            });
        let mut baseline_options = options.clone();
        baseline_options.name.push_str("-baseline");
        let baseline =
            pixels(render_scene_snapshot(baseline_options, &entries, None, None).unwrap());
        let background = &baseline[..3];
        for range in [0..32, 32..64] {
            let foreground = (0..64)
                .flat_map(|y| range.clone().map(move |x| y * 64 + x))
                .filter(|&index| {
                    baseline[index * 4..index * 4 + 3]
                        .iter()
                        .zip(background)
                        .any(|(a, b)| a.abs_diff(*b) > 10)
                })
                .count();
            assert!(
                foreground > 15,
                "both scene instances must remain visible at {msaa_samples}x"
            );
        }
        let mut particle = quad([0.0, 0.0, 1.0, 1.0], true);
        particle.size = [0.2; 2];
        let options = options
            .with_vfx_quads([particle])
            .with_vfx_textures([solid([255; 4])]);
        let scene = pixels(render_scene_snapshot(options.clone(), &entries, None, None).unwrap());
        let mut single_options = options;
        single_options.name.push_str("-combined");
        let single =
            pixels(render_weapon_model_snapshot_with_options(single_options, &combined).unwrap());
        assert!(
            scene == single,
            "scene and combined meshes must produce the same VFX composition at {msaa_samples}x"
        );
        let center = (32 * 64 + 32) * 4;
        assert!(
            scene[center + 2] > baseline[center + 2] + 20,
            "VFX must remain visible after drawing the scene"
        );
    }
}
