# 渲染器 × MeddleTools 着色器功能对照表

本文档逐家族对照本仓渲染器(`crates/xiv-companion-render/src/renderer/`)与
MeddleTools 的 Blender 着色器实现,记录每个功能点的实现状态与差距,作为渲染
改进工作的基线。创建于 2026-09-17,同日完成第一轮全量处置迭代(见文末
"处置记录")。

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
- 节点组与材质树已用无头 Blender 导出为文本对照(nodes + links + 默认值)。
- 本仓证据链:DXBC 审计与真机数据验证记录见
  `docs/weapon-render-review-plan.md` / `docs/weapon-render-review-history.md`,
  渲染管线结构见 `docs/weapon-render-pipeline.md`。

## 重要前提

**MeddleTools 不是游戏着色器的像素级复刻**,而是面向 Blender 预览/烘焙的
Principled BSDF 近似。多处公式是明牌近似:

- legacy GlossStrength→Roughness 用 `gloss/20` 缩放再变换(character.shpk 组
  "Legacy gloss to roughness approximation");本仓有 DXBC 证据的
  `exp2(-Gloss/15)`;
- mask.B(AO)直接近似为 Metallic("Diffuse AO Mask to Metalness approximation");
- IOR ≈ Roughness + 1.5("IOR Approx");
- 皮肤/头发的 Subsurface 为 Blender 艺术近似(游戏 DXBC 无对应光照项);
- 部分常量无语义名,按观察接线(`0x9A696A17` UV scroll、`0xB8ACCE58` fade 等)。

因此"对齐 MeddleTools"不是目标;它是**通道语义与常量消费的交叉验证源**,
以及缺失特性的灵感清单。本仓若有真机/DXBC 证据,以本仓证据为准。

## 状态标记

| 标记 | 含义 |
|---|---|
| ✅ | 已实现,语义与 MeddleTools 一致或更贴游戏 |
| 🟡 | 部分实现 / 近似实现,有明确改进空间 |
| ❌ | 未实现(双方中至少一方有实现或数据已就位) |
| ⛔ | 按证据策略不做(无双方实现证据,或属无游戏依据的发明) |

## 家族总览

| Shader 家族 (shpk) | 本仓家族枚举 | MeddleTools 节点组 | 总体状态 |
|---|---|---|---|
| skin.shpk | `Skin` | meddle skin.shpk | ✅(SSS 属 Blender 近似,⛔) |
| hair.shpk | `Hair` | meddle hair.shpk | ✅(明暗/挑染我们更细;SSS ⛔) |
| iris.shpk | `Iris` | meddle iris.shpk | ✅ 分侧/眼白/角膜环已实现 |
| charactertattoo.shpk | `CharacterTattoo` | meddle charactertattoo.shpk | ✅ |
| character*.shpk(6 变体) | `Character` + `Character*` | meddle character.shpk | ✅ |
| bg.shpk | `Bg` / `BgUvScroll` | meddle bg.shpk + UV Scale + detail/tile 组 | 🟡 UV scale ✅;jitter/fade 暂缓 |
| bgcolorchange.shpk | `Bg` | meddle bgcolorchange.shpk | ✅ 未染色路径等价;染色需运行态数据 |
| bgprop.shpk / bgcrestchange.shpk | `Bg` | meddle bgprop.shpk | ✅ 已并入 Bg 家族 |
| water.shpk / river.shpk | `Water` | meddle water.shpk | ✅ wave-as-normal + transparency 均已实现 |
| crystal.shpk | `Crystal` | meddle crystal.shpk | ✅ 等效(envmap 双方均无数学,⛔) |
| lightshaft.shpk | `LightShaft` | meddle lightshaft.shpk | ✅ 我们更完整(亮度发射) |
| scroll(常量簇) | `BgUvScroll`/scroll 路径 | meddle scroll | ✅ X 符号与 MeddleTools 一致 |
| decal(常量簇) | character decal 路径 | meddle decaluv | ✅ multiplier/offset/reversed 全有 |
| grass(专用几何) | 无 | grass_* 5 组 | ⛔ 超出模型预览范围 |

## 分家族明细

