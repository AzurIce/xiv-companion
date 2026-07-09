# 武器渲染解析审查与改进规划

本文整理当前 `xiv-companion` 武器模型预览与本地参考仓库 `Meddle`、`MeddleTools` 的对比结果，并按三层规划后续工作：

1. 数据解析
2. 解析后的结果处理
3. 渲染器与着色器管线

目标不是一次性复刻完整 FFXIV shader，而是把现有“可辨识预览”推进到“语义清楚、问题可定位、关键材质效果稳定”的状态。

## 核验依据

本轮核验时间：2026-07-09。

当前结论基于以下代码和参考实现抽查：

- 本仓 `crates/xiv-companion-data/src/model.rs`：`PackedModelId`、`ModelVertex`、`ModelMaterial`、`ModelTextureKind`、`bake_color_table_maps`、weapon model/material candidate path。
- 本仓 `crates/xiv-companion-data/src/mdl_geometry.rs` 与 `mdl_metadata.rs`：raw LOD0 mesh range、extra LOD、vertex declaration、secondary attributes、flow、bone table、shape metadata。
- 本仓 `crates/xiv-companion-data/src/weapon_models.rs`：MTRL sampler records、`.shpk` composed semantics、ColorTable bake、ColorDyeTable debug、alpha mode、sub-model load path。
- 本仓 `crates/xiv-companion-render/src/renderer/model.rs` 与 `model.wgsl`：GPU vertex layout、material bind group、opaque/transparent pass、mesh-level transparent sorting、实际消费的 texture/vertex fields。
- `E:\repos\Meddle\Meddle\Meddle.Utils\Export\Model.cs` 与 `Vertex.cs`：LOD0 mesh range、extra LOD、shape/attribute group、Meddle 顶点属性保留方式。
- `E:\repos\Meddle\Meddle\Meddle.Plugin\Utils\ParseMaterialUtil.cs` 与 `OnRenderMaterialUtil.cs`：运行时 material/texture handle、GPU ColorTable、decal/crest/on-render material output。
- `E:\repos\Meddle\Meddle\Meddle.Utils\Files\Structs\Material\ColorTableRow.cs`：Dawntrail/Legacy ColorTable 字段语义。
- `E:\repos\MeddleTools\MeddleTools\node_setup\node_configs.py`、`node_mappings.py`、`bake\bake_utils.py`：texture node config、UV scroll、ColorTable extra ramps、shader package mapping、diffuse/normal/roughness/glossy/transmission/emission bake。

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
- `ModelMeshDrawRole` 已把 MDL mesh category 映射成 renderer-friendly draw role；renderer 当前会跳过 shadow、terrainShadow、verticalFog、lightShaft，不再把这些 mesh 当普通 surface 画，materialChange/crestChange 暂作为 debugVisible 绘制。
- `WeaponModelData.loadDiagnostics` 已记录可选副手/子模型加载失败的 role、model、候选路径、失败状态和错误信息，phantom `model-summary.json` 会直接输出。
- `weapon-render-pipeline.md` 已同步当前实现：Legacy ColorTable bake、mesh-level transparent sorting、额外材质贴图绑定和剩余限制不再按旧状态描述。
- Dawntrail 与 Legacy ColorTable 都能通过 `_id.tex` 烘焙出 diffuse、specular、material-properties、tile、sheen、sphere、tile-matrix 等派生贴图。
- `characterglass.shpk` 已有独立 alpha/render mode，透明 batch 已做 mesh-level back-to-front 排序。
- renderer GPU 顶点格式已上传 `uv1-uv3`、`color1`、secondary normal/bitangent、`flow0/flow1`；WGSL 已声明这些输入 location，但当前 fragment 逻辑仍主要消费 `uv0`、primary normal/bitangent 和 `color0`。
- `PreparedModel` / `PreparedMesh` 已有第一版，按 mesh 输出 draw role、是否进入主 pass 和 prepared material；renderer 与 phantom `model-summary.json` 现在共用这一准备结果。
- `PreparedMaterial` / `PreparedRenderPass` 已提升到数据层；phantom `model-summary.json` 的主 surface mesh 会输出 prepared material 决策，包含 `Opaque`、`Cutout`、`Transparent`、`Glass` 与 culling policy。
- `MaterialShaderFamily` 已结构化常见 `.shpk`：character、characterGlass、characterTransparency、characterScroll、bg、lightShaft、water、unknown，并进入 `PreparedMaterial`。
- `PreparedTextureBindings` 已聚合现有材质贴图索引：base、normal、mask、material、multi、specular、emissive、material-properties、tile、sheen、sphere、tile-matrix，并随 prepared material 输出。
- `PreparedTextureSamplingSet` 已表达第一版 texture role 采样策略：base/specular/emissive 为 sRGB + linear + repeat，normal/mask/material/multi/material-properties 为 Non-Color + linear + repeat，index 与 ColorTable extra maps 为 Non-Color + nearest + repeat；renderer 已先区分 color sampler 与 data sampler。
- `PreparedMaterialFeatureFlags` 已有第一版，按材质字段和贴图绑定标出 vertex color、ColorTable、tile、detail、scroll 等 shader 需求；flow/dye 仍保守为 false，等待 mesh-level prepared model 和染色入口。
- `PreparedMaterialUvSources` 已有第一版，记录常规 texture role 默认 `uv0`，并按 MeddleTools `UV0Scroll` / `UV1Scroll` 节点保留 scroll 的 `uv0` / `uv1` 来源；renderer 尚未按该表切换采样。
- renderer 已绑定并消费 ColorTable extra maps：tile、sheen、sphere、tile-matrix 以 Non-Color texture view + nearest sampler 进入 WGSL，当前用于保守的 specular/sheen/sphere-like highlight 调制。
- `g_NormalScale` 已从 composed material constants 提升为 `ModelMaterial.normalScale`，支持 shader package default 与 material override；renderer 会用它缩放 tangent-space normal map 强度。
- `g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale` 已结构化进 `ModelMaterial` 和 renderer `shaderParams`，当前作为后续 multi/detail normal 组合的稳定输入，尚未改变 fragment shader 的实际法线混合。
- `g_TileIndex`、`g_TileAlpha`、`g_TileScale` 已结构化进 `ModelMaterial` 和 renderer `tileParams`，当前作为后续 tile array / UV repeat 逻辑的稳定输入，尚未驱动实际 tile 贴图选择。
- `g_DetailID`、`g_MultiDetailID`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 已结构化进 `ModelMaterial` 和 renderer detail uniforms，当前作为后续 detail color/normal 采样的稳定输入，尚未驱动实际 detail map 采样。
- `g_UVScrollTime` / `0x9A696A17` 已按 MeddleTools `UvScrollMapping` 结构化进 `ModelMaterial.uvScroll` 和 renderer uniform，当前保存 UV0/UV1 scroll multiplier，尚未引入 time uniform 或实际滚动采样。

