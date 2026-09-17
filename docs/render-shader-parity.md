# 渲染器 × MeddleTools 着色器功能对照表

本文档逐家族对照本仓渲染器(`crates/xiv-companion-render/src/renderer/`)与
MeddleTools 的 Blender 着色器实现,记录每个功能点的实现状态与差距,作为渲染
改进工作的基线。创建于 2026-09-17。

## 参照物与对照方法

- **MeddleTools 本地克隆**:`~/Files/repos/MeddleTools`。着色器实现载体:
  - `MeddleTools/shaders.blend` —— 36 个 Blender 节点组(真正的数学所在);
  - `MeddleTools/node_setup/node_configs.py` / `node_mappings.py` —— 按材质的
    `ShaderPackage` 复制模板并接线(贴图色彩空间/寻址、常量映射、ColorTable
    行 → A/B ramp);
  - `MeddleTools/bake2/` —— 烘焙引擎,把节点图接成 Principled BSDF 输出
    (diffuse/normal/roughness/metallic/emission 等 pass);
  - `MeddleTools/lighting.py` —— 游戏灯光(Sun/Moon/Ambient/Area/Point/Spot/
    Capsule)转 Blender 灯光。
- 节点组已用无头 Blender 导出为文本对照(生成脚本思路见
  `blender --background --python`,导出 `nodes + links + 默认值`)。
- 本仓证据链:DXBC 审计与真机数据验证记录见
  `docs/weapon-render-review-plan.md` / `docs/weapon-render-review-history.md`,
  渲染管线结构见 `docs/weapon-render-pipeline.md`。

## 重要前提

**MeddleTools 不是游戏着色器的像素级复刻**,而是面向 Blender 预览/烘焙的
Principled BSDF 近似。多处公式是明牌近似:

- legacy GlossStrength→Roughness 用 `gloss/20` 缩放再变换(character.shpk 组
  "Legacy gloss to roughness approximation");
- mask.B(AO)直接近似为 Metallic("Diffuse AO Mask to Metalness approximation");
- IOR ≈ Roughness + 1.5("IOR Approx");
- 部分常量无语义名,按观察接线(`0x9A696A17` UV scroll、`0xB8ACCE58` fade 等)。

因此"对齐 MeddleTools"不是目标;它是**通道语义与常量消费的交叉验证源**,
以及缺失特性的灵感清单。本仓若有真机/DXBC 证据,以本仓证据为准。

## 状态标记

| 标记 | 含义 |
|---|---|
| ✅ | 已实现,语义与 MeddleTools 一致或更贴游戏 |
| 🟡 | 部分实现 / 近似实现,有明确改进空间 |
| ❌ | 未实现(MeddleTools 有对应实现) |
| ⛔ | 有证据表明不该做/无法做,双方一致不做 |
| ❓ | 状态待核实(列在 P1 清单) |

## 家族总览

| Shader 家族 (shpk) | 本仓家族枚举 | MeddleTools 节点组 | 总体状态 |
|---|---|---|---|
| skin.shpk | `Skin` | meddle skin.shpk | 🟡 缺 SSS 近似;其余 ✅ |
| hair.shpk | `Hair` | meddle hair.shpk | 🟡 缺 subsurface;明暗/挑染我们更细 |
| iris.shpk | `Iris` | meddle iris.shpk | 🟡 仅乘色;眼白/环/分侧 ❌ |
| charactertattoo.shpk | `CharacterTattoo` | meddle charactertattoo.shpk | ✅ |
| character*.shpk(6 变体) | `Character` + `Character*` | meddle character.shpk | ✅(兼容/legacy/玻璃/透明/长袜均有分支) |
| bg.shpk | `Bg` / `BgUvScroll` | meddle bg.shpk + UV Scale + detail/tile 组 | 🟡 缺地形 UV jitter 与相机淡出 |
| bgcolorchange.shpk | `Bg` | meddle bgcolorchange.shpk | 🟡 StainColor 染色 ❓ |
| bgprop.shpk | (无映射 → Unknown) | meddle bgprop.shpk | ❌ 家族未映射(❓) |
| water.shpk / river.shpk | `Water` | meddle water.shpk | 🟡 双方都极简;我们的 wave/whitecap 贴图未消费 |
| crystal.shpk | `Crystal` | meddle crystal.shpk | 🟡 envmap 数据已导出未采样 |
| lightshaft.shpk | `LightShaft` | meddle lightshaft.shpk | ✅ 我们更完整(亮度发射) |
| scroll(常量簇) | `BgUvScroll`/scroll 路径 | meddle scroll | ✅(X 符号 ❓) |
| decal(常量簇) | character decal 路径 | meddle decaluv | ✅ multiplier/offset/reversed 全有 |
| grass(专用几何) | 无 | grass_* 5 组 | ⛔ 超出模型预览范围 |

