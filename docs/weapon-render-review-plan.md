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
- 本仓 `crates/xiv-companion-data/src/weapon_models.rs`：MTRL sampler records、`.shpk` composed semantics、ColorTable bake、ColorDyeTable debug、alpha mode、sub-model load path。
- 本仓 `crates/xiv-companion-render/src/renderer/model.rs` 与 `model.wgsl`：GPU vertex layout、material bind group、opaque/transparent pass、mesh-level transparent sorting、实际消费的 texture/vertex fields。
- `E:\repos\Meddle\Meddle\Meddle.Utils\Export\Model.cs` 与 `Vertex.cs`：LOD0 mesh range、extra LOD、shape/attribute group、Meddle 顶点属性保留方式。
- `E:\repos\Meddle\Meddle\Meddle.Plugin\Utils\ParseMaterialUtil.cs` 与 `OnRenderMaterialUtil.cs`：运行时 material/texture handle、GPU ColorTable、decal/crest/on-render material output。
- `E:\repos\Meddle\Meddle\Meddle.Utils\Constants\Names.cs`：已知 material parameter CRC、默认值和 shader 覆盖范围。
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
- `ModelMeshDrawRole` 已把 MDL mesh category 映射成 renderer-friendly draw role；renderer 当前会跳过 shadow、terrainShadow、verticalFog，不再把这些 mesh 当普通 surface 画；lightShaft 会作为 additive pass 绘制，materialChange/crestChange 暂作为 debugVisible 绘制。
- `WeaponModelData.loadDiagnostics` 已记录可选副手/子模型加载失败的 role、model、候选路径、失败状态和错误信息，phantom `model-summary.json` 会直接输出。
- `weapon-render-pipeline.md` 已同步当前实现：Legacy ColorTable bake、mesh-level transparent sorting、额外材质贴图绑定和剩余限制不再按旧状态描述。
- Dawntrail 与 Legacy ColorTable 都能通过 `_id.tex` 烘焙出 diffuse、specular、material-properties、tile、sheen、sphere、tile-matrix 等派生贴图。
- `characterglass.shpk` 已有独立 alpha/render mode，透明 batch 已做 mesh-level back-to-front 排序。
- renderer GPU 顶点格式已上传 `uv1-uv3`、`color1`、secondary normal/bitangent、`flow0/flow1`；WGSL 已把 `uv1-uv3` 传到 fragment，并按 prepared UV source 选择 texture-role 采样 UV，但当前规则仍基本选择 `uv0`，secondary normal/bitangent、`color1` 和 flow 尚未参与实际 shader。
- `PreparedModel` / `PreparedMesh` 已有第一版，按 mesh 输出 draw role、是否进入主 pass 和 prepared material；renderer 与 phantom `model-summary.json` 现在共用这一准备结果。
- `PreparedMaterial` / `PreparedRenderPass` 已提升到数据层；phantom `model-summary.json` 的主 surface mesh 会输出 prepared material 决策，包含 `Opaque`、`Cutout`、`Transparent`、`Glass`、`AdditiveLightShaft` 与 culling policy；lightshaft 不进入普通 surface pass，但 renderer 会保留为 additive batch。
- `MaterialShaderFamily` 已结构化常见 `.shpk`：character、characterStockings、characterGlass、characterReflection、characterTransparency、characterScroll、characterTattoo、characterOcclusion、bg、lightShaft、water、unknown，并进入 `PreparedMaterial`；lightshaft 已有最小 additive/tint/UV 动画 shader 行为，其它新增特殊 character family 目前仍主要用于准备层分类和 debug。
- `PreparedTextureBindings` 已聚合现有材质贴图索引：base、normal、mask、material、multi、specular、emissive、material-properties、tile、sheen、sphere、tile-matrix、ColorTable index，并随 prepared material 输出。
- `PreparedTextureSamplingSet` 已表达第一版 texture role 采样策略：base/specular/emissive 为 sRGB + linear + repeat，normal/mask/material/multi/material-properties 为 Non-Color + linear + repeat，index 与 ColorTable extra maps 为 Non-Color + nearest + repeat；renderer 已从该 prepared policy 派生 color/data/nearest 三组 sampler descriptor。
- `PreparedMaterialFeatureFlags` 已有第一版，按材质字段和贴图绑定标出 vertex color、ColorTable、tile、detail、scroll 等 shader 需求；`usesFlow` 已在 `PreparedModel` 阶段按 mesh 顶点 `flow0/flow1` presence 汇总，`usesDye` 会在 MTRL 存在 `ColorDyeTable` 时置位，但尚未应用 stain。
- `PreparedMaterialUnsupportedInputs` 已有第一版，按当前可可靠判断的数据标出 dye application、runtime ColorTable、decal/crest、tile array、detail array、incomplete shader family logic，phantom summary 会随 prepared material 输出这些缺口。
- `PreparedMaterialUvSources` 已有第一版，记录常规 texture role 默认 `uv0`，并按 MeddleTools `UV0Scroll` / `UV1Scroll` 节点保留 scroll 的 `uv0` / `uv1` 来源；renderer material uniform / WGSL 已按 prepared texture UV source 选择 base、normal、mask、specular、emissive、material-properties、material/multi debug view 与 ColorTable extra map debug view 的采样 UV，并会按 render time 对 `uv0` / `uv1` 来源叠加已解析的 scroll multiplier；当前 source 规则仍基本保守为 `uv0`，scroll 也尚未做到节点级 texture-role 路由。
- `ModelMesh` / `PreparedMesh` 已保留 mesh-level shape influence 摘要；`PreparedModelOptions.enabledShapeMask` 已可按显式 shape mask 标出 active/inactive shape influence，但当前不把 shape mask 当 draw visibility，也尚未执行 morph/vertex replacement。
- renderer 已绑定并消费 ColorTable extra maps：tile、sheen、sphere、tile-matrix 以 Non-Color texture view + nearest sampler 进入 WGSL，当前用于保守的 specular/sheen/sphere-like highlight 调制，并提供独立 debug view 检查这些烘焙 ramp。
- `g_NormalScale` 已从 composed material constants 提升为 `ModelMaterial.normalScale`，支持 shader package default 与 material override；renderer 会用它缩放 tangent-space normal map 强度。
- `g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale` 已结构化进 `ModelMaterial` 和 renderer `shaderParams`，当前作为后续 multi/detail normal 组合的稳定输入，尚未改变 fragment shader 的实际法线混合。
- `g_TileIndex`、`g_TileAlpha`、`g_TileScale` 已结构化进 `ModelMaterial` 和 renderer `tileParams`，当前作为后续 tile array / UV repeat 逻辑的稳定输入，尚未驱动实际 tile 贴图选择。
- `g_ToonIndex`、`g_ToonLightScale`、`g_SheenRate`、`g_SheenTintRate`、`g_SheenAperture`、`g_SphereMapIndex` 已按 Meddle `Names.cs` CRC/default 结构化进 `ModelMaterial`、phantom summary 与 renderer `toonSheenParams` / `sheenSphereParams` uniform；WGSL 已把 sheen/sphere 常量作为 ColorTable extra ramp 之外的保守高光/反射输入，toon lighting 仍未实现。
- `g_DetailID`、`g_MultiDetailID`、`g_DetailColor`、`g_MultiDetailColor`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 已结构化进 `ModelMaterial` 和 renderer detail uniforms；WGSL 当前把 detail color 与 UV scale 作为缺少真实 detail array 时的保守 tint fallback，真实 detail color/normal 贴图阵列仍未绑定。
- `g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_MultiEmissiveColor` 已按 Meddle `Names.cs` CRC/default 和 MeddleTools `ColorMapping` 结构化进 `ModelMaterial`、phantom summary 与 renderer uniforms；WGSL 已把 `g_DiffuseColor` 作为 base tint，把 `g_EmissiveColor` 作为附加发光，并在 mask/material 通道存在时保守加入 `g_MultiEmissiveColor`，`g_MultiDiffuseColor` 仍等待完整 multi map 通道解释。
- `g_OutlineColor`、`g_OutlineWidth`、`g_SpecularColorMask`、`g_SSAOMask`、`g_TextureMipBias`、`g_ShadowPosOffset` 已按 Meddle `Names.cs` CRC/default 结构化进 `ModelMaterial`、phantom summary 与 renderer `outlineParams` / `specularColorMask` / `surfaceParams` uniform；WGSL 已用 `g_SpecularColorMask` 调制高光颜色/强度，并用 `g_SSAOMask` 保守调制环境底光，outline、mip bias 和 shadow offset 仍未驱动实际行为。
- `g_GlassIOR`、`g_GlassThicknessMax` 已按 Meddle `Names.cs` CRC/default 结构化进 `ModelMaterial`、phantom summary 与 renderer `glassParams` uniform；当前不直接改变 glass opacity、tint 或折射，避免在没有确认 shader-family 公式前误改透明效果。
- `g_UVScrollTime` / `0x9A696A17` 已按 MeddleTools `UvScrollMapping` 结构化进 `ModelMaterial.uvScroll` 和 renderer uniform；`ModelRenderOptions.uv_scroll_time` 进入 camera uniform，WGSL 会对 `uv0` / `uv1` 来源叠加 UV0/UV1 scroll multiplier，Web 渲染循环用 RAF 时间驱动，native snapshot 默认时间为 0 保持稳定。
- `lightshaft.shpk` 的 `g_Color`、`g_TexAnim`、`g_TexU`、`g_TexV`、`g_Ray` 已结构化进 `ModelMaterial` 和 phantom summary；renderer uniform 已传入 WGSL，`LightShaft` draw role 会启用保守的 additive tint、`g_TexAnim.xy` UV 动画、`g_TexU/V` 仿射 UV 和 `g_Ray` 强度近似。完整 MeddleTools 节点语义仍未复刻。
- `g_Transparency` 已结构化进 `ModelMaterial.transparency` 和 phantom summary，默认 0.0，当前只作为 glass/transparency shader 后续实现的稳定输入；尚未直接改写 renderer opacity。
- `g_AlphaAperture`、`g_AlphaOffset`、`g_ShadowAlphaThreshold` 已按 Meddle `Names.cs` CRC/default 结构化进 `ModelMaterial`、phantom summary 与 renderer `alphaParams` uniform；当前只作为后续 cutout/transparency/shadow alpha 公式输入，不改变现有 alpha test 或 opacity。
- `ModelDebugMode` 已提供第一版 renderer debug 视图：final、base、normal、mask、material properties、specular、emissive、alpha、UV0-UV3、vertex color、mesh/draw-role color、ColorTable index、material map、multi map、tile/sheen/sphere properties、tile matrix；Web 控件和 snapshot/test render options 共用同一入口。material/multi 和 ColorTable extra maps 当前可作为 texture-role debug preview，但不代表完整 shader 通道解释已完成。

