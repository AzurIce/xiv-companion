# 武器渲染解析审查与改进规划

本文整理当前 `xiv-companion` 武器模型预览与本地参考仓库 `Meddle`、`MeddleTools` 的对比结果，并按三层规划后续工作：

1. 数据解析
2. 解析后的结果处理
3. 渲染器与着色器管线

目标不是一次性复刻完整 FFXIV shader，而是把现有“可辨识预览”推进到“语义清楚、问题可定位、关键材质效果稳定”的状态。

## 核验依据

本轮核验时间：2026-07-10。

当前结论基于以下代码和参考实现抽查：

- 本仓 `crates/xiv-companion-data/src/model.rs`：`PackedModelId`、`ModelVertex`、`ModelMaterial`、`ModelTextureKind`、`bake_color_table_maps`、weapon model/material candidate path。
- 本仓 `crates/xiv-companion-data/src/mdl_geometry.rs` 与 `mdl_metadata.rs`：raw LOD0 mesh range、extra LOD、vertex declaration、secondary attributes、flow、bone table、shape metadata。
- 本仓 `crates/xiv-companion-data/src/weapon_models.rs`：MTRL sampler records、`.shpk` composed semantics、ColorTable bake、ColorDyeTable 结构化行数据/debug、alpha mode、sub-model load path。
- 本仓 `crates/xiv-companion-render/src/renderer/model.rs` 与 `model.wgsl`：GPU vertex layout、material bind group、opaque/transparent pass、mesh-level transparent sorting、实际消费的 texture/vertex fields。
- `E:\repos\Meddle\Meddle\Meddle.Utils\Export\Model.cs` 与 `Vertex.cs`：LOD0 mesh range、extra LOD、shape/attribute group、Meddle 顶点属性保留方式。
- `E:\repos\Meddle\Meddle\Meddle.Plugin\Utils\ParseMaterialUtil.cs` 与 `OnRenderMaterialUtil.cs`：运行时 material/texture handle、GPU ColorTable、decal/crest/on-render material output。
- `E:\repos\Meddle\Meddle\Meddle.Utils\Constants\Names.cs`：已知 material parameter CRC、默认值和 shader 覆盖范围。
- `E:\repos\Meddle\Meddle\Meddle.Utils\Files\Structs\Material\ColorTableRow.cs`：Dawntrail/Legacy ColorTable 字段语义。
- `E:\repos\MeddleTools\MeddleTools\node_setup\node_configs.py`、`node_mappings.py`、`bake\bake_utils.py`：texture node config、UV scroll、ColorTable extra ramps、shader package mapping、diffuse/normal/roughness/glossy/transmission/emission bake。
- Penumbra.GameData `StmFile.cs`、`DyePack.cs`、`LegacyDyePack.cs` 与 ColorTable `ApplyDye`：Legacy/GUD STM 路径、1-based stain ID、column 编码和逐 flag ColorTable 覆盖规则。Meddle 的 `ColorDyeTableRow.cs` 注释直接引用该实现。

## 当前状态摘要

已经做得比较扎实的部分：