## 分家族明细

### skin.shpk

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| 肤色 × diffuse | Skin Color 混入(diffuse × skin) | `character_skin.rgb` 乘算(shading.wesl) | ✅ |
| Face/Body/BodyJJM 颜色切换 | GetMaterialValue 开关切换唇/发/肤 | 数据层 `MaterialSkinValueMode`(Face/Body/BodyJjm/FaceEmissive)已解析;Face/Body 路径分流 | 🟡 BodyJJM 侧差异未单独验证 |
| 唇色 | Lip Color × Lip Color Strength × 唇遮罩(遮罩无证据,按 face diffuse alpha 近似) | 同样近似(`character_lip` × samples.base.a) | ✅ 同一近似 |
| 发色/挑染(body JJM) | Hair/Highlights 按 mask 混入 | hair 分支处理(character_main/mesh) | ✅ |
| 面妆 decal | Decal Color × DecalTexture,强度可关 | character_decal + decal_uv(multiplier/offset/reversed) | ✅ |
| 面部 diffuse 寻址 | skin+Face → EXTEND(防敖龙男雀斑 UV 出界重复) | `Skin`+`Face` → `ClampToEdge`(xiv-companion-data/model.rs `PreparedTextureSamplingSet`) | ✅ 语义相同 |
| 法线 B → SSS | Map Range 0→0.03 "Kaj recommended SSS" | 无 SSS | ❌(P3,近似可行) |
| mask.R→Specular / mask.G→Roughness | 直连 | mask.g 进 roughness(surface.wesl),mask.r 进 specular factor | ✅ |
| 发光面件 | GetMaterialValueFaceEmissive × g_EmissiveColor | FaceEmissive 已解析 | ❓ 渲染侧消费待核实 |

### hair.shpk

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| 发色 × 挑染 | Highlights 混入(因子=法线 B);diffuse×发色(mask alpha 门控) | mask.G 挑染区域、mask.R 明暗(真机校准,更细) | ✅ |
| Alpha = 法线 alpha(UV0/UV1 按 GetSubColor 切换) | 是 | 法线 alpha 透明 + obj/face 毛发整形(character_params.y) | ✅ |
| mask.B → Subsurface Weight | 直连 Principled | 无 SSS | ❌(P3) |
| mask.R/G → Roughness/Specular | 直连 | 有 | ✅ |
| HDRtoSDR(g_DiffuseColor) | 逐通道 Reinhard 压缩 >1 的材质色 | 线性直乘 | 🟡 呈现层分歧(P3 评估) |

### iris.shpk

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| 虹膜色 × diffuse | 是 | `character_left_iris.rgb` 乘算 | ✅(仅左色) |
| 左右眼分侧(顶点色 R/G 选择) | GREATER_THAN 切换 left/right | 单眼材质双眼共享;右眼色入 uniform 备用未分侧 | ❌ |
| 眼白 g_WhiteEyeColor | 混入 | 恒等白(无离线来源) | ❌ |
| Limbal ring 强度 = 虹膜色 A | 混入环带 | 未实现 | ❌ |
| IrisRing(径向渐变环 + 环色 + g_IrisRingEmissiveIntensity) | Gradient+GTE/LTE+fade,约 15 个节点 | 未实现 | ❌(P2,视觉收益最大) |

### charactertattoo.shpk

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| OptionColor 平铺(法线 B 作因子) | 是 | `character_option` 覆写 | ✅ |
| Alpha = 法线 alpha | 是 | 透明策略由法线 alpha 给 | ✅ |

