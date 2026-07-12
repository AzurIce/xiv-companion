# 武器模型渲染管线调研与设计

本文记录 `xiv-companion` Web 武器预览从游戏数据到 GPU 渲染的完整管线、已知坑点、当前实现策略和后续扩展方向。调研参考了本地游戏 SqPack、Physis、Meddle 运行时导出逻辑，以及实际问题样本：逗猫之幻梦、浪漫之幻梦、冬雪之幻梦、绝境系列双手武器等。

## 1. 数据入口：Item EXD → WeaponCatalog

武器目录来自 `Item` EXD：

- `EquipSlotCategory` 判断是否武器：`1` 主手、`2` 副手、`13` 双手主手、`14` 双持主手。
- `Model{Main}` / `Model{Sub}` 是武器模型 ID。

同一个 `WeaponCatalogPackage` 还会读取 `Stain` EXD，保存具名染剂的 ID、名称、BGR 色值、UI RGBA、shade、sub-order 和 metallic。该颜色只用于选择器色块；实际材质覆盖仍由 STM 完成。

### 1.1 武器 Model packed ID 字段顺序

对武器而言，CSV 中形如：

```text
2001, 102, 1, 0
```

应解释为：

```text
model_id = 2001
body_id = 102
variant_id = 1
```

即路径首选：

```text
chara/weapon/w2001/obj/body/b0102/model/w2001b0102.mdl
```

不是 `w2001 b0001 v0102`。之前错误把第二/第三段反了，导致 `浪漫之幻梦` 加载到完全错误的 body。

当前实现：`PackedModelId::from_raw` 固定使用 `(model_id, body_id, variant_id)`。

## 2. 模型路径解析

武器模型路径格式：

```text
chara/weapon/w{model_id:04}/obj/body/b{body_id:04}/model/w{model_id:04}b{body_id:04}.mdl
```

候选 body 顺序：

1. `body_id`
2. `variant_id`（兼容部分旧数据）
3. `1`
4. `101`
5. `201`

候选读取仍按顺序探测。主模型全部失败时返回错误；可选副手/子模型失败时不阻塞主模型，但会写入
`WeaponModelData.loadDiagnostics`，记录 role、model、候选路径、missing/read/parse 状态和错误原因。

## 3. MDL → Mesh

使用 Physis 解析 MDL：

- 取 raw LOD0 mesh range，而不是只依赖 Physis 暴露的普通 LOD part。
- 覆盖 normal/water/shadow/terrainShadow/verticalFog，以及 extra LOD 的 lightShaft/glass/materialChange/crestChange。
- 每个 mesh/submesh range 转成 `WeaponModelMesh`，并保留 submesh index、attribute mask/name 与 bone range 信息。
- 数据层保留：position、blend weights/indices、normal、uv0-uv3、bitangent、secondary normal/bitangent、color0/color1、flow0/flow1、index、material_index、mesh category、submesh info、bone table，以及按 output vertex 保存的 sparse shape position/normal delta。
- mesh category 会映射成 `ModelMeshDrawRole`，并和 submesh attribute metadata 一起进入第一版 `PreparedModel` / `PreparedMesh`，作为 renderer-friendly 的第一步 prepared draw role。若调用方显式提供 `PreparedModelOptions.enabledAttributeMask`，prepared 阶段会按 submesh 所需 attribute mask 计算 visibility；默认离线模式不猜运行时 enabled mask。
- ignored phantom snapshot 的 `model-summary.json` 会输出 mesh category、submesh attributes、bone table、shape 影响摘要、shape target/delta count，并链接 full MDL metadata JSON。MDL `BaseIndicesIndex` 按 mesh-relative index-buffer position 解释；submesh remap 对不同 target signature 的索引出现位置拆点。`ModelRenderer::new_with_prepared_options` 可用显式 `enabledShapeMask` 累加 active target delta；默认 renderer 仍使用 base geometry。native snapshot options 同样可传 preparation options，42697 base/bit0 回归验证了真实 GPU 输出差异。Web 渲染面板会为含 shape 的模型显示 Base/shape-name 选择，并通过 canvas key 重建对应 GPU buffers。静态 MDL 不含 runtime shape name/bit mapping，离线 table-order bit 约定不代表游戏默认状态。