主要缺口集中在：

- 多套 UV、secondary normal/bitangent、`color1`、flow 已进 GPU 输入，但 shader-family-specific 逻辑还没有使用这些通道。
- mesh category、submesh attribute mask/name 和 shape influence 摘要已进入第一版 `PreparedModel` / `PreparedMesh`；`PreparedModelOptions.enabledAttributeMask` 已可按显式运行时 attribute mask 隐藏 disabled submesh，`enabledShapeMask` 已可审计 active/inactive shape influence，但 Web 离线默认仍不猜这些 mask。bone/skin/morph 和实际 shape vertex replacement 仍没有进入后续渲染决策。
- 材质语义仍被压缩成少量近似规则和 Opaque/Mask/Blend/Glass；ColorTable extra maps 已有第一版实时消费，但 MeddleTools 中完整 tile array、scroll、transparency、reflection 等节点逻辑大多没有实现。
- 染色、运行时 ColorTable、decal、crest、on-render material output 是 Meddle 运行时路径的优势；当前离线 Web 预览没有等价输入。
- 文档 `weapon-render-pipeline.md` 已同步到当前实现；后续设计和优先级以本文 roadmap 为准。

## 分层审查结论与计划总览

### 1. 数据解析

审查结论：

- 本仓已经对齐 Meddle 的 LOD0 mesh range 和 extra LOD 分类方式，能区分 normal/water/shadow/terrainShadow/verticalFog/lightShaft/glass/materialChange/crestChange，且顶点层保留了多套 UV、secondary normal/bitangent/color、flow、blend weights/indices。
- MTRL 解析已经不只靠文件名猜测：sampler role 会优先使用 `.shpk` resource parameter name，再退回 known CRC 和路径后缀；shader package default 与 material override 也已经进入 debug summary。
- ColorTable 解析已覆盖 Dawntrail 与 Legacy，并按 Meddle/MeddleTools 语义产出 diffuse、specular、material-properties、tile、sheen、sphere、tile-matrix 等派生贴图；TileAlpha 已明确不再被误当作材质 alpha。