### skin.shpk

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| 肤色 × diffuse | Skin Color 混入 | `character_skin.rgb` 乘算(shading.wesl) | ✅ |
| Face/Body/BodyJJM 切换 | GetMaterialValue 开关 | 数据层 `MaterialSkinValueMode` 解析 + Face/Body 分流 | ✅ |
| 唇色 | Lip Color × Strength × 唇遮罩(face diffuse alpha 近似) | 同一近似(`character_lip` × samples.base.a) | ✅ 同一近似 |
| 发色/挑染(body JJM) | Hair/Highlights 按 mask 混入 | hair 分支处理(character_main/mesh) | ✅ |
| 面妆 decal | Decal Color × DecalTexture | character_decal + decal_uv(multiplier/offset/reversed) | ✅ |
| 面部 diffuse 寻址 | skin+Face → EXTEND | `Skin`+`Face` → `ClampToEdge`(data model.rs `PreparedTextureSamplingSet`) | ✅ 语义相同 |
| mask.R→Specular / mask.G→Roughness | 直连 | mask.g 进 roughness、mask.r 进 specular factor | ✅ |
| 法线/mask B → Subsurface | MapRange 0→0.03 "Kaj recommended SSS" | 无,且游戏 DXBC 无 SSS 光照项 | ⛔ 艺术近似,不发明 |

### hair.shpk

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| 发色 × 挑染 | Highlights 混入(因子=法线 B) | mask.G 挑染区域、mask.R 明暗(真机校准,更细) | ✅ |
| Alpha = 法线 alpha | 是 | 法线 alpha 透明 + obj/face 毛发整形 | ✅ |
| mask.B → Subsurface Weight | 直连 Principled | 游戏 DXBC 无对应项 | ⛔ 同上 |
| mask.R/G → Roughness/Specular | 直连 | 有 | ✅ |

### iris.shpk(2026-09-17 已实现)

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| 左右眼分侧 | 顶点色通道 >0.5 选择 left/right | 顶点色 G>0.5 选右眼色(character_left/right_iris) | ✅ |
| 眼白 g_WhiteEyeColor | mix(眼白, 虹膜色, mask.B) 后乘 diffuse | `iris_white_eye` uniform 同构(shading.wesl) | ✅ |
| 虹膜乘色 | diffuse × 虹膜色 | 同 | ✅ |
| Limbal ring 强度 = 虹膜色 A | 每侧强度混入 | `iris_ring_a.w`/`iris_ring_b.z`(拼装侧 alpha,缺省 1.0);角色页「渲染」区块提供强度滑杆覆盖 | ✅ |
| IrisRing 环带 + 自发光 | Gradient(Spherical)+GTE/LTE+fade 软环 | d=\|uv0−(0.5,0.5)\|,radius.xy ± fade.xy 软环,环色 × factor × 每侧强度 × g_IrisRingEmissiveIntensity | ✅ |
| 数据常量 | 5 个 g_IrisRing*/g_WhiteEyeColor CRC | 同 CRC 解析 + Meddle 缺省值(0.25 强度、0.158/0.174 半径等) | ✅ |

### charactertattoo.shpk

OptionColor 平铺(法线 B 因子)、Alpha = 法线 alpha——双方一致,✅。

### character*.shpk(装备/通用角色)

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| Compatibility:colortable × 贴图 | ramp 插值 | 全分辨率逐像素 compose(packed texel floor) | ✅ 更贴游戏 |
| legacy Gloss→Roughness | `gloss/20` 近似 | DXBC 证据 `exp2(-Gloss/15)` | ✅ 本仓有真证据 |
| AO→Metalness | 近似直连 | 皮肤族介电质处理 + mask 语义 | ✅ |
| Stocking 强制 alpha=1 | "If stocking, force alpha" | `CharacterStockings` → `PreparedAlphaSource::Opaque` + Opaque pass | ✅ 等价 |
| 玻璃/透明/scroll 变体 | 布尔分支 | 家族分支 + characterglass 审计测试 | ✅ |
| 双通道染色 | 无 | 完整 staining 体系(烘焙进 ramp) | ✅ 本仓独有 |

