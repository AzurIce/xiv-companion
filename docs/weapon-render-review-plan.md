# 武器渲染审查与改进路线

本文是武器模型解析、材质准备和实时渲染工作的权威状态文档。详细调查过程、历史计划和已完成项目的长篇说明保存在
[`weapon-render-review-history.md`](weapon-render-review-history.md)，实际数据流说明见
[`weapon-render-pipeline.md`](weapon-render-pipeline.md)。

目标不是无证据复刻完整 FFXIV shader，而是让离线武器预览达到：

- 静态 SqPack 数据语义正确。
- prepared 决策可审计，unsupported/runtime-only 行为不静默降级。
- 有可靠证据的材质效果进入 renderer。
- 每个语义修正都有 focused test，重要视觉行为有真实或 synthetic WGPU 回归。

## 每轮推进规则

每轮实现前必须依次完成：

1. 确认仓库 clean。
2. 对照当前代码、`E:\repos\Meddle` 和 `E:\repos\MeddleTools` 调查目标语义。
3. 更新本文，使“当前状态、证据、拟实施范围和明确不做的猜测”准确。
4. 单独提交路线更新，再次确认仓库 clean。
5. 实现、补 focused tests、同步本文和 pipeline 文档。
6. 运行完整 native、wasm32、格式与 diff 门禁，提交实现并确认 clean。

不能从参考仓、真实资源或稳定回归证明的公式，不进入 WGSL；应保留 raw 数据并增加明确诊断。

## 核验依据

最近核验：2026-07-14；最近已提交实现基线：`08c44ca`；本轮路线基线：`110c396`；最近视觉回归诊断基线：`bdadb77`。

- 本仓数据层：`crates/xiv-companion-data/src/model.rs`、`mdl_geometry.rs`、`mdl_metadata.rs`、`weapon_models.rs`。
- 本仓渲染层：`crates/xiv-companion-render/src/renderer/model.rs`、`model.wgsl`。
- Meddle：模型/顶点导出、ColorTable 结构、material constant buffer、on-render material output、CRC 名称表。
- MeddleTools：shader node groups、node mappings、texture config、ColorTable ramp 和 bake 路径。
- Penumbra.GameData：Legacy/GUD STM、dye pack 和 ColorTable dye override 规则。
- 本地游戏目录：`E:\_ff14\game`，用于 ignored real-resource audit/snapshot。

## 当前基线

### 数据解析

- raw LOD0 与 extra LOD mesh ranges 已覆盖 normal、water、shadow、terrainShadow、verticalFog、lightShaft、glass、materialChange、crestChange。
- 顶点数据保留 UV0-UV3、color0/color1、primary/secondary normal 与 bitangent、flow0/flow1、blend weights/indices。
- sampler role 优先使用 SHPK resource name，再使用 known CRC 和路径后缀；shader package default 与 MTRL override 可审计。
- Legacy/Dawntrail ColorTable、ColorDyeTable、Legacy/GUD STM 和 stain0/stain1 已贯通同步/异步 loader。
- `GetValues`、flow、specular type、sub color、skin value、decal color、character scroll、lightshaft type 等 material key，以及 tile mip bias/vertex-movement constants 已结构化；需要审计未知值的 key 会保留 raw。
- equipment-style 拳套、PAP hash collision 和 stale material reference 已有确定的 candidate/fallback 规则与来源诊断。

### Prepared 层

- `PreparedModel`、`PreparedMesh`、`PreparedMaterial` 是 renderer 和 phantom summary 的共同输入。
- draw role、render pass、culling、alpha source、texture binding、sampler policy、UV source、feature flags、resource availability 和 runtime fallback 已结构化。
- Opaque、Cutout、Transparent、Glass、AdditiveLightShaft、dither depth 和 outline 路径已有明确分派。
- shadow、terrainShadow、verticalFog 默认不进入 surface pass；crest 使用透明 fallback，materialChange 使用基础材质 fallback。
- unsupported 输入已区分 runtime ColorTable、Option/Decal/Skin/Sub color、decal texture、tile/detail arrays、tile mip bias、vertex movement、AlphaMulti、MultiMap、reflection、environment、lightshaft clip 等原因。

### Renderer