主要缺口集中在：

- 多套 UV、secondary normal/bitangent、`color1`、flow 已进 GPU 输入，但 shader-family-specific 逻辑还没有使用这些通道。
- mesh category 和 submesh attribute mask/name 已进入第一版 `PreparedModel` / `PreparedMesh`；shape/attribute runtime enabled mask、bone/skin/morph 等信息仍没有进入后续处理或渲染决策。
- 材质语义仍被压缩成少量近似规则和 Opaque/Mask/Blend/Glass；ColorTable extra maps 已有第一版实时消费，但 MeddleTools 中完整 tile array、scroll、transparency、reflection 等节点逻辑大多没有实现。
- 染色、运行时 ColorTable、decal、crest、on-render material output 是 Meddle 运行时路径的优势；当前离线 Web 预览没有等价输入。
- 文档 `weapon-render-pipeline.md` 已同步到当前实现；后续设计和优先级以本文 roadmap 为准。

## 1. 数据解析改进

### P0: 让已解析语义可审计

需要补齐 debug 输出，确保之后修 shader 时可以定位是“数据没读到”还是“渲染没用”。

- 已完成：在 ignored phantom snapshot 的 `model-summary.json` 中保留每个 mesh 的 category、submesh attribute names、bone table、shape 信息摘要，并链接对应 full metadata JSON。
- 已完成：对副手/子模型加载失败不再完全吞掉；`WeaponModelData.loadDiagnostics` 会记录候选路径、missing/read/parse 状态和错误原因。
- 已完成：在 material debug 中明确列出 sampler role 的来源；目前覆盖 `.shpk` resource name、known CRC 和 unknown，文件名后缀来源仍应在 prepared texture config 中补齐。
- 已完成：给每个材质输出 shader keys、resolved constants、shader flags、texture flags、sampler flags 的紧凑摘要，便于和 MeddleTools 节点输入对照；resolved 值会标注 `shaderPackageDefault` 或 `materialOverride` 来源。