注意：renderer 当前 GPU 顶点格式已上传 position、normal、uv0-uv3、bitangent、color0/color1、secondary normal/bitangent、flow0/flow1，WGSL `VertexInput` 也已声明对应 location；`PreparedMaterial` 已记录 UV source、per-role scroll mask 和结构化 flow mode。只有 `CategoryFlowMapType=Flow` 且 mesh 有 `flow0` 时 `usesFlow` 才启用，fragment shader 会把 `flow0.xyz` 正交化后作为 primary normal tangent；`characterstockings.shpk` 普通 surface 会按最终 alpha=1 选择 opaque pass，但 runtime skin material 仍不可用并由 `runtimeSkinMaterial` 标记；`charactertattoo.shpk` 按 MeddleTools 节点从 normal texture Alpha 取透明度，颜色所需的 OptionColor/DecalColor 仍分别由 `runtimeOptionColor`/`runtimeDecalColor` 标记，不使用静态猜测；`flow1`、secondary normal/bitangent 和 color1 仍待后续 family-specific 逻辑。

注意：顶点色在 FFXIV 角色/武器 shader 里不一定是纯颜色，常参与遮罩/alpha/材质调制；当前 renderer 只作近似 tint 使用。

## 4. MTRL 路径解析

一般由模型路径推导材质目录：

```text
chara/weapon/w####/obj/body/b####/material/v####/*.mtrl
```

候选 version：

1. `variant_id`
2. `body_id`
3. `1`
4. `101`
5. `201`

### 4.1 副手引用主手材质

很多双手武器的 sub 模型在另一个 `w####` 目录，但 MDL 材质名仍引用 main 的材质文件，例如：

```text
sub model: w0387b0001.mdl
material name: /mt_w0337b0001_a.mtrl
actual path: chara/weapon/w0337/obj/body/b0001/material/v0001/mt_w0337b0001_a.mtrl
```

因此路径解析需要从材质文件名 `mt_w####b####_*.mtrl` 反推出真实材质根目录。

当前实现：`weapon_material_candidate_paths` 同时尝试模型自身根目录和材质文件名指定的根目录。

## 5. 贴图角色识别

MTRL 贴图需要分类为：

- BaseColor：`*_d.tex`、`*_base.tex`、`*_a.tex`、albedo/diff/base 等。
- Normal：`*_n.tex`、`*_norm.tex`。
- Mask/Specular：`*_m.tex`、`*_mask.tex`、`*_s.tex`。
- Emissive：`*_e.tex`、emissive/emit。
- Index：`*_id.tex` / `g_SamplerIndex`。

`_id.tex` 不是颜色图，不能当 mask 或 diffuse 直接采样。

当前实现：

- 优先解析 MTRL sampler records，并结合 `.shpk` resource parameter name 判定 sampler role。
- `.shpk` 名称缺失时使用 known CRC 表兜底；再结合文件名后缀分类。
- `g_SamplerEnvMap` 保留为独立 Environment role，prepared sampling 为 Non-Color/Linear/Repeat；`crystal.shpk` 会分类为 Crystal family，binding 存在时输出 `usesEnvironmentMap=true` 和 `environmentMapping=true` unsupported。当前没有可靠的反射坐标与混合公式，因此数据可审计但尚不进入 WGSL。
- `g_SamplerMulti` binding 存在时会输出 `multiMapInterpretation=true` unsupported；它与 `detailArray` 是否缺少共享数组独立，避免 arrays 完整时误报 MultiMap 已被正常着色消费。
- `GetSubColor` material key 会解析为 None/Face/Hair/Unknown 并保留到 prepared；characterOcclusion 或显式 Face/Hair 会输出 `runtimeSubColor=true`，因为离线武器 loader 没有 Meddle composer 的 customize color buffer。
- `characterreflection.shpk` 当前输出 `characterReflection=true` unsupported；没有可靠节点/资源证据时继续使用 generic character approximation，不复用 Environment/sphere/specular 猜测 reflection。
- 路径后缀 `_id` / `g_SamplerIndex` 会识别为 `Index`，不会当 mask 或 diffuse 直接采样。
- material debug 会输出 sampler 的 `textureUsageName` 和 `kindSource`，用于判断来源是 `.shpk` resource name、known CRC 还是 unknown。

### 5.1 共享 tile/detail texture arrays

MeddleTools 使用四个固定共享资源：

```text
chara/common/texture/tile_norm_array.tex
chara/common/texture/tile_orb_array.tex
bgcommon/nature/detail/texture/detail_d_array.tex
bgcommon/nature/detail/texture/detail_n_array.tex
```

character family 在 `usesTile` 时加载前两张，bg family 在 `usesDetail` 时加载后两张。四张都按 Non-Color + Closest/Repeat 处理。

