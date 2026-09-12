//! 角色组装器页面：按捏脸数据资产（CharaMakeType 菜单 + human.cmp 调色板）
//! 编辑 26 字节捏脸，经本地游戏目录实时装配 chara/human 模型并预览。

use std::rc::Rc;

use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;

use crate::app::data::{load_character_make, load_character_palette};
use crate::app::icons::{Icon, IconKind};
use crate::app::load_progress::{self, WeaponModelLoadProgress};
use crate::app::resources::load_character_assembly_with_skeleton_from_local;
use crate::app::ui::{
    Button, ButtonSize, ButtonVariant, EmptyState, GitHubRepoButton, input_class,
};
use crate::app::utils::format_integer;

use super::weapon_models::{
    AnimationControls, AnimationPlaybackState, AnimationSetHandle, WeaponModelCanvas,
    WeaponModelLoadingView, animation_playback,
};
use xiv_companion::renderer::WeaponRenderOptions;
use xiv_companion::{
    CHARACTER_CUSTOMIZE_LEN, CharacterAssemblyData, CharacterAssemblyLoadRequest,
    CharacterCustomize, CharacterMakeGroup, CharacterMakeMenu, CharacterMakeMenuKind,
    CharacterMakePackage, CharacterPalette, ModelAnimationSet, ModelSkeleton, RgbaColor,
    appearance_colors_from_palette, character_enabled_attribute_names,
};

/// 角色装配加载结果：模型 + 可选骨架 + 可选动画集（action.pap 等）。
#[derive(Clone)]
struct CharacterModelAssets {
    model: Rc<CharacterAssemblyData>,
    skeleton: Option<Rc<ModelSkeleton>>,
    animations: Option<Rc<ModelAnimationSet>>,
}

/// 路由路径常量仅在 wasm32 的 URL 同步中使用。
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const CHARACTER_ROUTE_PATH: &str = "/character";

/// 角色装配的 attribute 显隐按**名**判定（位是各 MDL 本地表序，跨模型数值
/// 不可比）：启用集合由 `character_enabled_attribute_names` 按捏脸
/// FacialFeatures 字节计算（裸装默认全开，atr_lod 恒关，atr_fv_* 特征件
/// 逐位开关）。

/// 种族 →（中文名，[(部族编号, 中文名)]）。部族编号与捏脸数据资产一致。
const CHARACTER_RACES: &[(u8, &str, &[(u8, &str)])] = &[
    (1, "中原人", &[(1, "中原之民"), (2, "高地之民")]),
    (2, "精灵", &[(3, "森林之民"), (4, "黑影之民")]),
    (3, "拉拉菲尔", &[(5, "平原之民"), (6, "沙漠之民")]),
    (4, "猫魅", &[(7, "逐日之民"), (8, "护月之民")]),
    (5, "鲁加", &[(9, "北洋之民"), (10, "红焰之民")]),
    (6, "敖龙", &[(11, "晨曦之民"), (12, "暮晖之民")]),
    (7, "硌狮", &[(13, "掠日之民"), (14, "迷踪之民")]),
    (8, "维埃拉", &[(15, "密林之民"), (16, "山林之民")]),
];

fn race_label(race: u8) -> &'static str {
    CHARACTER_RACES
        .iter()
        .find(|(id, _, _)| *id == race)
        .map(|(_, label, _)| *label)
        .unwrap_or("未知种族")
}

fn tribes_of_race(race: u8) -> &'static [(u8, &'static str)] {
    CHARACTER_RACES
        .iter()
        .find(|(id, _, _)| *id == race)
        .map(|(_, _, tribes)| *tribes)
        .unwrap_or(&[])
}

fn tribe_label(race: u8, tribe: u8) -> &'static str {
    tribes_of_race(race)
        .iter()
        .find(|(id, _)| *id == tribe)
        .map(|(_, label)| *label)
        .unwrap_or("")
}

/// 无 URL 捏脸时进入页面的兜底基底（中原人男，可通过校验）；捏脸菜单资产
/// 加载完成后会被该组默认捏脸替换。
fn fallback_customize() -> CharacterCustomize {
    CharacterCustomize {
        race: 1,
        tribe: 1,
        gender: 0,
        age: 1,
        height: 50,
        ..Default::default()
    }
}

/// 捏脸 → 52 字符小写 hex（`?c=` URL 参数）。
fn customize_to_hex(customize: &CharacterCustomize) -> String {
    customize
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// 52 字符 hex → 捏脸：长度/内容/关键字段校验全部通过才返回 Some。
fn customize_from_hex(value: &str) -> Option<CharacterCustomize> {
    let value = value.trim();
    if value.len() != CHARACTER_CUSTOMIZE_LEN * 2 || !value.is_ascii() {
        return None;
    }
    let mut bytes = [0_u8; CHARACTER_CUSTOMIZE_LEN];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk).ok()?;
        bytes[index] = u8::from_str_radix(text, 16).ok()?;
    }
    let customize = CharacterCustomize::from_bytes(&bytes).ok()?;
    customize.validate().ok()?;
    Some(customize)
}

/// 从 `#/character?c=...` hash 解析捏脸；无参数或非法返回 None。
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn character_customize_from_hash(hash: &str) -> Option<CharacterCustomize> {
    let route = hash.trim_start_matches('#');
    let (_path, query) = route.split_once('?')?;
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == "c" {
            return customize_from_hex(value);
        }
    }
    None
}

fn initial_character_state() -> (CharacterCustomize, bool) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(customize) = web_sys::window()
            .and_then(|window| window.location().hash().ok())
            .and_then(|hash| character_customize_from_hash(&hash))
        {
            return (customize, true);
        }
    }
    (fallback_customize(), false)
}

/// 按（种族, 部族, 性别）取捏脸组；精确 miss 时回退同种族同性别首组。
fn find_make_group<'a>(
    package: &'a CharacterMakePackage,
    race: u8,
    tribe: u8,
    gender: u8,
) -> Option<&'a CharacterMakeGroup> {
    package
        .groups
        .iter()
        .find(|group| group.race == race && group.tribe == tribe && group.gender == gender)
        .or_else(|| {
            package
                .groups
                .iter()
                .find(|group| group.race == race && group.gender == gender)
        })
}