验证：

- 扩展 ignored snapshot 测试生成的 `model-summary.json`，让 P0/P1 样本能直接看到缺失资源、sampler 来源和 mesh category。
- 为副手材质反推、sampler role 来源、shader constant override 增加 focused tests。
- 已增加 material semantic summary focused test，覆盖 key/constant 来源、known name、texture flags 与 sampler flags。

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
- 已完成：`g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale` 进入 `ModelMaterial`，默认 1.0，材质 override 优先于 shader package default；renderer uniform 已预留 y/z/w 三个通道并 clamp 到 0..4，但当前 WGSL 仍只消费 `normalScale`。
- 已完成：`g_TileIndex`、`g_TileAlpha`、`g_TileScale` 进入 `ModelMaterial`，默认值分别为 `0`、`1`、`[16,16]`；renderer uniform 已预留 `tileParams`，但当前 WGSL 仍只使用 ColorTable extra tile ramp 的第一版高光调制，没有实际选择 tile array。
- 已完成：`g_DetailID`、`g_MultiDetailID`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 进入 `ModelMaterial`，默认值分别为 `0`、`0`、`[4,4,4,4]`、`[4,4,4,4]`；renderer uniform 已预留 detail id 与 primary/multi UV scale，但当前 WGSL 还没有绑定或采样 detail map。
- 已完成：`g_UVScrollTime` / `0x9A696A17` 进入 `ModelMaterial.uvScroll`，按 MeddleTools 映射转换为 `[-x, y, -z, w]`，分别对应 UV0 与 UV1 scroll multiplier；renderer uniform 已预留，但当前 WGSL 还没有 time uniform 或滚动 UV 采样。

后续优先参数：

- glass/transparency 相关 shader keys 和 constants

验证：

- 用合成 MTRL fixture 测 shader constant 解析。
- 已增加 normal scale focused tests，覆盖 primary/multi/detail normal scale 的 shader package default、material override 和 clamp。
- 已增加 tile select focused tests，覆盖 `g_TileIndex`、`g_TileAlpha`、`g_TileScale` 的 shader package default、material override 和 renderer uniform 预留。
- 已增加 detail UV focused tests，覆盖 `g_DetailID`、`g_MultiDetailID`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 的 shader package default、material override 和 renderer uniform 预留。
- 已增加 UV scroll focused tests，覆盖 `g_UVScrollTime` / `0x9A696A17` 的 shader package default、material override、MeddleTools U 轴取反和 renderer uniform 预留。
- 用本地 SqPack 样本输出 material debug，对照 MeddleTools `node_configs.py` 中对应 mapping。

### P1: 处理染色数据入口

当前只 debug 暴露 ColorDyeTable，没有应用染色。

需要解析或加载：

- MTRL `ColorDyeTable`
- `chara/base_material/stainingtemplate.stm`
- EXD `Stain` / `StainTransient` 中可用于预览的颜色参数
- item 的默认 stain 或用户选择 stain 输入

离线 Web 预览不能拿到 Meddle 的 runtime GPU ColorTable，因此需要静态复刻染色表应用逻辑。

验证：