### bg.shpk / bgcolorchange / bgprop

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| 双 map(GetMultiValues)blend | Map0/1 × MultiBlendWeight | secondary base/normal/specular 采样 + blend | ✅ |
| specmap.G→Roughness、B→Metallic | 直连 | bg specular 通道语义(feature_params.w) | ✅ |
| ApplyVertexColor / vertex alpha | 开关 + 乘入 | 顶点色/alpha 通道 | ✅ |
| Emission | g_EmissiveColor × specmap.B | emissive 路径 | ✅ |
| 逐贴图 UV scale(g_ColorUVScale 等三常量 × map0/1) | UV Scale 组 6 路 scale | `g_ColorUVScale`/`g_NormalUVScale`/`g_SpecularUVScale` CRC 解析 → `uv_scale_a/b/c` uniform → 六个采样点直乘(2026-09-17) | ✅ |
| 相机距离淡出 + Voronoi UV jitter | UV Scale 组(MapRange×Camera × Voronoi crossfade) | 未实现;仅 jitter 常量启用时可见(缺省 0x88A3965A=off),游戏噪声数学未知,MeddleTools 为 Blender Voronoi 近似 | ⛔ 暂缓(见处置记录) |
| bgcolorchange StainColor 染色 | colormap × (StainColor²) (colormap alpha 门控) | 未染色时 StainColor=白 → 等价;染色需运行态 StainColor(家具染色 ID,无离线来源) | ✅ 未染色等价;染色预览留待 housing 数据 |
| bgprop/bgcrestchange 家族 | 专门组(同 bg 通道语义) | 已并入 `Bg` 家族(model.rs `material_shader_family`) | ✅(2026-09-17) |

### water.shpk / river.shpk

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| WaveMap 作法线源 | wave map → Normal_Fix → Normal | `effective_normal_texture`:Water 家族优先 `water_wave_texture`(params.rs) | ✅ |
| 深水底色 | deep color 底 | feature_params.y 开关换 water_deep_color | ✅ |
| Alpha = g_Transparency | 直连 | `MaterialTransparency` alpha 源 + transparency<1 时 Transparent pass | ✅ |
| Whitecap / Refraction 色 / IOR | 声明输入,组内无数学 | 贴图已导出、uniform 预留,不采样 | ⛔ 双方均无数学 |

### crystal.shpk

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| 基础(colormap + normal) | 是 | 家族识别,基础分支 | ✅ 等效 |
| EnvMap | 输入声明,组内无数学 | 数据层已导出,不采样 | ⛔ 双方均无数学(同 reflection 家族原则) |

### lightshaft.shpk

Sampler0/1 按 vertex_color.B 混合 × g_Color ✅;亮度加权发射 ✅(更完整);
g_TexAnim/TexU/TexV/TexRay **双方均无数学**,常量已备 ⛔(见处置记录)。

### 基础设施(非家族)

| 功能点 | MeddleTools | 本仓 | 状态 |
|---|---|---|---|
| 蒙皮 | glTF 蒙皮(Blender 原生) | GPU joint storage + WGSL 蒙皮,PAP 动画播放 | ✅ |
| 透明排序 | Blender 原生 | CPU 逐三角形排序 + transparent/glass/additive 管线 | ✅ 本仓独有 |
| 抗锯齿 | Blender 采样 | 4x MSAA(HDR resolve) | ✅ |
| 后处理 | 无(Reinhard 散在材质里) | HDR Rgba16Float + bloom + PBR Neutral tone map | ✅ 本仓独有 |
| 双面渲染 | backface culling | 逐批次 backface 管线变体 | ✅ |
| 法线重建 Z | Normal_Fix 组 | WGSL `decode_normal` | ✅ |
| 调试视图 | 无 | 30 种 debug 模式 + UnsupportedInputs 诊断色 | ✅ 本仓独有 |
| 测试基建 | 无 | 42 张 native GPU 快照 + 90 着色器/单元测试 + 绑定预算审计 | ✅ 本仓独有 |
| 覆盖面 | 角色/装备为主 | 装备/家具庭具(SGB 递归)/宠物坐骑/角色拼装/动画 | ✅ 本仓独有 |

## 双方一致不做(勿倒退)