- GPU 顶点格式和 WGSL 已接收所有已解析顶点通道；flow0 和 secondary tangent frame 已在有证据的路径消费。
- 15 个现有 texture bindings 使用逐 role sampler；base/emissive、data maps、index 和 extra ramps 不再共用错误采样策略。共享 tile/detail pair atlas 会逐 half、逐 layer 生成互不污染的 mip，并在 `fract` 前计算显式梯度做 nearest mip selection。
- ColorTable diffuse/specular/material/tile/sheen/sphere/tile-matrix，以及 tile/detail pair atlas 已进入 renderer 和 debug views。
- character normal-B alpha、tattoo normal-A alpha、stockings opaque、water direct alpha/primary wave、bguvscroll Map0/Map1、lightshaft 双纹理与 alpha test 已实现第一版。
- legacy Compatibility specular Default/Mask 已按 installed FXC 证据分别使用 1 与 `mask.r²` factor；其它 package/value mode 保持 generic 行为。
- TileMatrix 使用 float texture；Sheen/Sphere 保留 HDR；Dawntrail TileIndex/SphereIndex 已按 half-float 语义解析。
- 显式 shape mask 可应用 sparse position/normal morph；Web 可选择 Base 或离线 table-order shape。

### 验证

- 数据层和 renderer 有 focused unit tests。
- synthetic native WGPU 覆盖 Map1、tile/detail、ColorTable ramps、tattoo、water、lightshaft、secondary vertex debug 等最终像素差异。
- 透明三角形 synthetic WGPU 会在正背视角翻转同 mesh 红/蓝半透明层的主色，覆盖逐帧动态 index buffer 与实际 blend；45050 已在默认和两个额外视角验证毛片顺序。
- 45052 覆盖 baseline、第一染色通道和第二通道 metallic；45059 覆盖 glass、SphereIndex 与 HDR ramp。
- 45047、45048、45053、45068 的真实 final/tile-normal snapshot 已完成 atlas minification 回归；`fract` 后错误隐式梯度已消除。后续放大审查发现的 45053/45068 稳定网纹也已修复：TileIndex 离散化现按 MeddleTools `tile_select` 使用 `FLOOR`，约 21.58 会选择平缓 layer 21 而不是强网纹 layer 22；45068 final/tile-normal 前景高频分别下降约 53%/94%，45053 tile-normal 下降约 71%。45050 已验证 packed normal B/A 在 mip 中独立线性保留，前景覆盖保持稳定。
- 45050 的毛发透明回归已修复：透明 `_b.mtrl` 为 `GetValuesMultiMaterial + NormalBlue`，807 个顶点中 445 个 A=0，848 个三角形中 400 个全为 A=0；NormalBlue/NormalAlpha 现不再误乘 vertex A。MeddleTools `meddle character.shpk` 的 Alpha 只连接 normal Blue，tattoo Alpha 只连接 normal Alpha，两者都没有 vertex color A 输入。真实 final 前景像素由 29,559 增至 33,358，alpha debug 保留毛束边缘/缝隙透明而主体恢复不透明。
- character Compatibility diffuse 组合已通过 focused/installed tests 和父提交真实回归：当前 `GetValues=GetValuesCompatibility` 与旧式 `GetValuesTextureType=Compatibility` 共用 multiply gate；45068 固定 Compatibility、`#base-times-colorset` 与原 base index；45047/45048/45050/45053/45068 的 PNG 全部逐字节一致。
- WeaponCatalog audit 当前结果：8281 个条目、7365 个唯一模型、8112 个 model-material 引用、6399 个唯一 MTRL、4 个 exact SHPK、0 load/semantic failures；武器范围为 8091 character、15 skin、6 characterGlass。
- material semantic coverage 已完成第一轮全量审计：48 个 scoped key coverage、244 个 constant coverage；每项同时报告唯一 MTRL 资源和 catalog item-material 引用，material/system/scene defaults 与 MTRL override 分开。`characterlegacy.shpk` 的 5519 个资源对应 12145 次 catalog 引用，证明引用数不再误用 model-material 次数。
- `ApplyVertexMovement` 对 6399 个资源全部为 scene default Off，0 MTRL overrides；movement scale/max-length 虽有非默认静态值，仍只作为潜在参数诊断。`g_TileMipBiasOffset` 只有 47320=`+1`、46461/46462=`-1` 三个非零样本，尚无 tile-array LOD 公式证据。
- `CategorySpecularType=Mask` 仅有 4 个 legacy 资源/5 次 catalog 引用。installed SHPK/FXC 证明 Compatibility Mask 使用 `mask.r²` 调制 specular，Default 不使用 mask 调制；WGSL 已按 exact package/value mode 实现，30520 Mask/Default 原生快照 RGB difference 为 24048。
- 未知 constant `0xAD94E254` 在 character 只有 3 个非默认资源/4 次引用，其中 45050 透明毛 `_b.mtrl` 为 1；未知 key `0xF52CCF05` 广泛覆盖 character family。两者继续保留 raw/value histogram，不进入 WGSL。当前全量报告无 malformed、non-finite 或 unresolved constant value。