主要不足：

- 染色仍停留在 `ColorDyeTable` 存在性和行标志 debug，尚未接入 `stainingtemplate.stm`、EXD stain 参数或用户选择 stain 输入；`ModelMaterial.hasColorDyeTable` / `PreparedMaterialFeatureFlags.usesDye` / `unsupportedInputs.dyeApplication` 已能标出“这里需要染色应用”。
- Meddle 的 runtime 输入，包括 GPU ColorTable、resolved texture/material handle、decal、crest、on-render material output，目前只能记录为缺口，离线预览缺少显式 fallback。
- reflection/stockings/tattoo/occlusion 等 shader package 已能分类，但很多 shader keys/constants 还没有提升为结构化字段，也没有最小 fixture 覆盖；outline/specular/SSAO、toon/sheen/sphere、alpha aperture/offset/shadow threshold、glass IOR/thickness 和 transparency 已先进入结构化字段但未驱动完整 shader-family 行为，lightshaft 已有第一组结构化 constants 但 `g_Ray` 与节点级行为仍是近似。
- texture/sampler 语义仍有少量兜底路径依赖；MeddleTools 里 `_id.tex`、tile/detail arrays 使用 Non-Color + Closest/Repeat 的规则已经进入 prepared policy，其中 index 与 ColorTable extra maps 已进入 runtime sampler group；真实 tile/detail array 资源仍未绑定。

计划：