- 先用单色、双色染色 fixture 验证 ColorTable row flag 应用。
- 再选几件游戏中染色效果明显的武器做 visual snapshot。

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
- `PreparedMaterial` 已包含第一版 `PreparedTextureSamplingSet`，把 texture role 的 sRGB/Non-Color、linear/nearest、repeat/clip 语义从 renderer 私有实现中拆出来；当前没有 decal texture kind，因此 clip 仍只是预留枚举。
- `PreparedMaterial` 已包含第一版 `PreparedMaterialFeatureFlags`，聚合 `usesVertexColor`、`usesColorTable`、`usesTile`、`usesDetail`、`usesScroll`，并显式保留 `usesFlow` / `usesDye` 为后续 mesh/stain preparation 入口。
- `PreparedMaterial` 已包含第一版 `PreparedMaterialUvSources`，常规贴图源保守为 `uv0`，scroll 源显式分为 `uv0Scroll=uv0` 和 `uv1Scroll=uv1`，与 MeddleTools `UvScrollMapping` 的 `UV0Scroll` / `UV1Scroll` 节点对应。
- phantom `model-summary.json` 会在主 surface mesh 上输出 prepared material 决策，并通过第一版 `PreparedModel` 获得 mesh draw role / main pass 可见性。
- `PreparedModel` 仍是第一版：已包含 submesh attribute mask/name 作为 visibility 输入，但尚未包含 runtime enabled shape/attribute mask、skinning/morph 或 per-submesh prepared draw；sampler config、feature flags 与第一版 UV source 已有公共语义，但尚未完整驱动所有 runtime 绑定。

建议中间结构包含：

- mesh draw role：normal、glass、lightShaft、shadowOnly、ignored、debugVisible 等；已有第一版 `PreparedMesh`，并保留 submesh attribute mask/name
- material shader family：character、characterGlass、characterTransparency、characterScroll、bg、lightShaft、unknown
- texture bindings：base、normal、mask、material、multi、specular、emissive、tile/sheen/sphere/tileMatrix 已有第一版；per-role sampler config 已有第一版；index 仍未作为可直接渲染绑定保留，因为当前 `_id.tex` 会先用于 ColorTable bake
- UV source：每个 texture 或 shader family 应使用 uv0/uv1/uv2/uv3 哪一套；已有第一版 texture-role 默认与 scroll uv0/uv1 来源，后续还要补 shader-family-specific 规则
- alpha policy：opaque、cutout、blend、glass、additive/lightshaft
- culling policy：render backfaces / cull backfaces
- feature flags：usesVertexColor、usesFlow、usesColorTable、usesDye、usesScroll、usesTile、usesDetail；已有第一版材质级判定，mesh-level flow 和染色输入仍待补齐

好处：

- renderer 不需要理解所有 raw MTRL 细节。
- 后续支持多个 shader family 时，不会继续膨胀 `ModelMaterial`。
- snapshot/debug 可以输出 preparation 决策，定位问题更快。

验证：

- 已增加 focused tests 断言 `PreparedModel` mesh-level 决策、submesh attribute metadata 传播、alpha policy 与 mesh glass override 到 prepared render pass 的映射、shader family 分类、texture bindings 聚合、texture sampling policy、feature flags、UV source，以及 culling policy fallback。
- 后续仍需要用真实样本验证 sampler policy / UV source 到 renderer 绑定的覆盖率。
- P0/P1 phantom weapon 样本已具备 prepared summary 字段，仍需要跑 ignored snapshot 对比真实输出。

### P0: mesh category 和 submesh attribute 决策前置

已完成第一步：raw mesh category 会通过 `ModelMeshDrawRole` 转成 draw role，并进入第一版 `PreparedModel`；renderer 不再全部当普通 mesh 画。

当前策略：