FFXIV TEX header offset 14/15 分别是 `MipLevels:u8` 与 `ArraySize:u8`。当前 Physis 将它们合并为 `mip_levels:u16`，且普通 `to_rgba()` 只按 `depth` 解码第一个 slice。本仓读取 header byte 15，临时按 array size 解码连续 mip0 slices，并输出与 MeddleTools 一致的 vertical atlas：

```text
atlas_height = array_layer_height * array_size
```

`ModelTexture` 保留 `arraySize` 和 `arrayLayerHeight`；`ModelMaterial.textureArrays`、`PreparedTextureBindings` 与 `PreparedMaterial.resourceAvailability` 保留四个数组索引、加载错误和成对完整性。model-level preparation 还会输出每组数组的结构化 status 与 layer count，区分 missing binding/texture、wrong kind、non-canonical shared binding、invalid layout、incompatible pair 与 ready；standalone material preparation 因没有 texture 集合保持 unvalidated。真实 SqPack 验证结果：character arrays 为 `64x4096 / 64 layers`，bg detail arrays 为 `256x8192 / 32 layers`。

renderer 会把 tile normal/ORB 横向合并成一张 GPU pair atlas，把 detail diffuse/normal 合并成另一张，避免四个独立 binding 使 fragment sampled texture 数超过常见 WebGPU 16 张限制。WGSL 使用 nearest + repeat，并按以下规则选层：

- character tile：优先使用逐像素 `TileProperties.r * 64`，没有 ColorTable tile map 时回退 `g_TileIndex`；结合 TileMatrix 与 `g_TileScale` 生成层内 UV。
- bg detail：`g_DetailID` / `g_MultiDetailID` 取整并 clamp 到数组范围，color/normal 分别使用对应 UV scale；GetMultiValues 按 vertex alpha 在 primary/multi 层之间插值，Single 固定 primary。

tile normal 会与 primary tangent-space normal 组合，贡献权重为采样 normal Alpha × `TileAlpha`。MeddleTools `chara_detail_blend` 只消费 tile ORB Blue，将其作为黑色到 base color 的直接 darkening factor；ORB R/G/Alpha 不参与着色，不再猜测 AO、roughness 或 specular。detail diffuse 以 0.5 为中性值，与 detail color 组合；detail normal 使用 RG 重建 Z，再按 detail/multi-detail normal scale 处理，随后与 diffuse 一样按 MultiBlendWeight 选择 primary/multi，而不是同时叠加。model-level prepared 只有在数组实际验证为 Ready 时才清除 `unsupportedInputs.tileArray/detailArray`；renderer uniform 直接消费 prepared layer count/ready flag，GPU pair atlas 创建仍保留防御性验证并在失败时使用中性纹理。45052 的真实 summary 为 tile Ready/64 layers、detail MissingBindings。

验证边界：45052 已用 final/tile-normal/tile-ORB snapshot 验证逐像素选层；当前没有真实 bg 武器样本，detail 混合权重仍是保守实现，待样本校准。

## 6. ColorTable + `_id.tex` 烘焙

许多当前武器没有传统 diffuse/base 贴图，颜色来自：

- MTRL 内 32 行 Dawntrail ColorTable。
- `_id.tex` 逐像素选择 ColorTable 行。

实际编码：

- R 通道选择 16 个行对，取值近似 0、17、34、...、255。
- 行对 `i` 对应 ColorTable 行 `2i` 和 `2i+1`。
- G 通道在两行之间混合。

当前实现：

1. 读取 `_id.tex` RGBA。
2. 按 R/G 查 ColorTable。
3. 烘焙出 diffuse、specular、material-properties、tile、sheen、sphere、tile-matrix、emissive 贴图：
   - diffuse RGB 为 sRGB，Alpha 固定不透明；`TileAlpha` 是 tile 属性，不作为材质透明度。
   - specular RGB 为 sRGB，Alpha 保存 ColorTable `Anisotropy`。
   - material-properties 为线性 unorm，通道为 metalness / roughness / gloss strength / specular strength。
   - tile/sheen/sphere/tile-matrix 与 MeddleTools 的 extra ramps 对齐；Dawntrail raw TileIndex/SphereIndex 均先从 half bits 解码，再分别按 0..64/0..255 写入 UNORM。low-level material debug 仍保留原始 `u16`。tile-matrix 的 UU/UV/VU/VV 同时保留 float channels，并以 `Rgba32Float` 上传，避免 RGBA8 截断负 skew 和大于 1 的 repeat。
4. 若 ColorTable 有 emissive，则额外启用 emissive texture。