1. 先继续扩充可审计信息：在 material/prepared debug 中补齐 texture role 的最终来源、shader family、sampler policy、UV source、feature flags 和未支持 runtime 输入标记。
2. 把染色作为下一批解析入口：解析 `ColorDyeTable` 行标志，加载 `chara/base_material/stainingtemplate.stm`，定义 item/user stain 输入，再生成染色后的 ColorTable 或 renderer-friendly override。
3. 逐步结构化 shader-family 参数：优先 glass/transparency/lightshaft/scroll，再处理 reflection/stockings/tattoo/occlusion；每补一个参数都加合成 MTRL fixture 和真实样本 debug 对照。
4. 对 runtime-only 数据不盲猜：decal/crest 先提供空白或显式输入 fallback，GPU ColorTable 先只在 debug 中标明缺失，避免离线预览伪装成完整运行时渲染。

### 2. 解析后的结果处理

审查结论：

- `PreparedModel` / `PreparedMaterial` 已经把 raw parsed data 和 renderer binding 决策分开，renderer 与 phantom summary 共用 draw role、main-pass visibility、prepared pass、texture bindings、sampling policy、feature flags 和第一版 UV source。
- submesh attribute mask/name、显式 `enabledAttributeMask`、shape influence 摘要、显式 `enabledShapeMask` 审计和 mesh-level flow presence 已进入 preparation；这与 Meddle 的 shape/attribute group 思路一致，但 shape 目前只做 active/inactive 审计，不执行 morph。
- `PreparedRenderPass` 已能表达 `Opaque`、`Cutout`、`Transparent`、`Glass`、`AdditiveLightShaft`；lightshaft 不再误进主 surface pass。

主要不足：

- preparation 已有 `enabledShapeMask` 的 active/inactive shape influence 审计，但还没有真正应用 shape mesh/morph、skinning/morph runtime 输入，也没有 per-submesh draw batch 级别的可见性拆分。
- `PreparedMaterialUvSources` 已开始驱动 renderer 选择采样 UV；`PreparedTextureSamplingSet` 已开始驱动 renderer 的 color/data/nearest 三组 sampler descriptor，但还没有做到每个 texture role 独立 sampler。
- shader-family-specific 规则还没有进入中间层，例如 character base texture 如何与 ColorTable diffuse 混合、material/multi map 通道如何解释、scroll/reflection 使用哪套 UV/flow。
- `usesDye`、decal/crest、runtime ColorTable、tile/detail array 这些 capability flags 已有第一版 prepared unsupported summary，但还没有对应的真实 stain/decal/crest/tile/detail array 输入和 renderer 行为。

计划：

1. 扩展 `PreparedModelOptions`：继续加入 stain 输入、decal/crest fallback 或显式资源入口；`enabledShapeMask` 已先作为审计输入存在，默认仍保持离线保守行为。
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
- 多套 UV 已开始通过 prepared source 和 UV scroll 参与采样；secondary tangent frame、`color1`、flow、detail/multi maps、tile/detail arrays 还没有真正参与 shading，scroll 仍缺少 shader node 级别的 texture-role 路由。
- alpha/glass/transparency 仍是经验近似：glass opacity 固定范围，transparency/reflection/stockings/tattoo/occlusion 没有 family-specific WGSL 行为。
- renderer 已有第一版 debug view，能切换 base、normal、mask/material、specular、emissive、alpha、UV、vertex color、mesh/draw-role color、ColorTable index、material map、multi map 与 ColorTable extra maps；更细的 per-texture independent sampler policy、真实 tile/detail array 诊断仍未实现。

计划：

1. 先让 prepared pass 真正分管 pipeline：additive lightshaft 已有最小管线；后续继续拆独立 cutout、transparent/glass 行为，保持现有视觉输出尽量稳定，并补 synthetic pipeline tests。
2. 让 WGSL 继续按 prepared UV source 和 feature flags 消费更多通道：UV source 选择和保守 scroll time 已接入，后续优先补 shader-family-specific scroll 路由、tile matrix/tile index、detail map、flow，再做 secondary normal/bitangent。
3. 按 shader family 拆函数而不是继续堆主函数：base color、normal、material properties、alpha、emissive、glass、tile/sheen/sphere、scroll/reflection 分块，先用分支承载，必要时再拆 shader module/pipeline。
4. 继续补 debug render modes：base、normal、mask/material、specular、emissive、alpha、UV set、vertex color、mesh/draw-role color、ColorTable index、material map、multi map、ColorTable extra maps 已有第一版；后续补 per-texture independent sampler policy 和真实 tile/detail array 诊断，作为真实武器样本回归的主要判断工具。

## 1. 数据解析改进

### P0: 让已解析语义可审计

需要补齐 debug 输出，确保之后修 shader 时可以定位是“数据没读到”还是“渲染没用”。