- `PackedModelId` 已按武器语义解释为 `model_id / body_id / variant_id`，避免 body 与 variant 反转。
- MDL 解析已经改为读取 raw LOD0 mesh range，覆盖 normal、water、shadow、terrainShadow、verticalFog，以及 extra LOD 的 lightShaft、glass、materialChange、crestChange。
- 顶点解析保留了多套 UV、secondary normal/bitangent/color、flow、blend weights/indices，数据层接近 Meddle 的导出结构。
- MTRL 路径解析已支持从 `mt_w####b####_*.mtrl` 反推材质根目录，解决副手模型引用主手材质的问题。
- MTRL sampler 分类已结合 `.shpk` resource parameter 名称、CRC 和文件名兜底，比纯路径后缀可靠。
- Material debug 已输出 sampler 的 `textureUsageName` 和 `kindSource`；resource-aware debug 会加载对应 `.shpk`，区分 `shpkResourceName`、`knownCrc` 与未知来源。
- Material debug 已新增 compact `summary`，聚合 resolved shader keys、resolved constants、shader flags、texture flags、sampler flags，并标出 shader package default 与 material override 来源；phantom `model-summary.json` 的 material 条目也会输出该摘要。
- ignored phantom snapshot 的 `model-summary.json` 已把每个 mesh 的 metadata 文件、submesh attributes、bone table、shape 影响摘要提升出来，不必只靠逐个打开 raw model metadata。
- `ModelMeshDrawRole` 已把 MDL mesh category 映射成 renderer-friendly draw role；renderer 当前会跳过 shadow、terrainShadow、verticalFog，不再把这些 mesh 当普通 surface 画；lightShaft 会作为 additive pass 绘制，materialChange/crestChange 已拆成独立 role 并暂时保留在主 pass 供诊断。
- `WeaponModelData.loadDiagnostics` 已记录可选副手/子模型加载失败的 role、model、候选路径、失败状态和错误信息，phantom `model-summary.json` 会直接输出。
- `weapon-render-pipeline.md` 已同步当前实现：Legacy ColorTable bake、mesh-level transparent sorting、额外材质贴图绑定和剩余限制不再按旧状态描述。
- Dawntrail 与 Legacy ColorTable 都能通过 `_id.tex` 烘焙出 diffuse、specular、material-properties、tile、sheen、sphere、tile-matrix 等派生贴图。
- `characterglass.shpk` 已有独立 alpha/render mode，透明 batch 已做 mesh-level back-to-front 排序。
- renderer GPU 顶点格式已上传 `uv1-uv3`、`color1`、secondary normal/bitangent、`flow0/flow1`；WGSL 已传递这些通道，并按 prepared UV source + per-role scroll mask 选择采样 UV，Flow 模式会消费 `flow0` primary tangent。当前 source 仍基本选择 `uv0`，secondary normal/bitangent、`color1` 和 `flow1` 尚未参与实际 shader。
- `PreparedModel` / `PreparedMesh` 已有第一版，按 mesh 输出 draw role、是否进入主 pass 和 prepared material；renderer 与 phantom `model-summary.json` 现在共用这一准备结果。
- `PreparedMaterial` / `PreparedRenderPass` 已提升到数据层；phantom `model-summary.json` 的主 surface mesh 会输出 prepared material 决策，包含 `Opaque`、`Cutout`、`Transparent`、`Glass`、`AdditiveLightShaft` 与 culling policy；lightshaft 不进入普通 surface pass，但 renderer 会保留为 additive batch。
- `MaterialShaderFamily` 已结构化常见 `.shpk`：character、characterStockings、characterGlass、characterReflection、characterTransparency、characterScroll、characterTattoo、characterOcclusion、bg、bgUvScroll、lightShaft、water、unknown，并进入 `PreparedMaterial`；lightshaft、bguvscroll 已有第一版行为，其它特殊 family 仍有不同程度的节点缺口。
- `PreparedTextureBindings` 已聚合现有材质贴图索引：base、normal、mask、material、multi、specular、emissive、material-properties、tile、sheen、sphere、tile-matrix、ColorTable index，以及 character tile normal/ORB 和 bg detail diffuse/normal 四个共享数组 atlas，并随 prepared material 输出。
- `PreparedTextureSamplingSet` 已表达第一版 texture role 采样策略：base/specular/emissive 为 sRGB + linear + repeat，normal/mask/material/multi/material-properties 为 Non-Color + linear + repeat，index、ColorTable extra maps 和共享 tile/detail arrays 为 Non-Color + nearest + repeat；renderer 已从该 prepared policy 派生 color/data/nearest 三组 sampler descriptor。
- `ModelColorDyeTable` 已把 Legacy/Dawntrail 的 template、channel 和各可染通道 flag 从 debug 提升为 `ModelMaterial.colorDyeTable` 的可序列化结构化数据；保留 `hasColorDyeTable` 兼容旧数据，prepared `usesDye` 会识别任一入口，请求级 stain IDs 已接入实际 model load。
- 数据层已实现 Legacy `chara/base_material/stainingtemplate.stm` 与 Dawntrail `chara/base_material/stainingtemplate_gud.stm` 的通用 parser，覆盖 v1.1/v2.0/v2.1、u16/u32 keys、singleton/direct/indexed column 编码、1-based stain ID lookup，以及 Dawntrail template ID 减 1000 后回退 Legacy STM；同时已有按 Legacy/Dawntrail dye flags 覆盖 renderer-friendly ColorTable rows 的纯函数与诊断报告。
- `WeaponModelLoadRequest.stainIds` 已作为请求级 `[stain0, stain1]` 输入进入同步/异步 SqPack 加载；请求仅在存在非零 stain 时各加载一次 Legacy/GUD STM，材质会在 summary 和 ColorTable bake 前应用染色。`WeaponModelData.stainIds` 与 `ModelMaterial.stainingApplication` 会保留输入、模板路径、行统计和错误，phantom summary 可直接审计；资源 key 也包含 stain IDs，避免不同染色请求冲突。
- `WeaponCatalogPackage.stains` 已从 `Stain` EXD 导出 ID、中文名称、原始 BGR 色值、UI RGBA、shade、sub-order 和 metallic；当前本地客户端有 125 个具名染剂。Web 武器预览已提供 stain0/stain1 选择器、色块、金属标记和 URL 状态，并使用请求级 stain IDs 重新加载模型。EXD 色值仅用于 UI，不参与实际 ColorTable 覆盖。
- `CategoryFlowMapType` / `0x40D1481E` 已按 Meddle 的 `Standard=0x337C6BC4`、`Flow=0x71ADA939` 结构化为 `MaterialFlowMode::{Standard,Flow,Unknown}`，同步/异步 material composition、known shader label、phantom summary 均会保留。`PreparedMaterialFeatureFlags.usesFlow` 现在只在材质选择 Flow 且 mesh 存在 primary `flow0` 时启用；仅有 `flow1` 或 Standard/Unknown 模式不会误启用。Meddle `VertexUsage.Flow => TANGENT0` 是当前将 flow 解释为 tangent 而非 UV 动画的依据。
- `PreparedMaterialUnsupportedInputs` 已按当前可可靠判断的数据标出 dye application、runtime ColorTable、decal/crest、runtime material change、tile array、detail array、secondary map blend、incomplete shader family logic，并进一步拆出 `runtimeOptionColor`、`runtimeDecalColor` 与 `runtimeSkinMaterial`，phantom summary 会随 prepared material 输出这些缺口。MeddleTools `charactertattoo` 节点证明 tattoo 颜色依赖 OptionColor/DecalColor，Meddle composer/parse util 证明二者来自 customize/decal constant buffer；Meddle on-render util 证明 stockings 会复制 runtime skin material textures。当前不为这些缺失运行时输入伪造颜色或贴图。
- 数据层已从 SqPack 加载 `tile_norm_array.tex`、`tile_orb_array.tex`、`detail_d_array.tex`、`detail_n_array.tex`。由于 Physis 当前把 TEX header 的 `MipLevels:u8 + ArraySize:u8` 当成 `u16` 且普通 `to_rgba()` 只按 depth 解码，本仓会读取 header byte 15，把 mip0 slices 解码为与 MeddleTools 导出一致的 vertical atlas，并保留 `arraySize` 与 `arrayLayerHeight`。真实客户端验证结果为 character 两张 `64x4096 / 64 layers`，bg detail 两张 `256x8192 / 32 layers`。
- `ModelMaterial.textureArrays`、`PreparedTextureBindings` 和 `PreparedMaterial.resourceAvailability` 已表达共享数组索引、加载错误和成对完整性；phantom texture summary 会输出 atlas 总尺寸、层数和单层高度。renderer 将 normal/ORB 与 detail diffuse/normal 分别横向合并成两个 GPU pair atlas，保持 fragment sampled texture 总数不超过常见 WebGPU 16 张限制，并以 nearest + repeat 在 WGSL 中选层采样。prepared 会按索引成对完整性设置 `unsupportedInputs`；renderer 另行验证类型、层数、尺寸和 RGBA 长度，失败时使用中性 fallback。布局级失败尚未反馈回 prepared summary。
- Meddle `OnRenderMaterialUtil` 证明 weapon decal/FC crest 属于运行时 on-render 输入，不是静态 MTRL sampler。`PreparedMaterial.runtimeFallbacks` 已明确缺失 decal/crest 时使用透明纹理语义，materialChange 使用基础材质语义；renderer final 模式会 discard crest fallback，mesh-role debug 仍可见，materialChange 继续使用基础材质。
- `bguvscroll.shpk` 已单独分类为 `MaterialShaderFamily::BgUvScroll`；primary Color/Normal/Specular Map0 使用 UV0Scroll，secondary Map1 使用 UV1Scroll。三种 Map1 已有独立 texture kind、model/prepared binding、per-role source/scroll mask 和 Web diagnostics；WGSL 在 `GetMultiValues` 下按 vertex alpha 统一混合 color、color alpha、normal 与 specular。`characterscroll.shpk` 不会误继承该动画。
- `ModelMesh` / `PreparedMesh` 已保留 mesh-level shape influence 摘要；`PreparedModelOptions.enabledShapeMask` 已可按显式 shape mask 标出 active/inactive shape influence，但当前不把 shape mask 当 draw visibility，也尚未执行 morph/vertex replacement。
- renderer 已绑定并消费 ColorTable extra maps：tile、sheen、sphere、tile-matrix 以 Non-Color texture view + nearest sampler 进入 WGSL，当前用于保守的 specular/sheen/sphere-like highlight 调制，并提供独立 debug view 检查这些烘焙 ramp。
- `g_NormalScale` 已从 composed material constants 提升为 `ModelMaterial.normalScale`，支持 shader package default 与 material override；renderer 会用它缩放 tangent-space normal map 强度。
- `g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale` 已结构化进 `ModelMaterial` 和 renderer `shaderParams`；共享 detail normal atlas 可用时 WGSL 会按 detail/multi-detail ID 与各自 UV scale 采样并组合 tangent-space normal，缺图时才回到 primary normal 的受限 fallback。
- `g_TileIndex`、`g_TileAlpha`、`g_TileScale` 已结构化进 `ModelMaterial` 和 renderer `tileParams`；WGSL 优先用逐像素 ColorTable `TileProperties.r * 64` 选 tile layer，没有该贴图时回退 `g_TileIndex`，并结合 TileMatrix/TileScale 采样 tile normal/ORB。ORB 当前保守按 R=AO、G=roughness、B=specular scale 使用；`TileAlpha` 只控制 tile 效果权重，不作为材质透明度。
- `g_ToonIndex`、`g_ToonLightScale`、`g_ToonLightSpecAperture`、`g_ToonReflectionScale`、`g_ToonSpecIndex`、`g_SheenRate`、`g_SheenTintRate`、`g_SheenAperture`、`g_SphereMapIndex` 已按 Meddle `Names.cs` CRC/default 结构化进 `ModelMaterial`、phantom summary 与 renderer `toonSheenParams` / `toonParams` / `sheenSphereParams` uniform。`PreparedMaterialFeatureFlags.usesToon` 对 character family 启用；WGSL 已实现平滑双段 diffuse、aperture/spec-index band 与 reflection rim 的解析式近似，sheen/sphere 保持原高光/反射输入。MeddleTools 源码和 Blender character node group 均无 toon socket/node，24 组 phantom 原始参数均为默认值，因此当前不伪造 lookup texture，并保留近似限制。
- `g_DetailID`、`g_MultiDetailID`、`g_DetailColor`、`g_MultiDetailColor`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 已结构化进 `ModelMaterial` 和 renderer detail uniforms；WGSL 会按两个 detail ID 选层，用 color/normal 各自 UV scale 采样 detail pair atlas，并以 0.5 为 diffuse 中性值做保守 tint/normal 组合。当前缺少真实 bg 武器样本，混合权重仍待校准。
- `g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_MultiEmissiveColor` 已按 Meddle `Names.cs` CRC/default 和 MeddleTools `ColorMapping` 结构化进 `ModelMaterial`、phantom summary 与 renderer uniforms；WGSL 已把 `g_DiffuseColor` 作为 base tint，把 `g_MultiDiffuseColor` 作为 mask-gated 的保守 base tint 补充，把 `g_EmissiveColor` 作为附加发光，并在 mask/material 通道存在时保守加入 `g_MultiEmissiveColor`；完整 multi map 通道解释仍未实现。
- `g_OutlineColor`、`g_OutlineWidth`、`g_SpecularColorMask`、`g_SSAOMask`、`g_TextureMipBias`、`g_ShadowPosOffset` 已按 Meddle `Names.cs` CRC/default 结构化进 `ModelMaterial`、phantom summary 与 renderer `outlineParams` / `specularColorMask` / `surfaceParams` uniform。outline 已进入 prepared `usesOutline` 和独立 inverted-hull pass，原 fragment rim fallback 已移除；`g_SpecularColorMask` 继续调制高光颜色/强度，`g_SSAOMask` 继续保守调制环境底光。Meddle 只有 outline 参数适用 family/default 的证据，MeddleTools 无节点语义，现有 24 组 phantom 材质宽度全部为 0；当前直接使用 clamp 到 0.1 的模型空间宽度，并用 45052 synthetic override 验证，不伪造真实样本。mip bias 和 shadow offset 仍未驱动实际行为。
- `g_GlassIOR`、`g_GlassThicknessMax` 已按 Meddle `Names.cs` CRC/default 结构化进 `ModelMaterial`、phantom summary 与 renderer `glassParams` uniform；WGSL 当前把非默认 IOR/thickness 用作 glass tint、specular 与 rim fresnel 的轻量调节，不改变 glass opacity 或折射。
- `g_UVScrollTime` / `0x9A696A17` 已按 MeddleTools `UvScrollMapping` 结构化进 `ModelMaterial.uvScroll` 和 renderer uniform；`ModelRenderOptions.uv_scroll_time` 进入 camera uniform，Web 渲染循环用 RAF 时间驱动，native snapshot 默认时间为 0 保持稳定。prepared `usesScroll` 现在要求存在非零 multiplier 且至少一个明确可滚动 texture role，lightshaft 的独立 `g_TexAnim` 路径不受影响。
- `lightshaft.shpk` 的 `g_Color`、`g_TexAnim`、`g_TexU`、`g_TexV`、`g_Ray` 已结构化进 `ModelMaterial` 和 phantom summary；renderer uniform 已传入 WGSL，`LightShaft` draw role 会启用保守的 additive tint、`g_TexAnim.xy` UV 动画、`g_TexU/V` 仿射 UV 和 `g_Ray` 强度近似。完整 MeddleTools 节点语义仍未复刻。
- `g_Transparency` 已按 MeddleTools `meddle water.shpk` 的直接 Alpha 连接进入 `PreparedAlphaSource::MaterialTransparency`；小于 1 时 water 进入 Transparent pass，WGSL 直接输出该 alpha，不乘 vertex/base alpha 或 character alpha shaping。`g_WaterDeepColor`、`g_RefractionColor`、`g_WhitecapColor` 已结构化进 model/summary/uniform，其中 deep color 按节点直接作为 water base；refraction/whitecap 因节点未连接而只保留输入。`g_SamplerWaveMap` 通过现有 normal binding 使用 R/G 解码 normal。
- sampler 分类已将 `g_SamplerWaveMap`、`g_SamplerWaveMap1`、`g_SamplerWhitecapMap` 拆成 `WaterWave/WaterWaveSecondary/WaterWhitecap`，同步/异步 loader、`WeaponTextureSet`、`ModelMaterial`、prepared bindings 与 Web texture counts 均保留各自索引。三者不会再被 generic fallback 误当 base；renderer 只消费节点已证实的 primary wave，secondary/whitecap 暂不增加 GPU binding。
- `g_SamplerEnvMap` 已从 `Other` 拆成独立 `Environment` texture kind，并贯通同步/异步 loader slot、`WeaponTextureSet`、`ModelMaterial`、prepared binding/UV/sampling、phantom summary 与 Web texture counts。采样策略按 MeddleTools 固定为 Non-Color、Linear、Repeat。Meddle Names 表明 `g_EnvMapPower` 属于 bg/bgcolorchange/bgcrestchange/bgprop/bguvscroll/crystal，而不是 `characterreflection.shpk`；在缺少可靠反射坐标与混合节点证据时 Environment 暂不接入 WGSL，也不与 character reflection 混为一谈。
- `crystal.shpk` 已从 Unknown 拆成 `MaterialShaderFamily::Crystal`；Environment binding 会设置 `usesEnvironmentMap`，并在 renderer 尚未消费时设置 `environmentMapping` unsupported。Blender headless 检查 MeddleTools `meddle crystal.shpk`：接口包含 `g_SamplerEnvMap`，但该输入在现有节点图中没有任何连线，不能据此实现可信混合；因此 renderer 在获得真实节点/样本证据前继续不采样 Environment。
- `g_SamplerMulti` 已有独立 `multiMapInterpretation` unsupported 字段：只要存在 explicit MultiMap binding 就保持 true，与只表达共享 detail diffuse/normal array 是否齐全的 `detailArray` 分离；即使 arrays 完整，multi map 通道未实现仍可审计。源码搜索与 Blender headless 全节点接口扫描均确认当前 MeddleTools 没有 `g_SamplerMulti` texture config/socket，不能从该参考仓推导 mask 公式；该标记会保持到真实 SHPK/样本证据支持正式 WGSL 消费。
- `GetSubColor` 已结构化为 `MaterialSubColorMode::{None,Face,Hair,Unknown}`，贯通同步/异步 composition、`ModelMaterial`、`PreparedMaterial` 与 phantom raw material summary。Meddle Names 明确 key `0x24826489` 与 Face/Hair values，MeddleTools hair mapping 也消费两种模式；`GetSubColorFace` 同时适用于 characterocclusion/charactertattoo，实际颜色来自 Meddle composer 注入的 customize buffers。prepared 会在 characterOcclusion 或显式 Face/Hair 模式下设置 `runtimeSubColor`，离线 loader 不伪造颜色。
- `characterreflection.shpk` 已有独立 `characterReflection` unsupported 字段，并继续保留 `incompleteShaderFamilyLogic`。Meddle/MeddleTools 搜索未发现静态 reflection sampler、on-render replacement 或对应节点组，MeddleTools 也没有把它映射回 `character.shpk`；现 renderer 因而明确标为 generic character approximation，不把 Environment/sphere/specular 资源猜作 reflection 输入。该标记会保持到获得真实 SHPK 节点或样本证据。
- `g_AlphaAperture`、`g_AlphaOffset`、`g_ShadowAlphaThreshold` 已按 Meddle `Names.cs` CRC/default 结构化进 `ModelMaterial`、phantom summary 与 renderer `alphaParams` uniform；WGSL 当前只在 aperture/offset 非默认时对非 glass/lightshaft alpha 做受限 shaping，`g_ShadowAlphaThreshold` 仍未驱动 shadow pass。
- character transparency/glass 的控制入口已结构化：`ModelMaterial.drawDepthMode` 保留 `None/Dither/Unknown`，`lightingMode` 保留 default/enabled/disabled/unknown；`PreparedMaterial.alphaPolicy` 输出 alpha source、depth mode 与 lighting enabled。MeddleTools `shaders.blend` 进一步确认普通 character Alpha 输出来自 `g_SamplerNormal` Blue，stockings 分支强制为 1；renderer 已按该规则让 character blend/glass/transparency 从 normal B 取 alpha，并让 `EnableLightingOff` 的 character transparency 走 unlit surface。`GlassBlendMode` 已作为显式 scene option 进入 renderer、Web 和 snapshot；Mul/Add 当前分别使用现有 alpha blend 与 additive pipeline 近似。
- MeddleTools `meddle charactertattoo.shpk` 节点图明确把 `g_SamplerNormal` Alpha 连接到材质 Alpha，而不是复用普通 character 的 normal Blue。`PreparedAlphaSource::NormalAlpha` 已表达该差异，renderer uniform 编码为 `4.0`，dither depth 与主 fragment 的 alpha resolver 均接收 normal B/A 并按 source 选择；只修正可静态证明的 alpha，依赖运行时 `OptionColor`/`DecalColor` 的 base color 混合继续保持 unsupported。
- `ModelDebugMode` 已提供 renderer debug 视图：final、base、normal、mask、material properties、specular、emissive、alpha、UV0-UV3、vertex color、mesh/draw-role color、ColorTable index、material map、multi map、tile/sheen/sphere properties、tile matrix，以及 tile normal/ORB、detail diffuse/normal atlas 选层结果；Web 控件和 snapshot/test render options 共用同一入口。phantom 可通过 `XIV_PHANTOM_ARRAY_DEBUG=1` 输出四张数组诊断图。