当前实现同时支持 Dawntrail 32 行和 Legacy 16 行 ColorTable。renderer 已消费 diffuse/base、specular、material-properties、emissive，并已把 tile、sheen、sphere、tile-matrix extra maps 绑定进 WGSL；tile properties 还会驱动共享 tile normal/ORB atlas 的逐像素 layer selection。45059 真实材质包含 raw `SphereIndex=0x4000`，语义 half 值为 `2.0`；修正后的 sphere-properties 贴图平均 R 为精确的 `2/255`，验证不再把 raw bits 错误烘焙为饱和 1。TileMatrix binding 使用 unfilterable `Rgba32Float` 与 non-filtering nearest sampler，float payload 无效时逐通道回退 RGBA8/identity；Blender `tile_select` 证明它只形成 tile UV vector，WGSL 已删除旧的 matrix-delta specular。互补 synthetic fixtures 证明 patterned tile 在 scale 1/2 下输出不同，而 uniform tile 在同样矩阵变化下逐像素一致。独立 synthetic fixture 还确认只改变 sheen rate/tint/aptitude 或 sphere index/mask 会分别改变最终画面，证明两个 ramp 的 binding 与当前近似消费没有静默失效；该结果不代表近似公式等价于游戏 shader。完整 MeddleTools 节点图仍未复刻。

## 7. 材质模式

当前抽象为三类：

```rust
WeaponMaterialRenderMode::Opaque
WeaponMaterialRenderMode::Transparent
WeaponMaterialRenderMode::Glass
```

### 7.1 Opaque

普通角色/武器材质。使用 base/ColorTable、normal、mask、emissive 近似 Blinn-Phong 渲染。
`g_NormalScale` 会从 shader package default 与 material override 中解析为 `normalScale`，并用于缩放 tangent-space normal map 强度。
`g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale` 也已解析进材质数据和 renderer uniform；detail pair atlas 可用时 WGSL 会采样真实 detail normal，缺图时才把非默认 detail scale 作为 primary normal 的受限 fallback。
`g_TileIndex`、`g_TileAlpha`、`g_TileScale` 已解析进材质数据和 renderer uniform；WGSL 已绑定 tile pair atlas，按 ColorTable tile properties 或 `g_TileIndex` 选层，并结合 TileMatrix/TileScale 采样。`TileAlpha` 与 tile-normal Alpha 相乘控制法线贡献，不控制 ORB Blue 的 base-color darkening，也不作为材质透明度。
`g_ToonIndex`、`g_ToonLightScale`、`g_ToonLightSpecAperture`、`g_ToonReflectionScale`、`g_ToonSpecIndex`、`g_SheenRate`、`g_SheenTintRate`、`g_SheenAperture`、`g_SphereMapIndex` 已解析进材质数据、phantom summary 和 renderer uniform；默认值分别为 `0/2/50/2.5/约 0/0/0/1/0`。prepared `usesToon` 对 character family 默认启用；WGSL 用 `ToonLightScale` 缩放 NdotL，再与平滑双段 diffuse 以 35% 混合，用 `ToonLightSpecAperture/ToonSpecIndex` 生成 40% spec band，并以 `ToonReflectionScale/2.5` 调整 rim；index 只做受限阈值偏移。sheen/sphere 常量继续作为 ColorTable extra ramp 之外的保守高光/反射输入。MeddleTools 源码、`shaders.blend` 的 character group socket/node 均没有 toon 语义，也没有 toon texture/sampler 映射；当前 24 组 phantom 原始参数也全部为默认值，因此这是解析式近似，不伪造 lookup texture。45052 默认参数只温和调整明暗层次，`XIV_PHANTOM_TOON_OVERRIDE=1` 会覆盖为 `3/3/12/5/4` 以验证亮部、spec 与 rim 方向，默认 snapshot 已恢复。
`g_DetailID`、`g_MultiDetailID`、`g_DetailColor`、`g_MultiDetailColor`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 已解析进材质数据和 renderer uniform；WGSL 已绑定 detail pair atlas，按两个 ID 选层并分别采样 diffuse/normal。`detailParams.z` 记录 bg GetMultiValues，启用时用 vertex alpha 统一 mix primary/multi diffuse、normal 和 fallback tint；synthetic WGPU fixture 已验证 alpha 0/1 切层。当前缺少真实 bg 武器样本，detail 对 base 的最终 influence 仍待校准。
`g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_MultiEmissiveColor` 已解析进材质数据、phantom summary 和 renderer uniform；WGSL 当前把 `g_DiffuseColor` 作为 base tint，把 `g_MultiDiffuseColor` 作为 mask-gated 的保守 base tint 补充，把 `g_EmissiveColor` 作为附加发光，并在 mask/material 通道存在时保守加入 `g_MultiEmissiveColor`。完整 multi map 通道解释仍未实现。
`g_OutlineColor`、`g_OutlineWidth`、`g_SpecularColorMask`、`g_SSAOMask`、`g_TextureMipBias`、`g_ShadowPosOffset` 已解析进材质数据、phantom summary 和 renderer uniform；`g_SpecularColorMask` 会调制高光颜色/强度，`g_SSAOMask` 会保守调制环境底光。outline 已从 fragment rim fallback 升级为独立 inverted-hull geometry pass：prepared `usesOutline` 只对 character family、正有限宽度启用；renderer 在 opaque/cutout 与 dither depth 之后、透明颜色之前，沿 object-space normal 扩张顶点，剔除 front faces，以纯 `g_OutlineColor` 绘制且不写 depth，lightshaft 与 crest fallback 不参与。Meddle 只确认 outline 参数适用于 character/hair/iris/skin 系列且默认宽度为 0，MeddleTools 没有 outline node/bake 行为；当前 24 组 phantom summary 的 outline width 也全部为 0。当前直接使用模型单位并 clamp 到 `0.1`，不复刻未知的游戏宽度换算，也不处理 alpha-cutout 纹理内部边缘。`XIV_PHANTOM_OUTLINE_WIDTH=0.02` 的 45052 synthetic 红色轮廓已验证 geometry pass，默认 snapshot 已恢复。普通 base/emissive、normal、mask/specular/material-properties 上传现会生成完整 mip chain：sRGB RGB 在线性空间平均，normal 向量平均后重新归一化，其余 data/alpha 线性平均；linear sampler 使用 trilinear mip filtering。WGSL 只对 Meddle 声明适用的 character family 将 clamp 到 `[-16,15.99]` 的 `g_TextureMipBias` 应用于上述纹理和 dither alpha 采样。ColorTable extra maps、tile/detail pair atlas 与 TileMatrix 不应用该参数。shadow offset 仍未实现。
`g_GlassIOR`、`g_GlassThicknessMax` 已解析进材质数据、phantom summary 和 renderer uniform；WGSL 当前把非默认 IOR/thickness 用作 glass tint、specular 与 rim fresnel 的轻量调节，不改变固定 glass opacity 或折射。
`g_AlphaAperture`、`g_AlphaOffset`、`g_ShadowAlphaThreshold` 已解析进材质数据、phantom summary 和 renderer uniform；WGSL 当前只在 aperture/offset 非默认时对非 glass/lightshaft alpha 做受限 shaping，`g_ShadowAlphaThreshold` 仍未驱动 shadow pass。

