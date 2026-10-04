//! 诊断探针：dump 武器 MDL 骨骼包围盒（bind point 推导用）。
#![cfg(feature = "game-data")]
use physis::resource::{Resource, SqPackResource};

fn main() {
    let raw_dir = std::path::PathBuf::from(std::env::var("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("normalize");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("utf8"));
    let path = "chara/weapon/w0501/obj/body/b0060/model/w0501b0060.mdl";
    let bytes = resource.read(path).expect("read mdl");
    let meta = xiv_companion::mdl_metadata_from_mdl_bytes(path, &bytes).expect("metadata");
    eprintln!(
        "model bbox: {:?} .. {:?}",
        meta.model_bounding_box, meta.model_bounding_box
    );
    for (i, bb) in meta.bone_bounding_boxes.iter().enumerate() {
        eprintln!("bone {i}: {:?}", bb);
    }
}