主要缺口集中在：

- 多套 UV、secondary normal/bitangent、`color1`、flow 已进 GPU 输入；Flow material mode 已让 WGSL 将插值后的 `flow0.xyz` 正交化为 primary tangent，并结合已有 bitangent 方向构造 normal mapping frame。`flow1`、secondary normal/bitangent 和 `color1` 仍未进入实际 family-specific shading，继续保留给 secondary map/tangent set，不伪造未证实的动画用途。
- `g_SamplerColorMap1`、`g_SamplerNormalMap1`、`g_SamplerSpecularMap1` 已拆成 secondary logical roles；`GetValues` 已结构化为 Single/Multi/AlphaMulti/AlphaMulti2/AlphaMulti3/MultiMaterial/Compatibility/Unknown。MeddleTools `meddle bg.shpk` 已证明 `GetMultiValues` 的 vertex-alpha 混合公式并完成消费；`GetAlphaMultiValues2/3` 在 MeddleTools 配置中仍明确标为未映射，prepared 会报告 `secondaryMapBlend` unsupported 而不猜测。
- fragment stage 保持 15 张 sampled texture：仅 `BgUvScroll + GetMultiValues` 将 secondary color/normal/specular 复用到物理 binding 9/10/11，其它 family 仍使用这些 binding 的 tile/sheen/sphere 语义。Specular Map0/1 也已按 MeddleTools 从旧 sRGB 假设修正为 Non-Color + Linear/Repeat。
- mesh category、submesh attribute mask/name 和 shape influence 摘要已进入第一版 `PreparedModel` / `PreparedMesh`；`PreparedModelOptions.enabledAttributeMask` 已可按显式运行时 attribute mask 隐藏 disabled submesh，`enabledShapeMask` 已可审计 active/inactive shape influence，但 Web 离线默认仍不猜这些 mask。bone/skin/morph 和实际 shape vertex replacement 仍没有进入后续渲染决策。
- 材质语义仍被压缩成少量近似规则和 Opaque/Mask/Blend/Glass；ColorTable extra maps、tile/detail、bguvscroll Map0/Map1、Flow 与 water alpha/base/primary normal 已有第一版实时消费，但 water refraction/whitecap/WaveMap1、AlphaMulti variants、ORB/detail 通道权重、reflection 等节点逻辑仍不完整。
- 运行时 GPU ColorTable、decal、crest、on-render material output 是 Meddle 运行时路径的优势；当前离线 Web 预览没有等价输入。静态 stain0/stain1 已有完整离线输入和 STM 应用路径，decal/crest 与 materialChange 已执行默认 fallback，但仍不能显示真实运行时 crest/decal 内容。
- 文档 `weapon-render-pipeline.md` 已同步到当前实现；后续设计和优先级以本文 roadmap 为准。

## 分层审查结论与计划总览

### 1. 数据解析

审查结论：

- 本仓已经对齐 Meddle 的 LOD0 mesh range 和 extra LOD 分类方式，能区分 normal/water/shadow/terrainShadow/verticalFog/lightShaft/glass/materialChange/crestChange，且顶点层保留了多套 UV、secondary normal/bitangent/color、flow、blend weights/indices。
- MTRL 解析已经不只靠文件名猜测：sampler role 会优先使用 `.shpk` resource parameter name，再退回 known CRC 和路径后缀；shader package default 与 material override 也已经进入 debug summary。
- ColorTable 解析已覆盖 Dawntrail 与 Legacy，并按 Meddle/MeddleTools 语义产出 diffuse、specular、material-properties、tile、sheen、sphere、tile-matrix 等派生贴图；TileAlpha 已明确不再被误当作材质 alpha。

主要不足：

- 染色已接入同步/异步 weapon model load、material summary、ColorTable bake 和 Web stain0/stain1 选择器；EXD 名称、UI 色块、排序和 metallic metadata 也已导出。正式 phantom fixture 已加入 `45052` stain `[1,0]` case，当前视觉覆盖仍应继续扩展到第二通道和 metallic 染剂。
- Meddle 的 runtime 输入，包括 GPU ColorTable、resolved texture/material handle、decal、crest、on-render material output，仍不能由离线 SqPack 还原；其中 decal/crest 与 materialChange 已有显式 prepared fallback，GPU ColorTable 和 handle remap 继续只记录为缺口。
- reflection/stockings/tattoo/occlusion 等 shader package 已能分类，但很多 shader keys/constants 还没有提升为结构化字段，也没有最小 fixture 覆盖；outline/specular/SSAO、toon/sheen/sphere、alpha aperture/offset/shadow threshold、glass IOR/thickness 和 transparency 已先进入结构化字段但未驱动完整 shader-family 行为，lightshaft 已有第一组结构化 constants 但 `g_Ray` 与节点级行为仍是近似。
- texture/sampler 语义仍有少量兜底路径依赖；MeddleTools 里 `_id.tex`、tile/detail arrays 使用 Non-Color + Closest/Repeat 的规则已经进入 prepared policy。tile/detail vertical atlas 已进入 GPU/WGSL 并完成第一版选层和组合，后续重点转为通道解释、权重校准和 shader-family-specific UV 路由。

计划：

1. 先继续扩充可审计信息：在 material/prepared debug 中补齐 texture role 的最终来源、shader family、sampler policy、UV source、feature flags 和未支持 runtime 输入标记。
2. 染色体验链路已完成请求、STM、bake、EXD metadata、Web 双通道选择器、URL 状态和首个正式染色 snapshot；后续补第二通道与 metallic 染剂视觉组合。EXD 颜色继续只用于 UI，不作为实际覆盖值。
3. 逐步结构化 shader-family 参数：优先 glass/transparency/lightshaft/scroll，再处理 reflection/stockings/tattoo/occlusion；每补一个参数都加合成 MTRL fixture 和真实样本 debug 对照。
4. 对 runtime-only 数据不盲猜：decal/crest 已建立透明纹理 fallback，materialChange 已建立基础材质 fallback；下一步让 renderer 消费这些决策。GPU ColorTable 继续只在 debug 中标明缺失，避免离线预览伪装成完整运行时渲染。

### 2. 解析后的结果处理

审查结论：

- `PreparedModel` / `PreparedMaterial` 已经把 raw parsed data 和 renderer binding 决策分开，renderer 与 phantom summary 共用 draw role、main-pass visibility、prepared pass、texture bindings、sampling policy、feature flags 和第一版 UV source。
- submesh attribute mask/name、显式 `enabledAttributeMask`、shape influence 摘要、显式 `enabledShapeMask` 审计和 mesh-level flow presence 已进入 preparation；这与 Meddle 的 shape/attribute group 思路一致，但 shape 目前只做 active/inactive 审计，不执行 morph。
- `PreparedRenderPass` 已能表达 `Opaque`、`Cutout`、`Transparent`、`Glass`、`AdditiveLightShaft`；lightshaft 不再误进主 surface pass。

主要不足：

