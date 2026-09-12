// SPDX-FileCopyrightText: 2020 Inseok Lee
// SPDX-License-Identifier: MIT
// derived from physis (https://github.com/AzurIce/physis), MIT license
// vendored into xiv-companion-data so the havok tagfile reader can be maintained locally.

#![allow(unused)] // This isn't public API, so I don't care about the unused bits.

extern crate alloc;

pub mod animation;
pub mod animation_binding;
pub mod animation_container;
pub mod binary_tag_file_reader;
pub mod byte_reader;
pub mod object;
pub mod skeleton;
pub mod slice_ext;
pub mod spline_compressed_animation;
pub mod transform;

pub use animation::HavokAnimation;
pub use animation_container::HavokAnimationContainer;
pub use binary_tag_file_reader::HavokBinaryTagFileReader;
pub use object::HavokRootObject;
pub use skeleton::HavokSkeleton;
pub use spline_compressed_animation::HavokSplineCompressedAnimation;
pub use transform::HavokTransform;
