//! 诊断探针：dump 武器骨架骨骼（名称 + rest 平移）。
//! 用法：XIV_GAME_DIR=... cargo run --features game-data --example dump_skl -- \
//!   chara/weapon/w0501/skeleton/base/b0001/skl_w0501b0001.sklb

#![cfg(feature = "game-data")]

use physis::resource::{Resource, SqPackResource};

fn main() {
    let path = std::env::args().nth(1).expect("sklb sqpack path");
    let raw_dir = std::path::PathBuf::from(std::env::var("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("normalize");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("utf8"));
    let bytes = resource.read(&path).expect("read sklb");
    let skeleton =
        xiv_companion::load_skeleton_from_sklb_bytes(&bytes).expect("parse skeleton");
    let pose = xiv_companion::SkeletonPose::rest_pose(&skeleton);
    let world = xiv_companion::world_matrices(&skeleton, &pose);
    for (i, name) in skeleton.bone_names.iter().enumerate() {
        let m = world[i];
        eprintln!(
            "bone {i}: {name} parent={} world_pos=({:.3}, {:.3}, {:.3})",
            skeleton.parent_indices[i],
            m[12], m[13], m[14]
        );
    }
}