- preparation 已有 `enabledShapeMask` 的 active/inactive shape influence 审计，但还没有真正应用 shape mesh/morph、skinning/morph runtime 输入，也没有 per-submesh draw batch 级别的可见性拆分。
- `PreparedMaterialUvSources` 已开始驱动 renderer 选择采样 UV；`PreparedTextureSamplingSet` 已开始驱动 renderer 的 color/data/nearest 三组 sampler descriptor，但还没有做到每个 texture role 独立 sampler。
- shader-family-specific 规则还没有进入中间层，例如 character base texture 如何与 ColorTable diffuse 混合、material/multi map 通道如何解释、scroll/reflection 使用哪套 UV/flow。
- `usesDye`、decal/crest、runtime ColorTable、runtime material change、tile/detail array 这些 capability flags 已有第一版 prepared unsupported summary；stain 与共享数组已进入 renderer，crest/decal 缺失时的透明 fallback 和 materialChange 基础材质 fallback 也已执行。runtime ColorTable 与真实 on-render crest/decal 内容仍缺显式输入。

计划：

1. 扩展 preparation 输入：stain 已进入 `WeaponModelLoadRequest`，decal/crest 与 materialChange 已有默认 fallback；后续只在有真实调用方需求时增加显式 runtime decal/crest 资源入口。`enabledShapeMask` 继续保持审计输入，默认不猜运行时状态。
2. 把 prepared texture/sampler/UV source 从“输出给 debug”推进到“驱动 renderer binding”：UV source 已接入 renderer material uniform 和 WGSL 采样选择；prepared sampler policy 已驱动 color/data/nearest 三组 sampler descriptor；后续继续接入 shader-family-specific source、nearest data resources 和 per-texture sampler。
3. 将 shader-family-specific 规则下沉到 prepared 层：为 character/glass/transparency/scroll/lightshaft/reflection 等输出明确的 feature flags、UV source、blend/alpha policy 和需要的 texture roles。
4. 继续让 phantom `model-summary.json` 输出 preparation 结果，新增“为什么没画/为什么用了 fallback”的原因字段，作为后续真实样本验证的主要入口。

### 3. 渲染器与着色器管线

审查结论：

- renderer 已上传扩展顶点格式，并已将 base/normal/mask/emissive/specular/material-properties/tile/sheen/sphere/tile-matrix 绑定到 material bind group；color/data/nearest data sampler 已初步分开。
- 透明 batch 已按 mesh-level back-to-front 排序；glass 已进入透明管线；cutout 有 alpha test；ColorTable extra maps 已在 WGSL 中产生可观察的保守高光/反射近似。
- 当前 `model.wgsl` 仍是单个近似 shader，虽然已能按 prepared UV source 在 `uv0-uv3` 间选择采样 UV，但 shader-family-specific 规则仍少，实际材质多数仍走 `uv0`、primary normal/bitangent、`color0` 的近似路径；与 MeddleTools 节点图和 bake pass 的差距仍集中在 shader 行为，而不是字段缺失。

主要不足：

- cutout/glass 已有独立 wgpu pipeline 入口，但 shader 行为仍分别沿用现有 alpha test 与 glass 近似；additive-lightshaft 已有最小 additive pipeline，并已消费第一组 `lightshaft.shpk` 参数，但完整 lightshaft 节点行为尚未实现。
- 多套 UV 已开始通过 prepared source 和 per-role UV scroll 参与采样；Map1/UV1Scroll、tile/detail arrays 与 Flow primary tangent 已参与 shading，但 secondary tangent frame、`color1`、`flow1`、detail/multi maps 的完整解释仍未完成。
- alpha/glass/transparency 已从固定 glass opacity 前进到 prepared alpha source：character glass/transparency 强制进入对应 pass，normal B 驱动 alpha，`EnableLighting` 可控制 transparency lighting；`DrawDepthMode_Dither` 已驱动专用 depth-only prepass，使用与颜色 pass 一致的 prepared alpha source 和稳定 4x4 屏幕空间有序阈值。`GlassBlendMode` 已作为显式 scene option 进入 renderer/Web/snapshot，但 Mul 仍保留现有 alpha-blend 近似，Add 只选择硬件 additive pipeline；折射和真实厚度传输仍缺失，且尚无真实 charactertransparency 武器样本。Meddle 只确认 `DrawDepthMode_Dither` 的 material key/value 与适用 SHPK，没有游戏抖动公式；MeddleTools 不实现运行时 depth pass，因此当前公式是保守近似。它仍与 scene-level `ApplyDitherClip` 区分，后者覆盖更多 shader family。`GlassBlendMode` 也只有 scene key 的 Mul/Add 名字与默认值，没有 MTRL 来源或 MeddleTools 节点语义，因此没有写入 parsed material。
- `characterstockings.shpk` 已按 MeddleTools `meddle character.shpk` 的 `IS_STOCKING` 节点行为把最终 alpha source 和普通 surface render pass 都固定为 Opaque；即使静态 base alpha 或 alpha-test 预分类为 Mask/Blend，也不会误进 Cutout/Transparent。Glass/Crest/LightShaft 等 mesh draw role 仍保持更高优先级。Meddle `OnRenderMaterialUtil` 同时证明 stockings 会复制 runtime skin material textures 并应用 legacy body decal；离线 loader 尚无这些运行时输入，因此仍保留 `incompleteShaderFamilyLogic`，不宣称完整支持。
- renderer debug view 已能切换 base、normal、mask/material、specular、emissive、alpha、UV、vertex color、mesh/draw-role color、ColorTable index、material map、multi map、ColorTable extra maps 与四种 array 选层结果；更细的 per-texture independent sampler policy 仍未实现。

计划：

1. 先让 prepared pass 真正分管 pipeline：additive lightshaft 已有最小管线；后续继续拆独立 cutout、transparent/glass 行为，保持现有视觉输出尽量稳定，并补 synthetic pipeline tests。
2. 让 WGSL 继续按 prepared UV source 和 feature flags 消费更多通道：per-role scroll、Map1/UV1Scroll、tile/detail 和 Flow primary tangent 已接入，后续优先补 multi map mask，再做 secondary normal/bitangent、flow1 与 color1。
3. 按 shader family 拆函数而不是继续堆主函数：base color、normal、material properties、alpha、emissive、glass、tile/sheen/sphere、scroll/reflection 分块，先用分支承载，必要时再拆 shader module/pipeline。
4. 继续补 debug render modes：base、normal、mask/material、specular、emissive、alpha、UV set、vertex color、mesh/draw-role color、ColorTable index、material map、multi map、ColorTable extra maps 与四种 array 选层结果已可检查；后续补 per-texture independent sampler policy，并继续把这些视图作为真实武器样本回归的主要判断工具。

## 1. 数据解析改进

### P0: 让已解析语义可审计

需要补齐 debug 输出，确保之后修 shader 时可以定位是“数据没读到”还是“渲染没用”。

- 已完成：在 ignored phantom snapshot 的 `model-summary.json` 中保留每个 mesh 的 category、submesh attribute names、bone table、shape 信息摘要，并链接对应 full metadata JSON。
- 已完成：对副手/子模型加载失败不再完全吞掉；`WeaponModelData.loadDiagnostics` 会记录候选路径、missing/read/parse 状态和错误原因。
- 已完成：在 material debug 中明确列出 sampler role 的来源；目前覆盖 `.shpk` resource name、known CRC 和 unknown，文件名后缀来源仍应在 prepared texture config 中补齐。
- 已完成：给每个材质输出 shader keys、resolved constants、shader flags、texture flags、sampler flags 的紧凑摘要，便于和 MeddleTools 节点输入对照；resolved 值会标注 `shaderPackageDefault` 或 `materialOverride` 来源。
- 已完成：给 `PreparedMaterial` 增加显式 unsupported/runtime-only 输入摘要，区分 `dyeApplication`、`runtimeColorTable`、`decalOrCrest`、`runtimeMaterialChange`、`tileArray`、`detailArray`、`incompleteShaderFamilyLogic`。目标是让 snapshot 能区分“资源确实不存在”和“离线 Web/prepared/renderer 层还没有支持”。
- 已完成：共享 tile/detail arrays 从 SqPack 读取并按 TEX header `ArraySize` 解码为 vertical atlas；`ModelTexture` 保留 `arraySize/arrayLayerHeight`，材质与 prepared 层保留四个数组索引、错误和成对完整性。
- 已完成：按 Meddle on-render 语义拆分 `MaterialChange` / `CrestChange`，并在 prepared 层分别记录基础材质 fallback 与透明纹理 fallback。

验证：

- 扩展 ignored snapshot 测试生成的 `model-summary.json`，让 P0/P1 样本能直接看到缺失资源、sampler 来源和 mesh category。
- 为副手材质反推、sampler role 来源、shader constant override 增加 focused tests。
- 已增加 material semantic summary focused test，覆盖 key/constant 来源、known name、texture flags 与 sampler flags。
- 已增加 unsupported/runtime-only focused test，覆盖染色表存在、tile/detail 参数非默认、crest/material-change 独立标记、runtime fallback、共享数组完整性和特殊 shader family 标记。
- 已增加 ignored 真实 SqPack 测试，验证四个数组的层数、vertical-atlas 尺寸和 RGBA 长度；并用 `45052` 验证 character 材质挂载 tile normal/ORB、loaded path 与 prepared availability。

### P0: 修正文档和实现不一致

已完成：`weapon-render-pipeline.md` 已同步 Legacy ColorTable bake、mesh-level transparent sorting、GPU material bind group、顶点字段保留和剩余限制。

- 后续 roadmap 仍以本文为准，避免把独立 cutout/glass/lightshaft pipeline、submesh/shape visibility、tile/detail array 等未完成内容写成当前能力。

验证：

- 文档审查即可，不需要代码测试。

### P1: 扩展材质参数解析范围

现有 `.shpk` 解析主要用于 sampler role、material key 和常量默认值。后续需要把常见 character/bg/lightshaft/scroll/glass 参数提升成结构化字段，而不是只留在 debug。

当前进度：

