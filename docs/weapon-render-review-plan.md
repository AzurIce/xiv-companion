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
- `PreparedMaterial` / `PreparedRenderPass` 已提升到数据层；phantom `model-summary.json` 的主 surface mesh 会输出 prepared material 决策，包含 `Opaque`、`Cutout`、`Transparent`、`Glass` 与 culling policy。
- `MaterialShaderFamily` 已结构化常见 `.shpk`：character、characterGlass、characterTransparency、characterScroll、bg、lightShaft、water、unknown，并进入 `PreparedMaterial`。
- `PreparedTextureBindings` 已聚合现有材质贴图索引：base、normal、mask、material、multi、specular、emissive、material-properties、tile、sheen、sphere、tile-matrix，并随 prepared material 输出。
- `PreparedTextureSamplingSet` 已表达第一版 texture role 采样策略：base/specular/emissive 为 sRGB + linear + repeat，normal/mask/material/multi/material-properties 为 Non-Color + linear + repeat，index 与 ColorTable extra maps 为 Non-Color + nearest + repeat；renderer 已先区分 color sampler 与 data sampler。

主要缺口集中在：

- 多套 UV、secondary normal/bitangent、`color1`、flow 已进 GPU 输入，但 shader-family-specific 逻辑还没有使用这些通道。
- mesh category 已进入第一步 draw role 决策；submesh attribute mask、shape/attribute runtime mask、bone/skin/morph 等信息仍没有进入后续处理或渲染决策。
- 材质语义被压缩成少量贴图和 Opaque/Mask/Blend/Glass；MeddleTools 中的 tile、sphere、sheen、scroll、transparency、reflection 等节点逻辑大多没有实现。
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

- 后续 roadmap 仍以本文为准，避免把 prepared draw role、per-role sampler config 等未完成内容写成当前能力。

验证：

- 文档审查即可，不需要代码测试。

### P1: 扩展材质参数解析范围

现有 `.shpk` 解析主要用于 sampler role、material key 和常量默认值。后续需要把常见 character/bg/lightshaft/scroll/glass 参数提升成结构化字段，而不是只留在 debug。

优先参数：

- `g_NormalScale`
- `g_AlphaThreshold`
- `g_TileIndex`
- `g_TileAlpha`
- `g_TileScale`
- `g_DetailID`
- `g_DetailColorUvScale`
- `g_DetailNormalUvScale`
- UV scroll 相关参数，例如 MeddleTools `UvScrollMapping` 使用的 `0x9A696A17`
- glass/transparency 相关 shader keys 和 constants

验证：

- 用合成 MTRL fixture 测 shader constant 解析。
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
- 已增加第一版公共 `PreparedMaterial`，把 material alpha/render mode 与 mesh draw role 合成 `Opaque`、`Cutout`、`Transparent`、`Glass` 四类 prepared render pass，同时保留 culling policy。
- `PreparedMaterial` 已包含第一版 `MaterialShaderFamily` 分类，覆盖 MeddleTools 映射中的 character/glass/transparency/scroll/bg/lightshaft/water 常见包。
- `PreparedMaterial` 已包含第一版 `PreparedTextureBindings`，聚合 renderer 当前已知的材质贴图索引。
- `PreparedMaterial` 已包含第一版 `PreparedTextureSamplingSet`，把 texture role 的 sRGB/Non-Color、linear/nearest、repeat/clip 语义从 renderer 私有实现中拆出来；当前没有 decal texture kind，因此 clip 仍只是预留枚举。
- phantom `model-summary.json` 会在主 surface mesh 上输出 prepared material 决策。
- 完整 `PreparedModel`、UV source、feature flags 仍尚未建立；sampler config 已有公共语义，但尚未完整驱动所有 runtime 绑定。

建议中间结构包含：