- `normal`、`glass` 默认渲染。
- `glass` 会强制进入 transparent pass。
- `lightShaft` 已标为独立 draw role；additive/lightshaft pass 尚未实现前，默认不进入主 surface pass。
- `shadow`、`terrainShadow`、`verticalFog` 默认不作为主 surface 渲染。
- `materialChange`、`crestChange` 暂时作为 `debugVisible` 继续渲染，并在 snapshot summary 中标出。
- submesh attribute mask/name 已进入 `ModelMesh` 与 `PreparedMesh`，并随 phantom summary 输出；仍需要 runtime enabled shape/attribute mask，才能按 Meddle composer 那样真正隐藏 disabled submesh。

验证：

- 已增加 draw role mapping 单元测试和 renderer flatten 过滤测试，覆盖 glass/terrainShadow/lightShaft/shadow/verticalFog/materialChange/crestChange。
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
- `_id.tex` 必须以 nearest/closest 采样，不应使用 linear filtering；当前 prepared sampling policy 已表达这一点，renderer 仍没有直接绑定 index texture。
- material/special maps 需要 Non-Color，不走 sRGB。

验证：

- 继续保留现有 bake 单元测试。
- 已增加 prepared texture config 测试，确保 index 使用 nearest，base/specular 使用预期色彩空间。

### P1: 引入 shader-family-specific texture interpretation

当前 `mask` 被直接当 RGB 参与 roughness/specular/metalness 近似；`material_map`、`multi_map` 基本没有用。

建议先支持 character family：

- base/color map
- normal map + normal scale：`g_NormalScale` 已实际用于 primary normal；`g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale` 已进入数据/renderer 参数，后续需要接入 shader-family-specific normal map 组合
- mask/material map 的通道解释
- multi map/detail map 的第二层颜色/法线影响；tile select、detail UV 和 UV scroll 参数已进入数据/renderer 参数，后续需要接入 tile array、detail map、time uniform 与滚动 UV 采样
- vertex color 的具体启用条件

然后再支持：

- glass
- transparency
- scroll
- lightshaft

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
- `flow0` / `flow1`，缺省为零，用于后续 scroll 或特殊 shader

当前仍保持现有视觉行为：fragment shader 还只使用 `uv0`、primary normal/bitangent 和 `color0`。`PreparedMaterialUvSources` 已先记录 texture role 与 scroll 的 UV 来源，后续按 shader family、UV source 和 feature flag 决定何时消费 `uv1-uv3`、`color1`、secondary tangent frame 和 flow，避免一次性改变太多材质表现。

验证：

- 已增加单元测试 `GpuVertex::layout` stride/offset。
- 已增加 flatten 单元测试，确认 `ModelVertex` 的扩展字段不会在 CPU -> GPU 顶点转换时丢失，并覆盖 optional 字段 fallback。
- 后续仍需要 synthetic model 渲染不同 UV 层贴图，确认 shader-family / prepared UV source 逻辑能选择 uv0/uv1。

### P0: 按 prepared draw role 分 pass

当前进度：renderer batch 已记录 prepared render pass：

- `Opaque`
- `Cutout`：当前仍复用 opaque pipeline，写 depth，并由 shader alpha test discard。
- `Transparent`：复用 transparent pipeline，不写 depth，参与 mesh-level sorting。
- `Glass`：复用 transparent pipeline，不写 depth，参与 mesh-level sorting。

尚未完成的是把它们拆成独立 wgpu pipeline：

- opaque pass：写 depth
- cutout pass：写 depth，alpha test discard
- transparent pass：不写 depth，mesh-level sorted
- glass pass：不写 depth，单独 blend/参数
- additive/lightshaft pass：加法或 screen-like blend，不写 depth 或只读 depth

shadow、terrainShadow、verticalFog、lightShaft 在主预览中默认不画，避免错误 surface；lightShaft 的 additive pass 仍未实现。

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
- 已增加 focused test，确认 extra map flags 只在 texture index 实际存在时启用。

仍建议的后续顺序：

1. tile matrix / tile index 用于调整 UV 或选择 tile array 的近似层。
2. 用真实 tile array 近似替代当前只读 ColorTable tile properties 的 specular 调制。
3. sphere 作为环境/反射近似，接入更接近 MeddleTools 的 reflection/sphere 节点。

如果无法完整实现，至少保留这些贴图的 shader 绑定，并提供 debug view 开关，确认数据流通。