- 已完成：`g_AlphaThreshold` 进入 `ModelMaterial.alphaThreshold`，用于 cutout discard 阈值。
- 已完成：`g_NormalScale` 进入 `ModelMaterial.normalScale`，默认 1.0，材质 override 优先于 shader package default，renderer 会 clamp 到 0..4 后作用于 normal map XY 强度。
- 已完成：`g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale` 进入 `ModelMaterial`，默认 1.0，材质 override 优先于 shader package default；renderer uniform 已进入 WGSL，detail pair 可用时会组合真实 detail/multi-detail normal，缺图时才作为 primary normal map 强度的低权重 fallback；独立 multi normal map 与 family-specific 权重仍待后续实现。
- 已完成：`g_TileIndex`、`g_TileAlpha`、`g_TileScale` 进入 `ModelMaterial`，默认值分别为 `0`、`1`、`[16,16]`；renderer 已优先按 ColorTable tile properties、回退按 `g_TileIndex` 选择真实 tile normal/ORB atlas layer，并结合 TileMatrix/TileScale 采样。ORB 通道和组合权重仍是保守解释。
- 已完成：完整 toon/sheen/sphere 参数族进入 `ModelMaterial`；其中 `g_ToonLightSpecAperture=50`、`g_ToonReflectionScale=2.5`、`g_ToonSpecIndex≈0` 已补齐同步/异步 composition、known constant label、phantom summary、renderer uniform 和 focused tests。prepared `usesToon` 限定 character family；WGSL 默认启用克制的解析式 toon，`g_SheenRate` / `g_SheenTintRate` / `g_SheenAperture` 与 `g_SphereMapIndex` 保持既有消费。45052 默认/strong override snapshot 已验证参数方向。
- 已完成：`g_DetailID`、`g_MultiDetailID`、`g_DetailColor`、`g_MultiDetailColor`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 进入 `ModelMaterial`，默认值分别为 `0`、`0`、`[0.5,0.5,0.5,1]`、`[0.5,0.5,0.5,1]`、`[4,4,4,4]`、`[4,4,4,4]`；renderer 已按两个 ID 和各自 UV scale 采样真实 detail diffuse/normal atlas。缺图时保留轻量 tint fallback，真实 bg 样本下的混合权重仍待校准。
- 已完成：`g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_MultiEmissiveColor` 进入 `ModelMaterial`，默认值分别为白色、白色、黑色、黑色；renderer uniform 已进入 WGSL，当前 `g_DiffuseColor` 会调制 base，`g_MultiDiffuseColor` 会在 mask/material 通道存在时作为低权重 base tint 补充，`g_EmissiveColor` 会加到 emissive，`g_MultiEmissiveColor` 只在 mask/material 通道存在时保守加权；完整 multi map 通道解释仍待后续实现。
- 已完成：`g_OutlineColor`、`g_OutlineWidth`、`g_SpecularColorMask`、`g_SSAOMask`、`g_TextureMipBias`、`g_ShadowPosOffset` 进入 `ModelMaterial`，默认值分别为黑色、`0`、白色、`1`、`0`、`0`；renderer uniform 已进入 WGSL。`PreparedMaterialFeatureFlags.usesOutline` 会筛选 character family 的正有限宽度；独立 outline pipeline 使用 `vs_outline/fs_outline`、front-face culling、depth test 开启但不写 depth，原 fragment rim 混色已删除。`g_SpecularColorMask` 和 `g_SSAOMask` 保持现有消费；texture LOD 和 shadow offset 仍待后续实现。
- 已完成：`g_GlassIOR`、`g_GlassThicknessMax` 进入 `ModelMaterial`，默认值分别为 `1`、`0.01`；renderer uniform 已进入 WGSL，当前在非默认时轻量调节 glass tint、specular 与 rim fresnel；opacity、折射与真实厚度传输仍待后续 shader-family 语义确认。
- 已完成：`g_AlphaAperture`、`g_AlphaOffset`、`g_ShadowAlphaThreshold` 进入 `ModelMaterial`，默认值分别为 `2`、`0`、`0.5`；renderer uniform 已进入 WGSL，其中 aperture/offset 非默认时会对非 glass/lightshaft alpha 做受限 shaping；shadow alpha 与 transparency opacity 仍待后续 shader-family 语义确认。
- 已完成：`g_UVScrollTime` / `0x9A696A17` 进入 `ModelMaterial.uvScroll`，按 MeddleTools 映射转换为 `[-x, y, -z, w]`；`bguvscroll.shpk` 的 Map0/Map1 分别使用 UV0Scroll/UV1Scroll，`GetMultiValues` 以 vertex alpha 混合两套 color/alpha/normal/specular，其它 role 与 `characterscroll.shpk` 不继承该动画。
- 已完成：`lightshaft.shpk` 的 `g_Color`、`g_TexAnim`、`g_TexU`、`g_TexV`、`g_Ray` 进入 `ModelMaterial`，默认值分别为白色、零动画、identity U/V 和零 ray；renderer uniform 已按 draw role 只对 lightShaft batch 启用保守消费，其中 `g_Color` 控制 additive tint/alpha，`g_TexAnim.xy` 驱动 UV 动画，`g_TexU/V` 作为 UV 仿射基向量，`g_Ray` 当前只作强度近似。
- 已完成：`g_Transparency` 进入 `ModelMaterial.transparency`，material override 优先并 clamp 到 0..1；water prepared pass/alpha source 与 WGSL 已直接消费，character/glass 不受影响。`g_WaterDeepColor/g_RefractionColor/g_WhitecapColor` 及三种 water sampler role 也已结构化，primary wave 已复用 normal binding。

后续优先参数：

- `GlassBlendMode`、dither depth、water alpha/base/primary wave 与 Map1/UV1Scroll 已完成；后续补 reflection/stockings/tattoo/occlusion。water refraction/whitecap/WaveMap1 与 AlphaMulti variants 等待真实节点连接或游戏 shader 证据。

验证：

- 用合成 MTRL fixture 测 shader constant 解析。
- 已增加 normal scale focused tests，覆盖 primary/multi/detail normal scale 的 shader package default、material override 和 clamp；multi/detail normal scale fallback 的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 tile select focused tests，覆盖 `g_TileIndex`、`g_TileAlpha`、`g_TileScale` 的 shader package default、material override 和 renderer uniform 传递；tile specular fallback 的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 toon/sheen/sphere focused tests，覆盖 `g_ToonIndex`、`g_ToonLightScale`、`g_SheenRate`、`g_SheenTintRate`、`g_SheenAperture`、`g_SphereMapIndex` 的 shader package default、material override、非 finite fallback 和 renderer uniform 传递；sheen/sphere 常量的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 detail focused tests，覆盖 `g_DetailID`、`g_MultiDetailID`、`g_DetailColor`、`g_MultiDetailColor`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 的 shader package default、material override、短数组 fallback、非 finite fallback 和 renderer uniform 传递；detail tint fallback 的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 shader color focused tests，覆盖 `g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_MultiEmissiveColor` 的 shader package default、material override、短数组 fallback、非 finite fallback 和 renderer uniform 传递；diffuse/multi diffuse 与 emissive/multi emissive 的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 outline/specular/occlusion focused tests，覆盖 `g_OutlineColor`、`g_OutlineWidth`、`g_SpecularColorMask`、`g_SSAOMask`、`g_TextureMipBias`、`g_ShadowPosOffset` 的 shader package default、material override、短数组 fallback、非 finite fallback 和 renderer uniform 传递；outline rim fallback、`g_SpecularColorMask` / `g_SSAOMask` 的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 glass params focused tests，覆盖 `g_GlassIOR`、`g_GlassThicknessMax` 的 shader package default、material override、非 finite fallback 和 renderer uniform 传递；glass IOR/thickness 的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 alpha params focused tests，覆盖 `g_AlphaAperture`、`g_AlphaOffset`、`g_ShadowAlphaThreshold` 的 shader package default、material override、clamp、非 finite fallback 和 renderer uniform 传递；aperture/offset alpha shaping 的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 UV scroll focused tests，覆盖 `g_UVScrollTime` / `0x9A696A17` 的 shader package default、material override、MeddleTools U 轴取反、renderer uniform 传递和默认时间稳定性。
- 已增加 role-specific scroll/flow focused tests，覆盖 bguvscroll primary role mask、静态 role、零 multiplier、CharacterScroll 不自动滚动、Flow key default/override/unknown、Standard + flow attribute 不启用、Flow + flow0 才启用、flow1-only 不启用，以及 renderer scroll mask/flow uniform 编码；native WGPU snapshot 已验证 WGSL/bind layout，45052/45059 phantom 已回归。
- 已增加 Map1/UV1Scroll focused tests，覆盖三种 secondary sampler role、全部 `GetValues` 枚举、secondary alpha classification、Non-Color specular policy、prepared UV1 source/scroll mask、unsupported AlphaMulti variants、renderer binding 复用与 presence flags。ignored native synthetic bguvscroll 以 alpha-test 材质确认 vertex alpha 0/1 分别保留红色 Map0 和蓝色 Map1，且不会把 blend weight 二次乘到 opacity；45052/45059 phantom 已按正确 `game-data,render-test-support` feature 组合回归。
- 已增加 `GetSubColor` focused tests，覆盖 key 缺失、shader package default、material override、Face/Hair/Unknown 模式，以及 prepared mode 保真和 characterOcclusion/runtime sub-color diagnostics。
- 已增加 lightshaft focused tests，覆盖 `g_Color`、`g_TexAnim`、`g_TexU`、`g_TexV`、`g_Ray` 的 shader package default、material override、renderer uniform 默认值和 LightShaft draw-role shader 开关。
- 已增加 `g_Transparency` focused tests，覆盖 water/river family 默认 1.0、其它 family 默认 0.0、shader package default、material override 和 clamp。
- 已增加 water focused tests，覆盖 deep/refraction/whitecap constant default/override/fallback、三种 sampler role、specialized texture 不参与 base fallback、prepared direct-alpha/pass、primary wave effective normal、uniform finite fallback 与 Web/wasm 枚举消费。ignored native synthetic water 会比较解码 RGBA，确认 alpha 0.35/1.0 与 wave/flat normal 均产生可见差异；45052/45059 phantom 已回归。当前 phantom 列表没有真实 water/river 武器样本。
- 用本地 SqPack 样本输出 material debug，对照 MeddleTools `node_configs.py` 中对应 mapping。

### P1: 处理染色数据入口

当前已把 Legacy/Dawntrail `ColorDyeTable` 的 template、channel 和可染通道 flag 结构化到 `ModelMaterial.colorDyeTable`；`ModelMaterial.hasColorDyeTable` 继续作为兼容字段，`PreparedMaterialFeatureFlags.usesDye` 和 `unsupportedInputs.dyeApplication` 会识别两种入口。现有 material debug 复用同一转换逻辑。

数据层还已实现：

- Legacy `chara/base_material/stainingtemplate.stm` v1.1 parser。
- Dawntrail `chara/base_material/stainingtemplate_gud.stm` v2.0/v2.1 parser。
- 254 个 1-based stain ID 的 singleton、direct、indexed palette column 解码。
- Legacy/Dawntrail dye pack lookup。
- 按 dye row channel/flags 覆盖 `ColorTableRowColors` 的纯函数；包含黑色 specular 不覆盖、Legacy-on-Dawntrail fallback 和 kind mismatch/missing input 诊断。