- 已完成：在 ignored phantom snapshot 的 `model-summary.json` 中保留每个 mesh 的 category、submesh attribute names、bone table、shape 信息摘要，并链接对应 full metadata JSON。
- 已完成：对副手/子模型加载失败不再完全吞掉；`WeaponModelData.loadDiagnostics` 会记录候选路径、missing/read/parse 状态和错误原因。
- 已完成：在 material debug 中明确列出 sampler role 的来源；目前覆盖 `.shpk` resource name、known CRC 和 unknown，文件名后缀来源仍应在 prepared texture config 中补齐。
- 已完成：给每个材质输出 shader keys、resolved constants、shader flags、texture flags、sampler flags 的紧凑摘要，便于和 MeddleTools 节点输入对照；resolved 值会标注 `shaderPackageDefault` 或 `materialOverride` 来源。
- 已完成：给 `PreparedMaterial` 增加显式 unsupported/runtime-only 输入摘要，区分 `dyeApplication`、`runtimeColorTable`、`decalOrCrest`、`tileArray`、`detailArray`、`incompleteShaderFamilyLogic`。目标是让 snapshot 能区分“资源确实不存在”和“离线 Web/prepared 层还没有支持”。

验证：

- 扩展 ignored snapshot 测试生成的 `model-summary.json`，让 P0/P1 样本能直接看到缺失资源、sampler 来源和 mesh category。
- 为副手材质反推、sampler role 来源、shader constant override 增加 focused tests。
- 已增加 material semantic summary focused test，覆盖 key/constant 来源、known name、texture flags 与 sampler flags。
- 已增加 unsupported/runtime-only focused test，覆盖染色表存在、tile/detail 参数非默认、debugVisible decal/crest mesh 和特殊 shader family 标记。

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
- 已完成：`g_ToonIndex`、`g_ToonLightScale`、`g_SheenRate`、`g_SheenTintRate`、`g_SheenAperture`、`g_SphereMapIndex` 进入 `ModelMaterial`，默认值分别为 `0`、`2`、`0`、`0`、`1`、`0`；renderer uniform 已进入 WGSL，其中 `g_SheenRate` / `g_SheenTintRate` / `g_SheenAperture` 会补充 sheen 高光，`g_SphereMapIndex` 会影响已有 sphere-like rim tint；toon lighting 仍待后续实现。
- 已完成：`g_DetailID`、`g_MultiDetailID`、`g_DetailColor`、`g_MultiDetailColor`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 进入 `ModelMaterial`，默认值分别为 `0`、`0`、`[0.5,0.5,0.5,1]`、`[0.5,0.5,0.5,1]`、`[4,4,4,4]`、`[4,4,4,4]`；renderer uniform 已进入 WGSL，当前在 detail id 或 detail color 非默认时做轻量 tint fallback，UV scale 只影响这个 fallback 的弱纹理感；真实 detail map / detail normal array 仍待后续绑定。
- 已完成：`g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_MultiEmissiveColor` 进入 `ModelMaterial`，默认值分别为白色、白色、黑色、黑色；renderer uniform 已进入 WGSL，当前 `g_DiffuseColor` 会调制 base，`g_EmissiveColor` 会加到 emissive，`g_MultiEmissiveColor` 只在 mask/material 通道存在时保守加权，`g_MultiDiffuseColor` 仍等待完整 multi map 通道解释。
- 已完成：`g_OutlineColor`、`g_OutlineWidth`、`g_SpecularColorMask`、`g_SSAOMask`、`g_TextureMipBias`、`g_ShadowPosOffset` 进入 `ModelMaterial`，默认值分别为黑色、`0`、白色、`1`、`0`、`0`；renderer uniform 已进入 WGSL，其中 `g_SpecularColorMask` 会调制高光颜色/强度，`g_SSAOMask` 会保守调制环境底光；outline、texture LOD 和 shadow offset 仍待后续实现。
- 已完成：`g_GlassIOR`、`g_GlassThicknessMax` 进入 `ModelMaterial`，默认值分别为 `1`、`0.01`；renderer uniform 已预留 `glassParams`，但当前 WGSL 还没有用它们驱动 glass opacity、tint 或折射。
- 已完成：`g_AlphaAperture`、`g_AlphaOffset`、`g_ShadowAlphaThreshold` 进入 `ModelMaterial`，默认值分别为 `2`、`0`、`0.5`；renderer uniform 已预留 `alphaParams`，但当前 WGSL 还没有用它们驱动 alpha shaping、shadow alpha 或 transparency opacity。
- 已完成：`g_UVScrollTime` / `0x9A696A17` 进入 `ModelMaterial.uvScroll`，按 MeddleTools 映射转换为 `[-x, y, -z, w]`，分别对应 UV0 与 UV1 scroll multiplier；renderer 已用 `ModelRenderOptions.uv_scroll_time` / camera uniform 驱动 WGSL 对 `uv0` / `uv1` 来源做保守滚动采样，后续仍需按 shader family 和节点连接决定具体哪些 texture role 使用 scroll UV。
- 已完成：`lightshaft.shpk` 的 `g_Color`、`g_TexAnim`、`g_TexU`、`g_TexV`、`g_Ray` 进入 `ModelMaterial`，默认值分别为白色、零动画、identity U/V 和零 ray；renderer uniform 已按 draw role 只对 lightShaft batch 启用保守消费，其中 `g_Color` 控制 additive tint/alpha，`g_TexAnim.xy` 驱动 UV 动画，`g_TexU/V` 作为 UV 仿射基向量，`g_Ray` 当前只作强度近似。
- 已完成：`g_Transparency` 进入 `ModelMaterial.transparency`，默认 0.0，材质 override 优先于 shader package default 并 clamp 到 0..1；当前不直接参与 opacity 计算，避免把“透明度”误解成 alpha。