- mesh draw role：normal、glass、lightShaft、shadowOnly、ignored、debugVisible 等
- material shader family：character、characterGlass、characterTransparency、characterScroll、bg、lightShaft、unknown
- texture bindings：base、normal、mask、material、multi、specular、emissive、tile/sheen/sphere/tileMatrix 已有第一版；per-role sampler config 已有第一版；index 仍未作为可直接渲染绑定保留，因为当前 `_id.tex` 会先用于 ColorTable bake
- UV source：每个 texture 或 shader family 应使用 uv0/uv1/uv2/uv3 哪一套
- alpha policy：opaque、cutout、blend、glass、additive/lightshaft
- culling policy：render backfaces / cull backfaces
- feature flags：usesVertexColor、usesFlow、usesColorTable、usesDye、usesScroll、usesTile

好处：

- renderer 不需要理解所有 raw MTRL 细节。
- 后续支持多个 shader family 时，不会继续膨胀 `ModelMaterial`。
- snapshot/debug 可以输出 preparation 决策，定位问题更快。

验证：

- 已增加 focused tests 断言 alpha policy 与 mesh glass override 到 prepared render pass 的映射、shader family 分类、texture bindings 聚合、texture sampling policy，以及 culling policy fallback。
- 后续仍需要为 UV source 构造 fixture，并用真实样本验证 sampler policy 到 renderer 绑定的覆盖率。
- P0/P1 phantom weapon 样本已具备 prepared summary 字段，仍需要跑 ignored snapshot 对比真实输出。

### P0: mesh category 和 submesh attribute 决策前置

已完成第一步：raw mesh category 会通过 `ModelMeshDrawRole` 转成 draw role，renderer 不再全部当普通 mesh 画。

当前策略：

- `normal`、`glass` 默认渲染。
- `glass` 会强制进入 transparent pass。
- `lightShaft` 已标为独立 draw role；additive/lightshaft pass 尚未实现前，默认不进入主 surface pass。
- `shadow`、`terrainShadow`、`verticalFog` 默认不作为主 surface 渲染。
- `materialChange`、`crestChange` 暂时作为 `debugVisible` 继续渲染，并在 snapshot summary 中标出。
- submesh attribute mask 已在 summary 中保留，但仍需要进入后续 prepared mesh，用于 shape/attribute visibility。

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
- normal map + normal scale
- mask/material map 的通道解释
- multi map/detail map 的第二层颜色/法线影响
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

当前仍保持现有视觉行为：fragment shader 还只使用 `uv0`、primary normal/bitangent 和 `color0`。后续按 shader family 和 feature flag 决定何时消费 `uv1-uv3`、`color1`、secondary tangent frame 和 flow，避免一次性改变太多材质表现。

验证：

- 已增加单元测试 `GpuVertex::layout` stride/offset。
- 已增加 flatten 单元测试，确认 `ModelVertex` 的扩展字段不会在 CPU -> GPU 顶点转换时丢失，并覆盖 optional 字段 fallback。
- 后续仍需要 synthetic model 渲染不同 UV 层贴图，确认 shader-family 逻辑能选择 uv0/uv1。

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

renderer 应开始使用已 bake 的：

- tile properties
- sheen properties
- sphere properties
- tile matrix

建议顺序：

1. tile matrix / tile index 用于调整 UV 或选择 tile array 的近似层。
2. sheen 作为额外高光项。
3. sphere 作为环境/反射近似。

如果无法完整实现，至少把这些贴图绑定进 shader 并提供 debug view 开关，确认数据流通。

验证：

- synthetic ColorTable ramp 生成明显 tile/sheen/sphere 差异。
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
- tile/detail arrays 与 ColorTable extra maps: nearest 或 shader-family-specific，当前 policy 已把 extra maps 标为 nearest，但 renderer 尚未消费这些贴图
- decal: clip/extend 语义，当前尚无独立 texture kind

WebGPU bind group 已支持每材质 color/data 两个 sampler；后续若直接采 index/tile array，还需要 nearest sampler 或 per-texture sampler 选择。

验证：

- 已有 prepared texture sampling 测试覆盖 `_id.tex` nearest policy，避免颜色边界被 linear 混合污染。
- renderer 已用 `Rgba8Unorm` 创建 normal/mask/material-properties，并用 data sampler 采样；仍需 synthetic shader fixture 验证可见效果。

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
3. 绑定并消费 material/tile/sheen/sphere/tile-matrix 贴图。
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