/// 读捏脸字节（与 `CharacterCustomize::to_bytes` 布局一致）。
fn customize_byte(customize: &CharacterCustomize, offset: usize) -> u8 {
    match offset {
        0 => customize.race,
        1 => customize.gender,
        2 => customize.age,
        3 => customize.height,
        4 => customize.tribe,
        5 => customize.head,
        6 => customize.hair,
        7 => customize.highlight_type,
        8 => customize.skintone,
        9 => customize.r_eye_color,
        10 => customize.hair_tone,
        11 => customize.highlights,
        12 => customize.facial_features,
        13 => customize.facial_feature_color,
        14 => customize.eyebrows,
        15 => customize.l_eye_color,
        16 => customize.eyes,
        17 => customize.nose,
        18 => customize.jaw,
        19 => customize.mouth,
        20 => customize.lips_tone,
        21 => customize.ear_muscle_tail_size,
        22 => customize.tail_ears_type,
        23 => customize.bust,
        24 => customize.face_paint,
        25 => customize.face_paint_color,
        _ => 0,
    }
}

/// 写捏脸字节，返回新结构（越界偏移原样返回）。
fn set_customize_byte(
    customize: &CharacterCustomize,
    offset: usize,
    value: u8,
) -> CharacterCustomize {
    let mut next = *customize;
    match offset {
        0 => next.race = value,
        1 => next.gender = value,
        2 => next.age = value,
        3 => next.height = value,
        4 => next.tribe = value,
        5 => next.head = value,
        6 => next.hair = value,
        7 => next.highlight_type = value,
        8 => next.skintone = value,
        9 => next.r_eye_color = value,
        10 => next.hair_tone = value,
        11 => next.highlights = value,
        12 => next.facial_features = value,
        13 => next.facial_feature_color = value,
        14 => next.eyebrows = value,
        15 => next.l_eye_color = value,
        16 => next.eyes = value,
        17 => next.nose = value,
        18 => next.jaw = value,
        19 => next.mouth = value,
        20 => next.lips_tone = value,
        21 => next.ear_muscle_tail_size = value,
        22 => next.tail_ears_type = value,
        23 => next.bust = value,
        24 => next.face_paint = value,
        25 => next.face_paint_color = value,
        _ => {}
    }
    next
}

fn palette_key_label(key: &str) -> &str {
    match key {
        "skin" => "肤色",
        "hair" => "发色",
        "eye" => "瞳色",
        "highlight" => "挑染",
        "lip" => "唇色",
        "feature" => "特征色",
        "facepaint" => "面妆颜色",
        _ => "调色板",
    }
}

/// 捏脸菜单的中文展示名；未知菜单回退调色板键或字节偏移说明。
fn menu_label(menu: &CharacterMakeMenu) -> String {
    match menu.name.as_deref() {
        Some("height") => "身高".to_string(),
        Some("muscleSize") => "体格".to_string(),
        Some("bustSize") => "胸围".to_string(),
        Some("face") => "脸型".to_string(),
        Some("hair") => "发型".to_string(),
        Some("skintone") => "肤色".to_string(),
        Some("hairTone") => "发色".to_string(),
        Some("eyeColorRight") => "瞳色".to_string(),
        Some("eyePupil") => "瞳孔".to_string(),
        Some("eyebrows") => "眉毛".to_string(),
        Some("eyes") => "眼型".to_string(),
        Some("nose") => "鼻子".to_string(),
        Some("jaw") => "轮廓".to_string(),
        Some("mouth") => "嘴型".to_string(),
        Some("lipsTone") => "唇色".to_string(),
        Some("facialFeatures") => "面部特征".to_string(),
        Some("facialFeatureColor") => "特征色".to_string(),
        Some("facePaint") => "面妆".to_string(),
        Some("facePaintColor") => "面妆颜色".to_string(),
        Some("tailEarsType") => "尾/耳类型".to_string(),
        Some("earTailSize") => "耳/尾大小".to_string(),
        Some("menuOffset19") => "嘴型".to_string(),
        Some(other) => other.to_string(),
        None => menu
            .palette
            .as_deref()
            .map(palette_key_label)
            .unwrap_or("菜单")
            .to_string(),
    }
}

/// 字节 21 滑条的展示名：有尾/耳种族为耳/尾大小，其余为体格（肌肉）。
fn build_label_for_byte21(customize: &CharacterCustomize) -> &'static str {
    if customize.has_tail() || customize.has_zear() {
        "耳/尾大小"
    } else {
        "体格（肌肉）"
    }
}

/// 编辑控件排序键：数值/滑条 → 五官 → 发色组 → 特征 → 面妆。
fn menu_display_order(byte_offset: usize) -> u8 {
    match byte_offset {
        3 => 10,
        21 => 11,
        23 => 12,
        5 => 20,
        6 => 21,
        8 => 22,
        10 => 23,
        11 => 24,
        9 => 30,
        14 => 31,
        16 => 32,
        17 => 33,
        18 => 34,
        19 => 35,
        20 => 36,
        12 => 40,
        13 => 41,
        22 => 42,
        24 => 50,
        25 => 51,
        _ => 60,
    }
}

/// 菜单选项（写字节的值 + 展示名）。发型/面妆选项经组内索引表换算为 ID
/// （发型 ID 即文件号；面妆 0 = 无，不在索引表中）。
fn menu_options(menu: &CharacterMakeMenu, group: &CharacterMakeGroup) -> Vec<(u8, String)> {
    if menu.byte_offset == 6 {
        return group
            .hair_options
            .iter()
            .filter_map(|id| u8::try_from(*id).ok())
            .map(|id| (id, format!("发型 {id}")))
            .collect();
    }
    if menu.byte_offset == 24 {
        let mut options = vec![(0_u8, "无".to_string())];
        options.extend(
            group
                .face_paint_options
                .iter()
                .filter_map(|id| u8::try_from(*id).ok())
                .map(|id| (id, format!("面妆 {id}"))),
        );
        return options;
    }
    (0..menu.option_count)
        .filter_map(|value| u8::try_from(value).ok())
        .map(|value| (value, (value + 1).to_string()))
        .collect()
}