- Toon/Sheen/Sphere 专用光照公式(无节点证据;本仓只保留 ramp 数据通道);
- SSAO 伪造(runtime occlusion 不可得);
- 通用 RGB 顶点染色(ApplyVertexColorOn 的 RGB composition 无 verified formula);
- reflection 家族 envmap 采样(MeddleTools 无模板)。

## 处置记录(2026-09-17 迭代)

首轮 parity 迭代将原 P1/P2/P3 清单全部闭环。逐项处置与依据:

### 已修复(代码变更)

1. **`bgprop.shpk`/`bgcrestchange.shpk` 家族映射缺失** → 并入 `MaterialShaderFamily::Bg`
   (与 MeddleTools bgprop 组相同的 PBR 通道消费);附家族映射测试。
2. **iris.shpk 全套**(原 P2-5):分侧/眼白/limbal/角膜环 + 5 个材质常量解析,
   见上表明细;89→90 个渲染测试含 iris 回归断言,42 张 GPU 快照逐位一致。

### 核实为"已实现"(文档纠偏,无代码变更)

3. **UV scroll X 符号**(原 P1-1):数据层 `composed_material_uv_scroll`
   早已输出 `[-sx, sy, -sz, sw]`,与 MeddleTools `UvScrollMapping` 一致。
4. **Stocking 强制 alpha**(原 P1-3):`CharacterStockings` 已分配
   `PreparedAlphaSource::Opaque` + Opaque pass,与 MeddleTools force-1 等价。
5. **water wave 法线**(原 P2-7):`effective_normal_texture` 对 Water 家族
   优先 wave 贴图;`g_Transparency` → MaterialTransparency alpha 源 +
   Transparent pass。已超 MeddleTools(其 whitecap/refraction 无数学)。
6. **skin 面部寻址**:`Skin`+`Face` → ClampToEdge,与 MeddleTools EXTEND 语义相同。

### 新实现(原 P3-8 拆分)

7. **bg 逐贴图 UV scale**:三个常量 CRC 解析 + uniform + 六采样点直乘,
   仅 Bg 家族激活(其余乘 1 无操作)。

### 按证据策略闭环(⛔,无代码变更)

8. **SSS 近似**(原 P3-9):MeddleTools 的 mask.B→Subsurface 是 Blender
   艺术近似;游戏 DXBC 无 SSS 光照项。与 toon/sheen/sphere 同类,不发明。
9. **envmap 采样**(原 P2-6):MeddleTools crystal 组声明 envmap 输入但无数学,
   与 reflection 家族同原则;数据层导出保留,待有游戏证据再实现。
10. **bg 相机淡出 + Voronoi jitter**(原 P3-8 余项):仅 jitter 常量启用时可见
    (缺省关闭),游戏噪声数学未知,MeddleTools 为 Blender Voronoi 近似;
    预览语料中几乎不出现。常量语义已记录,待真机对照数据再实现。
11. **lightshaft TexAnim/TexU/TexV/TexRay**(原 P3-11):双方均无数学,常量
    已备;待游戏行为证据。
12. **HDRtoSDR 评估**(原 P3-10):MeddleTools 的逐通道 Reinhard 用于把
    >1 材质色压进 Blender 显示域;本仓管线已有 scene-linear HDR + PBR
    Neutral tone map,职责等价且更系统。不引入材质级 Reinhard。

### 后续触发条件(何时重新打开 ⛔ 项)

- envmap/reflection:拿到游戏 crystal/bg 的 envmap 采样语义证据(反编译或
  真机对照);
- jitter/fade:拿到地形材质真机对照或游戏噪声实现证据;
- TexAnim:真机录像对照光柱贴图动画方向与周期;
- 家具染色:预览引入 housing 染色数据源后,按 `colormap × StainColor²`
  (colormap alpha 门控)实现。

## 维护约定

- 改动某家族渲染行为时,更新对应行并注明证据来源(节点组/DXBC/真机);
- 新增 shader 家族支持时,先在"家族总览"加行;
- 重新打开 ⛔ 项时,在"处置记录"追加条目并更新"后续触发条件";
- 与 `docs/weapon-render-review-plan.md`(未决问题)联动:落地项移入
  review-history 的已验证记录。