### character*.shpk(装备/通用角色)

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| Compatibility:colortable 漫反射 × 贴图 | ramp 插值(colortablemix 组,id_mix 因子) | 全分辨率逐像素 compose(main.wesl `sample_color_table_base`,packed texel floor) | ✅ 更贴游戏 |
| legacy Gloss→Roughness | `gloss/20` 明牌近似 | DXBC 证据:`exp2(-Gloss/15)`(shading.wesl) | ✅ 本仓有真证据 |
| AO(mask.B)→Metalness | 近似直连 | 皮肤族按介电质处理;其余走 mask 语义 | ✅ 更贴游戏 |
| Stocking 强制 alpha=1 | "If stocking, force alpha" | 未见对应特例 | ❓ |
| 玻璃/透明/scroll 变体 | IS_GLASS/IS_TRANSPARENCY 等布尔分支 | `characterglass` 有审计测试 + 专属边界断言;transparency/scroll 家族识别 | ✅ |
| 双通道染色 | 无(装备染色不在节点图内) | 完整 staining 体系(staining.rs,烘焙进 ramp) | ✅ 本仓独有 |

### bg.shpk / bgcolorchange / bgprop

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| 双 map(GetMultiValues)blend | Map0/1 × MultiBlendWeight | secondary base/normal/specular 采样 + blend | ✅ |
| specmap.G→Roughness、specmap.B→Metallic | 直连 | bg specular 通道语义(feature_params.w) | ✅ |
| ApplyVertexColor / vertex alpha | 开关 + 乘入 | 顶点色/alpha 通道 | ✅ |
| Emission | g_EmissiveColor × specmap.B 因子 | emissive 路径 | ✅ |
| per-channel UV scale(g_ColorUVScale0/1 等) | UV Scale 组 6 路 scale | UV 源体系(uv_sources0-3) | 🟡 表达方式不同,需核对逐通道 scale 是否等效 |
| 相机距离淡出(FadeNear/Far,常量 0xB8ACCE58) | MapRange × Camera | 无 | ❌(P3,影响地形) |
| Voronoi UV jitter(地形去平铺,4 档 jitter 常量) | UV Scale 组 | 无 | ❌(P3,地形去平铺显著) |
| bgcolorchange StainColor 染色 | colormap × lerp(colormap, StainColor, specmap.R) | 家族并入 Bg;已修 alpha 遮罩穿洞;StainColor 染色未见 | ❓ |
| bgprop.shpk | 简单 PBR 组(colormap+normal+spec→PBR) | **无家族映射**,落 Unknown fallback | ❓ 补映射成本低 |

### water.shpk / river.shpk

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| 基础 | deep color 底 + 法线,仅此 | feature_params.y 开关换 water_deep_color 底 | ✅(我们分支更完整) |
| WaveMap 法线扰动 / Whitecap | 节点组声明了输入但未实现数学 | 数据层导出 `WaterWhitecap` 贴图,渲染不采样 | ❌ 双方都没有,我们贴图已就位(P3) |
| Refraction/Transparency 色 | IOR/Transmission 输出 | uniform 闲置 | ❌(P3) |

### crystal.shpk

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| 基础 | colormap + normal,envmap 仅声明 | 家族识别,走基础分支 | ✅ 等效 |
| EnvMap 反射 | 输入存在,组内未见数学 | 数据层 `ModelTextureKind::EnvMap` 已导出,渲染不采样 | ❌(P2,配合 studio_environment 降级) |

### lightshaft.shpk

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| Sampler0/1 按 vertex_color.B 混合 × g_Color | 是 | `resolve_lightshaft_color` 同构 | ✅ |
| 发射 | 直通 | 亮度加权的 emission_strength | ✅ 更完整 |
| g_TexAnim/g_TexU/g_TexV/g_Ray 动画 | 映射了常量,组内未见数学 | uniform 备用未消费 | 🟡 双方都未实现,常量已备 |