后续优先参数：

- reflection/stockings/tattoo/occlusion 等 shader-family keys/constants；transparency 还需要确认 `g_Transparency` 到 alpha/opacity 的方向和 shader-family 行为；lightshaft 仍需补 `g_Ray` 的真实节点行为和 synthetic render fixture

验证：

- 用合成 MTRL fixture 测 shader constant 解析。
- 已增加 normal scale focused tests，覆盖 primary/multi/detail normal scale 的 shader package default、material override 和 clamp。
- 已增加 tile select focused tests，覆盖 `g_TileIndex`、`g_TileAlpha`、`g_TileScale` 的 shader package default、material override 和 renderer uniform 预留。
- 已增加 toon/sheen/sphere focused tests，覆盖 `g_ToonIndex`、`g_ToonLightScale`、`g_SheenRate`、`g_SheenTintRate`、`g_SheenAperture`、`g_SphereMapIndex` 的 shader package default、material override、非 finite fallback 和 renderer uniform 传递；sheen/sphere 常量的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 detail focused tests，覆盖 `g_DetailID`、`g_MultiDetailID`、`g_DetailColor`、`g_MultiDetailColor`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 的 shader package default、material override、短数组 fallback、非 finite fallback 和 renderer uniform 传递；detail tint fallback 的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 shader color focused tests，覆盖 `g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_MultiEmissiveColor` 的 shader package default、material override、短数组 fallback、非 finite fallback 和 renderer uniform 传递；WGSL 编译通过 native snapshot 验证。
- 已增加 outline/specular/occlusion focused tests，覆盖 `g_OutlineColor`、`g_OutlineWidth`、`g_SpecularColorMask`、`g_SSAOMask`、`g_TextureMipBias`、`g_ShadowPosOffset` 的 shader package default、material override、短数组 fallback、非 finite fallback 和 renderer uniform 传递；`g_SpecularColorMask` / `g_SSAOMask` 的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 glass params focused tests，覆盖 `g_GlassIOR`、`g_GlassThicknessMax` 的 shader package default、material override、非 finite fallback 和 renderer uniform 预留。
- 已增加 alpha params focused tests，覆盖 `g_AlphaAperture`、`g_AlphaOffset`、`g_ShadowAlphaThreshold` 的 shader package default、material override、clamp、非 finite fallback 和 renderer uniform 预留。
- 已增加 UV scroll focused tests，覆盖 `g_UVScrollTime` / `0x9A696A17` 的 shader package default、material override、MeddleTools U 轴取反、renderer uniform 传递和默认时间稳定性。
- 已增加 lightshaft focused tests，覆盖 `g_Color`、`g_TexAnim`、`g_TexU`、`g_TexV`、`g_Ray` 的 shader package default、material override、renderer uniform 默认值和 LightShaft draw-role shader 开关。
- 已增加 transparency focused tests，覆盖 `g_Transparency` 的 shader package default、material override 和 clamp。
- 用本地 SqPack 样本输出 material debug，对照 MeddleTools `node_configs.py` 中对应 mapping。

### P1: 处理染色数据入口

