// SPDX-FileCopyrightText: 2020 Inseok Lee
// SPDX-License-Identifier: MIT
// derived from physis (https://github.com/AzurIce/physis), MIT license
// vendored into xiv-companion-data so the havok tagfile reader can be maintained locally.

#![allow(dead_code)]

use crate::vendor::physis_havok::transform::HavokTransform;

pub trait HavokAnimation {
    fn duration(&self) -> f32;
    fn sample(&self, time: f32) -> Vec<HavokTransform>;
}