`PreparedAlphaSource` 会区分普通 character 的 `NormalBlue` 与 tattoo 的 `NormalAlpha`。renderer uniform 分别编码为 `2` 和 `4`，主颜色 pass 与 `DrawDepthMode_Dither` depth pass 都采样 normal texture 的 B/A 并按 prepared source 选择；tattoo Alpha 直接使用 normal A，不扩展为普通 character/glass/transparency 的规则。
`g_UVScrollTime` / `0x9A696A17` 已按 MeddleTools `UvScrollMapping` 转换成 UV0/UV1 scroll multiplier 并进入 renderer uniform。`bguvscroll.shpk` 已单独分类：Color/Normal/Specular Map0 使用 UV0Scroll，独立 Map1 roles 使用 UV1Scroll；`GetMultiValues` 下 WGSL 按 vertex alpha 混合两套 color/alpha/normal/specular。三张 Map1 仅在该 family 复用 tile/sheen/sphere 物理 binding，sampled texture 数保持 15；AlphaMulti variants 仍显式 unsupported。Web RAF 驱动和 native snapshot 时间 0 的稳定行为保持不变。
`lightshaft.shpk` 的 `g_Color`、`g_TexAnim`、`g_TexU`、`g_TexV`、`g_Ray` 已解析进材质数据和 renderer uniform；LightShaft draw role 会启用保守 additive tint、`g_TexAnim.xy` UV 动画、`g_TexU/V` 仿射 UV 与 `g_Ray` 强度近似，尚未复刻完整节点语义。
`g_Transparency` 已解析进材质数据和 phantom summary；MeddleTools water group 证明它直接连接 Alpha。prepared 对 water 使用 `MaterialTransparency` alpha source，小于 1 时选择 Transparent pass；WGSL 直接输出该值，不乘 vertex/base alpha 或 character shaping。`g_WaterDeepColor` 直接作为 water base，primary `g_SamplerWaveMap` 复用 normal binding并按 R/G 解码；refraction/whitecap colors 与 WaveMap1/WhitecapMap 已保留在 model/prepared，但因 MeddleTools 当前没有输出连接而不参与着色。三种 water sampler 已脱离 `Other`，不会再被 fallback 当成 base texture。