/// 调色板段：肤色/发色取 (部族, 性别) 组，其余取公共区。
fn palette_segment<'a>(
    palette: &'a CharacterPalette,
    customize: &CharacterCustomize,
    key: &str,
) -> Option<&'a [RgbaColor]> {
    let tribe = palette
        .tribes
        .get(customize.palette_group_index())
        .or_else(|| palette.tribes.first());
    match key {
        "skin" => tribe.map(|group| group.skin.as_slice()),
        "hair" => tribe.map(|group| group.hair.as_slice()),
        "eye" => Some(palette.common.eye.as_slice()),
        "highlight" => Some(palette.common.highlight.as_slice()),
        "lip" => Some(palette.common.lip.as_slice()),
        "feature" => Some(palette.common.feature.as_slice()),
        "facepaint" => Some(palette.common.face_paint.as_slice()),
        _ => None,
    }
}

fn character_display_name(customize: &CharacterCustomize) -> String {
    format!(
        "{} {} · {}",
        race_label(customize.race),
        tribe_label(customize.race, customize.tribe),
        if customize.gender == 1 {
            "女性"
        } else {
            "男性"
        }
    )
}

#[allow(unused_variables)]
fn sync_character_url_state(customize: &CharacterCustomize) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(window) = web_sys::window() else {
            return;
        };
        let hash = format!(
            "#{}?c={}",
            CHARACTER_ROUTE_PATH,
            customize_to_hex(customize)
        );
        if window.location().hash().ok().as_deref() == Some(hash.as_str()) {
            return;
        }
        match window.history() {
            Ok(history) => {
                let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&hash));
            }
            Err(_) => {
                let _ = window.location().set_hash(hash.trim_start_matches('#'));
            }
        }
    }
}

#[derive(Clone, PartialEq)]
enum EditorRow {
    Slider {
        offset: usize,
        label: String,
        value: u8,
        max: u16,
    },
    Select {
        offset: usize,
        label: String,
        value: u8,
        options: Vec<(u8, String)>,
        sync_left_eye: bool,
    },
    Palette {
        offset: usize,
        label: String,
        value: u8,
        colors: Vec<RgbaColor>,
        limit: usize,
        toggle: Option<(bool, &'static str)>,
        sync_left_eye: bool,
    },
    Features {
        offset: usize,
        label: String,
        value: u8,
    },
}

impl EditorRow {
    fn key(&self) -> usize {
        match self {
            Self::Slider { offset, .. }
            | Self::Select { offset, .. }
            | Self::Palette { offset, .. }
            | Self::Features { offset, .. } => *offset,
        }
    }