当前已把 `ColorDyeTable` 存在性暴露到 `ModelMaterial.hasColorDyeTable`、`PreparedMaterialFeatureFlags.usesDye` 和 `unsupportedInputs.dyeApplication`，但还没有应用染色。

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
- `PreparedMaterial` 已包含第一版 `PreparedTextureSamplingSet`，把 texture role 的 sRGB/Non-Color、linear/nearest、repeat/clip 语义从 renderer 私有实现中拆出来；renderer 已消费 repeat/clamp/linear/nearest，`Clip` 目前在 sampler 层降级为 clamp，等待 shader 级 clip 逻辑。
- `PreparedMaterial` 已包含第一版 `PreparedMaterialFeatureFlags`，聚合 `usesVertexColor`、`usesColorTable`、`usesTile`、`usesDetail`、`usesScroll`，并显式保留 `usesFlow` / `usesDye` 为后续 mesh/stain preparation 入口。
- `PreparedMaterial` 已包含第一版 `PreparedMaterialUvSources`，常规贴图源保守为 `uv0`，scroll 源显式分为 `uv0Scroll=uv0` 和 `uv1Scroll=uv1`，与 MeddleTools `UvScrollMapping` 的 `UV0Scroll` / `UV1Scroll` 节点对应；renderer 已按 texture-role source 选择采样 UV。
- `PreparedMaterial` 已包含第一版 `PreparedMaterialUnsupportedInputs`，会把 dye application、runtime ColorTable、decal/crest、tile/detail array 和特殊 shader family 行为缺口输出到 prepared summary。
- phantom `model-summary.json` 会在主 surface mesh 上输出 prepared material 决策，并通过第一版 `PreparedModel` 获得 mesh draw role / main pass 可见性。
- `PreparedModel` 仍是第一版：已包含 submesh attribute mask/name，并新增 `PreparedModelOptions.enabledAttributeMask` 与 `PreparedMeshVisibility`，可在显式提供运行时 enabled attribute mask 时按 Meddle composer 语义隐藏 disabled submesh；mesh-level shape influence 已进入 `ModelMesh` / `PreparedMesh`，`PreparedModelOptions.enabledShapeMask` 可标出 active/inactive influence 但不改变 draw visibility；mesh-level flow presence 已进入 prepared material feature flags；第一版 UV source 已驱动 renderer 采样选择；尚未包含实际 shape morph、skinning/morph 或 per-submesh prepared draw；sampler config、feature flags 与 shader-family-specific UV source 仍未完整驱动所有 runtime 绑定。
- 当前缺口：`ColorDyeTable` 已进入 `ModelMaterial.hasColorDyeTable` 和 prepared summary，但 tile/detail array、runtime GPU ColorTable、decal/crest 和特殊 shader family 的 unsupported 标记仍只是审计信息，尚未接入真实替代输入或完整 renderer 行为。

建议中间结构包含：

- mesh draw role：normal、glass、lightShaft、shadowOnly、ignored、debugVisible 等；已有第一版 `PreparedMesh`，并保留 submesh attribute mask/name、attribute visibility 决策与 shape influence active/inactive 状态
- material shader family：character、characterStockings、characterGlass、characterReflection、characterTransparency、characterScroll、characterTattoo、characterOcclusion、bg、lightShaft、unknown；已有第一版分类，后续逐个补 shader-family-specific 行为
- texture bindings：base、normal、mask、material、multi、specular、emissive、tile/sheen/sphere/tileMatrix、ColorTable index 已有第一版；per-role sampler config 已有第一版，renderer 当前把它折叠成 color/data/nearest 三组 sampler；index 当前主要用于 ColorTable bake 和 debug preview，material/multi 已可用 Non-Color + linear sampler 做 debug preview，tile/sheen/sphere/tileMatrix 已可用 nearest sampler 做 debug preview；这些贴图尚未进入完整 shader 通道解释
- UV source：每个 texture 或 shader family 应使用 uv0/uv1/uv2/uv3 哪一套；已有第一版 texture-role 默认与 scroll uv0/uv1 来源，后续还要补 shader-family-specific 规则
- alpha policy：opaque、cutout、blend、glass、additive/lightshaft；`AdditiveLightShaft` 已作为 prepared pass 分类存在，并进入最小 wgpu additive pass
- culling policy：render backfaces / cull backfaces
- feature flags：usesVertexColor、usesFlow、usesColorTable、usesDye、usesScroll、usesTile、usesDetail；已有第一版材质级判定，mesh-level flow presence 已进入 `PreparedModel`，`usesDye` 由 `hasColorDyeTable` 驱动，实际 stain 输入仍待补齐
- unsupported/runtime-only inputs：dye application、runtime ColorTable、decal/crest、tile/detail array、shader-family-specific incomplete behavior；已有第一版，只基于当前可可靠判断的数据置位，不猜测运行时状态

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
- `materialChange`、`crestChange` 暂时作为 `debugVisible` 继续渲染，并在 snapshot summary 中标出。
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
- normal map + normal scale：`g_NormalScale` 已实际用于 primary normal；`g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale` 已进入数据/renderer 参数，后续需要接入 shader-family-specific normal map 组合
- mask/material map 的通道解释
- multi map/detail map 的第二层颜色/法线影响；detail color/UV scale 已先进入保守 tint fallback，tile select、detail normal 和 UV scroll 参数已进入数据/renderer 参数，scroll time 已接入保守滚动采样，后续需要接入 tile array、detail map/detail normal array 与 shader-family-specific scroll 路由
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
- `flow0` / `flow1`，缺省为零，用于后续 scroll 或特殊 shader

当前仍保持近似视觉行为：fragment shader 已按 prepared UV source 选择 texture-role 采样 UV，并会用 render time 对 `uv0` / `uv1` 来源做保守 scroll 偏移；source 规则仍基本默认 `uv0`，并且 primary normal/bitangent 与 `color0` 仍是主要着色输入。后续按 shader family、UV source 和 feature flag 决定何时让 `uv1-uv3` 产生差异、哪些 texture role 使用 scroll UV，以及何时消费 `color1`、secondary tangent frame 和 flow。

验证：