请求级 stain IDs 已接入 `WeaponModelLoadRequest`、同步/异步 resource path、resource key、`WeaponModelData` 和 `ModelMaterial.stainingApplication`。材质会先把原始 ColorTable 转成统一 rows，应用 STM，再用同一份 rows 计算 summary 与 bake textures；无 stain 请求不会读取 STM。

EXD 与 Web 输入已接入：

- `WeaponCatalogPackage.stains` 保存 `Stain` ID、名称、原始 BGR 色值、转换后的 UI RGBA、shade、sub-order 和 metallic；浏览器本地 SqPack 目录缓存已升级版本并预加载 `Item` 与 `Stain` sheet
- Web stain0/stain1 选择器按 `shade/sub-order/id` 排序，显示色块和 metallic 标记；URL 使用 `stain0/stain1`，模型资源 key、canvas key 和旧结果过滤都包含两个 stain ID
- 当前本地 `Stain` EXD 有 125 个具名行；STM 的 254 是 1-based 调色板容量上限，不代表 EXD 必须有 254 个可选名称

离线 Web 预览不能拿到 Meddle 的 runtime GPU ColorTable，因此需要在静态 MTRL ColorTable bake 前应用 STM。Meddle 从运行时装备实例读取 stain0/stain1，不存在可从静态 Item 可靠推断的默认 stain。

验证：

- 已增加 focused tests，覆盖 Legacy/Dawntrail `ColorDyeTable` 到结构化 model rows 的字段保真，以及 structured table 独立于旧 bool 时仍会触发 prepared dye/unsupported 标记。
- 已增加 STM focused tests，覆盖 Legacy/GUD header、u16/u32 key、singleton/direct/indexed column、1-based stain ID、单色/双色 channel、逐 flag 覆盖、黑色 specular 保留和 kind mismatch。
- 已用本地 SqPack ignored test 验证当前 Legacy STM 为 v1.1、GUD STM 为 v2.1，均有 43 个模板；key 范围分别为 100..612 与 1100..1612，验证了 GUD 到 Legacy 的 `-1000` fallback 关系。
- 已用真实武器 `45052` 做 ignored integration test：stain `[1,0]` 只处理启用 dye flags 的行，application report 无 missing template/error，且 baked base texture 与 `[0,0]` 未染色版本不同。
- 已用本地 `Stain` sheet ignored test 验证当前 125 个具名染剂、BGR 到 RGBA 转换、shade/sub-order 排序和 metallic 字段；Web URL parser 覆盖双通道与保留值 255 拒绝。
- phantom fixture schema v2 已允许每个 case 指定 `stainIds`；`45052 奶油之幻梦` 的 `[0,0]` 与 `[1,0]` 会走正式 `WeaponModelLoadRequest` 同轮出图。染色 case 会断言返回 stain IDs 一致且至少一个材质 `rowsChanged > 0`；当前两张 PNG 哈希不同，染色材质报告 `rowsChanged=2`、missing=0、error=None。
- 后续再选第二通道和 metallic 染色效果明显的武器扩充 visual snapshot，而不是只依赖单通道素雪白样本。

### P2: 解析更多 runtime-only 或 shader-specific 信息的替代输入

Meddle 能从运行时拿到：

- resolved model/material/texture handle path
- GPU ColorTable texture
- on-render material output
- decal/crest texture
- character customize buffers

Web 离线模式拿不到这些，需要决定哪些提供替代输入。

建议：

- weapon preview 先支持 decal/crest 的显式空白 fallback，不阻塞主模型。
- 对运行时 ColorTable 先不模拟，只保证静态 MTRL + 染色路径正确。
- 对 texture handle runtime remap 先作为 debug 缺口记录，不强行猜。

## 2. 解析后的结果处理改进

这一层负责把 raw parsed data 转成 renderer-friendly 的中间表示。当前这层较薄，很多决策直接塞进 material fields 或 WGSL。建议显式引入 material preparation 阶段。

### P0: 建立 `PreparedModel` / `PreparedMaterial` 概念

目标是把“原始资源解析结果”和“渲染器可以直接绑定的数据”分开。

当前进度：

- 已先抽出 `ModelMeshDrawRole` 作为 mesh-level preparation 语义，并被 renderer 和 phantom summary 共用。
- 已增加第一版公共 `PreparedModel` / `PreparedMesh`，把每个 raw mesh 的 `meshIndex`、`materialSlot`、draw role、main-pass 可见性和 prepared material 汇总到数据层；renderer flatten 与 phantom summary 已改为消费该结果。
- 已增加第一版公共 `PreparedMaterial`，把 material alpha/render mode 与 mesh draw role 合成 `Opaque`、`Cutout`、`Transparent`、`Glass` 四类 prepared render pass，同时保留 culling policy。
- `PreparedMaterial` 已包含第一版 `MaterialShaderFamily` 分类，覆盖 MeddleTools 映射中的 character/glass/transparency/scroll/bg/lightshaft/water 常见包。
- `PreparedMaterial` 已包含第一版 `PreparedTextureBindings`，聚合 renderer 当前已知的材质贴图索引。
- `PreparedMaterial` 已包含第一版 `PreparedTextureSamplingSet`，把 texture role 的 sRGB/Non-Color、linear/nearest、repeat/clip 语义从 renderer 私有实现中拆出来；renderer 已消费 repeat/clamp/linear/nearest，`Clip` 目前在 sampler 层降级为 clamp，等待 shader 级 clip 逻辑。
- `PreparedMaterial` 已包含 `PreparedMaterialFeatureFlags`，聚合 `usesVertexColor`、`usesColorTable`、`usesTile`、`usesDetail`、`usesScroll`、`usesFlow` 与 `usesDye`；flow 已结合 material mode + mesh flow0，scroll 已结合非零 multiplier + per-role mask。
- `PreparedMaterial` 已包含 `PreparedMaterialUvSources` 与 `PreparedTextureScrollSet`；renderer 按 texture-role source 选择采样 UV，并只对 mask 明确启用的 role 应用对应 UV0/UV1 multiplier。
- `PreparedMaterial` 已包含第一版 `PreparedMaterialUnsupportedInputs`，会把 dye application、runtime ColorTable、decal/crest、runtime material change、tile/detail array 和特殊 shader family 行为缺口输出到 prepared summary。
- `PreparedMaterial` 已包含 `resourceAvailability` 和 `runtimeFallbacks`：共享数组是否成对完整、crest/decal 缺失时透明纹理 fallback、materialChange 的基础材质 fallback 均可审计。
- phantom `model-summary.json` 会在主 surface mesh 上输出 prepared material 决策，并通过第一版 `PreparedModel` 获得 mesh draw role / main pass 可见性。
- `PreparedModel` 仍是第一版：已包含 submesh attribute mask/name，并新增 `PreparedModelOptions.enabledAttributeMask` 与 `PreparedMeshVisibility`，可在显式提供运行时 enabled attribute mask 时按 Meddle composer 语义隐藏 disabled submesh；mesh-level shape influence 已进入 `ModelMesh` / `PreparedMesh`，`PreparedModelOptions.enabledShapeMask` 可标出 active/inactive influence 但不改变 draw visibility；mesh-level flow presence 已进入 prepared material feature flags；第一版 UV source 已驱动 renderer 采样选择；尚未包含实际 shape morph、skinning/morph 或 per-submesh prepared draw；sampler config、feature flags 与 shader-family-specific UV source 仍未完整驱动所有 runtime 绑定。
- 当前缺口：stain template/application 与 tile/detail array 已有真实数据入口和第一版 renderer 行为；runtime GPU ColorTable 仍无离线替代输入，真实 crest/decal 内容仍不可用，特殊 shader family 的 unsupported 标记仍主要是审计信息。

建议中间结构包含：

- mesh draw role：normal、glass、lightShaft、shadowOnly、ignored、materialChange、crestChange；已有第一版 `PreparedMesh`，并保留 submesh attribute mask/name、attribute visibility 决策与 shape influence active/inactive 状态
- material shader family：character、characterStockings、characterGlass、characterReflection、characterTransparency、characterScroll、characterTattoo、characterOcclusion、bg、lightShaft、unknown；已有第一版分类，后续逐个补 shader-family-specific 行为
- texture bindings：base、normal、mask、material、multi、specular、emissive、tile/sheen/sphere/tileMatrix、ColorTable index，以及 tile normal/ORB、detail diffuse/normal arrays 已有第一版；renderer 将四张数组合并为两个 GPU pair atlas，并复用 nearest sampler 做选层采样
- UV source：每个 texture 或 shader family 应使用 uv0/uv1/uv2/uv3 哪一套；已有第一版 texture-role 默认与 scroll uv0/uv1 来源，后续还要补 shader-family-specific 规则
- alpha policy：opaque、cutout、blend、glass、additive/lightshaft；`AdditiveLightShaft` 已作为 prepared pass 分类存在，并进入最小 wgpu additive pass
- culling policy：render backfaces / cull backfaces
- feature flags：usesVertexColor、usesFlow、usesColorTable、usesDye、usesScroll、usesTile、usesDetail；`usesFlow` 已收紧为 Flow material mode + mesh flow0 presence，`usesScroll` 已收紧为非零 multiplier + 可滚动 texture role；`usesDye` 由兼容 bool 或结构化 dye table 驱动，实际 stain 输入已通过 `WeaponModelLoadRequest.stainIds` 接入
- unsupported/runtime-only inputs：dye application、runtime ColorTable、decal/crest、runtime material change、tile/detail array、shader-family-specific incomplete behavior；已有第一版，只基于当前可可靠判断的数据置位，不猜测运行时状态

好处：

- renderer 不需要理解所有 raw MTRL 细节。
- 后续支持多个 shader family 时，不会继续膨胀 `ModelMaterial`。
- snapshot/debug 可以输出 preparation 决策，定位问题更快。

验证：

- 已增加 focused tests 断言 `PreparedModel` mesh-level 决策、submesh attribute metadata 传播、显式 enabled attribute mask visibility、mesh-level shape influence active/inactive 状态、mesh-level flow feature flag、alpha policy、lightshaft additive prepared pass 与 mesh glass override 到 prepared render pass 的映射、shader family 分类、texture bindings 聚合、texture sampling policy、feature flags、prepared UV source 及 renderer uniform 编码，以及 culling policy fallback。
- 后续仍需要用真实样本验证 sampler policy / UV source 到 renderer 绑定的覆盖率。
- P0/P1 phantom weapon 样本已具备 prepared summary 字段，仍需要跑 ignored snapshot 对比真实输出。

### P0: mesh category 和 submesh attribute 决策前置

已完成第一步：raw mesh category 会通过 `ModelMeshDrawRole` 转成 draw role，并进入第一版 `PreparedModel`；renderer 不再全部当普通 mesh 画。