### 7.2 Transparent

base texture alpha < 250 的材质。使用 alpha blending，关闭 depth write。

### 7.3 Glass

`shader_package_name` 包含 `glass`，例如 `characterglass.shpk`。

`冬雪之幻梦` 的雪球外壳即此类。调研发现：

- MTRL 是 `characterglass.shpk`。
- 透明度并不直接由 TileAlpha 给出，而是 glass shader 语义控制。
- ColorTable `TileAlpha` 属于 tile 属性；即使存在变化，也不能直接当作 diffuse/material alpha。

当前实现做简化 glass：

- render mode 标记为 `Glass`。
- prepared alpha source 使用 normal texture Blue；ColorTable baked base 保持 opaque，不再写入固定 glass alpha。
- `g_GlassIOR` / `g_GlassThicknessMax` 已解析并进入 uniform，当前只轻量调节 tint、specular 与 fresnel，不驱动 opacity、折射或真实厚度传输。
- WGSL 使用较亮的 transmission tint、normal-B alpha 与 fresnel/specular；`EnableLightingOff` 的 character transparency 可绕过 surface lighting。
- 进入透明 pass。

真实 45059 `characterglass.shpk` 样本只有 normal/mask/index 与 ColorTable 派生 base，没有独立 base texture；MTRL 覆盖 `DrawDepthMode_Dither`，`g_GlassIOR=1`、`g_GlassThicknessMax=0`，normal Blue 实测范围为 `57..255`。改用 normal-B alpha 后，雪景玻璃罩不再是灰暗球体，内部景物保持可见且表面纹理可观察。

`DrawDepthMode` 与 `EnableLighting` 已进入 material/prepared policy。renderer 会在 opaque/cutout 后、透明颜色 pass 前重绘 `DrawDepthMode_Dither` 的 Transparent/Glass batch，使用与颜色 pass 相同的 prepared alpha source 和稳定 4x4 屏幕空间有序阈值，只写 depth、不写两个颜色 target；透明颜色 pass 仍按 mesh center 排序、alpha blend 且不写 depth。Meddle `Names.cs` 只能确认该 material key 适用于 `characterglass.shpk` / `charactertransparency.shpk`，没有暴露游戏使用的抖动矩阵或噪声公式；MeddleTools 也不实现运行时 depth pass，因此当前阈值公式是保守近似。该行为与覆盖更多 shader family 的 scene key `ApplyDitherClip` 分开处理。

`GlassBlendMode` 是 scene key而非 MTRL material key。Meddle 只提供默认 `GlassBlendMode_Mul` 与可选 `GlassBlendMode_Add` 的名字/CRC，MeddleTools 没有对应节点或 bake 行为，因此不能从静态材质恢复场景选择，也没有证据把 Mul 直接解释为某个硬件 blend equation。renderer 已提供显式 `ModelGlassBlendMode::{Multiply, Additive}` scene option：默认 Multiply 保持当前 alpha-blend glass 近似以避免改变现有结果，Additive 只让 Glass batch 选择 additive pipeline；Web 的 Glass 下拉框、`ModelRenderOptions` 和 phantom snapshot 的 `XIV_PHANTOM_GLASS_BLEND=additive` 共用该入口。45059 的 Additive 验证会明显提高玻璃亮度，证明 pipeline 分派生效，也表明它仍只是硬件 additive 近似。真实乘法/scene-color composition 仍需游戏 shader 或运行时捕获证据。

这不是完整游戏 glass shader，目前只能显示内部模型并提供近似透明外壳。

## 8. GPU 渲染管线

当前 WebGPU/WGSL 渲染流程：

1. CPU flatten meshes，合并 vertex/index buffer。
2. 每个 material 建 bind group：
   - material uniform
   - base texture
   - color sampler（repeat/linear，用于 base/specular/emissive）
   - normal texture
   - mask texture
   - emissive texture
   - material-properties texture
   - specular texture
   - data sampler（repeat/linear，用于 normal/mask/material-properties）
   - tile-properties texture
   - sheen-properties texture
   - sphere-properties texture
   - tile-matrix texture
   - nearest data sampler（repeat/nearest，用于 ColorTable extra maps）
   - tile normal/ORB pair atlas
   - detail diffuse/normal pair atlas