### 基础设施(非家族)

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| 蒙皮 | glTF 蒙皮(Blender 原生) | GPU joint storage buffer + WGSL 蒙皮,PAP 动画采样播放 | ✅ |
| 透明排序 | Blender 原生 | CPU 逐三角形视向排序 + 专用 transparent/glass/additive 管线 | ✅ 本仓独有 |
| 抗锯齿 | Blender 采样 | 4x MSAA(HDR 目标 resolve) | ✅ |
| 后处理 | 无(HDRtoSDR Reinhard 散在材质里) | HDR Rgba16Float + bloom + PBR Neutral tone map | ✅ 本仓独有 |
| 双面渲染 | RenderBackfaces 材质属性→backface culling | 逐批次 backface 管线变体 | ✅ |
| 法线重建 Z | Normal_Fix 组(Blue=1 → Normal Map 节点) | WGSL `decode_normal`(RG 重建 Z) | ✅ |
| 调试视图 | 无 | 30 种 debug 模式 + UnsupportedInputs 诊断色 | ✅ 本仓独有 |
| 测试基建 | 无 | 42 张 native GPU 快照 + 88 着色器/单元测试 + 绑定预算审计 | ✅ 本仓独有 |
| 覆盖面 | 角色/装备为主 | 装备/家具庭具(SGB 递归)/宠物坐骑/角色拼装/动画 | ✅ 本仓独有 |

## 双方一致不做(勿倒退)

以下功能 MeddleTools 未实现、本仓经 DXBC/真机证据确认**无依据实现**,保持现状:

- Toon/Sheen/Sphere 专用光照公式(MeddleTools shaders.blend 的 character 组无
  Toon 节点;本仓只保留 ramp 数据通道供 debug 视图);
- SSAO 伪造(runtime occlusion 不可得);
- 通用 RGB 顶点染色(ApplyVertexColorOn 的 RGB composition 无 verified formula);
- reflection 家族无 MeddleTools 模板,不能推断UV2 语义之外的内容。

## 改进优先级

### P1 — 疑似缺陷核实(小改动、可测试)

1. **UV scroll X 符号**:MeddleTools 显式取负(`[-sx, sy]`,
   node_mappings.py `UvScrollMapping`);本仓 `uv + scroll * t` 直用原值
   (shaders/surface.wesl `resolve_uv`)。需动图材质实测确认游戏方向。
2. **`bgprop.shpk` 家族映射**:现落 Unknown fallback
   (xiv-companion-data/model.rs `material_shader_family` 未列出);MeddleTools
   有专门组(colormap+normal+specmap.G/B→PBR)。补映射 + 简单分支。
3. **Stocking 强制 alpha=1**:MeddleTools character 组有此分支;本仓未找到对应
   特例,确认长袜 alpha 源是否正确。
4. **bgcolorchange 的 StainColor 染色**:MeddleTools 按 specmap.R 因子 lerp;
   本仓 bgcolorchange 并入 Bg 后染色是否生效待核实(已修的只有 alpha 遮罩)。

### P2 — 特性补齐(数据已就位,渲染侧缺失)

5. **iris 全套**:眼白 g_WhiteEyeColor、limbal ring(虹膜色 A)、左右分侧
   (顶点色 R/G)、IrisRing 渐变环 + 环自发光。视觉收益最大,数学简单。
6. **envmap 采样**(crystal/bg):`ModelTextureKind::EnvMap` 已导出;采样 +
   `studio_environment` 降级路径。
7. **water 的 wave/whitecap**:`WaterWhitecap` 贴图已导出;法线扰动 + whitecap
   混合 + refraction 色。

### P3 — 视觉近似与低频项

8. bg 地形 Voronoi UV jitter 与相机距离淡出(去平铺,常量已备)。
9. skin/hair 的 SSS 近似(MeddleTools:法线 B→0.03 起步的 subsurface;可做
   wrap-diffuse 近似或先挂 debug 视图)。
10. 呈现层评估:MeddleTools 对 >1 材质色做 Reinhard(HDRtoSDR),本仓线性直乘;
    与 PBR Neutral tone map 的交互需要一次统一评估。
11. lightshaft 的 TexAnim/TexU/TexV/TexRay 消费(双方都未实现,常量已备)。

## 维护约定

- 改动某家族渲染行为时,更新对应行并注明证据来源(节点组/DXBC/真机)。
- 新增 shader 家族支持时,先在"家族总览"加行。
- 与 `docs/weapon-render-review-plan.md`(未决问题)联动:P 项落地后从本表
  移到 review-history 的已验证记录。