验证：

- 已有 native snapshot 覆盖 bind layout/WGSL 编译；后续仍需要 synthetic ColorTable ramp 生成明显 tile/sheen/sphere 差异。
- 与 MeddleTools ramp 输出对照。

### P1: 改善 alpha/glass/transparency

当前 glass 固定 opacity `0.28`。后续需要：

- 从 shader keys/constants 中解析 glass/transparency 参数。
- 区分 cutout、blend、glass、additive。
- 对 alpha test 使用真实阈值和 shader package 规则。
- 透明排序保留 mesh-level，但为复杂模型预留 per-triangle 或 weighted blended OIT 方案。

验证：

- `冬雪之幻梦`：外壳透明且内部可见。
- `茶歇之幻梦`：透明/灰背面问题不回归。

### P1: 纹理采样配置

当前进度：数据层已有第一版 `PreparedTextureSamplingSet`，renderer 的 material bind group 已分出 color sampler 与 data sampler；WGSL 现在用 color sampler 采 base/specular/emissive，用 data sampler 采 normal/mask/material-properties。

仍需要按 texture role 完整落地：

- base/specular/emissive: sRGB 视具体语义
- normal/mask/material/multi/material-properties: Non-Color，当前 renderer 已用 data sampler
- index: nearest/closest，当前仅在 prepared policy 和 bake 入口语义上表达，未作为 runtime shader 绑定
- tile/detail arrays 与 ColorTable extra maps: nearest 或 shader-family-specific，当前 renderer 已用 nearest sampler 消费 ColorTable extra maps；tile/detail array 仍未绑定
- decal: clip/extend 语义，当前尚无独立 texture kind

WebGPU bind group 已支持每材质 color/data 两个 sampler；后续若直接采 index/tile array，还需要 nearest sampler 或 per-texture sampler 选择。

验证：

- 已有 prepared texture sampling 测试覆盖 `_id.tex` nearest policy，避免颜色边界被 linear 混合污染。
- renderer 已用 `Rgba8Unorm` 创建 normal/mask/material-properties，并用 data sampler 采样；ColorTable extra maps 已用 nearest sampler 采样。仍需 synthetic shader fixture 验证 tile/sheen/sphere 可见效果。

### P2: 视觉验证和调试视图

建议在 UI 和 snapshot 工具加入 debug render mode：

- base color
- normal
- mask/material properties
- specular
- emissive
- alpha
- mesh category colors
- UV set preview
- ColorTable row index preview

这些视图能显著缩短后续对照 Meddle/MeddleTools 的定位时间。

## 建议实施顺序

### 第一阶段：可审计和不误画

1. 更新过期文档。
2. 增强 summary/debug 输出。
3. 引入 prepared draw role。
4. 默认过滤 shadow/terrainShadow/verticalFog。
5. 记录副手加载失败。

完成标准：

- P0/P1 snapshot summary 能说明每个 mesh/material 为什么这样画。
- 不再把明显非 surface mesh 当普通材质误画。当前 shadow/terrainShadow/verticalFog/lightShaft 已默认不进主 surface pass；lightShaft 的 additive pass 仍在后续阶段。

### 第二阶段：让解析结果真正进 shader

1. 已完成 GPU 顶点格式扩展，后续让 shader-family 逻辑实际消费 uv1/color1/flow。
2. 增加 per-material texture/sampler config。
3. material/tile/sheen/sphere/tile-matrix 已进入 renderer；后续让 tile array、UV source 和 shader-family 规则真正参与。
4. 加入 shader family 和 alpha policy。

完成标准：

- MeddleTools 中 ColorTable extra ramp 对应的数据能在 Web renderer 中产生可观察效果。
- `_id.tex` 边界、mask/material map 色彩空间不再依赖统一 sampler 侥幸工作。

### 第三阶段：shader family 和运行时替代输入

1. character glass/transparency/scroll/lightshaft 逐个补齐。
2. 接入染色。
3. 设计 decal/crest fallback 或显式输入。
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
