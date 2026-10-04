//! 诊断探针：dump 武器模型包围盒（bind point 参考）。
#![cfg(feature = "game-data")]
use physis::resource::SqPackResource;
use xiv_companion::ModelRenderData;

fn main() {
    let raw_dir = std::path::PathBuf::from(std::env::var("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("normalize");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("utf8"));
    let catalog = xiv_companion::game_data::export_weapon_catalog_from_resource(
        SqPackResource::from_existing(game_dir.to_str().expect("utf8")),
        game_dir.display().to_string(),
        xiv_companion::game_data::game_version(&game_dir),
        "bounds-probe".to_string(),
    )
    .expect("catalog");
    for item_id in [16053_u32, 15264] {
        let item = catalog
            .items
            .iter()
            .find(|i| i.id == item_id)
            .expect("item");
        let request = xiv_companion::WeaponModelLoadRequest::from(item);
        let model = xiv_companion::load_weapon_model_from_resource_request(&mut resource, &request)
            .expect("model");
        let b = model.bounds();
        eprintln!(
            "item {item_id}: bounds min={:?} max={:?} center={:?} r={:.3}",
            b.min, b.max, b.center, b.radius
        );
    }
}