## 当前工作队列

队列只记录尚未完成且能实际推进的工作。完成后从本节移除，并在“完成能力摘要”增加一条简述；详细证据写入历史档案或提交记录。

### P0：减少静默近似

1. **补齐 shader-family-specific texture/UV 决策**
   - 继续确认 character、skin、glass、reflection、scroll、occlusion 的实际 texture role 和 UV source。
   - 处理仍依赖通用 UV0/repeat fallback 的角色。
   - 保持 per-role sampler 与 prepared UV source 为唯一 renderer 输入。

2. **拆分主 WGSL 的 family 逻辑**
   - normal、alpha、emissive、tile、detail 已拆为明确函数；继续拆分 `fs_main` 中的纹理采样编排、base/material/specular、glass lighting 和最终输出组合。
   - 保持现有输出和 snapshot 稳定，避免继续扩大 `fs_main` 的交叉分支。
   - family-specific 行为应能单独测试和审计。

### P1：有证据时补视觉语义

3. **MultiMap、MultiMaterial、AlphaMulti 和 detail influence**
   - `GetMultiValues` 的 vertex-alpha Map0/Map1 混合已完成。
   - character Compatibility diffuse 组合已修正：MeddleTools character 节点明确以当前 `GetValues=GetValuesCompatibility` 或旧式 `GetValuesTextureType=Compatibility` 驱动原 diffuse 与 baked ColorTable 相乘的唯一 Factor；同步/异步 loader 只在该 gate 命中时使用 `base × ColorTable`，MultiMaterial 使用 baked ColorTable diffuse。原 diffuse 仍保留在 raw texture indices；其它 family 不改变。
   - `GetValuesMultiMaterial` 的 vertex alpha 已确认不能作为 opacity；准确的材质/ColorTable 分区公式仍待节点或游戏 shader 证据。
   - AlphaMulti/2/3 当前保留 mode/raw 并报告 `alphaMultiValues`；MeddleTools 对应输入未连接，暂不实现公式。
   - `g_SamplerMulti` 当前只报告 `multiMapInterpretation`；等待通道证据。
   - detail A/B 层混合已完成，但 detail 对 base 的最终 influence 在 MeddleTools 中仍标为 borked。

4. **特殊 character families**
   - reflection：当前为 generic character approximation，等待可靠 reflection/environment/sphere 输入证据。
   - occlusion：保留 runtime sub-color 诊断，尚无完整 family 公式。
   - skin：Face clamp 已完成；SkinColor、body/face decal 和完整 skin 节点需要显式 runtime 输入。
   - characterScroll：variant/raw 已保留，MeddleTools 未提供专用 scroll 公式。
   - stockings/tattoo：静态可证明的 alpha/pipeline 已完成，运行时颜色/skin material 仍缺失。

5. **Glass、cutout 与透明合成**
   - Glass Mul/Add 目前是显式近似；仍缺真实乘法、折射、厚度和 scene-color transmission。
   - cutout 已有独立 pipeline 和 alpha test，但缺少更多 family-specific cutout 行为。
   - 逐三角形透明排序已完成；互相穿插或循环遮挡的透明面仍需后续评估 weighted blended OIT。
   - `g_ShadowAlphaThreshold` 与 `g_ShadowPosOffset` 等 shadow-only 语义等待 shadow pass 方案。

6. **Water 和 environment 扩展**
   - water refraction、whitecap、WaveMap1 已解析但未消费；MeddleTools 当前输出未连接。
   - crystal/environment binding 已结构化并报告 unsupported，尚无可信坐标和混合公式。
   - sphere/reflection 目前仅为明确标注的 rim 近似，不继续无证据调参。

### P2：运行时输入与几何能力

7. **显式 runtime material inputs**
   - GPU ColorTable、resolved material/texture handles、SkinColor、OptionColor、DecalColor、DecalTexture、crest 仍不能由静态 SqPack 还原。
   - 默认 fallback 已存在；只有调用方能提供真实资源时才设计显式输入和 GPU binding。
   - decal 的 shader-level Clip/Extend 需要与显式 runtime texture 一起设计；当前不预占第 16 个 sampler。