当前策略：

- `normal`、`glass` 默认渲染。
- `glass` 会强制进入 transparent pass。
- `lightShaft` 已标为独立 draw role，并映射到 `AdditiveLightShaft` prepared pass；renderer 会保留为 additive batch，不再作为普通 surface 绘制。
- `shadow`、`terrainShadow`、`verticalFog` 默认不作为主 surface 渲染。
- `materialChange`、`crestChange` 已拆为独立 draw role，暂时继续进入主 pass；prepared summary 分别标出基础材质与透明纹理 fallback，但 renderer 尚未消费该 fallback。
- submesh attribute mask/name 已进入 `ModelMesh` 与 `PreparedMesh`，并随 phantom summary 输出；`PreparedModelOptions.enabledAttributeMask` 已支持显式运行时 mask，按 `requiredMask & !enabledMask == 0` 判断 submesh 是否可见。mesh-level shape influence 已进入 `ModelMesh` 与 `PreparedMesh`，`PreparedModelOptions.enabledShapeMask` 已支持 active/inactive 审计；Web 离线默认仍不猜 mask，实际 shape morph 仍未应用。

验证：

- 已增加 draw role mapping 单元测试和 renderer flatten 测试，覆盖 glass/terrainShadow/lightShaft/shadow/verticalFog/materialChange/crestChange，并确认 lightShaft 作为 additive batch 保留。
- 对 `冬雪之幻梦`、`茶歇之幻梦` 等样本做 snapshot 对比。

### P1: 把 ColorTable bake 产物转成稳定语义贴图集合

ColorTable bake 已能产出多张贴图，但目前 renderer 只消费其中一部分。

建议整理为：

- `colorset_diffuse`: sRGB, alpha 仅由 alpha policy 决定，不使用 TileAlpha 当材质透明度
- `colorset_specular`: sRGB + anisotropy alpha
- `colorset_material`: metalness / roughness / gloss strength / specular strength
- `colorset_tile`: tile index / tile alpha
- `colorset_sheen`: sheen rate / sheen tint / sheen aperture
- `colorset_sphere`: sphere index / sphere mask
- `colorset_tile_matrix`: float channels 优先，RGBA8 仅 debug/fallback

处理规则：

- 如果材质已有 base texture，要明确是 multiply、replace 还是 shader-family-specific blend。
- `_id.tex` 必须以 nearest/closest 采样，不应使用 linear filtering；当前 prepared sampling policy 已表达这一点，renderer 已绑定 index texture 并在 debug view 中用 nearest sampler 预览。
- material/special maps 需要 Non-Color，不走 sRGB。

验证：

- 继续保留现有 bake 单元测试。
- 已增加 prepared texture config 测试，确保 index 使用 nearest，base/specular 使用预期色彩空间。

### P1: 引入 shader-family-specific texture interpretation

当前 `mask` 被直接当 RGB 参与 roughness/specular/metalness 近似；`material_map`、`multi_map` 基本没有用。

建议先支持 character family：

- base/color map
- normal map + normal scale：`g_NormalScale` 已实际用于 primary normal；`g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale` 已作为低权重 primary normal fallback，后续仍需要接入 shader-family-specific multi/detail normal map 组合
- mask/material map 的通道解释
- multi map/detail map 的第二层颜色/法线影响；detail color/UV scale、tile index/scale、multi/detail normal scale 与 bguvscroll Map0/Map1 已进入第一版组合；后续重点是校准 ORB/detail 权重和补 multi map mask
- vertex color 的具体启用条件

然后再支持：

- glass
- transparency
- scroll
- lightshaft
- reflection / stockings / tattoo / occlusion 等特殊 character family

验证：

- 每个 shader family 一个最小 fixture。
- P0/P1 真实武器 snapshot。

### P2: 离线 bake 路线

MeddleTools 会在 Blender 中通过节点图 bake diffuse、normal、roughness、glossy、transmission、emission。Web 端不适合复刻完整节点系统，但可以引入离线/构建期 bake 思路：

- 对复杂材质先生成 renderer-friendly atlas 或 baked maps。
- Web renderer 保持较简单的实时 shader。
- 原始解析结果仍保留，方便 debug 和后续重 bake。

适合处理：

- tile array
- sphere/reflection
- scroll 静态预览
- transmission/glass 近似图

## 3. 渲染器与着色器管线改进

### P0: 扩展 GPU 顶点格式

已完成第一步：GPU 顶点格式和 WGSL `VertexInput` 已上传/声明以下字段：

- `uv1`
- `uv2`
- `uv3`
- `color1`，缺省为 `[1, 1, 1, 1]`
- `normal1` / `bitangent1`，缺省回落到 primary normal/bitangent
- `flow0` / `flow1`，缺省为零；Meddle 导出映射确认它们是 tangent 语义，`flow0` 已由 Flow material mode 驱动 primary normal tangent frame，`flow1` 留给后续 secondary normal set

当前仍保持近似视觉行为：fragment shader 已按 prepared UV source 与 per-role mask 选择静态/滚动 UV，bguvscroll Map1 会消费 UV1Scroll，Flow 模式会消费 `flow0` tangent；其它 source 规则仍基本默认 `uv0`，primary normal/bitangent 与 `color0` 仍是主要输入。后续重点是其它 `uv1-uv3` 用途、`color1` 与 secondary tangent frame。

验证：

- 已增加单元测试 `GpuVertex::layout` stride/offset。
- 已增加 flatten 单元测试，确认 `ModelVertex` 的扩展字段不会在 CPU -> GPU 顶点转换时丢失，并覆盖 optional 字段 fallback。
- renderer 已把 `uv1-uv3`、`flow0/flow1` 传入 fragment，按 prepared UV source + scroll mask 选择各 texture role 的采样 UV，并用 prepared flow flag 选择 tangent frame。synthetic native Map1 快照已确认 vertex alpha 0/1 分别输出 primary 红色与 secondary 蓝色；真实客户端 Map1/Flow 样本仍待补充。

### P0: 按 prepared draw role 分 pass

当前进度：数据层已记录 prepared render pass：

- `Opaque`
- `Cutout`：已有独立 cutout pipeline，写 depth，并由 shader alpha test discard。
- `Transparent`：复用 transparent pipeline，不写 depth，参与 mesh-level sorting。
- `Glass`：已有独立 glass pipeline，不写 depth，参与 mesh-level sorting；当前仍使用透明 alpha blending 和现有 glass 近似参数。
- `AdditiveLightShaft`：lightshaft 已有 prepared 分类，renderer 会保留为 additive batch，使用加法混合且不写 depth。

当前仍未完成的是把 cutout/glass/lightshaft 行为做成更完整的 shader-family-specific 管线：

- opaque pass：写 depth
- cutout pass：已有独立 pipeline，写 depth，alpha test discard；尚未有 shader-family-specific cutout 行为
- transparent pass：不写 depth，mesh-level sorted
- glass pass：已有独立 pipeline，不写 depth，参与 mesh-level sorted；尚未有独立 blend/参数模型
- additive/lightshaft pass：已有最小加法混合、不写 depth；已解析并保守消费 `lightshaft.shpk` 的 `g_Color`、`g_TexAnim`、`g_TexU/V`、`g_Ray`，但尚未复刻完整 lightshaft 节点语义

shadow、terrainShadow、verticalFog 在主预览中默认不画，避免错误 surface；lightShaft 不作为普通 surface，但会通过 additive pass 绘制。

验证：

- 已增加 prepared material / render pass 单元测试，覆盖 opaque、cutout、transparent、glass、mesh glass override 和 culling policy。
- 后续仍需要透明/glass/lightshaft synthetic fixture。
- P0 样本 snapshot。

### P1: 着色器模块化

当前 `model.wgsl` 是单个近似 shader。后续应把逻辑拆为可维护的函数块：

- base color resolve
- normal resolve
- color table material properties resolve
- mask/material/multi map resolve
- alpha resolve
- emissive resolve
- glass resolve
- tile/sheen/sphere resolve

不一定要立刻多 pipeline shader module，但代码结构要能承载 shader family 分支。

验证：

- WGSL 编译通过。
- 现有 snapshot 不发生明显退化。

### P1: 消费 ColorTable extra ramps

当前进度：renderer 已开始使用已 bake 的：

- tile properties
- sheen properties
- sphere properties
- tile matrix

已完成第一步：

- material bind group 增加 tile/sheen/sphere/tile-matrix 四张 Non-Color texture。
- WGSL 使用 nearest sampler 采样这些 extra maps。
- tile alpha 只用于 specular scale 的保守调制，不作为材质透明度。
- sheen/sphere/tile-matrix 进入额外高光与 sphere-like rim 近似，确保数据流能产生可观察效果。
- renderer debug view 可直接预览 tile、sheen、sphere、tile-matrix 四张烘焙 ramp。
- 已增加 focused test，确认 extra map flags 只在 texture index 实际存在时启用。

仍建议的后续顺序：

1. 用更多真实 character 样本确认 TileMatrix 与 `g_TileScale` 的组合边界，并校准 ORB 通道权重。
2. 寻找真实 bg 武器样本，校准 detail/multi-detail diffuse、normal 与 mask 权重。
3. sphere 作为环境/反射近似，接入更接近 MeddleTools 的 reflection/sphere 节点。

这些贴图已经进入 shader binding 并提供 debug view；后续重点是校准 tile/detail 组合，并实现更接近 MeddleTools 的 reflection/sphere 节点。

验证：

- 已有 native snapshot 覆盖 bind layout/WGSL 编译；后续仍需要 synthetic ColorTable ramp 生成明显 tile/sheen/sphere 差异。
- 与 MeddleTools ramp 输出对照。

### P1: 改善 alpha/glass/transparency

character transparency/glass 与 water 已完成第一版 alpha source / prepared policy：

