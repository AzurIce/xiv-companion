// SPDX-FileCopyrightText: 2020 Inseok Lee
// SPDX-License-Identifier: MIT
// derived from physis (https://github.com/AzurIce/physis), MIT license
// vendored into xiv-companion-data so the havok tagfile reader can be maintained locally.

#![allow(dead_code)]

use crate::vendor::physis_havok::HavokAnimation;
use crate::vendor::physis_havok::object::HavokObject;
use crate::vendor::physis_havok::spline_compressed_animation::HavokSplineCompressedAnimation;
use core::cell::RefCell;
use std::sync::Arc;

#[repr(u8)]
pub enum HavokAnimationBlendHint {
    Normal = 0,
    Additive = 1,
}

impl HavokAnimationBlendHint {
    pub fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Normal,
            1 => Self::Additive,
            _ => panic!(),
        }
    }
}

pub struct HavokAnimationBinding {
    pub transform_track_to_bone_indices: Vec<u16>,
    pub blend_hint: HavokAnimationBlendHint,
    pub animation: Box<dyn HavokAnimation>,
}

impl HavokAnimationBinding {
    pub fn new(object: Arc<RefCell<HavokObject>>) -> Self {
        let root = object.borrow();

        let raw_transform_track_to_bone_indices =
            root.get("transformTrackToBoneIndices").as_array();
        let transform_track_to_bone_indices = raw_transform_track_to_bone_indices
            .iter()
            .map(|x| x.as_int() as u16)
            .collect::<Vec<_>>();

        let blend_hint = HavokAnimationBlendHint::from_raw(root.get("blendHint").as_int() as u8);

        let raw_animation = root.get("animation").as_object();
        let animation = match &*raw_animation.borrow().object_type.name {
            "hkaSplineCompressedAnimation" => {
                Box::new(HavokSplineCompressedAnimation::new(raw_animation.clone()))
            }
            _ => panic!(),
        };

        Self {
            transform_track_to_bone_indices,
            blend_hint,
            animation,
        }
    }
}