    fn sort_key(&self) -> (u8, usize) {
        (menu_display_order(self.key()), self.key())
    }
}

/// 由捏脸组菜单构建编辑行（按字节偏移去重；声音菜单字节 0 与字节 15 不渲染）。
fn build_editor_rows(
    group: &CharacterMakeGroup,
    palette: &CharacterPalette,
    customize: &CharacterCustomize,
) -> Vec<EditorRow> {
    let mut rows = Vec::new();
    let mut seen_offsets = Vec::new();
    for menu in &group.menus {
        let offset = menu.byte_offset;
        // 声音（字节 0）不在 26 字节布局内；字节 15 由瞳色选择器联动写入
        // （建号时双眼同色，menu 244 的独立语义未核实，v1 不单独渲染）。
        if offset == 0 || offset == 15 || seen_offsets.contains(&offset) {
            continue;
        }
        seen_offsets.push(offset);
        let value = customize_byte(customize, offset);
        let label = if offset == 21 {
            build_label_for_byte21(customize).to_string()
        } else {
            menu_label(menu)
        };
        match menu.kind {
            CharacterMakeMenuKind::Slider => rows.push(EditorRow::Slider {
                offset,
                label,
                value,
                max: menu.option_count,
            }),
            CharacterMakeMenuKind::List | CharacterMakeMenuKind::IconList => {
                rows.push(EditorRow::Select {
                    offset,
                    label,
                    value,
                    options: menu_options(menu, group),
                    sync_left_eye: offset == 9,
                });
            }
            CharacterMakeMenuKind::Palette | CharacterMakeMenuKind::HairPalette => {
                let Some(key) = menu.palette.as_deref() else {
                    continue;
                };
                let Some(colors) = palette_segment(palette, customize, key) else {
                    continue;
                };
                let limit = usize::from(menu.option_count).min(colors.len());
                let toggle: Option<(bool, &'static str)> =
                    (offset == 20).then_some((customize.mouth & 0x80 != 0, "启用唇色"));
                rows.push(EditorRow::Palette {
                    offset,
                    label,
                    value,
                    colors: colors.to_vec(),
                    limit,
                    toggle,
                    sync_left_eye: offset == 9,
                });
            }
            CharacterMakeMenuKind::FeatureBitmask => rows.push(EditorRow::Features {
                offset,
                label,
                value,
            }),
        }
    }
    // 挑染没有捏脸菜单驱动（highlight_type bit7 开关 + highlights 字节），
    // 作为固定编辑段插入发色之后。
    if !seen_offsets.contains(&11) {
        rows.push(EditorRow::Palette {
            offset: 11,
            label: "挑染".to_string(),
            value: customize.highlights,
            colors: palette.common.highlight.clone(),
            limit: palette.common.highlight.len(),
            toggle: Some((customize.highlight_type & 0x80 != 0, "启用挑染")),
            sync_left_eye: false,
        });
    }
    rows.sort_by_key(EditorRow::sort_key);
    rows
}

#[component]
pub fn CharacterPage() -> Element {
    let (initial_customize, initial_adopted) = initial_character_state();
    let mut customize = use_signal(move || initial_customize);
    let mut default_adopted = use_signal(move || initial_adopted);
    let make_package = use_resource(load_character_make);
    let palette_package = use_resource(load_character_palette);
    let mut model_progress = use_signal(|| None::<WeaponModelLoadProgress>);
    let render_options = use_signal(WeaponRenderOptions::default);
    // 装配模型每次重新加载到达时递增，驱动画布在 instance key 不变（同种族
    // 换发型/颜色）时也重新 set_model。
    let mut model_revision = use_signal(|| 0_u64);
    // 动画选择：None=rest；action.pap 动画表在 race code 间一致，换发型保留选择。
    let mut animation_selection = use_signal(|| None::<usize>);

    // 捏脸菜单资产就绪后，以当前（种族, 部族, 性别）组默认捏脸为基底
    // （URL 带合法 ?c= 时跳过，保留链接里的捏脸）。
    use_effect(move || {
        if default_adopted() {
            return;
        }
        let package_snapshot = make_package.read();
        let Some(Ok(package)) = package_snapshot.as_ref() else {
            return;
        };
        let current = customize();
        if let Some(group) = find_make_group(package, current.race, current.tribe, current.gender) {
            customize.set(group.default_customize);
        }
        default_adopted.set(true);
    });

    let model = use_resource(move || {
        let customize = customize();
        let adopted = default_adopted();
        let palette = palette_package
            .read()
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .cloned();
        async move {
            if !adopted {
                return None;
            }
            let palette = palette?;
            if customize.validate().is_err() {
                return None;
            }
            let appearance = appearance_colors_from_palette(&customize, &palette.palette);
            let request =
                CharacterAssemblyLoadRequest::new(customize, character_display_name(&customize))
                    .with_appearance(appearance);
            Some(
                load_character_assembly_with_skeleton_from_local(request)
                    .await
                    .map(|(data, skeleton, animations)| CharacterModelAssets {
                        model: Rc::new(data),
                        skeleton: skeleton.map(Rc::new),
                        animations: animations.map(Rc::new),
                    }),
            )
        }
    });

    use_effect(move || {
        let current = model.read().cloned().flatten();
        if matches!(current, Some(Ok(_))) {
            let next_revision = *model_revision.peek() + 1;
            model_revision.set(next_revision);
        }
    });

    use_effect(move || {
        load_progress::set_weapon_model_progress_sink(move |progress| {
            if let Ok(mut slot) = model_progress.try_write() {
                *slot = progress;
            }
        });
    });

    use_drop(move || {
        load_progress::clear_weapon_model_progress();
    });

    use_effect(move || {
        sync_character_url_state(&customize());
    });

    let customize_snapshot = customize();
    let make_snapshot = make_package.read().as_ref().cloned();
    let palette_snapshot = palette_package.read().as_ref().cloned();
    let current_model = model.read().cloned().flatten();
    let progress_snapshot = model_progress();

    rsx! {
        div { class: "flex h-[calc(100dvh-3.5rem)] min-w-0 flex-col overflow-hidden bg-background lg:h-screen",
            div { class: "border-b px-4 py-2 sm:px-5 lg:px-6",
                div { class: "flex flex-wrap items-center justify-between gap-2",
                    div { class: "flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1",
                        h1 { class: "text-xl font-semibold leading-tight", "角色" }
                        crate::app::modules::ModuleCapabilityBadges { module_id: "character" }
                    }
                    div { class: "flex flex-wrap items-center gap-2 text-xs text-muted-foreground",
                        if let Some(Ok(package)) = &make_snapshot {
                            span { "{package.game_version}" }
                        }
                        GitHubRepoButton {}
                    }
                }
            }

            match (&make_snapshot, &palette_snapshot) {
                (Some(Ok(make)), Some(Ok(palette_package_data))) => {
                    let group = find_make_group(
                        make,
                        customize_snapshot.race,
                        customize_snapshot.tribe,
                        customize_snapshot.gender,
                    );
                    let rows = group.map(|group| {
                        build_editor_rows(group, &palette_package_data.palette, &customize_snapshot)
                    });
                    let race_code = customize_snapshot.race_code();
                    let current_progress = progress_snapshot
                        .filter(|progress| progress.item_id == u32::from(race_code));
                    let playback: Option<AnimationPlaybackState> = current_model
                        .as_ref()
                        .and_then(|result| result.as_ref().ok())
                        .and_then(|assets| {
                            animation_playback(
                                &assets.skeleton,
                                &assets.animations,
                                animation_selection(),
                            )
                        });
                    rsx! {
                        div { class: "grid min-h-0 flex-1 overflow-hidden lg:grid-cols-[340px_minmax(0,1fr)]",
                            aside { class: "min-h-0 overflow-y-auto border-r bg-card p-3",
                                div { class: "space-y-4",
                                    RaceSelect {
                                        customize: customize_snapshot,
                                        on_change: move |(race, tribe, gender)| {
                                            let package_snapshot = make_package.read();
                                            let Some(Ok(package)) = package_snapshot.as_ref() else {
                                                return;
                                            };
                                            let next = find_make_group(
                                                    package,
                                                    race,
                                                    tribe,
                                                    gender,
                                                )
                                                .map(|group| group.default_customize)
                                                .unwrap_or(CharacterCustomize {
                                                    race,
                                                    tribe,
                                                    gender,
                                                    age: 1,
                                                    height: 50,
                                                    ..Default::default()
                                                });
                                            customize.set(next);
                                        },
                                    }
                                    if let Some(rows) = rows {
                                        section { class: "space-y-3 border-t pt-3",
                                            div { class: "text-sm font-semibold", "捏脸" }
                                            for row in rows {
                                                EditorRowView {
                                                    key: "{row.key()}",
                                                    row,
                                                    customize,
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            section { class: "flex min-h-0 min-w-0 flex-col overflow-hidden bg-background",
                                div { class: "flex min-h-0 flex-1 flex-col overflow-hidden xl:flex-row",
                                    div { class: "relative min-h-0 min-w-0 flex-1 overflow-hidden bg-[#0e1117]",
                                        WeaponModelCanvas {
                                            model: current_model
                                                .as_ref()
                                                .and_then(|result| result.as_ref().ok())
                                                .map(|assets| assets.model.clone()),
                                            render_options,
                                            shape_mask: None,
                                            race_id: race_code,
                                            attribute_mask: 0,
                                            attribute_parts_only: false,
                                            enabled_attribute_names: current_model
                                                .as_ref()
                                                .and_then(|result| result.as_ref().ok())
                                                .map(|assets| {
                                                    character_enabled_attribute_names(
                                                        &customize_snapshot,
                                                        &assets.model,
                                                    )
                                                }),
                                            component_preview_layout: false,
                                            model_revision: model_revision(),
                                            animation: playback,
                                        }
                                        match &current_model {
                                            Some(Ok(_)) => rsx! {},
                                            Some(Err(error)) => rsx! {
                                                div { class: "absolute inset-0 flex items-center justify-center bg-[#0e1117] p-6",
                                                    EmptyState {
                                                        icon: rsx! { Icon { kind: IconKind::User, class: "h-6 w-6" } },
                                                        title: "角色装配失败".to_string(),
                                                        description: Some(error.clone()),
                                                    }
                                                }
                                            },
                                            None => rsx! {
                                                div { class: "absolute inset-0 bg-[#0e1117]",
                                                    WeaponModelLoadingView { progress: current_progress }
                                                }
                                            },
                                        }
                                    }
                                    aside { class: "h-56 shrink-0 overflow-y-auto border-t bg-card p-3 xl:h-auto xl:w-64 xl:border-l xl:border-t-0",
                                        match &current_model {
                                            Some(Ok(assets)) => rsx! {
                                                div { class: "space-y-4",
                                                    if let Some(animations) = assets.animations.clone() {
                                                        AnimationControls {
                                                            animations: AnimationSetHandle(animations),
                                                            selected: animation_selection(),
                                                            on_select: move |selected| {
                                                                animation_selection.set(selected);
                                                            },
                                                        }
                                                    }
                                                    CharacterModelStats {
                                                        model: assets.model.clone(),
                                                        customize: customize_snapshot,
                                                    }
                                                }
                                            },
                                            _ => rsx! {
                                                div { class: "space-y-3",
                                                    div { class: "h-8 animate-pulse rounded-md bg-muted" }
                                                    div { class: "h-8 animate-pulse rounded-md bg-muted" }
                                                    div { class: "h-8 animate-pulse rounded-md bg-muted" }
                                                }
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                (Some(Err(error)), _) | (_, Some(Err(error))) => rsx! {
                    div { class: "flex min-h-0 flex-1 items-center justify-center p-6",
                        EmptyState {
                            icon: rsx! { Icon { kind: IconKind::Database, class: "h-6 w-6" } },
                            title: "捏脸数据不可用".to_string(),
                            description: Some(error.clone()),
                            action: rsx! {
                                a { href: "#/",
                                    Button {
                                        variant: ButtonVariant::Outline,
                                        size: ButtonSize::Sm,
                                        Icon { kind: IconKind::Database, class: "h-4 w-4" }
                                        "数据来源"
                                    }
                                }
                            },
                        }
                    }
                },
                _ => rsx! {
                    div { class: "flex min-h-0 flex-1 items-center justify-center p-6",
                        div { class: "flex items-center gap-3 text-sm text-muted-foreground",
                            Icon { kind: IconKind::LoaderCircle, class: "h-4 w-4 animate-spin" }
                            "正在读取捏脸数据"
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn RaceSelect(customize: CharacterCustomize, on_change: EventHandler<(u8, u8, u8)>) -> Element {
    let race_button = |active: bool| {
        if active {
            "flex h-8 items-center justify-center rounded-md bg-background text-xs font-medium text-foreground shadow-sm transition-colors"
        } else {
            "flex h-8 items-center justify-center rounded-md text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
        }
    };

    rsx! {
        section { class: "space-y-2",
            div { class: "text-sm font-semibold", "种族与部族" }
            div { class: "grid grid-cols-4 gap-1 rounded-lg bg-muted/60 p-1",
                for (race, label, _) in CHARACTER_RACES {
                    button {
                        r#type: "button",
                        class: race_button(*race == customize.race),
                        onclick: move |_| {
                            let tribe = tribes_of_race(*race)
                                .first()
                                .map(|(tribe, _)| *tribe)
                                .unwrap_or(customize.tribe);
                            on_change.call((*race, tribe, customize.gender));
                        },
                        "{label}"
                    }
                }
            }
            div { class: "flex gap-1 rounded-lg bg-muted/60 p-1",
                for (tribe, label) in tribes_of_race(customize.race) {
                    button {
                        r#type: "button",
                        class: {
                            let active = *tribe == customize.tribe;
                            if active {
                                "flex h-7 flex-1 items-center justify-center rounded bg-background text-xs font-medium text-foreground shadow-sm transition-colors"
                            } else {
                                "flex h-7 flex-1 items-center justify-center rounded text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
                            }
                        },
                        onclick: move |_| {
                            on_change.call((customize.race, *tribe, customize.gender));
                        },
                        "{label}"
                    }
                }
            }
            div { class: "flex gap-1 rounded-lg bg-muted/60 p-1",
                for (gender, label) in [(0_u8, "男性"), (1_u8, "女性")] {
                    button {
                        r#type: "button",
                        class: {
                            let active = gender == customize.gender;
                            if active {
                                "flex h-7 flex-1 items-center justify-center rounded bg-background text-xs font-medium text-foreground shadow-sm transition-colors"
                            } else {
                                "flex h-7 flex-1 items-center justify-center rounded text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
                            }
                        },
                        onclick: move |_| {
                            on_change.call((customize.race, customize.tribe, gender));
                        },
                        "{label}"
                    }
                }
            }
        }
    }
}

#[component]
fn EditorRowView(row: EditorRow, customize: Signal<CharacterCustomize>) -> Element {
    match row {
        EditorRow::Slider {
            offset,
            label,
            value,
            max,
        } => rsx! {
            SliderControl {
                label,
                value,
                max,
                on_change: move |next| {
                    customize.set(set_customize_byte(&customize(), offset, next));
                },
            }
        },
        EditorRow::Select {
            offset,
            label,
            value,
            options,
            sync_left_eye,
        } => rsx! {
            SelectControl {
                label,
                value,
                options,
                on_change: move |next| {
                    let mut next_customize = set_customize_byte(&customize(), offset, next);
                    if sync_left_eye {
                        next_customize.l_eye_color = next;
                    }
                    customize.set(next_customize);
                },
            }
        },
        EditorRow::Palette {
            offset,
            label,
            value,
            colors,
            limit,
            toggle,
            sync_left_eye,
        } => rsx! {
            PaletteControl {
                label,
                value,
                colors,
                limit,
                toggle,
                on_change: move |next| {
                    let mut next_customize = set_customize_byte(&customize(), offset, next);
                    if sync_left_eye {
                        next_customize.l_eye_color = next;
                    }
                    customize.set(next_customize);
                },
                on_toggle: move |enabled| {
                    let mut next_customize = customize();
                    match offset {
                        11 => {
                            next_customize.highlight_type = if enabled {
                                next_customize.highlight_type | 0x80
                            } else {
                                next_customize.highlight_type & 0x7F
                            };
                        }
                        20 => {
                            next_customize.mouth = if enabled {
                                next_customize.mouth | 0x80
                            } else {
                                next_customize.mouth & 0x7F
                            };
                        }
                        _ => {}
                    }
                    customize.set(next_customize);
                },
            }
        },
        EditorRow::Features {
            offset: _,
            label,
            value,
        } => rsx! {
            FeatureBitmaskControl {
                label,
                value,
                on_change: move |next| {
                    customize.set(set_customize_byte(&customize(), 12, next));
                },
            }
        },
    }
}

#[component]
fn SliderControl(label: String, value: u8, max: u16, on_change: EventHandler<u8>) -> Element {
    rsx! {
        div { class: "space-y-1",
            div { class: "flex items-center justify-between gap-2",
                span { class: "text-xs text-muted-foreground", "{label}" }
                span { class: "text-xs font-medium tabular-nums", "{value}" }
            }
            input {
                class: "h-1.5 w-full cursor-pointer accent-foreground",
                r#type: "range",
                min: "0",
                max: "{max}",
                value: "{value}",
                onchange: move |event| {
                    let value = event.value().parse::<u8>().unwrap_or(0);
                    on_change.call(value);
                },
            }
        }
    }
}

#[component]
fn SelectControl(
    label: String,
    value: u8,
    options: Vec<(u8, String)>,
    on_change: EventHandler<u8>,
) -> Element {
    let mut options = options;
    if !options
        .iter()
        .any(|(option_value, _)| *option_value == value)
    {
        options.push((value, format!("自定义 {value}")));
    }
    rsx! {
        label { class: "flex items-center justify-between gap-2",
            span { class: "w-16 shrink-0 text-xs text-muted-foreground", "{label}" }
            select {
                class: input_class("h-8 flex-1 text-xs"),
                value: "{value}",
                onchange: move |event| {
                    if let Ok(value) = event.value().parse::<u8>() {
                        on_change.call(value);
                    }
                },
                for (option_value, option_label) in options {
                    option { value: "{option_value}", "{option_label}" }
                }
            }
        }
    }
}

#[component]
fn PaletteControl(
    label: String,
    value: u8,
    colors: Vec<RgbaColor>,
    limit: usize,
    toggle: Option<(bool, &'static str)>,
    on_change: EventHandler<u8>,
    on_toggle: EventHandler<bool>,
) -> Element {
    let selected = usize::from(value);
    rsx! {
        div { class: "space-y-1",
            div { class: "flex items-center justify-between gap-2",
                span { class: "text-xs text-muted-foreground", "{label}" }
                span { class: "text-xs font-medium tabular-nums", "{value}" }
            }
            if let Some((enabled, toggle_label)) = toggle {
                label { class: "flex items-center gap-2 text-xs text-muted-foreground",
                    input {
                        r#type: "checkbox",
                        class: "h-3.5 w-3.5 accent-foreground",
                        checked: enabled,
                        onchange: move |event| on_toggle.call(event.checked()),
                    }
                    "{toggle_label}"
                }
            }
            div { class: "grid max-h-24 grid-cols-[repeat(auto-fill,minmax(1.25rem,1fr))] gap-1 overflow-y-auto rounded-md border bg-background p-1",
                for (index, color) in colors.iter().take(limit).enumerate() {
                    button {
                        r#type: "button",
                        title: "{index}",
                        class: if index == selected {
                            "h-5 w-full rounded-sm ring-2 ring-foreground ring-offset-1 ring-offset-background"
                        } else {
                            "h-5 w-full rounded-sm ring-1 ring-foreground/10 transition-shadow hover:ring-foreground/40"
                        },
                        style: "background-color: rgba({color[0]}, {color[1]}, {color[2]}, {color[3]});",
                        onclick: move |_| on_change.call(index as u8),
                    }
                }
            }
        }
    }
}

#[component]
fn FeatureBitmaskControl(label: String, value: u8, on_change: EventHandler<u8>) -> Element {
    rsx! {
        div { class: "space-y-1",
            div { class: "flex items-center justify-between gap-2",
                span { class: "text-xs text-muted-foreground", "{label}" }
                span { class: "text-xs font-medium tabular-nums", "0x{value:02X}" }
            }
            // 游戏内该字节为面部特征 bitmask（两组菜单合计 7 位）。FacialFeatures
            // 位 → attribute 属性名的映射未核实，v1 按字节位直编。
            div { class: "grid grid-cols-4 gap-1",
                for bit in 0..8u8 {
                    label { class: "flex items-center gap-1.5 text-[11px] text-muted-foreground",
                        input {
                            r#type: "checkbox",
                            class: "h-3.5 w-3.5 accent-foreground",
                            checked: value & (1 << bit) != 0,
                            onchange: move |event| {
                                let next = if event.checked() {
                                    value | (1 << bit)
                                } else {
                                    value & !(1 << bit)
                                };
                                on_change.call(next);
                            },
                        }
                        "特征 {bit + 1}"
                    }
                }
            }
        }
    }
}

#[component]
fn CharacterModelStats(
    model: Rc<xiv_companion::WeaponModelData>,
    customize: CharacterCustomize,
) -> Element {
    let vertex_count: usize = model.meshes.iter().map(|mesh| mesh.vertices.len()).sum();
    let index_count: usize = model.meshes.iter().map(|mesh| mesh.indices.len()).sum();
    let customize_hex = customize_to_hex(&customize);

    rsx! {
        div { class: "space-y-4",
            section { class: "space-y-2",
                div { class: "text-sm font-semibold", "模型" }
                StatRow { label: "Mesh", value: format_integer(model.meshes.len() as f64) }
                StatRow { label: "Material", value: format_integer(model.materials.len() as f64) }
                StatRow { label: "Texture", value: format_integer(model.textures.len() as f64) }
                StatRow { label: "Vertex", value: format_integer(vertex_count as f64) }
                StatRow { label: "Index", value: format_integer(index_count as f64) }
                StatRow { label: "Radius", value: format!("{:.3}", model.bounds.radius) }
            }
            section { class: "space-y-2",
                div { class: "text-sm font-semibold", "角色" }
                StatRow { label: "模型", value: format!("c{:04}", customize.race_code()) }
                StatRow {
                    label: "部族",
                    value: format!(
                        "{} {}",
                        race_label(customize.race),
                        tribe_label(customize.race, customize.tribe),
                    ),
                }
                StatRow {
                    label: "性别",
                    value: if customize.gender == 1 { "女性".to_string() } else { "男性".to_string() },
                }
                div { class: "break-all rounded-md border bg-background px-2 py-1.5 font-mono text-[11px] text-muted-foreground",
                    title: "{customize_hex}",
                    "{customize_hex}"
                }
            }
        }
    }
}

#[component]
fn StatRow(label: &'static str, value: String) -> Element {
    rsx! {
        div { class: "flex items-center justify-between gap-2 border-b border-border/60 py-1 text-xs last:border-b-0",
            span { class: "text-muted-foreground", "{label}" }
            span { class: "min-w-0 truncate font-medium", "{value}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xiv_companion::{CharacterPaletteCommon, CharacterTribePalette};

    fn test_customize() -> CharacterCustomize {
        CharacterCustomize {
            race: 4,
            gender: 1,
            age: 1,
            height: 42,
            tribe: 8,
            head: 2,
            hair: 116,
            highlight_type: 0x80,
            skintone: 8,
            r_eye_color: 30,
            hair_tone: 103,
            highlights: 7,
            facial_features: 0b101,
            facial_feature_color: 2,
            eyebrows: 1,
            l_eye_color: 30,
            eyes: 3,
            nose: 5,
            jaw: 2,
            mouth: 0x80,
            lips_tone: 35,
            ear_muscle_tail_size: 25,
            tail_ears_type: 1,
            bust: 60,
            face_paint: 26,
            face_paint_color: 12,
            ..Default::default()
        }
    }

    fn test_group(
        key: u32,
        race: u8,
        tribe: u8,
        gender: u8,
        hair_options: &[u16],
    ) -> CharacterMakeGroup {
        CharacterMakeGroup {
            key,
            race,
            tribe,
            gender,
            race_code: CharacterCustomize::race_code_from_parts(race, tribe, gender),
            menus: Vec::new(),
            default_customize: CharacterCustomize {
                race,
                tribe,
                gender,
                age: 1,
                height: 50,
                ..Default::default()
            },
            hair_options: hair_options.to_vec(),
            face_paint_options: vec![24, 25, 26],
            facial_feature_options: Vec::new(),
        }
    }

    fn test_palette() -> CharacterPalette {
        let color =
            |base: u8| -> RgbaColor { [base, base.wrapping_add(1), base.wrapping_add(2), 255] };
        let colors = |base: u8, len: usize| -> Vec<RgbaColor> {
            (0..len)
                .map(|i| color(base.wrapping_add(i as u8)))
                .collect()
        };
        CharacterPalette {
            common: CharacterPaletteCommon {
                eye: colors(1, 192),
                highlight: colors(50, 208),
                lip: colors(70, 128),
                feature: colors(90, 192),
                face_paint: colors(110, 128),
            },
            tribes: (1..=16_u8)
                .flat_map(|tribe| {
                    [0_u8, 1]
                        .into_iter()
                        .map(move |gender| CharacterTribePalette {
                            tribe,
                            gender,
                            skin: colors(tribe.wrapping_mul(8).wrapping_add(gender), 192),
                            hair: colors(
                                tribe.wrapping_mul(8).wrapping_add(gender).wrapping_add(100),
                                208,
                            ),
                        })
                })
                .collect(),
        }
    }

    #[test]
    fn customize_hex_round_trips_through_url_param() {
        let customize = test_customize();
        let hex = customize_to_hex(&customize);
        assert_eq!(hex.len(), 52);
        assert!(
            hex.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_eq!(customize_from_hex(&hex), Some(customize));
        // 大写 hex 同样可解析。
        assert_eq!(customize_from_hex(&hex.to_uppercase()), Some(customize));
        // 长度错误/非 hex/非法捏脸（race 越界）拒绝。
        assert_eq!(customize_from_hex(&hex[..51]), None);
        assert_eq!(customize_from_hex(&format!("{hex}ff")), None);
        let mut bad = customize;
        bad.race = 0;
        assert_eq!(customize_from_hex(&customize_to_hex(&bad)), None);
    }

    #[test]
    fn route_hash_parses_customize_param() {
        let customize = test_customize();
        let hash = format!("#/character?c={}", customize_to_hex(&customize));
        assert_eq!(character_customize_from_hash(&hash), Some(customize));
        assert_eq!(character_customize_from_hash("#/character"), None);
        assert_eq!(character_customize_from_hash("#/character?c=not-hex"), None);
        assert_eq!(character_customize_from_hash("#/other?c=abc"), None);
    }

    #[test]
    fn make_group_lookup_matches_race_tribe_gender() {
        let package = CharacterMakePackage {
            schema_version: 1,
            generated_at: "2026-09-12T00:00:00Z".to_string(),
            game_version: "test".to_string(),
            source: "test".to_string(),
            groups: vec![
                test_group(0, 1, 1, 0, &[1, 2, 3]),
                test_group(1, 1, 1, 1, &[4, 5, 6]),
                test_group(2, 4, 7, 0, &[7, 8, 9]),
            ],
        };
        let group = find_make_group(&package, 1, 1, 1).expect("hyur female group");
        assert_eq!(group.key, 1);
        assert_eq!(group.gender, 1);
        // 部族 miss 时回退同种族同性别首组。
        let fallback = find_make_group(&package, 4, 8, 0).expect("miqo fallback group");
        assert_eq!(fallback.key, 2);
        assert!(find_make_group(&package, 9, 1, 0).is_none());
    }

    #[test]
    fn option_edits_write_back_expected_bytes() {
        // 脸型（字节 5）切换写入 0x05。
        let customize = test_customize();
        let next = set_customize_byte(&customize, 5, 5);
        assert_eq!(next.head, 5);
        assert_eq!(next.to_bytes()[5], 0x05);
        // 肤色色板（字节 8）选择写入 0x08。
        let next = set_customize_byte(&next, 8, 8);
        assert_eq!(next.skintone, 8);
        assert_eq!(next.to_bytes()[8], 0x08);
        // 瞳色（字节 9）选择并联动左眼（字节 15）。
        let mut next = set_customize_byte(&next, 9, 21);
        next.l_eye_color = next.r_eye_color;
        assert_eq!(next.to_bytes()[9], 21);
        assert_eq!(next.to_bytes()[15], 21);
        // 全偏移写读往返。
        let mut full = customize;
        for offset in 0..26 {
            full = set_customize_byte(&full, offset, (offset + 3) as u8);
            assert_eq!(customize_byte(&full, offset), (offset + 3) as u8);
        }
        assert_eq!(CharacterCustomize::from_bytes(&full.to_bytes()), Ok(full));
    }

    #[test]
    fn menu_options_map_hair_and_face_paint_ids() {
        let mut group = test_group(0, 1, 1, 0, &[11, 22, 33]);
        let hair_menu = CharacterMakeMenu {
            slot: 0,
            menu: 234,
            name: Some("hair".to_string()),
            byte_offset: 6,
            kind: CharacterMakeMenuKind::IconList,
            option_count: 3,
            default_value: 0,
            palette: None,
        };
        let hair = menu_options(&hair_menu, &group);
        assert_eq!(
            hair,
            [
                (11, "发型 11".to_string()),
                (22, "发型 22".to_string()),
                (33, "发型 33".to_string())
            ]
        );
        let paint_menu = CharacterMakeMenu {
            slot: 1,
            menu: 249,
            name: Some("facePaint".to_string()),
            byte_offset: 24,
            kind: CharacterMakeMenuKind::IconList,
            option_count: 4,
            default_value: 0,
            palette: None,
        };
        let paint = menu_options(&paint_menu, &group);
        assert_eq!(
            paint,
            [
                (0, "无".to_string()),
                (24, "面妆 24".to_string()),
                (25, "面妆 25".to_string()),
                (26, "面妆 26".to_string()),
            ]
        );
        // 面妆字节经索引表换算为面妆号。
        let picked = paint[2].0;
        assert_eq!(
            set_customize_byte(&group.default_customize, 24, picked).face_paint,
            25
        );
        // 眉/眼/鼻等数值菜单按 0 基序号直列。
        let list_menu = CharacterMakeMenu {
            slot: 2,
            menu: 246,
            name: Some("nose".to_string()),
            byte_offset: 17,
            kind: CharacterMakeMenuKind::List,
            option_count: 6,
            default_value: 0,
            palette: None,
        };
        let noses = menu_options(&list_menu, &group);
        assert_eq!(noses.len(), 6);
        assert_eq!(noses[5], (5, "6".to_string()));
        group.hair_options = Vec::new();
        assert!(menu_options(&hair_menu, &group).is_empty());
    }

    #[test]
    fn palette_segments_follow_group_and_common_layout() {
        let palette = test_palette();
        let mut customize = CharacterCustomize {
            tribe: 8,
            gender: 1,
            ..Default::default()
        };
        let group_index = customize.palette_group_index();
        assert_eq!(group_index, (8 - 1) * 2 + 1);
        let skin = palette_segment(&palette, &customize, "skin").expect("skin segment");
        assert_eq!(skin, palette.tribes[group_index].skin);
        let hair = palette_segment(&palette, &customize, "hair").expect("hair segment");
        assert_eq!(hair.len(), 208);
        assert_eq!(
            palette_segment(&palette, &customize, "eye"),
            Some(palette.common.eye.as_slice())
        );
        assert_eq!(
            palette_segment(&palette, &customize, "highlight"),
            Some(palette.common.highlight.as_slice())
        );
        assert_eq!(
            palette_segment(&palette, &customize, "lip"),
            Some(palette.common.lip.as_slice())
        );
        assert_eq!(
            palette_segment(&palette, &customize, "facepaint"),
            Some(palette.common.face_paint.as_slice())
        );
        assert_eq!(palette_segment(&palette, &customize, "unknown"), None);
        // 部族组缺省时回退首组。
        customize.tribe = 0;
        customize.gender = 0;
        let skin = palette_segment(&palette, &customize, "skin").expect("fallback skin");
        assert_eq!(skin, palette.tribes[0].skin);
    }

    #[test]
    fn fallback_customize_is_valid_and_labelled() {
        let customize = fallback_customize();
        assert!(customize.validate().is_ok());
        assert_eq!(race_label(customize.race), "中原人");
        assert_eq!(tribe_label(customize.race, customize.tribe), "中原之民");
        assert_eq!(tribes_of_race(8).len(), 2);
        assert_eq!(build_label_for_byte21(&customize), "体格（肌肉）");
        let miqo = CharacterCustomize {
            race: 4,
            ..Default::default()
        };
        assert_eq!(build_label_for_byte21(&miqo), "耳/尾大小");
    }
}