8. **runtime geometry state**
   - 静态 MDL 不包含 runtime shape name 到 bit 的映射；当前 table-order mask 必须保持显式离线约定。
   - 后续在调用方可提供 `ShapeMasks`、enabled attribute mask、skeleton/pose 时接入真实状态。
   - skinning、runtime submesh visibility 和 race-specific equipment pose 尚未实现。

9. **验证覆盖扩展**
    - 为每个新增 family 行为增加最小 synthetic fixture。
    - 继续寻找真实 Map1、Flow、water、reflection、occlusion 样本；武器目录没有 bg/bguvscroll 真实校准样本。
    - 评估把 P0/P1 phantom 子集作为可选 CI 任务。
    - sampler policy debug preview 仅在实际定位需求出现时增加。

## 证据不足而明确延后

以下项目不是“忘记实现”，而是当前证据不足：

- AlphaMulti/2/3 通道公式。
- MultiMap 通道解释。
- detail 对 base 的最终 influence。
- lightshaft Type/AngleClip/NearClip 游戏裁剪公式。
- character reflection、crystal environment 和 sphere map 的真实混合公式。
- water refraction、whitecap、WaveMap1 输出公式。
- Glass 真正的 blend equation、折射和厚度传输。
- `color1`、`flow1` 及其它 UV1-UV3 的 family-specific 最终用途。

这些输入应继续保留 raw/structured data、debug view 或 unsupported 标记，不以视觉近似冒充完成。

## 完成能力摘要

以下能力已经完成，不再作为活动待办：

- 模型路径、LOD/extra mesh、draw role、submesh metadata 和加载失败诊断。
- prepared model/material/pass、resource validation、runtime fallback 和 unsupported summary。
- GPU 扩展顶点格式、逐 role UV/scroll/flow、secondary tangent frame 和 debug views。
- 独立 sampler policy、ColorTable extra maps 和 float/HDR payload。
- 逐 half/layer pair-atlas mip、`fract` 前显式 LOD 梯度，以及 RG 法线与 B/A packed payload 分离的 mip 语义。
- ColorTable TileIndex 与 `g_TileIndex` fallback 按 MeddleTools `FLOOR` 语义离散选层。
- character NormalBlue 与 tattoo NormalAlpha 透明度不再误乘 vertex A；normal channel 继续控制边缘透明。
- character 当前/旧式 Compatibility key 使用 `base × ColorTable`，MultiMaterial 使用 baked ColorTable diffuse；同步/异步 loader 共用同一策略。
- exact-SHPK material semantic audit：scoped key/default/override、constant/default/override、shader flags、资源/catalog 引用双计数、unknown/malformed/non-finite/unresolved 与代表样本。
- `MaterialSpecularType`、tile mip bias 和 vertex-movement 参数已结构化；legacy Compatibility Default/Mask 已按 FXC 证据分别使用 1 与 `mask.r²` specular factor，tile bias/movement 无公式部分保持 unsupported。
- Legacy/Dawntrail staining、Web 双通道染色选择、正式染色视觉回归。
- Opaque/Cutout/Transparent/Glass/AdditiveLightShaft、dither depth、outline 和逐三角形透明排序。
- character/tattoo/stockings/water/bguvscroll/lightshaft 的证据可支持部分。
- equipment-style fist、stale material reference、shape morph loader/renderer/Web 路径。
- decal color、GetValues raw、scroll variant、ambient occlusion、lightshaft clip 等可审计语义。

完整完成记录、真实样本数值和历史决策见
[`weapon-render-review-history.md`](weapon-render-review-history.md)。

## 每轮验证门禁

实现提交前至少运行：

```powershell
cargo test --workspace --all-features --exclude xtask-update-craft-data
cargo check --workspace --all-features --target wasm32-unknown-unknown
cargo fmt --all -- --check
git diff --check
```

涉及最终像素、GPU layout、texture format、blend/depth 或 shader entry point 时，还必须运行对应 native WGPU fixture；涉及真实资源语义时运行 focused installed-game regression 或 catalog audit。

## 完成判定

本路线只有在以下条件全部满足时才能标记完成：

- 当前工作队列为空，或剩余项均被证明只能由未提供的外部 runtime 输入解决，并已有明确接口/fallback 结论。
- 所有已解析且有可靠公式的关键材质语义都进入 prepared/renderer，不存在静默错误降级。
- P0/P1 代表武器在颜色、材质分区、透明、发光和 shape 上有稳定回归。
- native、wasm32、格式和 diff 门禁通过，仓库 clean。