3. scene pass：
   - renderer 先计算第一版 `PreparedModel`，为每个 mesh 记录 draw role、main-pass 可见性、submesh attribute metadata、attribute visibility 和 `PreparedMaterial`；`PreparedMaterial` 会把材质 alpha/render mode 与 mesh draw role 合成 `Opaque`、`Cutout`、`Transparent`、`Glass`、`AdditiveLightShaft` 五类 prepared render pass，并记录第一版 shader family 分类、texture bindings、texture sampling policy、material feature flags、UV source 和 unsupported/runtime-only 输入摘要。
   - opaque pipeline：写 depth，绘制 `Opaque` batch。
   - cutout pipeline：写 depth，绘制 `Cutout` batch；当前仍由 WGSL alpha test discard。
   - transparent pipeline：alpha blending，不写 depth，绘制 `Transparent` batch。
   - glass pipeline：alpha blending，不写 depth，绘制 `Glass` batch；仍沿用现有 glass 近似参数。
   - dither depth prepass：opaque/cutout 后、transparent/glass 颜色 pass 前，仅重绘 `DrawDepthMode_Dither` batch；独立双面/背面剔除 pipeline 使用与颜色 pass 一致的 base-alpha/normal-B prepared source 和稳定 4x4 屏幕空间阈值，开启 depth write，并把两个 MRT 的 color write mask 设为空。Meddle/MeddleTools 没有提供游戏公式，因此该 pass 只解决透明表面的深度覆盖近似，不代表复刻 `ApplyDitherClip` scene key。
   - additive pipeline：additive blending，不写 depth，绘制 `AdditiveLightShaft` batch。
   - opaque/cutout/transparent/glass/additive 各有 backface 与 culled pipeline，按材质 `render_backfaces` 选择。
   - `PreparedMesh` 先过滤非 surface：shadow、terrainShadow、verticalFog 不进入当前渲染；lightShaft 不作为普通 surface，但会分类为 `AdditiveLightShaft` 并保留到 additive pass；materialChange/crestChange 已拆为独立 draw role 并暂时保留在主 pass；mesh category glass 会强制进入 `Glass` prepared pass。
4. bloom pass：从 bright attachment 提取高亮并 blur。
5. compose pass：scene + bloom 输出到 canvas。

### 8.1 Runtime on-render fallback

Meddle `OnRenderMaterialUtil` 表明 weapon decal 与 FC crest 来自运行时 `OnRenderMaterial`，不是静态 MTRL sampler。运行时没有 decal/crest 时，游戏路径使用透明纹理；materialChange 则应回落到基础材质。

当前 prepared 层已表达：

- `CrestChange`：`TransparentTexture` fallback，同时标记 `decalOrCrest` unsupported。
- `MaterialChange`：`BaseMaterial` fallback；renderer 继续使用基础材质，不再标记 `runtimeMaterialChange` unsupported。

renderer final 模式会让 `CrestChange` 进入 transparent pass 并 discard，避免透明 fallback 写 depth；mesh-role debug 模式仍显示该几何。`MaterialChange` 继续使用基础材质。真实运行时 crest/decal 内容仍不可用，因此 `decalOrCrest` 保持 unsupported。

透明排序目前做到 mesh-level：`Transparent` 与 `Glass` batch 按相机方向和 mesh center back-to-front 排序。还没有逐三角排序或 weighted blended OIT。

## 9. Meddle 调研结论

Meddle 作为 Dalamud 插件不主要靠离线猜路径，它从运行时对象读取：

- `model->ModelResourceHandle->FileName`
- `characterBase->ResolveMdlPath(model->SlotIndex)`
- `material->MaterialResourceHandle->FileName`
- `TextureResourceHandle->FileName`
- GPU color table texture
- on-render material output

因此 Meddle 可以拿到游戏实际 resolve 后的路径和参数。Web 离线预览无法访问运行时，只能复刻路径和常用 shader 语义。当前实现已吸收的重点：

- 不要只从 item packed ID 推 variant，还要以真实存在路径/材质名为准。
- ColorTable 需要按 `_id.tex` 查表。
- MDL mesh 提取不依赖 physis LOD parts 过滤，改为读取 raw LOD0 mesh ranges，覆盖 normal/water/shadow/terrainShadow/verticalFog 以及 extra LOD 的 lightShaft/glass/materialChange/crestChange。
- Dawntrail ColorTable 的 `unknown1/unknown2` 在 physis 中分别对应 Meddle 的 GlossStrength / SpecularStrength。
- `characterglass` 需要单独模式。