- `DrawDepthMode` / `EnableLighting` 已结构化并进入 `PreparedMaterial.alphaPolicy`；character glass/transparency 会强制进入 Glass/Transparent pass。
- MeddleTools `shaders.blend` 已确认普通 character alpha 使用 normal Blue、stockings 强制 1，而 `charactertattoo.shpk` 使用 normal Alpha。tattoo 已使用独立 `NormalAlpha` prepared source 和 WGSL 通道选择；native WGPU synthetic fixture 固定 normal B、只改变 A，并验证颜色输出随 A 变化。ColorTable baked base 不再写固定 glass alpha，glass 材质 opacity 不再重复乘 0.28。
- WGSL 已用 normal B 驱动 character glass/transparency alpha，`EnableLightingOff` 可关闭 transparency lighting，并提高 glass transmission tint，45059 灰暗球体回归已明显改善。
- `DrawDepthMode_Dither` 已执行专用 depth prepass：opaque/cutout 后、transparent/glass 颜色 pass 前，只选择 prepared depth mode 为 Dither 的透明 batch；WGSL 复用颜色 pass 的 prepared alpha source（包括普通 character 的 normal B 与 tattoo 的 normal A）计算，按稳定 4x4 屏幕空间阈值 discard；pipeline 只写 depth，两个颜色 target 的 write mask 为空，原透明颜色 pass 继续不写 depth。双面和背面剔除材质各有对应 pipeline。该公式是缺少游戏 shader 实现时的保守近似，不扩展为 `ApplyDitherClip` scene-key 行为。
- `GlassBlendMode` 已成为显式 `ModelGlassBlendMode::{Multiply, Additive}` renderer input。默认 Multiply 保持当前 alpha-blend glass pipeline；Additive 只把 Glass batch 切到 additive pipeline，不影响普通 Transparent 或 LightShaft 分类。Web 渲染面板已增加 Glass Mul/Add 选项；phantom harness 可通过 `XIV_PHANTOM_GLASS_BLEND=additive` 验证。45059 默认 Mul 保持原透明雪景，Add 会明显高亮，说明分派生效。由于没有真实 blend equation 证据，Multiply 暂不改成硬件乘法，Add 也只视为近似；折射、真实厚度和 scene color transmission 仍未实现。
- water/river 已确认并接入 `g_Transparency -> Alpha`、`g_WaterDeepColor -> Base Color`、primary wave RG normal；refraction/whitecap/WaveMap1 等待额外证据。
- 区分 cutout、blend、glass、additive。
- 对 alpha test 使用真实阈值和 shader package 规则。
- 透明排序保留 mesh-level，但为复杂模型预留 per-triangle 或 weighted blended OIT 方案。

验证：

- `冬雪之幻梦`：外壳透明且内部可见。
- `茶歇之幻梦`：透明/灰背面问题不回归。

### P1: 纹理采样配置

当前进度：数据层已有第一版 `PreparedTextureSamplingSet`，renderer 的 material bind group 已分出 color/data/nearest 三组 sampler，并从 prepared policy 派生对应 `wgpu::SamplerDescriptor`；WGSL 现在用 color sampler 采 base/specular/emissive，用 data sampler 采 normal/mask/material-properties/material/multi debug view，用 nearest sampler 采 index 与 ColorTable extra maps。

仍需要按 texture role 完整落地：

- base/specular/emissive: sRGB 视具体语义
- normal/mask/material/multi/material-properties: Non-Color；当前 renderer 已用 data sampler 采 normal/mask/material-properties，并在 material/multi debug view 中用 data sampler 预览 material/multi
- index: nearest/closest，当前 renderer 已绑定 `_id.tex` 并在 ColorTable row index debug view 中用 nearest sampler 预览；尚未进入正常 shader 着色
- tile/detail arrays 与 ColorTable extra maps: nearest 或 shader-family-specific；当前 renderer 已用 nearest sampler 消费 ColorTable extra maps 和两个 pair atlas，共享 arrays 按 Non-Color + nearest + repeat 采样
- decal: clip/extend 语义；decal/crest 已确认为 runtime-only on-render texture，并有透明 fallback 元数据，当前尚无显式 runtime texture 输入和独立 GPU binding

WebGPU bind group 已支持每材质 color/data/nearest 三组 sampler；`Clip` address policy 当前只能在 sampler descriptor 层降级为 clamp，后续 decal 或 face override 仍需要 shader 级 UV clip/extend 行为。

验证：

- 已有 prepared texture sampling 测试覆盖 `_id.tex` nearest policy，避免颜色边界被 linear 混合污染。
- renderer 已用 `Rgba8Unorm` 创建 normal/mask/material-properties，并用 data sampler 采样；ColorTable extra maps 已用 nearest sampler 采样并提供 debug view；三组 sampler descriptor 已由 prepared sampling policy 派生。仍需 synthetic shader fixture 验证 tile/sheen/sphere 可见效果。

### P2: 视觉验证和调试视图

UI 和 snapshot/test render options 已加入第一版 debug render mode：

- base color
- normal
- mask/material properties
- specular
- emissive
- alpha
- UV set preview
- vertex color preview
- mesh category / draw role colors
- ColorTable row index preview
- material map / multi map preview
- ColorTable tile / sheen / sphere / tile-matrix ramp preview

仍待补：

- per-texture sampler policy preview / independent sampler binding
- 已完成 tile normal/ORB、detail diffuse/normal array preview；detail 仍需真实 bg 武器样本校准

这些视图能显著缩短后续对照 Meddle/MeddleTools 的定位时间。

## 建议实施顺序

### 当前下一批工作队列

从当前状态继续推进时，优先级应按依赖关系排：

1. 数据解析：共享 arrays 与 runtime fallback 本轮已贯通；stockings/tattoo runtime input diagnostics、Environment role 与 `GetSubColor` Face/Hair mode 均已结构化，occlusion 的 runtime sub-color 依赖已进入 prepared diagnostics。下一步继续 shader-family-specific texture role/UV 规则和真实样本调查。runtime GPU ColorTable 继续只作为 unsupported 输入标记。
2. 结果处理：Crystal/Environment 的“已解析、未渲染”状态已进入明确 prepared family/feature/unsupported 字段；`multiMapInterpretation` 也已区分共享 detail array 缺失与 MultiMap 通道未实现。下一步把当前在 WGSL 中的 tile layer、detail layer、ORB 通道和组合权重逐步提升为更明确的 prepared 规则，尤其处理 TileMatrix float channels、detail/multi-detail mask 和越界诊断，减少 renderer 内部猜测。
3. 渲染器：characterTransparency/glass、dither depth、GlassBlend、outline、toon、bguvscroll Map0/Map1、Flow、stockings opaque alpha/pipeline、tattoo normal-A alpha 与 water direct alpha/deep color/primary wave 已完成第一版。tattoo 的 OptionColor/DecalColor 混色仍不猜测。secondary color/normal/specular 只在 `BgUvScroll + GetMultiValues` 中复用 tile/sheen/sphere 物理 binding，按 UV1Scroll 和 vertex alpha 混合；其它 `GetValues` 变体保持显式未支持，sampled texture 数仍为 15。characterReflection generic approximation 与 stockings/tattoo/occlusion runtime 输入均已有独立 diagnostic；后续再寻找真实 reflection 节点/样本。
4. runtime 输入：默认 crest/decal 透明 fallback 与 materialChange 基础材质 fallback 已执行；后续只在调用方能提供真实 on-render texture 时增加显式输入，不从静态 MTRL 伪造。
5. 验证：继续扩充第二通道/metallic 染色 case，寻找 bg detail 样本，并为 transparency/glass/scroll 等下一批行为增加 synthetic 与真实 snapshot。

### 第一阶段：可审计和不误画

1. 已完成：更新过期文档。
2. 已完成：增强 summary/debug 输出。
3. 已完成：引入 prepared draw role。
4. 已完成：默认过滤 shadow/terrainShadow/verticalFog。
5. 已完成：记录副手加载失败。
6. 已完成：在 prepared summary 中补 runtime-only / unsupported inputs 审计字段。

完成标准：

- P0/P1 snapshot summary 能说明每个 mesh/material 为什么这样画。
- 不再把明显非 surface mesh 当普通材质误画。当前 shadow/terrainShadow/verticalFog 已默认不进主 surface pass；lightShaft 已从普通 surface 分离并进入 additive pass。

### 第二阶段：让解析结果真正进 shader

1. 已完成 GPU 顶点格式扩展；prepared UV source、per-role scroll、Map1/UV1Scroll 和 Flow primary tangent 已进入 WGSL。后续让 shader-family 逻辑继续消费其它 `uv1-uv3` 用途、`color1`、secondary tangent frame 和 `flow1`。
2. 已完成第一版 per-material texture/sampler config，renderer 已派生 color/data/nearest 三组 sampler；共享 tile/detail arrays 已进入两个 GPU pair atlas、选层采样与 debug view。后续补 per-texture independent sampler 和 shader 级 clip/extend。
3. ColorTable diffuse/specular/material-properties/tile/sheen/sphere/tile-matrix 与共享 tile/detail arrays 已进入 renderer；`g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_OutlineColor/g_OutlineWidth`、`g_SpecularColorMask`、`g_SSAOMask`、sheen/sphere 常量、detail tint/normal、tile normal/ORB、alpha aperture/offset 和 glass IOR/thickness 已有第一版 WGSL 消费。后续补 shader-family-specific source/scroll 和更准确的通道权重。
4. 已完成第一版 shader family 分类和 alpha policy/prepared pass 分类；后续把 character/glass/transparency/scroll/lightshaft/reflection 等 family 的关键节点拆成明确 WGSL 函数块，而不是继续扩大单个主 shader 分支。

完成标准：

- MeddleTools 中 ColorTable extra ramp 对应的数据能在 Web renderer 中产生可观察效果。
- `_id.tex` 边界、mask/material map 色彩空间不再依赖统一 sampler 侥幸工作。

### 第三阶段：shader family 和运行时替代输入

1. character glass/transparency/scroll/lightshaft/reflection/stockings/tattoo/occlusion 逐个补齐；这些 shader package 已进入 `MaterialShaderFamily` 分类，stockings 已对齐可由静态输入证明的 opaque alpha/pipeline，tattoo 已对齐可证明的 normal-A alpha；runtime skin texture/body decal 与 OptionColor/DecalColor 仍缺失，其它 family 继续补具体节点逻辑。
2. 已完成 STM lookup/row override、同步/异步 weapon load、prepared diagnostics、EXD metadata、Web stain0/stain1 选择器和首个正式染色视觉 snapshot；后续扩充第二通道与 metallic case。
3. 已完成 decal/crest 透明 fallback 与 materialChange 基础材质 fallback 的 prepared 语义和 renderer 执行；后续在需要时设计显式 runtime texture 输入。
4. 评估离线 bake/atlas 路线。

完成标准：

- P0/P1 真实武器样本在颜色、透明、发光、材质分区上稳定接近 MeddleTools bake 结果。
- 对无法离线复刻的 runtime-only 行为有明确 fallback 和 debug 标记。

## 当前测试建议

保留已有单元测试，并补充：

- prepared material / draw role 单元测试
- texture sampler config 单元测试
- shader family classification 单元测试
- ignored phantom snapshot 的 P0/P1 子集 CI 可选任务
- 每次修复一个语义问题时添加最小 fixture，避免再次把路径、sampler、ColorTable 或 alpha 语义改坏
