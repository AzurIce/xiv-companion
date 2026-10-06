//! 角色捏脸共享工具：种族/部族常量、52 字符 hex 编解码、捏脸组查找与
//! 种族/部族/性别选择器。角色页与幻化套装预览共用。

use dioxus::prelude::*;

use xiv_companion::{
    CHARACTER_CUSTOMIZE_LEN, CharacterCustomize, CharacterMakeGroup, CharacterMakePackage,
};

/// 种族 →（中文名，[(部族编号, 中文名)]）。部族编号与捏脸数据资产一致。
pub(crate) const CHARACTER_RACES: &[(u8, &str, &[(u8, &str)])] = &[
    (1, "中原人", &[(1, "中原之民"), (2, "高地之民")]),
    (2, "精灵", &[(3, "森林之民"), (4, "黑影之民")]),
    (3, "拉拉菲尔", &[(5, "平原之民"), (6, "沙漠之民")]),
    (4, "猫魅", &[(7, "逐日之民"), (8, "护月之民")]),
    (5, "鲁加", &[(9, "北洋之民"), (10, "红焰之民")]),
    (6, "敖龙", &[(11, "晨曦之民"), (12, "暮晖之民")]),
    (7, "硌狮", &[(13, "掠日之民"), (14, "迷踪之民")]),
    (8, "维埃拉", &[(15, "密林之民"), (16, "山林之民")]),
];

pub(crate) fn race_label(race: u8) -> &'static str {
    CHARACTER_RACES
        .iter()
        .find(|(id, _, _)| *id == race)
        .map(|(_, label, _)| *label)
        .unwrap_or("未知种族")
}

pub(crate) fn tribes_of_race(race: u8) -> &'static [(u8, &'static str)] {
    CHARACTER_RACES
        .iter()
        .find(|(id, _, _)| *id == race)
        .map(|(_, _, tribes)| *tribes)
        .unwrap_or(&[])
}

pub(crate) fn tribe_label(race: u8, tribe: u8) -> &'static str {
    tribes_of_race(race)
        .iter()
        .find(|(id, _)| *id == tribe)
        .map(|(_, label)| *label)
        .unwrap_or("")
}

/// 无持久化/URL 捏脸时的兜底基底（中原人男，可通过校验）；捏脸菜单资产
/// 加载完成后会被该组默认捏脸替换。
pub(crate) fn fallback_customize() -> CharacterCustomize {
    CharacterCustomize {
        race: 1,
        tribe: 1,
        gender: 0,
        age: 1,
        height: 50,
        ..Default::default()
    }
}

/// 捏脸 → 52 字符小写 hex（角色页 `?c=` URL 参数 / 幻化预览持久化共用）。
pub(crate) fn customize_to_hex(customize: &CharacterCustomize) -> String {
    customize
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// 52 字符 hex → 捏脸：长度/内容/关键字段校验全部通过才返回 Some。
pub(crate) fn customize_from_hex(value: &str) -> Option<CharacterCustomize> {
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

/// 按（种族, 部族, 性别）取捏脸组；精确 miss 时回退同种族同性别首组。
pub(crate) fn find_make_group<'a>(
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

pub(crate) fn character_display_name(customize: &CharacterCustomize) -> String {
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

#[component]
pub(crate) fn RaceSelect(
    customize: CharacterCustomize,
    on_change: EventHandler<(u8, u8, u8)>,
) -> Element {
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