- 已增加单元测试 `GpuVertex::layout` stride/offset。
- 已增加 flatten 单元测试，确认 `ModelVertex` 的扩展字段不会在 CPU -> GPU 顶点转换时丢失，并覆盖 optional 字段 fallback。
- renderer 已把 `uv1-uv3` 传入 fragment，并按 prepared UV source uniform 选择各 texture role 的采样 UV；`uv0` / `uv1` 来源已可按 render time 应用 scroll multiplier。后续仍需要 synthetic model 渲染不同 UV 层与 scroll 路由贴图，确认 shader-family-specific source 规则能产生可见差异。

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

1. tile matrix / tile index 用于调整 UV 或选择 tile array 的近似层。
2. 用真实 tile array 近似替代当前只读 ColorTable tile properties 的 specular 调制。
3. sphere 作为环境/反射近似，接入更接近 MeddleTools 的 reflection/sphere 节点。

这些贴图已经保留 shader 绑定并提供 debug view 开关；后续重点是把 tile index/matrix 接入真实 tile array 或更接近 MeddleTools 的 reflection/sphere 节点。

验证：

- 已有 native snapshot 覆盖 bind layout/WGSL 编译；后续仍需要 synthetic ColorTable ramp 生成明显 tile/sheen/sphere 差异。
- 与 MeddleTools ramp 输出对照。

### P1: 改善 alpha/glass/transparency

当前 glass 固定 opacity `0.28`；`g_Transparency` 已解析但尚未驱动 opacity。后续需要：

- 继续解析 glass 相关 shader keys/constants。
- 确认 `g_Transparency` 在各 shader family 中是透明度还是 alpha，并接入 renderer alpha。
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
- tile/detail arrays 与 ColorTable extra maps: nearest 或 shader-family-specific，当前 renderer 已用 nearest sampler 消费 ColorTable extra maps；tile/detail array 仍未绑定
- decal: clip/extend 语义，当前尚无独立 texture kind

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
- real tile/detail array preview

这些视图能显著缩短后续对照 Meddle/MeddleTools 的定位时间。

## 建议实施顺序

### 当前下一批工作队列

从当前状态继续推进时，优先级应按依赖关系排：

1. 数据解析：优先补染色输入链路。先把 `ColorDyeTable` 行语义、`stainingtemplate.stm`、EXD stain 参数和用户 stain 输入定义清楚，输出可测试的 ColorTable override；runtime GPU ColorTable 仍只作为 unsupported 输入标记。
2. 结果处理：把 stain、decal/crest fallback、tile/detail array availability 变成 `PreparedModelOptions` / `PreparedMaterial` 能表达的显式输入或 fallback 原因。默认离线模式继续保守，不猜运行时 shape、decal、crest 或 GPU ColorTable。
3. 渲染器：继续消费已经结构化但尚未实际影响画面的低风险参数；detail tint/UV 已有保守 fallback，下一步顺序为 tile index/scale 的保守近似、alpha aperture/offset 的受限 alpha shaping、glass IOR/thickness 的轻量 fresnel/specular 调整；`g_Transparency`、真实 alpha/glass 行为和 toon lighting 需要先确认 shader-family 语义。
4. 验证：每个语义修正都配一个 focused unit test；涉及 WGSL 的改动至少跑 renderer 单测、native snapshot、wasm check，并在真实 phantom 样本上做 ignored snapshot 回归。

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

1. 已完成 GPU 顶点格式扩展；prepared UV source 和保守 scroll time 已进入 WGSL。后续让 shader-family 逻辑实际消费 `uv1-uv3`、`color1`、secondary tangent frame 和 flow。
2. 已完成第一版 per-material texture/sampler config，renderer 已派生 color/data/nearest 三组 sampler。后续补 per-texture independent sampler、shader 级 clip/extend 和真实 tile/detail array 绑定。
3. ColorTable diffuse/specular/material-properties/tile/sheen/sphere/tile-matrix 已进入 renderer；`g_DiffuseColor`、`g_EmissiveColor`、`g_SpecularColorMask`、`g_SSAOMask`、sheen/sphere 常量和 detail tint/UV 已有第一版 WGSL 消费。后续继续补 tile select、multi diffuse/detail normal 和 shader-family-specific source/scroll 规则。
4. 已完成第一版 shader family 分类和 alpha policy/prepared pass 分类；后续把 character/glass/transparency/scroll/lightshaft/reflection 等 family 的关键节点拆成明确 WGSL 函数块，而不是继续扩大单个主 shader 分支。

完成标准：

- MeddleTools 中 ColorTable extra ramp 对应的数据能在 Web renderer 中产生可观察效果。
- `_id.tex` 边界、mask/material map 色彩空间不再依赖统一 sampler 侥幸工作。

### 第三阶段：shader family 和运行时替代输入

1. character glass/transparency/scroll/lightshaft/reflection/stockings/tattoo/occlusion 逐个补齐；其中这些 shader package 已先进入 `MaterialShaderFamily` 分类，具体节点逻辑仍待实现。
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