## 10. 已知限制与后续计划

当前实现目标是“武器模型预览可辨识、颜色/透明大体正确”，仍非完整 FFXIV shader 复刻。

后续优先级：

1. Prepared draw role / pass：`PreparedModel` / `PreparedMesh` 已完成第一步主 pass 过滤并保留 submesh attribute metadata；显式 `enabledAttributeMask` 输入已可隐藏 disabled submesh，显式 `enabledShapeMask` 已可在 renderer creation 时应用 sparse morph target，默认离线模式仍保持不过滤/base geometry；renderer 内部已有 `Opaque/Cutout/Transparent/Glass/AdditiveLightShaft` prepared pass 分类，Cutout/Glass 已有独立 wgpu pipeline 入口，`AdditiveLightShaft` 已进入最小 additive wgpu pipeline 并消费第一组 lightshaft 参数；后续还需要 runtime shape name/bit mapping、更完整的 cutout/glass shader 行为和 lightshaft 节点语义。
2. GPU 顶点格式：uv1-uv3、color1、secondary normal/bitangent、flow 已进入 GPU 顶点输入；PreparedMaterial 已有 feature flags、UV source、per-role scroll mask、flow/value mode 和 unsupported/runtime-only 输入摘要。`usesFlow` 已收紧为 Flow mode + flow0，WGSL 已消费 primary flow tangent，Map1 已消费 UV1Scroll；下一步是 flow1、secondary tangent 与 color1。
3. Glass：normal-B alpha、`DrawDepthMode` / `EnableLighting` prepared policy、dither depth prepass 与显式 `GlassBlendMode` scene input 已接入；后续实现折射与真实厚度传输。
4. Material params：alpha/glass/normal/tile/toon/detail/color/outline/scroll/lightshaft 参数、`CategoryFlowMapType` 与 `GetValues` 已结构化；water colors/wave 和 bguvscroll Map1 sampler roles 也已加入。共享 tile/detail atlas、Map0/Map1 scroll、Flow tangent 与 water alpha/base/primary normal 已有 WGSL 消费。后续处理 multi map mask、water refraction/whitecap、detail influence 和其它 shader-family 行为。
5. Tile/Sphere/Sheen：renderer 已消费 ColorTable extra maps 和共享 tile/detail pair atlas；ORB Blue darkening 与 tile-normal Alpha 权重已按 MeddleTools 节点对齐。后续要补 shader-family-specific UV source，并实现更接近 MeddleTools 的 reflection/sphere 规则。
6. 纹理采样配置：数据层已有第一版 role policy，renderer 已从 prepared policy 派生 color/data/nearest-data sampler，并已绑定 `_id.tex`、ColorTable extra maps 与共享 arrays；后续重点是 per-texture independent sampler 和 shader 级 clip/extend。
7. 染色：Legacy/Dawntrail `ColorDyeTable` 的 template、channel 和可染通道 flag 已结构化进 `ModelMaterial.colorDyeTable`；数据层已支持 Legacy `stainingtemplate.stm` 与 Dawntrail `stainingtemplate_gud.stm` 的 v1.1/v2.x 解析、1-based stain lookup、GUD template ID `-1000` 的 Legacy fallback 和逐 flag `ColorTableRowColors` override。`WeaponModelLoadRequest.stainIds` 已进入同步/异步加载，STM 按请求缓存并在 material summary/ColorTable bake 前应用；`WeaponModelData.stainIds`、`ModelMaterial.stainingApplication`、phantom summary 与 prepared unsupported 会记录结果。`WeaponCatalogPackage` 已导出 EXD stain UI metadata，Web 提供 stain0/stain1 色块选择器并把值写入 URL 和模型资源 key。默认 `[0,0]` 不加载 STM，EXD `Stain.Color` 仅用于 UI，不替代 STM 数据。phantom fixture 已支持 case-level `stainIds`，并用 `45052` 的 `[0,0]`/`[1,0]` 正式 snapshot 验证染色画面和 application report；后续扩展第二通道与 metallic 样本。
8. 特殊 shader：lightshaft、transparency/glass、bguvscroll、Flow 与 water 已有第一版消费；stockings 已对齐 opaque alpha/pipeline，tattoo 已按 normal Alpha 处理透明度。reflection 与 occlusion 仍主要停留在 package 分类/runtime diagnostics，后续补各 family 的实际 shader 行为。
