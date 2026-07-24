# 武器渲染审查历史档案

## 2026-07-23 Legacy Gloss permutation/pass provenance 审计

- vertex/PS pairing 现把 1440 个主颜色 Gloss PS 经 9216 条 SHPK node/pass 记录归约到 16 个唯一 VS；16/16 的 `TEXCOORD4.w` 都执行同一高度控制公式：`clamp((position.y * g_ModelParameter.x + g_InstanceParameter[4].y) * g_InstanceParameter[4].x, g_InstanceParameter[4].z, g_InstanceParameter[4].w)`。TEXCOORD4 根据 permutation 打包到 `o3/o4/o6`，但 16/16 同时与投影前 position XYZ 共用同一 semantic，且 `w` 写入全部为上述 `mad -> mul -> max -> min` 链。SHPK scalar parameter metadata 将 `cb2/cb3` 分别固定为 `g_InstanceParameter/g_ModelParameter`；参考仓没有这两个 runtime buffer 的分量级 provider，不能把它误命名为静态模型 bounds。
- 对上述 16 个 VS 的 DXBC reflection 进一步确认 `g_InstanceParameter[4]` 是 `InstanceParameter.m_Wetness`（offset 64，buffer size 176），`g_ModelParameter[0]` 是 `ModelParameter.m_Params`（offset 0，buffer size 16）。因此该高度公式可准确归类为 runtime wetness 垂直控制，而不是未知 bounds；Meddle/MeddleTools 不导出其运行时值。prepared runtime requirement 新增 `modelWetnessParameters`、`sceneAmbientParameters` 与 `reflectionArrayTexture`，分别表达 render-object wetness、160-byte ambient state 和 scene cube-array 所有权。
- Gloss environment audit 进一步证明全部 1440 个主颜色 shader 都把 Gloss 传播到两次 `sample_l texturecubearray`，共 2880 次且无未分类。864 个 forward 变体直接使用上述 `TEXCOORD4.w`，576 个 deferred 变体从 `g_SamplerGBuffer1.w` 读取控制输入。对应 producer 也已闭环：288 个 PS 将 TEXCOORD4.w 传播到 `o1.w`，经 6144 条 node/pass 记录连接到 16 个唯一 VS，16/16 均执行同一语义 height clamp（四个 permutation 因额外 constant buffer 而把 Instance/Model slot 从 `cb2/cb3` 平移到 `cb3/cb4`）。正控制值 `h` 会形成 `effectiveGloss = mix(Gloss, 10-Gloss, h²)`，否则保持 Gloss；随后显式 LOD 为 `6 * (1 - (1 - 8 / (effectiveGloss + 9))²)`。ReflectionArray 后处理也已全量分类：1440 次当前 `envLocationIndex` 与 1440 次上一 `envLocationIndexPrev` 采样全部执行 `rgb²/(alpha+0.0001)` HDR 解码且传播到 `o0.rgb`，1440/1440 shader 按 `envLocationInterpRate` 混合两层、应用 `reflectionScale/reflectionOffset`，并以 `bakeLightRate` 组合带 `2.356194` 系数的 ambient 分支。16 个最初未匹配 shader 只是 RGB 写在 YZW lane、literal 首项为零；放宽 swizzle 形状后未分类为零。这些字段均由 DXBC reflection 明确归属 `g_AmbientParam`。renderer 当前使用 procedural studio environment，没有游戏 ReflectionArray 或 runtime Instance/Model/Ambient parameter provider，因此本轮把公式与边界写入审计而不把它近似塞进现有 roughness 接口。
- Legacy environment 与 SpecularStrength 现形成严格集合分区：864 个同时含 ReflectionArray cube、Gloss 与 material-properties SpecularStrength 的 shader，864/864 都在 HDR environment/ambient 结果之后汇入同一乘加链，cube 子集中未 join 为零；128 个同时含 Gloss/SpecularStrength 的 shader 没有 cube sample，代表 PS24 即属非环境 pass；另有 16 个 SpecularStrength shader不采样 Gloss。三类合计精确覆盖 1008 个 SpecularStrength sample。deferred GBuffer1.w 的 576 个 cube permutation 使用独立 deferred material/color 输入而不重新采样该 table lane，因此不能沿用 forward join 公式。此前把 non-cube shader误列成 environment unjoined 已修正。
- material-strength/vertex DXBC audit 现将 `g_SamplerGBuffer1` 的物理 resource lane、直接 consumer opcode、到 `o0.rgb` 的 taint propagation 与对应 producer `o1` 写入一并固定。576 个 Legacy deferred shader 全部读取 X/W，两个 lane 均进入 Final；288 个匹配 producer 都写 `o1.xyzw`，其中 `o1.y` 都是 Gloss 指数结果，来自 272 个固定与 16 个动态 Gloss/W table sample。`o1.x` 则严格分为 144 个 `mul`、48 个 `mov`、96 个 `movc` 写入；mul 分支同时接收 144 个 SpecularStrength/W sample 和 X=6.5/7.5 的多 lane 输入。因此 X 已证明不是可单独命名的 raw SpecularStrength，暂不改写 WGSL。另有 64 个 modern deferred shader 读取独立的 GBuffer1.Y/Z/W，不能混入 Legacy 结论。
- Legacy `o1.x` producer 现按最终写入 opcode 映射回 SHPK node/pass/material keys。`mul` 为 144 PS / 3072 node-pass records，唯一 pass ID `0x03ac862e`；`mov` 为 48 / 1024、`movc` 为 96 / 2048，二者唯一 pass ID 均为 `0x6006067f`。`mov` 的 8 组 node key 全部为 `GetDecalColorOff`；`movc` 的 16 组全部为 Alpha/RGBA；`mul` 的 24 组覆盖三种 decal mode。代表 assembly 证明 `movc` 条件来自 Decal texture Alpha 与 runtime `g_DecalColor.w` 的乘积，并以 `0.75` 为阈值选择 `0.125490/0.172549`。这将 X 的边界从“多 Table-lane composite”进一步收紧为“pass-specific、且部分依赖 runtime decal 的 composite”，所以当前静态 weapon preview 继续不实现该 deferred attachment。
- Legacy deferred consumer 新增按分量 terminal-multiplier 审计。它按 source swizzle 将 `movc rN.xy` 的 X/Y 分开，并在 `if/else/endif` 汇合处合并控制流状态；旧的寄存器级 first-join 统计因会产生跨 lane 假阳性而删除。installed 结果为 576/576 X consumer 经 replicated scalar `mul` 进入 RGB、576/576 到达 `o0.rgb`、576/576 随后首个合成为 `mad`，0 unclassified；全部属于 pass `0x955c0b73` 的 3072 node records。乘法侧始终含 ReflectionArray，288 个 permutation 同时含 LightDiffuse/LightSpecular；后继独立项不含 ReflectionArray/LightSpecular并新增 384 个 Diffuse sample。该结果确认 X 是 environment/scene-light 复合支路的 deferred multiplier boundary，而非 raw SpecularStrength，故没有据此改写 preview WGSL。
- SpecularStrength terminal product 现按 cube/Gloss 资源集合回连 SHPK node/pass/material keys。Legacy 1008 个 sample 严格分为：864 个 `cube_gloss` Final shader，product 864/864 到达 `o0.rgb`，pass 仅 `0xc885bbd3/0xf21a038f`；128 个 `non_cube_gloss` 与 16 个 `no_gloss` product 均 0 次到达 `o0.rgb`，pass 只为 `0x03ac862e`。后两类合计 144，精确等于 producer audit 的 144 个 `o1.x=mul` PS，代表 assembly 直接把 SpecularStrength 与另一 Table composite 相乘写入 MRT。此前“128+16 可能是未分类 Final pass”的待办由此关闭；它们无需进入 preview surface，剩余差异仍集中于 864 Final environment composition 与 576 deferred consumer。
- 上述 864 个 Final shader 曾按 strength product 的首个后继 opcode 分成 144 个 `first-log` / 720 个 `first-mul`；完整代表汇编证明这个分区只反映乘法因子的结合顺序，后者会在取 log 前继续补乘其它因子。新增的分量 taint assertion 固定两组共 864/864 最终都执行 `log → ×0.2 → exp` 的五次方根与相同 wetness `max/mad/min/movc` 整形，不再把 720 组误述为另一套线性 product family。该 forward 公式与 deferred GBuffer1.X 开头同构，统一 GGX 的 `SpecularStrength → F0` 因此继续明确定位为 preview，而非官方 Final path 的复原。
- forward SpecularStrength composite 的 terminal boundary 进一步完成全量 swizzle-aware 审计：864/864 都作为 replicated scalar 乘入到达 `o0.rgb` 的三分量支路，随后 864/864 首个合成均为 `mad`，0 unclassified。乘法侧 864/864 含 ReflectionArray 和完整 material provenance，576 个同时含 LightDiffuse/LightSpecular；后继独立项不含 ReflectionArray/LightSpecular，并在 576 个 shader 新增 Diffuse。由于这与局部 `SpecularStrength → GGX F0` 数据流直接矛盾，WGSL 现仅对 exact Legacy ColorTable 分支把该 raw strength 从 preview F0 隔离；float payload/debug/unsupported 保持，modern/general preview 不变。native fixture 固定 Legacy strength 1/100 的 Final 完全一致而 MaterialProperties debug 不同。
- terminal `mad` 链的 constant-buffer 所有权新增 reflection-aware 审计：`cube_gloss_first_log` 144/144 与 `cube_gloss_first_mul` 720/720 都声明并读取独立 16-byte `g_MaterialParameterDynamic.m_EmissiveColor`（offset 0），且 RGB 全部传播到 `o0.rgb`；128+16 个 producer class 均为 0。Meddle/MeddleTools 没有导出该动态 cbuffer，且它与静态 MTRL `g_EmissiveColor` 不同。prepared 因此新增 `materialDynamicEmissiveColor` runtime requirement，并继续保留静态 emissive/ColorTable emissive 作为各自已知数据，不把两者静默合并。
- dynamic emissive 的另一乘法侧现按 Table sample 坐标和分量 taint 闭环：两个 Legacy cube class 合计 864/864 都由 `g_SamplerTable` texel 2.5 的 RGB 到达该乘法并继续到 `o0.rgb`，四个 producer subclass 均为 0。继续追踪其后继时发现两种等价 composition：216 个短路径直接 `mad`，648 个长路径先 `mul` 再继续合成；两者 864/864 都使用 `max(dot(preEmissiveLighting, vec3(0.298910, 0.586610, 0.114480)), 1)` 缩放 emissive 并到达输出，0 unclassified。luma 源本身在 864/864 中还通过独立 `mad` 作为 RGB 照明项进入 Final，故可从未知 `runtimeComposite` 收紧为 pre-emissive lighting composite。modern Character 的 256 个 ColorTable Final shader 同样全部采用该公式（64 `mad` / 192 `mul`），luma 源也 256/256 独立进入 RGB。provenance 显示它全覆盖 Ambient、Instance、material 与 Normal/Occlusion/Table/Tile，部分 permutation 再加入 Camera、Diffuse/Light/GBuffer/Decal；Meddle/MeddleTools 搜索只找到静态 emissive/ColorTable 映射，没有这些 runtime buffer。WGSL 现使用已有 `lit` 作为离线 pre-emissive lighting，按 verified Rec.601/max 形状只缩放 exact Character/Legacy ColorTable emissive；静态/shader emissive 保持独立。随后 renderer 通过 `ModelRenderOptions.dynamic_emissive_color` 将该 cbuffer 语义作为 per-render camera uniform 输入接入，默认单位元且 native exact/control 回归证明范围只限 ColorTable Final。
- 同一 reflection-aware audit 进一步检查 176-byte `g_InstanceParameter`：`m_MulColor`、`m_CameraLight.m_DiffuseSpecular` 与 `m_CameraLight.m_Rim` 在两个 cube class 合计 864/864 都传播到 `o0.rgb`，`m_EnvParameter` 则为 0/864；四个 non-cube/producer subclass 对这些字段均为 0。prepared 新增 `instanceMulColor` 与 `instanceCameraLightParameters`，明确当前 procedural studio key/rim 只是离线 preview，而不是这些 runtime vector 的恢复；没有为未消费的 `m_EnvParameter` 制造 requirement。
- renderer 的统一 GGX preview 现以独立 exact-package flag 消费 Legacy material-properties Z 中的 raw Gloss，并在 WGSL 采样、A/B 混合与染色之后计算 `exp2(-Gloss/15)` 作为 roughness 参数。没有在 parse/bake 阶段派生，因此 Legacy STM 对 `gloss_strength` 的修改不会留下过期 roughness；modern、glass 和无 ColorTable fallback 不受影响。native fixture 固定相同 raw payload 在 modern/legacy Final 中产生差异、两个 Legacy Gloss 值产生差异，同时 MaterialProperties debug 完全相同。
- 进一步的完整寄存器回溯证明 1440/1440 个主颜色幂次链不是早期简写的 `N·H`：它们从同一 camera-relative position 构造 view 与 `(0,0.2,0)` offset light，执行 `reflect(-V,N)`，最终 lobe 为 `min(3-3(1-saturate(N·L))²,1) × saturate(R·L)^Gloss`。31 个最初未分类 shader 只是把平方结果写入另一寄存器，另一个 shader 用 `mad_sat` 代替等价的 `mad+min`；泛化 parser 后未分类为零。WGSL 现以独立 Legacy direct-specular 分支消费完整 lobe，environment 仍使用上述 MRT roughness 参数。
- installed `characterlegacy.shpk` 的 1712 个 Gloss sample 现按直接 consumer opcode、literal、pixel shader、SHPK node material-key 与 pass ID 联合分类。固定为五类：`Gloss-1` 88、`Gloss*-0.066667` 184，以及三类包含 `9/10` 的多 consumer 组合 288/576/576；每个 sample 都对应唯一 pixel shader。正式 JSON 同时保留每类的代表 shader、consumer context 和 material-key 分布，不再只给全局 opcode 总数。
- 分量级 forward-taint 审计进一步固定输出覆盖边界：modern `character.shpk` 的 Roughness 为 976 次采样、768 次到达 `o1.y`；Legacy Gloss 为 1712 次采样、272 次到达 `o1.y`；glass Roughness 为 32 次采样、0 次到达 `o1.y`。Legacy 的 272 恰好等于 `Gloss-1` 88 与 `Gloss*-0.066667` 184 两类之和。剩余 1440 个 sample 全部到达 `o0.rgb`，并且 1440/1440 都匹配相邻的 `log → ×Gloss → exp` 幂次链；五个 consumer class 逐类固定为 288/0、0/88、576/0、576/0、0/184（`o0.rgb`/`o1.y`），两类输出没有交叉。
- `Gloss-1` 类只出现在 `GetValuesCompatibility + CategorySpecularType=Mask` 的六组 node keys；其两个 pass 随后按 mask 条件在 `1` 与 Gloss 之间选择，再执行 `exp(value * -0.066667)`。非该特例的多 render-target 路径直接执行 `exp(Gloss * -0.066667)` 并写入 `o1.y`。这里的 DXBC `exp` 是 base-2，因此证据对应 `exp2(-Gloss/15)`，不是自然指数；尚未给未知 pass CRC 冒充官方名称。
- 其他 pass 仍把 raw Gloss 作为高光幂次；所以 `o1.y` 的单一输出变换不能冒充全部 legacy Final lighting。后续完整寄存器回溯已经把早期简写的 `N·H` 更正为 camera-reflection direct lobe，并把 `Gloss+9` / `10-Gloss` cube-array LOD、height-control 来源及 deferred `GBuffer1.w` producer 闭环。当前仍未闭环的是 `g_InstanceParameter/g_ModelParameter` 的 runtime 值、ReflectionArray 资源及后续 environment/scene color accumulation；renderer 因此只接入当前 pipeline 能同构表达的 MRT roughness 与 direct lobe，并继续以 structured unsupported 标记剩余 pass-specific 差异。
- `PreparedMaterialUnsupportedInputs` 新增 `legacyGlossComposition`：exact `characterlegacy.shpk` 且保留 ColorTable rows 时显式报告该 preview 差异，Unsupported debug 使用独立颜色。此前 `legacySpecularType` 只描述 key/Mask 组合，不能代表 Gloss 已被正确消费；两者现在分开诊断。

## 2026-07-22 Gloss/SpecularStrength installed DXBC consumer 审计

- 新的 installed SHPK audit 按 `g_SamplerTable` texel X 与 resource swizzle 解析物理 W lane，并追踪该寄存器在覆盖前的直接 consumer。modern `character.shpk` 没有 Diffuse/W Gloss sample；Specular/W 共 256 个样本且 256/256 都直接进入 `mul`。`characterlegacy.shpk` 的 Gloss 为 1712/1712 consumer，直接 opcode 分布为 add 2968、mad 1440、mov 864、movc 576、mul 1624；SpecularStrength 为 1008/1008，第一消费者全部为 `mul`。`characterglass.shpk` 两者均为零。
- 结合 installed SpecularStrength 最大值 `100`，先前 WGSL 在组合前执行的 `[0,1]` clamp 会破坏游戏 shader 明确保留的 raw multiplier。renderer 现只拒绝负值，不再设置上限；`surface.material_specular * 0.08 * strength` 后的最终物理 F0 仍限制在 `[0,1]`。native HDR Final fixture 固定 strength `100` 相比 `1` 产生更强响应，source regression 禁止提前 clamp 返回。
- Legacy Gloss 的 consumer 明确包含 `-0.066667` 指数路径和 `9/10` 常数的多分支组合，不能归约为一个统一的 `gloss -> roughness` 公式。本轮因此只把完整计数写入 full audit/report，并继续保留 float raw/debug，不把单个代表 permutation 冒充全部 legacy Final。

## 2026-07-22 ColorTable MaterialProperties float payload 修正

- full installed audit 新增 Metalness、Roughness、GlossStrength 与 SpecularStrength 全量值域。6,394 个含 modern extra fields 的资源中 Metalness/Roughness 均为 `0..1`；6,397 个含颜色字段的资源中 GlossStrength 为 `0.7998047..193.375`、SpecularStrength 为 `0..100`。此前 material-properties A/B ramp 只有 RGBA8 UNORM，后两者所有大于 1 的真实值都会在 bake 时静默变成 1。
- `BakedColorTableMaps` 现同时保存兼容 UNORM 与未 clamp 的 material-properties float/A-B payload；同步/异步武器 loader 将 float channels 附到 canonical baked texture，renderer 检测后以 `Rgba16Float` 上传。Metalness/Roughness/GlossStrength/SpecularStrength 的通道顺序保持 MeddleTools `PackedColorTableRampLookup` 定义。
- MaterialProperties debug 被加入 HDR-preserving debug 范围，避免 GPU 已正确采样后又在 debug output clamp 回 1。数据层测试固定 GlossStrength `193.375` / SpecularStrength `100` 的 bake 保真；native pre-compose 回读固定 MaterialProperties `(Metalness, Roughness, SpecularStrength) = (1, 0.5, 100)`。Final 中 SpecularStrength 到 Principled/F0 的现有消费边界保持不变，GlossStrength 的最终用途不因数据保真修复而推测扩展。

## 2026-07-22 WaterDeepColor HDR 直通修正

- Water 已验证子集中的 `g_WaterDeepColor` 此前在 WGSL Final 被限制到 `0..4`。MeddleTools water 配置通过普通 `ColorMapping` 将该字段直接连接到 water 节点颜色输入，现有参考没有这个 preview-only 上限；Rust uniform 端已经按通用颜色策略对非有限分量做默认 fallback。
- WGSL 现直接使用有限线性 `g_WaterDeepColor.rgb`。Refraction、Whitecap、WaveMap1 和折射输出仍保持既有 unsupported 边界，没有借此引入未连接的 water 公式。
- native WGPU 在 Water family 的 BaseColor debug 下回读 `g_WaterDeepColor=[12,0,0]`，pre-compose `Rgba16Float` scene 中心像素在 half-float 精度内保持 12；原 transparency 与 primary WaveMap normal 最终像素断言继续通过。

## 2026-07-22 shader color HDR 直通修正

- WGSL 此前把 `g_DiffuseColor` / verified BG `g_MultiDiffuseColor` 限制到 `0..4`，并把材质 emissive fallback 与 `g_EmissiveColor` 分别限制到 `0..4` / `0..8`。这些上限只存在于 preview shader，没有 Meddle、MeddleTools 或 installed SHPK 证据，会让未来 HDR 或负向线性颜色输入在进入 `Rgba16Float` scene 前静默改变。
- MeddleTools 的 `ColorMapping` 将 `g_DiffuseColor` / `g_EmissiveColor` 通过 `toBlenderColor` 做 Rec.709 linear 到工作空间转换后直接写入节点颜色输入，没有上述 clamp；本项目 Rust uniform 端也只对非有限值执行默认 fallback。WGSL 现保持这些有限线性 float 原值，verified BG Map0/Map1 路径的两个 diffuse tint 同样不再被 preview 常数截断。
- native HDR scene readback 直接锁定 `g_DiffuseColor=9` 的 BaseColor debug 与 `g_EmissiveColor=16` 的 Emissive debug 均在 half-float 精度内保真。full installed audit 同时固定当前四个实际 package 的值域：character/legacy/glass Diffuse 均为 `[1,1,1]`、Emissive 均为零，唯一 skin Diffuse 为 `[1.4,1.4,1.4]`；7365 models、8112 material references、6399 unique MTRL、0 failures/semantic failures。

## 2026-07-22 native Map1 fixture 与正式验证覆盖修正

- `render_mock_secondary_scroll_map_snapshot` 此前把单个 RGBA texel 声明为 `2x1`，renderer 因 payload 少于 `width * height * 4` 而不会上传该纹理，Map1 因而采样为黑色。fixture 现使用与 payload 一致的 `1x1` 尺寸，并让 secondary specular 保持中性，避免 BG material-properties 响应遮蔽本测试真正要验证的 Map0/Map1 color blend 与 secondary normal tangent frame。
- 修正后该 fixture 的 vertex-alpha Map0/Map1 色相断言和 `normal1/bitangent1` 最终像素差异均通过，完整 native WGPU suite 为 40/40。
- `scripts/verify-weapon-render.ps1` 的 native 命令此前只启用 `render-test-support`，三个 `#[cfg(feature = "game-data")]` installed GPU fixture 会在编译阶段被排除。正式入口现启用 `game-data,render-test-support`，并在已解析的 `XIV_GAME_DIR` 下实际执行全部 40 个 ignored native fixture。
- 正式脚本已端到端实跑通过：full installed WeaponCatalog/SHPK audit、40 个 native fixture、7 个 P0/P1 phantom、renderer/workspace tests、wasm32 check、format 与 diff gate 全部成功；audit 仍为 7365 models、8112 material references、6399 unique MTRL、0 failures。
- phantom harness 现在会在写 summary 前验证最终 PNG 含有足够多与角落背景显著不同的像素，避免 mesh/pass/binding 回归生成全背景图却仍被当作成功。阈值为 viewport 的 `0.1%`（最低 256 pixels），并忽略 RGB Manhattan distance 不超过 18 的轻微背景误差；同一统计还要求可见包围盒与 viewport 保持 `max(minDimension / 256, 2)` pixels 的安全边距，以捕获 bounds/camera 回归造成的裁切。`coveredPixels` 与 `[minX,minY,maxX,maxY]` bounds 同时写入每个 `model-summary.json.snapshotVisibility`，并作为 `visible pixels` / `bounds` 两列进入生成的 `index.md`，可直接做跨版本审计。7 个 P0/P1 实样覆盖为 28,452..82,065 pixels，最小实际边距仍超过 100 pixels；细长武器、双持、透明毛发和 glass 均有充分余量。focused test 同时锁定纯背景为 0、轻微色差不计数和包围盒坐标。

## 2026-07-22 alpha shaping VS TEXCOORD4 空间审计

- installed SHPK 审计现按 node/pass 配对 alpha shaping PS 与实际 VS，而不是假定同编号 shader 相连。Character 的 768 个 alpha PS 只连接 64 个 VS，CharacterGlass 的 34 个 alpha PS 只连接 8 个 VS；这些 paired VS 的 `TEXCOORD4` 全部打包到 `o6`，与 PS 的 `v6 = TEXCOORD4` 一致。
- Physis 暴露的 VS bytecode 比 DXBC 容器头声明长度多 8 字节。审计先按 DXBC header 的 total-size 裁剪再反汇编，修复此前 `D3DDisassemble HRESULT 0x80004005`；本机 Windows SDK `fxc /dumpbin` 保留为 fallback。
- paired VS 的 `o6.xyz` 源寄存器在全部 64/64 Character、8/8 Glass permutation 中同时作为 22..25 四行矩阵的投影输入并生成 `SV_Position`。结合 PS 中归一化 `-v6`，现可确认 shaping view operand 是指向相机的投影前位置向量；点积在 view/world 旋转之间不变，因此当前 `cameraPosition - worldPosition` 的方向构造不再是主要缺口。
- WGSL 仍不消费 aperture/offset。新增 operand audit 显示 Character 的 shaping scale/base 分别出现 18/12 种寄存器形态，Glass 为 9/5 种；根来源审计锁定所有 scale 都经过 Normal sampler、vertex color、UV 与 material/global constants，base 还跨越 Index/Mask/Table/TileOrb（Character 另有 Decal）及 runtime constant buffers。它们不是一个可直接替换为 Final opacity 的统一输入。非 view dot 操作数也按 producer opcode 单独计数，避免只证明视线来源就宣称完整 normal 等价。在这些输入完成逐 permutation provenance 之前继续输出 `alphaShaping` unsupported。

## 2026-07-21 `0xAD94E254` vertex-alpha remap 闭环

- installed `character.shpk`、`characterlegacy.shpk`、`characterglass.shpk` 的全部消费者都执行 `mix(vertexAlpha, 1, constant)`；结果随后要么乘入 surface alpha，要么与 texture alpha 一起进入 discard threshold test。`skin.shpk` 不消费该常量。audit 现在会断言这两类 alpha consumer 与常量 use count 完全一致。
- 代表 character permutation 进一步证明另一乘数来自主 Normal Blue，并最终进入 `o0.w`。因此先前“NormalBlue 永远忽略 vertex A”的修复范围过宽；正确规则是仅上述三个 DXBC package 使用 remapped vertex alpha。
- `ModelMaterial.vertexAlphaToOne` 以中性描述名保存 CRC `0xAD94E254`，不冒充官方常量名。WGSL 在 texture alpha 组合前应用该 remap。45050 透明毛 `_b.mtrl` 的值为 `1`，所以其 445 个 vertex-A=0 顶点仍会被提升到 alpha 1；默认值 `0` 的 character 材质则恢复 vertex alpha 影响。
- synthetic native WGPU 同时固定 constant `1` 时 vertex A 0/1 输出一致、constant `0` 时输出显著不同；45050 真实 phantom 回归通过。

## 2026-07-21 ColorTable Diffuse/Specular HDR 值域与 Diffuse payload 修正

- installed audit 新增 Diffuse RGB、Specular RGB 与 SheenAptitude 的全量值域：6,397 个含颜色字段的资源中 Diffuse 最大 `6.7929688`、Specular 最大 `4900`；6,394 个 extra-field 资源中 SheenAptitude 最大 `52.09375`。这证明这些并非理论越界值。
- Specular 与 SheenAptitude 已有 float payload；本轮补齐 canonical baked Diffuse 的线性 float A/B payload，并让 renderer 对带 `rgba_f32` 的 BaseColor 使用 `Rgba16Float`。兼容 sRGB RGBA8 继续保留，不再作为唯一数据源。
- Compatibility `base × colorset` 不再只乘两个已经 clamp 的 sRGB8 texel；它现在把 source base 解码为线性值，乘以未 clamp colorset float RGB，并保留 base Alpha。byte fallback 维持原行为。
- native test-support 从 pre-compose scene 直接回读 BaseColor `6.7929688`、Specular `4900`、Emissive `61.46875` 与 SheenAptitude `52.09375`，均在 half-float 精度内匹配 GPU 采样值。审计同时确认 A/B ColorTable ramp 主路径使用 mip 0 `textureLoad` 后显式混合，单级 float ramp 不会引入 A/B mip 边界污染。
- full installed audit 新增 float-upload 守卫：Diffuse/Specular/Emissive、Anisotropy、SheenRate/Tint/Aptitude 与 SphereMask 中任一非有限值或绝对值超过 `Rgba16Float` 最大有限值 `65504` 都会成为 semantic failure；当前 6,399 个资源全部通过。

## 2026-07-21 ColorTable Emissive HDR payload 修正

- installed audit 已锁定 ColorTable Emissive 最大值为 `61.46875`；此前 canonical baked emissive 只有 sRGB RGBA8，所有大于 1 的线性发光值都会在上传 HDR scene 前被静默截断。
- `BakedColorTableMaps` 现在同时保存兼容 RGBA8 与线性 float emissive payload，loader 将 float channels 附到 canonical baked texture，renderer 优先以 `Rgba16Float` 上传。byte-only/source emissive 继续使用既有 sRGB mip 路径。
- 数据层测试锁定 `[61.46875, 2.0, 0.5]` 的 bake/loader 保真；native WGPU fixture 对比发光强度 `2.0` 与 `8.0`，输出分别为 `[251, 101, 101]` 与 `[254, 210, 210]`，证明 tone mapping 前的 HDR 细节仍可区分。test-support 现在还可读取 pre-compose `Rgba16Float` scene；Emissive debug 对源值 `61.46875` 的 GPU 采样在 half-float 精度内直接匹配，不再只依赖最终 PNG 的间接差异。

## 2026-07-21 ColorTable Anisotropy HDR payload 修正

- installed audit 已锁定 6,394 个 ColorTable 材质中的 Anisotropy 值域为 `0..=7`；此前 baked specular Alpha 使用 RGBA8 UNORM，导致大于 1 的值在进入 modern shaping 前被静默夹成 1。
- installed `stainingtemplate_gud.stm` 的 43 个模板、10,922 个 Anisotropy dye scalar 当前全部为 `0`，因此真实高值来自 MTRL ColorTable 而不是染色覆盖；ignored audit 保留有限性和范围统计，以便未来游戏数据变化时重新审查。
- canonical baked specular ramp 现在附带线性 RGB + 未 clamp Anisotropy 的 float payload，renderer 以 `Rgba16Float` 上传；原始 packed specular 仍保持 Non-Color RGBA8。普通 GGX anisotropy 仍在 BRDF 处限制到 `[0,1]`，只有 installed DXBC 已证明的 modern shaping 使用原始值。
- 数据层测试锁定 Anisotropy `7.0` 的 bake/loader 保真，native WGPU 对比 `1.0` 与 `7.0` 并要求 shaping 产生显著像素差异。

## 2026-07-21 GUD STM SphereMapIndex 值域审计

- installed `stainingtemplate_gud.stm` 的 43 个模板、每模板 254 个染剂共 10,922 个 SphereMapIndex scalar 全部为有限整数 `0`；当前转换为 ColorTable `u16` SphereIndex 不会损失 installed 数据。
- 数据层新增 ignored audit，锁定该列保持有限、离散且处于 `u16` 范围；未来游戏数据若出现非整数或越界值会显式失败，而不是静默截断后进入 sphere ramp。

## 2026-07-20 ColorTable Tile A/B 双采样对齐

- ColorTable bake 不再只保存 A/B 插值后的 TileIndex、TileAlpha 和 TileMatrix；每个源 index texel 现在按相邻 `[A, B]` texel 打包两套 tile properties 与 float TileMatrix，并保留原始 index texture G。installed DXBC 证明偶数行是 A、奇数行是 B，最终使用 `1 - G` 作为从 A 到 B 的插值因子；数据层静态 bake 与 WGSL A/B tile blend 已按该方向修正。
- renderer 对 A/B 分别执行离散选层、TileMatrix UV 变换、矩阵轴归一化后的 tile-normal XY 旋转，以及 `g_TileMipBiasOffset` LOD 计算。Tile Normal/ORB 都在各自 layer/matrix/LOD 下采样后再混合，不再先插值矩阵后单采样。
- TileAlpha 不作为材质透明度。对 ORB 使用游戏的中性混合 `neutral + TileAlpha * (sample - neutral)`，其中 neutral 为 `(1, 0.5, 1)`；对 tile normal 则继续与采样 normal Alpha 相乘形成贡献权重。
- 数据层测试锁定多像素相邻 A/B packed layout、原始 G payload 与 float matrix payload；native WGPU fixture 锁定 `1-G` 的 A/mid/B layer 混合、A/B 独立法线旋转和 TileAlpha=0 的 ORB 中性回退。现场 audit 还确认 modern character 的部分 permutation 会按 roughness/view 对该基础权重继续整形。

## 2026-07-21 modern ColorTable blend shaping 对齐

- installed `character.shpk` DXBC 的 Table texel 4/W shaping sample 与 `1-index.G` 逆权重共同证明了 modern 分支：`(1 - pow(1 - abs(dot(primaryWorldNormal, view)), 1 / anisotropy)) * baseWeight`；Meddle 的 ColorTable 布局将 texel 4/W 定义为 Anisotropy，legacy audit 没有对应 consumer。
- shaping 来源审计确认 modern 的 768/768 个相关 permutation 都采偶数 A 行的 Anisotropy，而不是 material-properties Roughness 或 A/B 平均；WGSL 改为读取 packed specular A alpha，native 非对称 fixture 固定 B anisotropy 不改变 shaping。
- A/B 来源审计按 DXBC 物理 lane 传播 Table 行标签，覆盖 packed/swizzled register 变体；character 的 720/720 与 legacy 的 276/276 个 ORB blend 都确认先采偶数 A 行、再采奇数 B 行，并将 `1-index.G`（modern 为 shaped 结果）作为 `mix(A, B, t)` 的插值因子。
- renderer 将 generic ColorTable A/B、Tile A/B 和 modern shaping 拆成独立 uniform 能力位。generic ramp 负责 base/specular/material/sheen/sphere/emissive 的 A/B 取样，Tile ramp 负责 TileProperties/TileMatrix；modern shaping 只有在 generic material-properties 与 `character.shpk` gate 同时成立时启用。
- packed Tile 路径从 TileProperties 的原始 G payload 读取权重，generic-only 路径从 ColorTable index texture 读取；这避免 tile-only fallback 因缺少独立 index binding 而退化成固定权重。renderer 单元测试与 native A/B fixture 分别锁定能力位和视觉混合。
- native WGPU shaping fixture 使用斜视角平面分别渲染 modern low/high roughness 与 legacy 对照，锁定 modern 权重差异及 legacy 不变性；完整 ignored native snapshot 集合为 40/40 通过。

## 2026-07-21 CharacterGlass DXBC 边界复核

- installed `characterglass.shpk` 共 38 个 pixel shader；实际采样统计为 Index/Normal/Table/TileNormal/TileOrb 各 34，Mask/ReflectionArray/SphereMap 各 24，Dissolve/Dissolve1 各 19，DepthWithWater/ViewPosition/Sky 各 8。
- 其中 31 个 shader 含 discard，32 个写 output alpha；但全部 shader 都没有绑定命名的 `g_GlassIOR` 或 `g_GlassThicknessMax` 参数。
- MeddleTools 的 package mapping 将 `characterglass.shpk` 复用 `character.shpk` 节点组，未提供 glass-specific final consumer。当前 renderer 因此保留基础 glass pass/alpha 与显式 unsupported 诊断，不猜测 ReflectionArray、SphereMap、Dissolve、DepthWithWater 的最终组合。

## 2026-07-20 g_TileMipBiasOffset DXBC LOD 对齐

- installed audit 现在直接从 SqPack 读取 SHPK 本体并通过系统 `D3DCompiler_47.dll` 现场反汇编，不再依赖含重复/派生文件的 `target` probe 目录。`character.shpk` 本体有 1038 个 pixel shader，其中 1024 个声明该字段、600 个实际消费；`characterlegacy.shpk` 有 1740 个，其中 1728 个声明并消费。
- 所有实际 consumer 都恰好执行两次 bias，直接锁定 A/B 两条 tile 路径：character 1200 次、legacy 3456 次，共 4656 次。每次均同号执行 `max(log2(min(length(TileMatrix.xz), length(TileMatrix.yw)) / 128), 0) + g_TileMipBiasOffset`，没有取反或其它缩放。
- consumer scope 只包含 Tile ORB，或 Tile ORB + Tile Normal：character 为 ORB-only 128 次、ORB+Normal 1072 次；legacy 为 ORB-only 1152 次、ORB+Normal 2304 次。detail arrays、primary maps 与 `g_TextureMipBias` 路径互不混用。renderer 以 `exp2(bias)` 缩放 pair-atlas 的显式梯度，保持 `fract` 前求导与 atlas half/layer 尺度不变。
- installed audit 锁定 offset 276、size 4、default 0；仅 character 有 3 个非零资源/4 次引用，值域 `-1/+1`。native WGPU fixture 在 TileMatrix scale 128 的 mip 边界同时验证 Tile Normal/ORB 随 `-1/+1` 改变。
- `tileMipBiasOffset` 不再作为 unsupported 输入。该阶段遗留的“先 bake 插值 TileMatrix 再单次采样”结论已被本节上方的 A/B 双采样实现取代；后续 DXBC 审计进一步确认有效 blend weight 以 `1 - index.G` 为基础，modern character 另有 roughness/view-dependent 整形，仍需单独对齐。

## 2026-07-20 g_TextureMipBias DXBC sampler scope 修正

- Meddle 只提供 `g_TextureMipBias` 的名称、character-family 适用范围和默认值 0；MeddleTools
  没有对应 mapping/node。installed audit 现直接从 SHPK 本体现场反汇编：`character.shpk`
  1038 个 pixel shader 中有 1032 个 consumer，`characterlegacy.shpk` 1740 个中有 1734 个，
  `characterglass.shpk` 38 个中有 34 个。
- 每个 consumer 都恰好一次同号计算 `g_TextureMipBias + g_PbrParameterCommon.m_MipBias`，
  没有取反或缩放；直接带 bias 的采样只覆盖主 Diffuse、Normal、Mask。Index/Table、
  ColorTable ramps、secondary maps、emissive、lightshaft、tile arrays 和环境采样没有该数据流。
- WGSL 保留 primary Base/Normal/Mask 及 dither Base/Normal 的 `textureSampleBias`，把 specular、
  material-properties、secondary、emissive 和 lightshaft 恢复为各自普通采样。source regression
  固定精确 sampler 边界，native fixture 固定 Base 随 bias 变化而 Specular/Emissive debug 不变。
- full installed MTRL audit 新增断言：字段 offset 228/size 4/default 0；只有 `character.shpk`
  有 6 个非默认资源/9 次引用，其中 `+1` 为 3/6、`-1` 为 3/3，其余三个 package 全为 0。

## 2026-07-20 ApplyVertexColor RGB composition 证据边界修正

- Meddle 将 `ApplyVertexColor` 定义为 BG/Crystal/LightShaft 等 family 的 material key；
  MeddleTools 只提供 Off/On 到布尔 socket 的 mapping，没有可验证的通用 RGB 组合公式。
- full installed weapon audit 仍只有 character/characterLegacy/characterGlass/skin，报告中没有
  `ApplyVertexColor` key coverage，也没有 BG/BGUvScroll candidate；因此旧的 generic
  `base * vertexColor.rgb` 没有真实武器覆盖，却会让未来输入静默采用未经证明的公式。
- prepared 新增 `vertexColorComposition` unsupported。key、`usesVertexColor`、uniform、color0/color1
  和直接 debug 保留；WGSL Final 删除 generic RGB tint，BG/BGUvScroll 的 vertex Alpha 与
  LightShaft 的 vertex Blue/Alpha 专用路径不变。
- native fixture 锁定强 RGB payload 与 ApplyVertexColor 开关不改变 Final，同时验证独立诊断色和
  VertexColor debug 差异。透明排序 fixture 改用同 mesh 的 2-texel base texture 提供红/蓝层，避免
  测试基础设施依赖该未验证公式。

## 2026-07-20 g_SpecularColorMask composition 证据边界修正

- Meddle 只记录 `g_SpecularColorMask` 的名称、适用 character family 和三通道默认值
  `[1,1,1]`；MeddleTools 没有对应 mapping、node 或最终 specular 组合公式。
- 当前 WGSL 曾把 RGB 乘到 material specular，并把 uniform 补出的第四通道再作为 scalar mask；
  两者都没有证据。installed `character.shpk`/`characterlegacy.shpk`/`characterglass.shpk`/
  `skin.shpk` 的全部 6399 个资源均为默认值，0 override。
- prepared 新增 `specularColorMaskComposition`。解析、summary、uniform 和 debug 保留，Final
  不再消费 RGB 或 Alpha；已验证的 legacy Compatibility `mask.r²` 路径仍独立保留。
- native metallic fixture 锁定强 override Final 不变并显示独立诊断色。

## 2026-07-20 Outline composition 证据边界修正

- Meddle 只记录 `g_OutlineColor=[0,0,0]`、`g_OutlineWidth=0` 的名称、默认值和适用
  character family；MeddleTools 没有 outline mapping、node、空间单位或外扩公式。
- 当前 renderer 曾把 width 直接解释为模型/世界空间 `position + normal * width`，再用 front-face
  culling 绘制纯色背壳。该 silhouette 技术本身常见，但不能据此冒充 FFXIV shader 语义。
- prepared 新增 `outlineComposition`。参数、`usesOutline`、uniform 和独立 pipeline 保留；正常
  prepared 出现非默认静态输入时不提交 outline draw，只显示 structured diagnostic。
- installed 四个 SHPK 的 6399 个资源均为 OutlineColor/Width 默认值、0 override。native fixture
  锁定强 width/color override 不改变 Final，并显示独立诊断色。

## 2026-07-20 Alpha aperture/offset 证据边界修正

- Meddle 只证明 `g_AlphaAperture=2`、`g_AlphaOffset=0` 的名称、默认值和适用 family；
  MeddleTools 没有对应节点、mapping 或 alpha shaping 公式。
- installed character audit 锁定非默认 aperture 为 7 个资源/10 次引用，非默认 offset 为
  3 个资源/4 次引用；这些真实输入此前会经过未经证明的 `pow` shaping。
- prepared 新增 `alphaShaping` unsupported。解析、summary 与 uniform 保留，WGSL 不再让
  aperture/offset 改变 Final；native fixture 锁定强 override Final 不变并显示独立诊断色。

## 2026-07-20 BG detail Final influence 证据边界修正

- MeddleTools 只证明 BG detail 的 layer selection、UV scale 和 `GetMultiValues` primary/multi
  sample mix；`node_configs.py` 明确注明 terrain detail influence 仍为 borked。
- `detailComposition` 已加入 prepared unsupported。BG/BgUvScroll 的 detail 输入或完整 detail
  array 仍可被采样和直接 debug，但不再通过固定 `0.22/0.14/0.32` 权重、程序化波形或 detail
  normal scale fallback 改变 Final base/normal。
- native fixture 现在要求 primary/multi detail debug 输出不同、Final 逐像素一致，并验证淡紫色
  `detailComposition` 诊断；直到取得游戏 shader 或非武器 BG 样本前，不校准最终 influence。

## 2026-07-20 Multi color 证据边界修正

- Meddle 只证明 `g_MultiDiffuseColor` / `g_MultiEmissiveColor` 属于 BG/Crystal family；
  MeddleTools 证明 `GetMultiValues` 驱动 BG primary/secondary A/B mix，但没有 generic
  character mask-R 公式。
- WGSL 已删除 `smoothstep(0.22, 1, mask.r) * 0.35` 的 generic MultiDiffuse 混色；
  `BgUvScroll + GetMultiValues + secondary base` 仍保留已验证的 vertex-alpha Map0/Map1
  diffuse mix。
- 非默认 MultiDiffuse 若不在上述 verified path，或任意非默认 MultiEmissive，prepared 输出
  `multiColorComposition` unsupported。native WGPU fixture 锁定 override 不改变 Final，但显示
  独立橙红色诊断。

## 2026-07-20 Emissive source 边界修正

- ColorTable emissive 已被烘焙为逐像素 sRGB texture；材质中的 `emissiveColor` 同时保留行摘要，
  只应作为贴图缺失时的 preview fallback，不能与同一 ColorTable texture 再次相加。
- WGSL 已删除 emissive 亮度 `smoothstep`、mask-B、vertex-alpha 和 MultiEmissive 的经验门控。
  Final 现在选择 emissive texture 或 material fallback，并直接叠加已映射的
  `g_EmissiveColor`；无证据的 `g_MultiEmissiveColor` 继续保留解析/uniform，不参与 Final。
- native WGPU fixture 锁定有纹理时不同 fallback 输出逐像素相同，以及无纹理时 fallback
  仍产生可见输出；installed audit 新增 ColorTable emissive 非零覆盖统计。

## 2026-07-20 Specular mask 单次消费修正

- installed `characterlegacy.shpk` SHPK/FXC 证据只支持 Compatibility Default=`1` 与
  Mask=`mask.r²`；其它 family/value-mode 没有第二次 mask-R 调制语义。
- WGSL 已删除普通路径的经验式 `mix(1, mask.r * 1.35, hasMask)`。无 ColorTable
  material-properties 时，mask R 继续通过 `SpecularStrength` 消费一次；legacy Compatibility
  Mask 仍保留已验证的额外 `mask.r²` factor。
- 同时修正了旧的 properties-texture bypass：普通 ColorTable 路径使用
  `SpecularStrength * mask.r`，legacy Compatibility Default 使用 `SpecularStrength`，legacy
  Compatibility Mask 使用 `SpecularStrength * mask.r²`；是否存在 ColorTable properties 不再
  抹掉 Default/Mask permutation。
- renderer source regression 锁定普通路径为中性 `1.0`，并禁止 `1.35` 放大或 generic
  mask-presence gate 返回。

## 2026-07-20 Toon 证据边界修正

- Meddle 证明五个 `g_Toon*` 常量名称、family 和默认值；MeddleTools 源码与 checked
  `shaders.blend` character group 没有 Toon node/socket/texture/mapping，不能证明任何 band 或
  reflection 公式。
- installed audit 锁定 character、characterlegacy、characterglass、skin 共 6399 个材质资源：
  五个 Toon 常量全部只有 SHPK 默认值，MTRL non-default override 均为 0。
- WGSL 已删除黄金比例 index phase、35% diffuse band、40% spec band 与 reflection scale 等经验
  公式。通用 PBR 直接消费一次 NdotL，direct GGX 使用固定 preview specular scale。常量与
  `usesToon` 元数据继续保留；未来非默认输入通过 `toonLighting` 诊断，不改变 Final。
- native WGPU fixture 锁定默认/强 override Final 像素一致，并验证 override 的独立诊断色。
  已移除不再具有视觉验证意义的 `XIV_PHANTOM_TOON_OVERRIDE` 入口。

## 2026-07-19 Sheen/Sphere 证据边界修正

- Meddle 与 MeddleTools 证明了 SheenRate/Tint/Aptitude、SphereIndex/Mask 的字段布局、ramp
  bake 与节点接口，但 checked character 材质树只把两组 ramp 接到没有下游消费者的
  mix-group interface，不能证明 Final BSDF 组合。
- WGSL 已删除固定 `24..160` power、`0.42` sheen strength 和 `0.18` sphere rim 等经验公式。
  ramp float payload、GPU binding、直接 debug view 和 shader 常量仍保留；非零输入通过
  `sheenLighting` / `sphereLighting` 进入独立 unsupported 诊断。
- installed audit 锁定 6394 个 ColorTable 资源：非零 SheenRate 为 857 个资源/1335 次引用，
  非零 SphereMask 为 121 个资源/183 次引用。native WGPU fixture 同时锁定 active/neutral
  Final 像素一致和两个诊断色可区分，避免以后重新引入无证据贡献。

> 本文是截至 2026-07-14 本轮实现的完整调查与推进记录，不再作为当前待办的权威来源。
> 当前状态、剩余工作和每轮更新要求统一维护在
> [`weapon-render-review-plan.md`](weapon-render-review-plan.md)。

本文整理当前 `xiv-companion` 武器模型预览与本地参考仓库 `Meddle`、`MeddleTools` 的对比结果，并按三层规划后续工作：

1. 数据解析
2. 解析后的结果处理
3. 渲染器与着色器管线

目标不是一次性复刻完整 FFXIV shader，而是把现有“可辨识预览”推进到“语义清楚、问题可定位、关键材质效果稳定”的状态。

## 核验依据

本轮核验时间：2026-07-14。

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

## 2026-07-14 特殊 character family 证据闭环

- installed WeaponCatalog 的 family coverage 只有 8091 character、6 characterGlass、15 skin；characterReflection、characterOcclusion、characterScroll、characterStockings、characterTattoo 均为零，无法从武器真实快照校准专用公式。
- 15 个 skin 引用全部指向同一 `chara/human/c0101/obj/body/b0001/material/v0001/mt_c0101b0001_a.mtrl`，用于 equipment-style 拳套 fallback。该资源固定 `GetMaterialValueBody`、`GetDecalColorOff`，只绑定 `g_SamplerDiffuse/Normal/Mask`；非默认 diffuse/tile 常量已由现有通用参数与 tile 管线消费，没有额外静态 Skin* 或 decal 输入。
- Meddle `MaterialComposer`/`ParseMaterialUtil` 从角色 customize/runtime buffer 取得 SkinColor、OptionColor、DecalColor；`OnRenderMaterialUtil` 解析 decal texture/color，并仅在 characterStockings on-render 路径从 slot skin material 复制 Skin* textures。MeddleTools 的 skin Body mapping 仍标为 TODO，tattoo 只映射 runtime OptionColor，stockings/scroll 复用 character group，reflection 没有节点模板。
- 因此没有新增 WGSL 近似。现有 Face clamp、stockings opaque、tattoo NormalAlpha、scroll raw variant，以及 runtime Skin/Option/Decal/SubColor/skin-material 与 reflection unsupported 均保持；installed audit 新增全量边界断言，family 或唯一 Skin 语义变化时会要求重新审计。

## 2026-07-14 Glass surface 证据收紧

- installed coverage 有 5 个唯一 `characterglass.shpk` MTRL/12 次引用，全部 `DrawDepthMode_Dither`，只使用 Normal/Mask/Index 与 ColorTable；`g_GlassIOR` 为 1/1.5，`g_GlassThicknessMax` 为 0/0.01。
- FXC 检查 installed SHPK 的全部 38 个 pixel shader，均未绑定 `g_GlassIOR` 或 `g_GlassThicknessMax`。MeddleTools 只将 characterglass 映射到通用 character group；该 group 的 IOR 是 mask roughness 的 Blender 近似，没有 glass tint、Thickness 或 Transmission 专用节点。
- WGSL 删除基于两个常量的蓝色 tint/specular/rim 公式和专用 glass lighting，glass 现复用通用 character surface，仅由 NormalBlue alpha、Dither depth 与独立透明 pass 区分。原始常量仍保留在 model/summary，prepared 新增 `glassShaderParameters` unsupported；installed audit 固定 38 个 PS 的资源边界。
- 原 `ModelGlassBlendMode::Multiply` 实际一直使用 WebGPU `ALPHA_BLENDING`，并非 multiply。现更名为 `Alpha`，Web 显示同步修正；旧 `multiply`/`mul` 输入兼容映射到 Alpha，Additive 继续作为显式预览近似。真实 blend equation、折射、scene-color transmission 与厚度用途仍等待运行时证据。
- 45059 已重跑 Alpha final/alpha debug 与 Additive final。Alpha/Add RGB difference 为 11,936,622、变化像素 56,333；Alpha final SHA-256 为 `B28F8DFE2F48F986370CEDF1FC1001CA1F49164A9DC1E3D9B70EAEB68C164A09`，alpha debug 为 `4F6F8B6F696D55DAF1102E77476D1F6722E6CFB8572C712FBC38A5D0D5665336`，Additive final 为 `95BB336997932B683895737EB9C0583767F35A4C7C8C66B3F3897C599D24AC01`。

## 2026-07-14 Cutout evidence boundary

- installed 武器只有 character/characterLegacy/characterGlass/skin 四个 exact SHPK；全量 material-key coverage 没有 `ApplyAlphaTest`，当前 24 组 phantom summary 也没有 Mask/Cutout 材质。
- Meddle 将 `ApplyAlphaTest` key 限定为 bg/bg variants/bguvscroll/crystal，只有 On value 额外列出 lightshaft。上述 family 在武器目录中没有 surface cutout 样本，无法校准额外 family 公式。
- character 系列另有 scene-level `ApplyAlphaClip`。known CRC label 已补齐，full audit 断言四个 installed SHPK 均为 Off、0 non-default override，并继续断言不存在 `ApplyAlphaTest` coverage。
- 因此保留现有独立 Cutout pipeline、depth write、prepared alpha source 与 `g_AlphaThreshold` discard，不新增无真实样本的行为；后续只有出现 bg/crystal 等非武器 fixture 时重新打开。

## 2026-07-14 Transparency/OIT/shadow evidence boundary

- WeaponCatalog audit 新增 LOD0 mesh-range model/mesh 双计数。7365 个唯一 installed 武器模型只有 normal category，共 8114 个 mesh；shadow 与 terrainShadow 均为零。24 组 phantom 的 47 个 mesh 同样全部为 normal。
- 四个实际 SHPK 的 `g_ShadowAlphaThreshold` 全部固定默认 0.5；`g_ShadowPosOffset` 仅 character 3 个、characterglass 1 个、skin 1 个非零资源。Meddle 只证明字段与通用 MDL range 的存在，没有离线 light matrix、bias、shadow sampling 或 alpha 公式。
- snapshot harness 会在渲染前检查真实透明 geometry：45050 为 848 个透明三角形，45059 为 320 个，均无非相邻三角形 edge 穿过另一三角形内部的 proper intersection。两者不存在必须靠 OIT 解除的循环遮挡，逐三角形 back-to-front 仍是更精确的 alpha composition。
- 因此不增加 weighted blended OIT 的权重近似，也不以 normal mesh 自创 shadow map。全量 audit 固定唯一 normal range 计数，phantom regression 固定两组 transparency geometry；真实覆盖或相交样本变化时测试会要求重新评估。

## 2026-07-15 Water/environment evidence boundary

- installed WeaponCatalog 的 family 仍只有 character、characterGlass、skin；7365 个模型的 LOD0 range 也只有 normal。sampler 与 material constant coverage 均没有 water、river、crystal 或 Environment 记录，因此武器目录没有可校准 refraction、whitecap、secondary wave 或 environment mapping 的真实样本。
- Blender headless 复核 MeddleTools `meddle water.shpk` 节点图：仅连接 `g_WaterDeepColor -> Base Color`、`g_SamplerWaveMap -> Normal`、`g_Transparency -> Alpha`。`g_RefractionColor`、`g_SamplerWaveMap1`、`g_SamplerWhitecapMap` 均未连接，当前节点接口也没有 `g_WhitecapColor` socket。
- `meddle crystal.shpk` 只连接 ColorMap0 与 NormalMap0，`g_SamplerEnvMap` 输入未连接。Meddle `Names.cs` 提供 water/crystal 参数名称、CRC、默认值与 package 范围，但没有最终坐标或混合公式。
- renderer 保持现有三条 water 可信连接；refraction/whitecap/WaveMap1 与 Environment 继续作为 structured raw/unsupported 输入，不新增 WGSL 近似。全量 audit 新增专用边界断言，family、water mesh range、相关 sampler 或 package constant 一旦出现就要求重新审计。

## 2026-07-15 Runtime material input ownership

- 当前应用只通过 local/Web `WeaponModelResource` JSON 取得 item/model/stain，renderer 的 `PreparedModelOptions` 只接受 attribute/shape mask；没有 live character pointer、on-render material output、GPU texture 或 resolved handle provider。
- Meddle 的 `ParseMaterialUtil` 从 `CharacterBase.ColorTableTexturesSpan` 导出 runtime R16G16B16A16F ColorTable；`MaterialComposer` 从 customize parameter 取得 SkinColor、OptionColor、DecalColor/SubColor；`OnRenderMaterialUtil` 从 Weapon/Human/Demihuman 与 `CharacterUtility` 取得 decal/crest texture/color、slot skin material textures 和透明 fallback。上述输入均依赖游戏进程实例状态，静态 SqPack 无法重建。
- `PreparedMaterial.runtimeInputRequirements` 现结构化报告四类 provider requirement：character instance state、on-render material output、GPU ColorTable texture、resolved resource handles。它由已有 exact unsupported 细项推导，不接受伪造的默认值，也不改变 transparent/base fallback。
- focused tests 固定 tattoo、stockings、Skin、Skin decal、crest 与 runtime ColorTable 的 ownership 组合。installed 武器没有 crest/materialChange range 或 tattoo/stockings/occlusion family，唯一 Skin fallback 为 Body + DecalOff；因此当前不新增无人能填充的 GPU binding、第 16 个 sampler 或 decal Clip/Extend 近似。

## 2026-07-15 Runtime geometry requirements

- Meddle 从 live `ModelResourceHandle.Shapes/Attributes` 取得 name/id 表，并读取 `EnabledShapeKeyIndexMask` / `EnabledAttributeIndexMask`；pose/attach 来自 `CharacterBase.Skeleton`，equipment race deformation 来自 GenderRace 与 PBD。静态 MDL 只有 morph、submesh mask、bone table 和 vertex weights，不能恢复这些实例状态。
- `PreparedModel.runtimeGeometryRequirements` 现报告 `shapeNameIdMapping`、缺失的 enabled shape/attribute mask、`skeletonPose`、`skinningMatrices` 与 `raceDeformer`。显式 `PreparedModelOptions` mask 会清除对应 mask requirement，但 shape name/id mapping 仍保持，因为当前 Web table-order bit 不是 live id。
- requirement 从 mesh 静态 payload 推导：bone table 与 blend weights/indices 同时存在才视为 skinning；`chara/equipment/` skinned mesh 额外要求 race deformer。没有 live skeleton/PBD provider 时不上传 identity matrices，也不把 c0101 equipment fallback 冒充实际种族 pose。
- focused tests 覆盖 attribute/shape option fulfillment、shape mapping 保留以及 equipment skinning/race requirement。full audit 的 33 个 shape 模型/34 个静态 shape 继续作为真实覆盖边界；现有 42697 显式离线 shape 快照行为不变。

## 2026-07-15 Render verification entrypoint

- GitHub hosted runner 无法取得 FFXIV SqPack，仓库也没有已配置的 game-data self-hosted runner；因此没有添加会永久排队或跳过真实输入的假 workflow。phantom harness 已支持 `XIV_PHANTOM_CASES`，fixture 当前动态选出 7 个 P0/P1 case。
- 新增 `scripts/verify-weapon-render.ps1`：校验 `XIV_GAME_DIR`/`-GameDir`，从 fixture 动态生成 P0/P1 filter，依次运行 full installed shader audit、P0/P1 phantom、workspace all-features、wasm32、fmt 与 diff。Cargo 使用 `--jobs 1`，避免 Windows 在连续 WGPU/Web 链接时触发 `LNK1102`/pagefile 峰值，不缩减测试范围。
- 入口已端到端通过。full audit 保持 7365 models、8112 material references、6399 MTRL、4 SHPK、0 failures；7 个 phantom 全部完成，45050/45059 proper intersections 仍为 0，45052 baseline/stain0/stain1 metallic RGB differences 为 203495 / 2773725 / 2955242。
- 当前每个已实现 family 行为已有 focused 或 synthetic native fixture；installed audit 明确证明 Map1、Flow、water、reflection、occlusion、bg/bguvscroll 真实武器样本为零。sampler-policy debug 没有实际定位需求，不新增常驻 UI；新资源或 package 出现时由审计边界触发下一轮。

## 2026-07-14 MultiMaterial 证据闭环

- WeaponCatalog material-key coverage 显示 character/characterlegacy 的实际 `GetValues` 只有 MultiMaterial 与 Compatibility；没有 AlphaMulti/2/3。sampler coverage 只有 BaseColor/Normal/Mask/Index，没有 `g_SamplerMulti`，family coverage 也没有 bg/bguvscroll。
- 对 installed `character.shpk` 按除 `GetValues` 外完全相同的 system/scene/material/subview keys 成对比较 node passes。代表 surface pair 的 MultiMaterial PS 不声明 `g_SamplerDiffuse`，Compatibility PS 在相同 Normal/Mask/Index/Table/tile/sphere/reflection/occlusion 资源管线上新增 Diffuse texture 并执行采样。
- 该结果与 MeddleTools character group 的 Compatibility multiply gate 一致，也证明当前 loader 的 MultiMaterial=`ColorTable diffuse`、Compatibility=`base × ColorTable` 不需要继续修改。`GetValuesMultiMaterial` 的 vertex alpha 仍不能作为 opacity。
- AlphaMulti 接口未连接、MultiMap 无 socket/config、detail influence 明确标为 borked；结合武器零覆盖，这三项移入证据不足区，保留 mode/raw、binding 与独立 unsupported，避免无样本调参。

## 2026-07-14 WGSL surface composition 重构

- MeddleTools 的 legacy/stockings/glass/scroll/transparency 都复用 character surface group，skin/water/bg 也最终输出共同的 base/normal/material/alpha/emission channels；因此没有为每个 package 复制 fragment shader。
- 原 `fs_main` 约 212 行，同时编排 texture sampling、normal/tile/detail、material/specular、family base/alpha、opaque/glass lighting、discard 和 bloom。现拆为 `SurfacePassFlags`、`SurfaceSamples`、`SurfaceState` 与 `resolve_surface_output`，入口缩减到 55 行，只保留阶段调度、debug、discard 和 lightshaft 分派。
- uniform/binding ABI、公式、浮点常量、采样策略与 pass 分派均未改变。结构 focused test 会拒绝在 `fs_main` 重新内联 texture sampling、glass lighting 或 bloom 公式。
- workspace 与 wasm32 通过；完整 native WGPU 19/19 通过。45047/45048/45050/45053/45068 的 final SHA-256 均与重构前精确一致。

## 2026-07-14 sampler/UV 审计

- MeddleTools `shaders.blend` 的实际 vector links 证明主 Diffuse/Normal/Mask/Index 使用 UV0，character SkinDiffuse/SkinNormal/SkinMask 使用 UV2，Decal 使用 UV1；reflection 没有可采信节点。
- MTRL sampler 解析新增 exact logical role，Skin* 即使与主纹理共享 BaseColor/Normal/Mask 的底层数据语义，也会进入独立材质槽、prepared binding 与 UV2 source，不再被 first-match 合并。renderer 暂不增加 texture binding，并以 `skinSamplerComposition` unsupported 标记缺少的组合公式。
- SHPK semantic debug 现输出 sampler resource name/CRC/slot/size/logical role；WeaponCatalog audit 按 exact SHPK、resource、logical role、flags 统计唯一 MTRL、catalog 引用和代表样本。
- installed 全量结果仍为 7365 个模型、8112 个材质引用、6399 个唯一 MTRL、4 个 SHPK、0 failures；新增 sampler coverage 共 53 行，0 unknown role、0 unresolved name。武器范围只出现 BaseColor/Normal/Mask/Index 四类主 sampler，没有任何 Skin* MTRL 使用记录。
- focused tests 固定 Skin* 与主槽不会合并、prepared UV2/unsupported 传播、SHPK 精确资源名优先级和 sampler coverage 去重。45047/45048/45050/45053/45068 final 快照逐字节保持原基线。

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
- renderer GPU 顶点格式已上传 `uv1-uv3`、`color1`、secondary normal/bitangent、`flow0/flow1`；WGSL 已传递 UV、flow 与 secondary normal/bitangent，并按 prepared UV source + per-role scroll mask 选择采样 UV。Flow 模式会消费 `flow0` primary tangent，bguvscroll Map1 normal 会消费 secondary frame；`color1` 和 `flow1` 尚未参与最终材质公式。
- `PreparedModel` / `PreparedMesh` 已有第一版，按 mesh 输出 draw role、是否进入主 pass 和 prepared material；renderer 与 phantom `model-summary.json` 现在共用这一准备结果。
- `PreparedMaterial` / `PreparedRenderPass` 已提升到数据层；phantom `model-summary.json` 的主 surface mesh 会输出 prepared material 决策，包含 `Opaque`、`Cutout`、`Transparent`、`Glass`、`AdditiveLightShaft` 与 culling policy；lightshaft 不进入普通 surface pass，但 renderer 会保留为 additive batch。
- `MaterialShaderFamily` 已结构化常见 `.shpk`：character、skin、characterStockings、characterGlass、characterReflection、characterTransparency、characterScroll、characterTattoo、characterOcclusion、bg、bgUvScroll、lightShaft、water、unknown，并进入 `PreparedMaterial`；lightshaft、bguvscroll 已有第一版行为，skin 会显式报告 runtime SkinColor/完整节点缺口，其它特殊 family 也仍有不同程度的节点缺口。
- `PreparedTextureBindings` 已聚合现有材质贴图索引：base、normal、mask、material、multi、specular、emissive、material-properties、tile、sheen、sphere、tile-matrix、ColorTable index，以及 character tile normal/ORB 和 bg detail diffuse/normal 四个共享数组 atlas，并随 prepared material 输出。
- `PreparedTextureSamplingSet` 已表达并执行 texture role 采样策略：base/emissive 为 sRGB + linear，normal/mask/specular/material/multi/material-properties 为 Non-Color + linear，index、ColorTable extra maps 和共享 tile/detail arrays 为 Non-Color + nearest；renderer 的 15 个现有 texture binding 均有独立 sampler binding，并从对应 role policy 派生 descriptor。
- `ModelColorDyeTable` 已把 Legacy/Dawntrail 的 template、channel 和各可染通道 flag 从 debug 提升为 `ModelMaterial.colorDyeTable` 的可序列化结构化数据；保留 `hasColorDyeTable` 兼容旧数据，prepared `usesDye` 会识别任一入口，请求级 stain IDs 已接入实际 model load。
- 数据层已实现 Legacy `chara/base_material/stainingtemplate.stm` 与 Dawntrail `chara/base_material/stainingtemplate_gud.stm` 的通用 parser，覆盖 v1.1/v2.0/v2.1、u16/u32 keys、singleton/direct/indexed column 编码、1-based stain ID lookup，以及 Dawntrail template ID 减 1000 后回退 Legacy STM；同时已有按 Legacy/Dawntrail dye flags 覆盖 renderer-friendly ColorTable rows 的纯函数与诊断报告。
- `WeaponModelLoadRequest.stainIds` 已作为请求级 `[stain0, stain1]` 输入进入同步/异步 SqPack 加载；请求仅在存在非零 stain 时各加载一次 Legacy/GUD STM，材质会在 summary 和 ColorTable bake 前应用染色。`WeaponModelData.stainIds` 与 `ModelMaterial.stainingApplication` 会保留输入、模板路径、行统计和错误，phantom summary 可直接审计；资源 key 也包含 stain IDs，避免不同染色请求冲突。
- `WeaponCatalogPackage.stains` 已从 `Stain` EXD 导出 ID、中文名称、原始 BGR 色值、UI RGBA、shade、sub-order 和 metallic；当前本地客户端有 125 个具名染剂。Web 武器预览已提供 stain0/stain1 选择器、色块、金属标记和 URL 状态，并使用请求级 stain IDs 重新加载模型。EXD 色值仅用于 UI，不参与实际 ColorTable 覆盖。
- `CategoryFlowMapType` / `0x40D1481E` 已按 Meddle 的 `Standard=0x337C6BC4`、`Flow=0x71ADA939` 结构化为 `MaterialFlowMode::{Standard,Flow,Unknown}`，同步/异步 material composition、known shader label、phantom summary 均会保留。`PreparedMaterialFeatureFlags.usesFlow` 现在只在材质选择 Flow 且 mesh 存在 primary `flow0` 时启用；仅有 `flow1` 或 Standard/Unknown 模式不会误启用。Meddle `VertexUsage.Flow => TANGENT0` 是当前将 flow 解释为 tangent 而非 UV 动画的依据。
- `PreparedMaterialUnsupportedInputs` 已按当前可可靠判断的数据标出 dye application、runtime ColorTable、decal/crest、runtime material change、tile array、detail array、secondary map blend、incomplete shader family logic，并进一步拆出 `runtimeOptionColor`、`runtimeDecalColor`、`runtimeDecalTexture` 与 `runtimeSkinMaterial`，phantom summary 会随 prepared material 输出这些缺口。MeddleTools `charactertattoo` 节点证明 tattoo 颜色依赖 OptionColor/DecalColor，skin decal 节点同时依赖 DecalColor 与 DecalTexture；Meddle composer/parse util 证明颜色来自 customize/decal constant buffer，Meddle on-render util 证明 decal texture 与 stockings skin material texture 都是运行时输入。当前不为这些缺失运行时输入伪造颜色或贴图。
- 数据层已从 SqPack 加载 `tile_norm_array.tex`、`tile_orb_array.tex`、`detail_d_array.tex`、`detail_n_array.tex`。由于 Physis 当前把 TEX header 的 `MipLevels:u8 + ArraySize:u8` 当成 `u16` 且普通 `to_rgba()` 只按 depth 解码，本仓会读取 header byte 15，把 mip0 slices 解码为与 MeddleTools 导出一致的 vertical atlas，并保留 `arraySize` 与 `arrayLayerHeight`。真实客户端验证结果为 character 两张 `64x4096 / 64 layers`，bg detail 两张 `256x8192 / 32 layers`。
- `ModelMaterial.textureArrays`、`PreparedTextureBindings` 和 `PreparedMaterial.resourceAvailability` 已表达共享数组索引、加载错误和成对完整性；phantom texture summary 会输出 atlas 总尺寸、层数和单层高度。`PreparedTextureArrayStatus` 进一步区分 `MissingBindings/Unvalidated/MissingTexture/WrongTextureKind/NonCanonicalBinding/InvalidLayout/IncompatiblePair/Ready`，ready 时保留 layer count。standalone material preparation 因无 texture 集合只到 Unvalidated；`prepare_model_for_render` 会验证实际索引、kind、canonical shared binding、atlas 高度/RGBA 长度和 pair 尺寸/层数，并据此更新 unsupported。renderer array uniform 只消费 prepared ready/layer count，不再重复 material-level 判断；45052 summary 已验证 tile array 为 Ready/64 layers，detail 为 MissingBindings。
- tile/detail pair atlas 已补齐缩小采样链：每个 half 与 vertical-array layer 独立生成 mip，不跨 normal/ORB、diffuse/normal 或相邻 layer 平均；normal half 只从 RG 重建/平均/归一化法线，B/A 仍按独立线性 payload 保留。WGSL 在 `fract` 层内 wrap 前求导，并用按 half/layer 缩放的显式梯度选择 nearest mip，避免高频 UV 先混叠后产生三角形相关的错误 LOD。CPU focused test 固定各 half/layer 与 B/A 结果；synthetic native WGPU checker 与预平均 atlas 最终 PNG 完全相同。45047、45048、45053、45068 已重跑 final/tile-normal，45050 已重跑 normal-B alpha；其中 45047/45053 的 tile-normal 高频指标较旧基线下降约 98%/99%。
- TileIndex 离散化已从错误的 `round` 改为 MeddleTools `tile_select` 节点明确使用的 `FLOOR`，逐像素 ColorTable ramp 与 `g_TileIndex` fallback 共用同一 WGSL helper。45053/45068 的主要 TileProperties R=86 解码为约 21.58；旧实现误选平均切向幅度约 0.596 的强网纹 layer 22，新实现选择约 0.031 的 layer 21。synthetic native WGPU fixture 分别固定 ramp/fallback 的 fractional 与 exact-layer 输出逐像素相同；真实 45068 final/tile-normal 前景高频下降约 53%/94%，45053 tile-normal 下降约 71%。
- character normal-channel alpha 最初依据 MeddleTools 图删除了 vertex-A 乘法，恢复了 45050 被错误抹除的毛发；后续 installed DXBC 审计已把该行为收紧为 `0xAD94E254` 控制的 remap。45050 的常量值为 `1`，所以结果仍正确地忽略原 vertex A；默认值 `0` 的 character/legacy/glass 则会保留 vertex A。tattoo 等未被这三个 package DXBC 证明的 family 继续按各自节点边界处理。
- Transparent/Glass 已从 mesh-center 排序升级为逐三角形动态索引排序：flatten 阶段保留每个透明三角形的全局索引和中心，renderer 每帧按当前相机方向全局 back-to-front 排序、上传独立 `INDEX | COPY_DST` buffer，并仅合并相邻同 batch draw run；静态索引仍用于 opaque、dither depth、outline 和 additive。45050 的单个毛发 batch 含 848 个三角形和 25 个断开组件，原始 MDL 顺序在代表视角有约 41%-53% 深度逆序，仅按组件中心仍不足；CPU tests 覆盖相机反转和跨材质 draw run，synthetic WGPU 的同 mesh 红/蓝透明层会随正背视角正确翻转主色，真实 45050 已在默认及两个额外视角回归。
- character ColorTable diffuse 组合已按 Compatibility gate 收紧：MeddleTools `meddle character.shpk` 以当前 `GetValues=GetValuesCompatibility` 或旧式 `GetValuesTextureType=Compatibility` 驱动同一布尔 socket，作为 ColorTable diffuse 与 `g_SamplerDiffuse` 的 MULTIPLY Factor，默认 MultiMaterial 不启用。同步/异步 loader 现共用 family/key policy；Character/Stockings/Glass/Transparency/Scroll 的 Compatibility 保留线性空间 `base × ColorTable` 和 base alpha，MultiMaterial 选择 baked ColorTable diffuse，同时保留原 diffuse 的 raw texture index。Skin/Reflection/Tattoo/Occlusion/Bg 等无相同节点证据的 family 不改变。focused tests 覆盖新旧 key、policy 和 Replace/Multiply 纹理选择；installed 45068 回归固定 Compatibility、`#base-times-colorset` 和原 base index；父提交与当前实现的 45047/45048/45050/45053/45068 PNG 全部逐字节一致。
- WeaponCatalog material semantic audit 已从 family 计数扩展为 exact-SHPK coverage：SHPK debug API 分开 material/system/scene key，并保留全部 material parameter 声明、byte offset/size 与可选 defaults；无 defaults 和零宽参数不再被误作 package 外 override，PS3 资源按大端解析。audit 对唯一 MTRL 资源和 catalog item-material 引用分别计数，MTRL key 会按实际 resolver 语义覆盖任一 scope default；unknown override 不继承 default label。duplicate constant ID 先聚合为最后一个可解析 override，malformed/non-finite/raw byte size 与 unresolved effective value 独立统计，不会重复放大资源数。synthetic tests 覆盖共享资源多物品引用、scene override、unknown value、duplicate valid+malformed、无 defaults、零宽和大端 SHPK。
- 2026-07-14 全量 semantic audit 扫描 8281 个 catalog 条目、7365 个唯一模型、8112 个 model-material 引用和 6399 个唯一 MTRL，得到 4 个 exact SHPK、48 个 scoped key coverage、244 个 constant coverage，0 load/semantic failures、0 malformed/non-finite/unresolved values。资源/catalog 引用双计数差异清晰：`character.shpk` 874/1359、`characterlegacy.shpk` 5519/12145、`characterglass.shpk` 5/12、`skin.shpk` 1/35。`0xAD94E254` 在 character 仅 3 个非默认资源/4 次引用；`0xF52CCF05` 保持未知 raw key，character 的 `0xA7D2FF60` 为 856/1338、`0xDFE74BAC` 为 18/21，未进入 WGSL。
- 第一轮审计提升了 `MaterialSpecularType::{Default,Mask,Unknown}`、`g_TileMipBiasOffset`、`g_VertexMovementScale` 与 `g_VertexMovementMaxLength`。`ApplyVertexMovement` 对全部 6399 个资源均为 scene default Off、0 MTRL override，因此非默认 movement constants 只报告 `vertexMovementParameters`，不驱动顶点动画；tile mip bias 仅 47320=`+1`、46461/46462=`-1`，保留 `tileMipBiasOffset` unsupported。`CategorySpecularType=Mask` 只有 4 个 legacy 资源/5 次引用；installed SHPK/FXC 证明 MultiMaterial 的 Default/Mask pass 相同，而 Compatibility Mask permutation 额外采样 mask，`mask.r²` 调制 specular，mask G gloss 路径又被全武器为 0 的 `0x15B70E35` 关闭。renderer 因而只对 `characterlegacy.shpk + Compatibility` 编码 Default=`1`、Mask=`2`：Default specular factor 为 1，Mask 为 `mask.r²`，并在无 ColorTable material-properties 时不再把 mask G/B 误作 roughness/metalness。30520 installed structured test 与原生 Mask/Default snapshot 通过，RGB difference=24048；45047/45048/45050/45053/45068 phantom 子集重跑通过。
- Meddle `OnRenderMaterialUtil` 证明 weapon decal/FC crest 属于运行时 on-render 输入，不是静态 MTRL sampler。`PreparedMaterial.runtimeFallbacks` 已明确缺失 decal/crest 时使用透明纹理语义，materialChange 使用基础材质语义；renderer final 模式会 discard crest fallback，mesh-role debug 仍可见，materialChange 继续使用基础材质。
- `bguvscroll.shpk` 已单独分类为 `MaterialShaderFamily::BgUvScroll`；primary Color/Normal/Specular Map0 使用 UV0Scroll，secondary Map1 使用 UV1Scroll。三种 Map1 已有独立 texture kind、model/prepared binding、per-role source/scroll mask 和 Web diagnostics；WGSL 在 `GetMultiValues` 下按 vertex alpha 统一混合 color、color alpha、normal 与 specular。`characterscroll.shpk` 不会误继承该动画。
- `ModelMesh` / `PreparedMesh` 已保留 mesh-level shape influence 摘要；`ModelShapeTarget` 进一步保留 sparse output vertex 的 position/normal delta。Meddle `MeshBuilder.BuildShapes` 已确认 `ShapeValue.BaseIndicesIndex` 是 mesh 内的 index-buffer position、`ReplacingVertexIndex` 是 mesh vertex index；submesh remap 会按索引出现位置和 target signature 拆分共享顶点，不会把前者误当 vertex index 或连带替换 UV/color/skinning。`PreparedModelOptions.enabledShapeMask` 继续负责 active/inactive 审计；`model_mesh_vertices_with_shape_mask` 与 `ModelRenderer::new_with_prepared_options` 会对显式 active target 累加 base-relative delta，默认入口仍使用 base geometry，shape mask 不参与 draw visibility。
- renderer 已绑定 ColorTable extra maps：tile、sheen、sphere、tile-matrix 进入 WGSL，并提供独立 debug view 检查这些烘焙 ramp。tile-matrix 使用 `Rgba32Float` unfilterable texture + non-filtering nearest sampler，直接消费 `ModelTexture.rgbaF32` 的 UU/UV/VU/VV；payload 非法时逐通道回退 RGBA8/identity。Blender `tile_select` 证明矩阵只形成 tile UV vector，因此 WGSL 只保留 UV 变换。sheen 与 sphere ramp 的 synthetic fixture 现证明 binding/debug/unsupported 链路生效，同时锁定它们不改变 Final。45059 两行 raw `SphereIndex=0x4000` 已确认是 half `2.0`；`ColorTableRowColors` 会先 half 解码再按 `/255` bake，low-level debug 继续保留 raw `u16`。
- ColorTable extra ramp 的 HDR 保真已补齐到 sheen/sphere：`BakedColorTableMaps` 和 `ModelTexture.rgbaF32` 会保留 `SheenProperties` / `SphereProperties` 的未 clamp float payload，renderer 在 ColorTable 模式上传基线可过滤的 `Rgba16Float`，RGBA8 只作为兼容/debug 与非法 payload 的逐通道 fallback。MeddleTools `PackedColorTableRampLookup` 直接把 ColorTable float 写入 Blender ramp、不做 UNORM clamp，是保留 HDR 的依据。45059 真实材质的 `SheenAperture=4.0` 现可从 parse/bake 一直保留到 GPU 输入；同两个 binding 在 bguvscroll 模式仍复用 RGBA8 secondary normal/specular，因此继续保持线性采样兼容。
- `g_NormalScale` 已从 composed material constants 提升为 `ModelMaterial.normalScale`，支持 shader package default 与 material override；renderer 会用它缩放 tangent-space normal map 强度。
- `g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale` 已结构化进 `ModelMaterial` 和 renderer `shaderParams`；共享 detail normal atlas 可用时 WGSL 会按 detail/multi-detail ID 与各自 UV scale 采样并组合 tangent-space normal，缺图时才回到 primary normal 的受限 fallback。
- `g_TileIndex`、`g_TileAlpha`、`g_TileScale` 已结构化进 `ModelMaterial` 和 renderer `tileParams`；WGSL 优先用逐像素 ColorTable `TileProperties.r * 64` 选 tile layer，没有该贴图时回退 `g_TileIndex`，两条路径均 floor 后再结合 TileMatrix/TileScale 采样 tile normal/ORB。Blender 节点检查确认 `chara_detail_blend` 只把 ORB Blue 作为黑色到 base color 的直接 darkening factor，R/G 与 Orb Alpha 均未连接；normal detail 权重为 tile-normal Alpha × TileAlpha。WGSL 已按该公式修正 color/normal，删除无证据的 R=AO/G=roughness/B=specular property 映射和程序化 tile specular wave；ORB debug 仍保留原始 RGB。
- `g_ToonIndex`、`g_ToonLightScale`、`g_ToonLightSpecAperture`、`g_ToonReflectionScale`、`g_ToonSpecIndex`、`g_SheenRate`、`g_SheenTintRate`、`g_SheenAperture`、`g_SphereMapIndex` 已按 Meddle `Names.cs` CRC/default 结构化进 `ModelMaterial`、phantom summary 与 renderer uniform。`usesToon` 对 character family 保留为能力元数据；Toon 与 Sheen/Sphere 都因缺少最终节点证据而不改变 Final，非默认/非零输入进入各自 unsupported 诊断。
- `g_DetailID`、`g_MultiDetailID`、`g_DetailColor`、`g_MultiDetailColor`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 已结构化进 `ModelMaterial` 和 renderer detail uniforms；WGSL 会按两个 detail ID 选层，用 color/normal 各自 UV scale 采样 detail pair atlas，并以 0.5 为 diffuse 中性值做保守 tint/normal 组合。Blender 节点检查确认 `bg_detail_blend.MultiBlendWeight` 同时驱动 primary/multi diffuse 与 normal 的 A/B mix；renderer 现已在 bg `GetMultiValues` 时用 vertex alpha 统一 mix 两层，Single 固定 primary，数组缺失时的 procedural tint fallback 也使用同一规则。detail 对 base 的最终 influence reroute 在 MeddleTools 中仍未连接且源码标为 `currently borked`，因此没有可采信的最终权重公式。ignored WeaponCatalog shader-family audit 会轻量读取 MDL metadata/MTRL，不解码几何和纹理；修正 equipment-style 拳套及剩余 collision/stale reference 后，本地完整扫描 8281 个武器条目、7365 个唯一模型、8112 个可解析材质，结果为 8091 `character`、15 `skin`、6 `characterGlass`、0 `bg/bguvscroll`、0 unknown。因而武器预览范围内不存在真实 bg detail 校准样本，在获得游戏 shader 或非武器 bg fixture 前不继续猜最终 influence。
- 23 个 equipment-style 拳套已恢复静态加载。代表值 `0x0000000000012276` 会识别为 equipment set `0x2276=8822`、variant `1`，在普通 weapon candidates 之后追加默认人族 `chara/equipment/e8822/model/c0101e8822_glv.mdl`；模型引用的 `/mt_c0101b0001_a.mtrl` 会反推到 `chara/human/c0101/obj/body/b0001/material/v0001`。Meddle 对 Human 模型通过 `EquipmentModelId + HumanEquipmentSlotIndex + ResolveMdlPath` 使用真实 race-specific MDL；离线预览当前明确固定 c0101 bind pose，没有角色种族/骨架输入。`skin.shpk` 已结构化为独立 `MaterialShaderFamily::Skin`，prepared 会标出缺少 runtime `SkinColor` 与完整 skin family logic，不伪造角色肤色。49100 真实 loader 与 native WGPU snapshot 均通过。WeaponCatalog 剩余 4 项已完成分类与处理：`w0371b0033/material/v0201/mt_w0321b0033_a.mtrl` 实际返回 `pap ` 动画头，是不存在路径的 SqPack hash collision；审计现在验证资源类型后继续候选，并成功回到 `w0321` 的真实 MTRL。`w3054b0001.mdl` 的 `/mt_w3103b0001_a.mtrl` 则是客户端 MDL 内的悬空引用，`w3103` model/MTRL 与 `w3054`、关联主模型 `w3004` 根下同名文件均不存在；同步/异步 loader 仅在当前材质完全未命中时复用已加载主模型相同 material index，不引入邻近编号猜测。43624 真实 loader 已验证副模型复用 `w3004/v0002/mt_w3004b0001_a.mtrl` 及其纹理。`ModelMaterial.referenceFallback` 现会结构化记录 `sameIndexLoadedMaterial`、requested name 与 source slot/index/name/path，并进入 phantom summary；helper 只接受实际加载且自身未回退的 source，禁止 fallback 链式传播。focused tests 与 43624 真实回归均覆盖来源字段，使离线回退达到 Meddle runtime material handle/path 同等级的来源可审计性。完整审计现为 8112 个可解析材质、0 failures、2 个已绕过 collision、2 个显式 unresolved reference；报告支持 `XIV_WEAPON_SHADER_SCAN_LIMIT`、`XIV_WEAPON_SHADER_ITEM_IDS`，并单列 collision、unresolved reference 与 unclassified materials。
- `g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_MultiEmissiveColor` 已按 Meddle `Names.cs` CRC/default 和 MeddleTools `ColorMapping` 结构化进 `ModelMaterial`、phantom summary 与 renderer uniforms。早期 WGSL 曾把 MultiDiffuse/MultiEmissive 通过 mask 低权重加入 Final；后续证据边界修正已删除两者的 generic mask 消费，仅保留 verified BG secondary diffuse mix，完整 MultiMap 通道解释仍待实现。
- `GetValues` 已区分 Single、Multi、AlphaMulti/2/3、MultiMaterial、Compatibility 与 Unknown，并保留 composed raw value，贯通同步/异步 loader、`ModelMaterial`、`PreparedMaterial` 与 phantom summary。renderer 只有 `Multi` 会启用 bg/bguvscroll 双层混合；三个 AlphaMulti variant 会设置独立 `alphaMultiValues` unsupported，bguvscroll 含 Map1 时同时保留更具体的 `secondaryMapBlend`，不再静默按 Single。Meddle `Names.cs` 明确三种 value 的 family 范围；MeddleTools `node_configs.py` 只映射第一个 AlphaMulti，并注明 2/3 用于 bguvscroll/lightshaft。Blender headless 检查 `meddle bg.shpk` 进一步确认 `GetMultiValues` 实际连接到 mix factor，而 `GetAlphaMultiValues` 接口完全未连接，没有可采信的通道公式。focused tests 覆盖 absent、SHPK default、全部已知值、unknown raw、prepared raw 传播及三种诊断边界；WGSL 继续不猜公式。
- `g_OutlineColor`、`g_OutlineWidth`、`g_SpecularColorMask`、`g_SSAOMask`、`g_TextureMipBias`、`g_ShadowPosOffset` 已按 Meddle `Names.cs` CRC/default 结构化进 `ModelMaterial`、phantom summary 与 renderer uniform。Outline 的独立 inverted-hull pipeline 仍保留，但缺少空间单位/外扩公式时由 `outlineComposition` 阻止自动提交；`g_SpecularColorMask` 与 SSAO/AO 均因缺少最终组合证据只保留数据和 structured unsupported，不再改变 Final。installed character/legacy/glass DXBC 证明 `g_TextureMipBias` 同号加到全局 PBR bias，直接作用域仅主 Diffuse/Normal/Mask；WGSL 因而只对 primary Base/Normal/Mask 和 dither Base/Normal 使用 bias，不扩散到 specular/material ramps、secondary、emissive 或 lightshaft。普通纹理仍按各自 sRGB/data/packed-normal 语义生成完整 mip chain，tile/detail pair atlas 使用独立逐层 mip 与显式梯度 LOD。shadow offset 仍等待 shadow pass 语义。
- `g_GlassIOR`、`g_GlassThicknessMax` 已按 Meddle `Names.cs` CRC/default 结构化进 `ModelMaterial` 与 phantom summary；installed PS 证明当前 shader 不绑定它们，因此 renderer 已删除 `glassParams` uniform/WGSL 消费并改为显式 unsupported。
- `g_UVScrollTime` / `0x9A696A17` 已按 MeddleTools `UvScrollMapping` 结构化进 `ModelMaterial.uvScroll` 和 renderer uniform；`ModelRenderOptions.uv_scroll_time` 进入 camera uniform，Web 渲染循环用 RAF 时间驱动，native snapshot 默认时间为 0 保持稳定。prepared `usesScroll` 现在要求存在非零 multiplier 且至少一个明确可滚动 texture role；Blender headless 检查确认 lightshaft 节点没有连接 `g_TexAnim`，因此后续不再把它计作可滚动输入。
- `lightshaft.shpk` 的 `g_Color`、`g_TexAnim`、`g_TexU`、`g_TexV`、`g_Ray` 已结构化进 `ModelMaterial` 和 phantom summary。Blender headless 检查 MeddleTools `meddle lightshaft.shpk` 确认 `g_Sampler0` 与 `g_Sampler1` 均为 sRGB/Linear/Repeat：renderer 现将 `g_Sampler1` 独立保留为 secondary base，复用已有 secondary sRGB binding，以 vertex color B 驱动 `mix(Sampler0, Sampler0 * Sampler1)`，再乘 `g_Color`；颜色转 scalar 后同时作为 emission strength，并与 vertex alpha 相乘得到 additive alpha。`g_TexAnim/U/V/Ray` sockets 在节点图中均未连接，WGSL 已删除旧 UV/强度推测，原始参数仍保留用于审计。synthetic native WGPU fixture 切换 vertex B 0/1 并断言最终 RGB difference `>100000`。Meddle `Names.cs` 明确列出的 `Type` key 现已结构化为 `MaterialLightShaftType::{None,Type0,Type1,Unknown}` 并保留 raw value；`g_AngleClip`、`g_NearClip` 也按默认 `0/0.25` 与 finite override 进入同步/异步 loader、`ModelMaterial` 和 phantom summary。prepared 会保留 type/raw，并以独立 `lightshaftClip` unsupported 标记说明三个输入尚未消费；MeddleTools 节点没有对应 sockets，因此 WGSL 不猜裁剪公式。`ApplyAlphaTestOn` 会对 lightshaft 生成 Mask alpha mode，`g_AlphaThreshold` 在 additive branch 前执行 discard；prepared test 覆盖 Mask 仍选择 `AdditiveLightShaft`，synthetic native WGPU fixture 已验证阈值 `0/0.9` 产生 `>100000` RGB difference。
- `g_Transparency` 已按 MeddleTools `meddle water.shpk` 的直接 Alpha 连接进入 `PreparedAlphaSource::MaterialTransparency`；小于 1 时 water 进入 Transparent pass，WGSL 直接输出该 alpha，不乘 vertex/base alpha 或 character alpha shaping。`g_WaterDeepColor`、`g_RefractionColor`、`g_WhitecapColor` 已结构化进 model/summary/uniform，其中 deep color 按节点直接作为 water base；refraction/whitecap 因节点未连接而只保留输入。`g_SamplerWaveMap` 通过现有 normal binding 使用 R/G 解码 normal。
- sampler 分类已将 `g_SamplerWaveMap`、`g_SamplerWaveMap1`、`g_SamplerWhitecapMap` 拆成 `WaterWave/WaterWaveSecondary/WaterWhitecap`，同步/异步 loader、`WeaponTextureSet`、`ModelMaterial`、prepared bindings 与 Web texture counts 均保留各自索引。三者不会再被 generic fallback 误当 base；renderer 只消费节点已证实的 primary wave，secondary/whitecap 暂不增加 GPU binding。
- `g_SamplerEnvMap` 已从 `Other` 拆成独立 `Environment` texture kind，并贯通同步/异步 loader slot、`WeaponTextureSet`、`ModelMaterial`、prepared binding/UV/sampling、phantom summary 与 Web texture counts。采样策略按 MeddleTools 固定为 Non-Color、Linear、Repeat。Meddle Names 表明 `g_EnvMapPower` 属于 bg/bgcolorchange/bgcrestchange/bgprop/bguvscroll/crystal，而不是 `characterreflection.shpk`；在缺少可靠反射坐标与混合节点证据时 Environment 暂不接入 WGSL，也不与 character reflection 混为一谈。
- `crystal.shpk` 已从 Unknown 拆成 `MaterialShaderFamily::Crystal`；Environment binding 会设置 `usesEnvironmentMap`，并在 renderer 尚未消费时设置 `environmentMapping` unsupported。Blender headless 检查 MeddleTools `meddle crystal.shpk`：接口包含 `g_SamplerEnvMap`，但该输入在现有节点图中没有任何连线，不能据此实现可信混合；因此 renderer 在获得真实节点/样本证据前继续不采样 Environment。
- `g_SamplerMulti` 已有独立 `multiMapInterpretation` unsupported 字段：只要存在 explicit MultiMap binding 就保持 true，与只表达共享 detail diffuse/normal array 是否齐全的 `detailArray` 分离；即使 arrays 完整，multi map 通道未实现仍可审计。源码搜索与 Blender headless 全节点接口扫描均确认当前 MeddleTools 没有 `g_SamplerMulti` texture config/socket，不能从该参考仓推导 mask 公式；该标记会保持到真实 SHPK/样本证据支持正式 WGSL 消费。
- `GetSubColor` 已结构化为 `MaterialSubColorMode::{None,Face,Hair,Unknown}`，贯通同步/异步 composition、`ModelMaterial`、`PreparedMaterial` 与 phantom raw material summary。Meddle Names 明确 key `0x24826489` 与 Face/Hair values，MeddleTools hair mapping 也消费两种模式；`GetSubColorFace` 同时适用于 characterocclusion/charactertattoo，实际颜色来自 Meddle composer 注入的 customize buffers。prepared 会在 characterOcclusion 或显式 Face/Hair 模式下设置 `runtimeSubColor`，离线 loader 不伪造颜色。
- `GetMaterialValue` 已从 material debug known-name 白名单提升为 `MaterialSkinValueMode::{None,Face,Body,BodyJjm,FaceEmissive,Unknown}`，贯通同步/异步 composition、`ModelMaterial`、`PreparedMaterial` 与 phantom summary。Meddle `Names.cs` 明确 `skin.shpk` key `0x380CAED0`、默认 Face 及四种 value；MeddleTools 只对 `skin.shpk + Face` diffuse 设置 Linear/EXTEND，以避免越界 UV wrap 出牙齿，Body mapping 仍标为 TODO。prepared 现仅在 `Skin + Face` 时把 base-color address mode 改为 ClampToEdge，normal/mask/index 与 Body variants 继续 Repeat；focused test 覆盖 family/mode 边界。49100 安装数据确认 equipment-style glove skin 实际为 Body 并保持 Repeat，没有把 face 规则泛化到所有 skin。
- `GetDecalColor / 0xD2777173` 已结构化为 `MaterialDecalColorMode::{Off,Alpha,Rgba,Unknown}` 并保留 raw value，贯通同步/异步 loader、`ModelMaterial`、`PreparedMaterial` 与 phantom summary。Meddle 明确 Off/Alpha/RGBA values；MeddleTools skin 映射 Alpha 并同时连接 runtime `DecalColor`、strength 与 `DecalTexture`，tattoo 则无条件连接 runtime `g_DecalColor`。Meddle composer 证明颜色来自 customize/decal constant buffer，on-render output 证明纹理是运行时输入。prepared 对 skin 的任意非 Off mode 标记 `runtimeDecalColor`、`runtimeDecalTexture` 与独立 `decalColorMode`，tattoo 现有无条件 runtime decal color 保持不变；focused test 覆盖 absent、SHPK default、MTRL override、unknown raw、prepared 传播和 Skin/Tattoo 诊断。material debug 的 known shader label 也已覆盖 Off/Alpha/RGBA，默认值和 override 不再退化为裸 CRC。WGSL 不猜混色公式。
- `characterreflection.shpk` 已有独立 `characterReflection` unsupported 字段，并继续保留 `incompleteShaderFamilyLogic`。Meddle/MeddleTools 搜索未发现静态 reflection sampler、on-render replacement 或对应节点组，MeddleTools 也没有把它映射回 `character.shpk`；现 renderer 因而明确标为 generic character approximation，不把 Environment/sphere/specular 资源猜作 reflection 输入。该标记会保持到获得真实 SHPK 节点或样本证据。
- `g_AlphaAperture`、`g_AlphaOffset`、`g_ShadowAlphaThreshold` 已按 Meddle `Names.cs` CRC/default 结构化进 `ModelMaterial`、phantom summary 与 renderer `alphaParams` uniform；后续证据审查已移除无节点依据的 aperture/offset `pow` shaping，非默认值现在进入 `alphaShaping` unsupported，`g_ShadowAlphaThreshold` 仍未驱动 shadow pass。
- character transparency/glass 的控制入口已结构化：`ModelMaterial.drawDepthMode` 保留 `None/Dither/Unknown`，`lightingMode` 保留 default/enabled/disabled/unknown；`PreparedMaterial.alphaPolicy` 输出 alpha source、depth mode 与 lighting enabled。MeddleTools `shaders.blend` 进一步确认普通 character Alpha 输出来自 `g_SamplerNormal` Blue，stockings 分支强制为 1；renderer 已按该规则让 character blend/glass/transparency 从 normal B 取 alpha，并让 `EnableLightingOff` 的 character transparency 走 unlit surface。`GlassBlendMode` 已作为显式 scene option 进入 renderer、Web 和 snapshot；Alpha/Add 分别使用标准 alpha blend 与 additive pipeline 近似。
- MeddleTools `meddle charactertattoo.shpk` 节点图明确把 `g_SamplerNormal` Alpha 连接到材质 Alpha，而不是复用普通 character 的 normal Blue。`PreparedAlphaSource::NormalAlpha` 已表达该差异，renderer uniform 编码为 `4.0`，dither depth 与主 fragment 的 alpha resolver 均接收 normal B/A 并按 source 选择；只修正可静态证明的 alpha，依赖运行时 `OptionColor`/`DecalColor` 的 base color 混合继续保持 unsupported。
- `ModelDebugMode` 已提供 renderer debug 视图：final、base、normal、mask、material properties、specular、emissive、alpha、UV0-UV3、vertex color、mesh/draw-role color、ColorTable index、material map、multi map、tile/sheen/sphere properties、tile matrix，以及 tile normal/ORB、detail diffuse/normal atlas 选层结果；Web 控件和 snapshot/test render options 共用同一入口。phantom 可通过 `XIV_PHANTOM_ARRAY_DEBUG=1` 输出四张数组诊断图。

主要缺口集中在：

- 多套 UV、secondary normal/bitangent、`color1`、flow 已进 GPU 输入；Flow material mode 已让 WGSL 将插值后的 `flow0.xyz` 正交化为 primary tangent，并结合已有 bitangent 方向构造 normal mapping frame。bguvscroll Map1 normal 已使用 secondary normal/bitangent frame；`flow1` 和 `color1` 仍未进入实际 family-specific shading，不伪造未证实的动画用途。
- `g_SamplerColorMap1`、`g_SamplerNormalMap1`、`g_SamplerSpecularMap1` 已拆成 secondary logical roles；`GetValues` 已结构化为 Single/Multi/AlphaMulti/AlphaMulti2/AlphaMulti3/MultiMaterial/Compatibility/Unknown，并保留 raw。MeddleTools `meddle bg.shpk` 已证明 `GetMultiValues` 的 vertex-alpha 混合公式并完成消费；AlphaMulti 输入未连接，prepared 会统一报告 `alphaMultiValues`，bguvscroll Map1 另报告 `secondaryMapBlend`，不猜测公式。
- fragment stage 保持 15 张 sampled texture：仅 `BgUvScroll + GetMultiValues` 将 secondary color/normal/specular 复用到物理 binding 9/10/11，其它 family 仍使用这些 binding 的 tile/sheen/sphere 语义。Specular Map0/1 也已按 MeddleTools 从旧 sRGB 假设修正为 Non-Color + Linear/Repeat。
- mesh category、submesh attribute mask/name 和 shape influence 摘要已进入 prepared；sparse morph target 保留在 `ModelMesh` 并由 renderer creation 消费。`PreparedModelOptions.enabledAttributeMask` 可隐藏 disabled submesh，`enabledShapeMask` 可审计并实际应用 active shape，但 Web 离线默认仍不猜这些 mask。静态 MDL 不含 runtime `ShapeMasks` 名称到 bit id 的映射，因此离线 `shape index -> bit` 只能是显式 table-order 约定，不能冒充游戏默认状态。bone/skinning 和 runtime mask 输入仍没有进入渲染决策。
- 材质语义仍被压缩成少量近似规则和 Opaque/Mask/Blend/Glass；ColorTable extra maps、tile/detail、bguvscroll Map0/Map1、Flow 与 water alpha/base/primary normal 已有第一版实时消费，但 water refraction/whitecap/WaveMap1、AlphaMulti variants、detail 最终 influence、reflection 等节点逻辑仍不完整。
- 运行时 GPU ColorTable、decal、crest、on-render material output 是 Meddle 运行时路径的优势；当前离线 Web 预览没有等价输入。静态 stain0/stain1 已有完整离线输入和 STM 应用路径，decal/crest 与 materialChange 已执行默认 fallback，但仍不能显示真实运行时 crest/decal 内容。
- 文档 `weapon-render-pipeline.md` 已同步到当前实现；后续设计和优先级以本文 roadmap 为准。

## 分层审查结论与计划总览

### 1. 数据解析

审查结论：

- 本仓已经对齐 Meddle 的 LOD0 mesh range 和 extra LOD 分类方式，能区分 normal/water/shadow/terrainShadow/verticalFog/lightShaft/glass/materialChange/crestChange，且顶点层保留了多套 UV、secondary normal/bitangent/color、flow、blend weights/indices。
- MTRL 解析已经不只靠文件名猜测：sampler role 会优先使用 `.shpk` resource parameter name，再退回 known CRC 和路径后缀；shader package default 与 material override 也已经进入 debug summary。
- ColorTable 解析已覆盖 Dawntrail 与 Legacy，并按 Meddle/MeddleTools 语义产出 diffuse、specular、material-properties、tile、sheen、sphere、tile-matrix 等派生贴图；TileAlpha 已明确不再被误当作材质 alpha。

主要不足：

- 染色已接入同步/异步 weapon model load、material summary、ColorTable bake 和 Web stain0/stain1 选择器；EXD 名称、UI 色块、排序和 metallic metadata 也已导出。正式 phantom fixture 现包含 `45052` baseline、stain `[1,0]` 与第二通道 metallic `[0,113]`（闪耀金）；该材质的 Dawntrail dye table 有 30 行 channel0、2 行 channel1，两种染色 case 都精确改动 2 行。snapshot runner 支持 case-level `expectedRowsChanged`，并在三例同时生成时断言最终 PNG 两两 RGB difference 大于 `100000`；本地结果分别为 `202032`、`2743702`、`2923818`，视觉检查确认 metallic case 只显著改变蛋糕夹层区域。
- Meddle 的 runtime 输入，包括 GPU ColorTable、resolved texture/material handle、decal、crest、on-render material output，仍不能由离线 SqPack 还原；其中 decal/crest 与 materialChange 已有显式 prepared fallback，GPU ColorTable 和 handle remap 继续只记录为缺口。
- reflection/stockings/tattoo/occlusion 等 shader package 已能分类，但很多 shader keys/constants 还没有提升为结构化字段，也没有最小 fixture 覆盖；outline/specular/SSAO、toon/sheen/sphere、alpha aperture/offset/shadow threshold、glass IOR/thickness 和 transparency 已先进入结构化字段但未驱动完整 shader-family 行为。lightshaft 已对齐 MeddleTools 当前连接的双纹理、`g_Color`、emission/alpha 节点；`Type/AngleClip/NearClip` 与未连接 constants 仍缺游戏行为证据。
- texture/sampler 语义仍有少量兜底路径依赖；MeddleTools 里 `_id.tex`、tile/detail arrays 使用 Non-Color + Closest/Repeat 的规则已经进入 prepared policy。tile/detail vertical atlas 已进入 GPU/WGSL 并完成第一版选层和组合，后续重点转为通道解释、权重校准和 shader-family-specific UV 路由。

计划：

1. 先继续扩充可审计信息：在 material/prepared debug 中补齐 texture role 的最终来源、shader family、sampler policy、UV source、feature flags 和未支持 runtime 输入标记。
2. 染色体验链路已完成请求、STM、bake、EXD metadata、Web 双通道选择器、URL 状态，以及 45052 第一/第二通道与 metallic 正式 snapshot；application row count 和三图像素差异均有断言。EXD 颜色继续只用于 UI，不作为实际覆盖值。
3. 逐步结构化 shader-family 参数：优先 glass/transparency/lightshaft/scroll，再处理 reflection/stockings/tattoo/occlusion；每补一个参数都加合成 MTRL fixture 和真实样本 debug 对照。
4. 对 runtime-only 数据不盲猜：decal/crest 已建立透明纹理 fallback，materialChange 已建立基础材质 fallback；renderer final pass 已执行 crest discard 与基础材质路径，mesh-role debug 仍保留几何可见性。GPU ColorTable 继续只在 debug 中标明缺失，避免离线预览伪装成完整运行时渲染。

### 2. 解析后的结果处理

审查结论：

- `PreparedModel` / `PreparedMaterial` 已经把 raw parsed data 和 renderer binding 决策分开，renderer 与 phantom summary 共用 draw role、main-pass visibility、prepared pass、texture bindings、sampling policy、feature flags 和第一版 UV source。
- submesh attribute mask/name、显式 `enabledAttributeMask`、shape influence、显式 `enabledShapeMask` 审计和 mesh-level flow presence 已进入 preparation；sparse shape target 作为几何输入保留在 `ModelMesh`。这与 Meddle 的 shape/attribute group 思路一致。Meddle 将每个 shape 导出为独立 glTF morph target并对 enabled shape 赋权重 1；本仓现在同样累加各 target 相对 base geometry 的 delta，不按遍历顺序覆盖。
- `PreparedRenderPass` 已能表达 `Opaque`、`Cutout`、`Transparent`、`Glass`、`AdditiveLightShaft`；lightshaft 不再误进主 surface pass。

主要不足：

- shape mesh/morph 已支持显式离线应用：MDL geometry 保留 shape value，submesh remap 按受影响的 index-buffer position 拆点并输出稀疏 position/normal target，renderer 构建 GPU vertex buffer 时仅在调用方显式提供 mask 后累加 active target delta，默认保持 base geometry。仍缺 runtime name/bit mapping、skinning runtime 输入和更细的 per-submesh prepared draw。
- `PreparedMaterialUvSources` 已驱动 renderer 选择采样 UV；`PreparedTextureSamplingSet` 也已逐 texture binding 驱动独立 sampler descriptor。
- shader-family-specific 规则还没有进入中间层，例如 character base texture 如何与 ColorTable diffuse 混合、material/multi map 通道如何解释、scroll/reflection 使用哪套 UV/flow。
- `usesDye`、decal/crest、runtime ColorTable、runtime material change、tile/detail array 这些 capability flags 已有第一版 prepared unsupported summary；stain 与共享数组已进入 renderer，crest/decal 缺失时的透明 fallback 和 materialChange 基础材质 fallback 也已执行。runtime ColorTable 与真实 on-render crest/decal 内容仍缺显式输入。

计划：

1. 扩展 preparation 输入：stain 已进入 `WeaponModelLoadRequest`，decal/crest 与 materialChange 已有默认 fallback；后续只在有真实调用方需求时增加显式 runtime decal/crest 资源入口。`enabledShapeMask` 继续保持审计输入，默认不猜运行时状态。
2. 把 prepared texture/sampler/UV source 从“输出给 debug”推进到“驱动 renderer binding”：UV source 已接入 renderer material uniform 和 WGSL 采样选择；prepared sampler policy 已逐 binding 驱动 15 个 sampler；后续继续接入 shader-family-specific source 和 nearest data resources。
3. 将 shader-family-specific 规则下沉到 prepared 层：为 character/glass/transparency/scroll/lightshaft/reflection 等输出明确的 feature flags、UV source、blend/alpha policy 和需要的 texture roles。
4. 继续让 phantom `model-summary.json` 输出 preparation 结果，新增“为什么没画/为什么用了 fallback”的原因字段，作为后续真实样本验证的主要入口。draw/runtime fallback 已有 prepared 字段；stale MTRL reference 也已通过 `ModelMaterial.referenceFallback` 输出 kind、requested name 与 source slot/index/name/path。

### 3. 渲染器与着色器管线

审查结论：

- renderer 已上传扩展顶点格式，并已将 base/normal/mask/emissive/specular/material-properties/tile/sheen/sphere/tile-matrix 绑定到 material bind group；每个现有 texture binding 已有独立 sampler。
- 透明 batch 已按 mesh-level back-to-front 排序；glass 已进入透明管线；cutout 有 alpha test；ColorTable extra maps 已在 WGSL 中产生可观察的保守高光/反射近似。
- 当前 `model.wgsl` 仍是单个近似 shader，虽然已能按 prepared UV source 在 `uv0-uv3` 间选择采样 UV，但 shader-family-specific 规则仍少，实际材质多数仍走 `uv0`、primary normal/bitangent、`color0` 的近似路径；与 MeddleTools 节点图和 bake pass 的差距仍集中在 shader 行为，而不是字段缺失。

主要不足：

- cutout/glass 已有独立 wgpu pipeline 入口，但 shader 行为仍分别沿用现有 alpha test 与 glass 近似；additive-lightshaft 已对齐 MeddleTools 当前连接的双纹理、vertex B、`g_Color`、emission/alpha 与 alpha-test threshold，未连接 constants 和 Type/AngleClip/NearClip 保持显式 diagnostic。
- 多套 UV 已开始通过 prepared source 和 per-role UV scroll 参与采样；Map1/UV1Scroll、secondary tangent frame、tile/detail arrays 与 Flow primary tangent 已参与 shading，但 `color1`、`flow1` 和 multi map 的完整解释仍未完成。
- alpha/glass/transparency 已从固定 glass opacity 前进到 prepared alpha source：character glass/transparency 强制进入对应 pass，normal B 驱动 alpha，`EnableLighting` 可控制 transparency lighting；`DrawDepthMode_Dither` 已驱动专用 depth-only prepass，使用与颜色 pass 一致的 prepared alpha source 和稳定 4x4 屏幕空间有序阈值。`GlassBlendMode` 已作为显式 scene option 进入 renderer/Web/snapshot，但 Mul 仍保留现有 alpha-blend 近似，Add 只选择硬件 additive pipeline；折射和真实厚度传输仍缺失，且尚无真实 charactertransparency 武器样本。Meddle 只确认 `DrawDepthMode_Dither` 的 material key/value 与适用 SHPK，没有游戏抖动公式；MeddleTools 不实现运行时 depth pass，因此当前公式是保守近似。它仍与 scene-level `ApplyDitherClip` 区分，后者覆盖更多 shader family。`GlassBlendMode` 也只有 scene key 的 Mul/Add 名字与默认值，没有 MTRL 来源或 MeddleTools 节点语义，因此没有写入 parsed material。
- `characterstockings.shpk` 已按 MeddleTools `meddle character.shpk` 的 `IS_STOCKING` 节点行为把最终 alpha source 和普通 surface render pass 都固定为 Opaque；即使静态 base alpha 或 alpha-test 预分类为 Mask/Blend，也不会误进 Cutout/Transparent。Glass/Crest/LightShaft 等 mesh draw role 仍保持更高优先级。Meddle `OnRenderMaterialUtil` 同时证明 stockings 会复制 runtime skin material textures 并应用 legacy body decal；离线 loader 尚无这些运行时输入，因此仍保留 `incompleteShaderFamilyLogic`，不宣称完整支持。
- renderer debug view 已能切换 base、normal、mask/material、specular、emissive、alpha、UV、两套 vertex color、secondary normal、flow0/flow1、mesh/draw-role color、ColorTable index、material map、multi map、ColorTable extra maps 与四种 array 选层结果。15 个 texture binding 已使用独立 sampler；`skin.shpk + GetMaterialValueFace` diffuse 的 ClampToEdge 不再连带改变 emissive。另一个明确的 CLIP 角色是 runtime `Decal`，仍等待显式输入和 shader 级 UV clip。

计划：

1. prepared pass 已分管 opaque、cutout、transparent、glass、additive lightshaft、dither depth 与 outline pipeline；后续重点是补完整 family-specific 行为和真实 blend/depth 语义，保持现有视觉输出稳定并继续扩充 synthetic pipeline tests。
2. 让 WGSL 继续按 prepared UV source 和 feature flags 消费更多通道：per-role scroll、Map1/UV1Scroll、tile/detail 和 Flow primary tangent 已接入，后续优先补 multi map mask，再做 secondary normal/bitangent、flow1 与 color1。
3. 按 shader family 拆函数而不是继续堆主函数：base color、normal、material properties、alpha、emissive、glass、tile/sheen/sphere、scroll/reflection 分块，先用分支承载，必要时再拆 shader module/pipeline。
4. 继续补 debug render modes：base、normal、mask/material、specular、emissive、alpha、UV set、vertex channels、mesh/draw-role color、ColorTable index、material map、multi map、ColorTable extra maps 与四种 array 选层结果已可检查；per-texture sampler policy 已独立执行，后续按需要增加 policy preview。

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

- 后续 roadmap 仍以本文为准；独立 cutout/glass/lightshaft pipeline、显式 submesh visibility、tile/detail array 与显式 shape morph 已是当前能力。runtime 名称到 mask bit 映射、runtime 默认 visibility、skinning 和完整 family shader 仍是缺口。

验证：

- 文档审查即可，不需要代码测试。

### P1: 扩展材质参数解析范围

现有 `.shpk` 解析主要用于 sampler role、material key 和常量默认值。后续需要把常见 character/bg/lightshaft/scroll/glass 参数提升成结构化字段，而不是只留在 debug。

当前进度：

- 已完成：`g_AlphaThreshold` 进入 `ModelMaterial.alphaThreshold`，用于 cutout discard 阈值。
- 已完成：`g_NormalScale` 进入 `ModelMaterial.normalScale`，默认 1.0，材质 override 优先于 shader package default，renderer 会 clamp 到 0..4 后作用于 normal map XY 强度。
- 已完成：`g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale` 进入 `ModelMaterial`，默认 1.0，材质 override 优先于 shader package default；renderer uniform 和 detail debug sampling 已进入 WGSL，但 detail 对 Final normal 的 influence 不再猜测，`detailComposition` 显式标记该边界。
- 已完成：`g_TileIndex`、`g_TileAlpha`、`g_TileScale` 进入 `ModelMaterial`，默认值分别为 `0`、`1`、`[16,16]`；renderer 已优先按 ColorTable tile properties、回退按 `g_TileIndex` 选择真实 tile normal/ORB atlas layer，并结合 TileMatrix/TileScale 采样。ORB 已从错误的 RGB property 解释改为节点证明的 Blue color darkening，tile normal Alpha 与 TileAlpha 共同控制 normal 权重。
- 已完成：完整 toon/sheen/sphere 参数族进入 `ModelMaterial`；其中 `g_ToonLightSpecAperture=50`、`g_ToonReflectionScale=2.5`、`g_ToonSpecIndex≈0` 已补齐同步/异步 composition、known constant label、phantom summary、renderer uniform 和 focused tests。prepared `usesToon` 限定 character family；所有无节点证据的 Toon/Sheen/Sphere 输入均不进入 Final，非默认或非零值转为独立 unsupported。
- 已完成：`g_DetailID`、`g_MultiDetailID`、`g_DetailColor`、`g_MultiDetailColor`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 进入 `ModelMaterial`，默认值分别为 `0`、`0`、`[0.5,0.5,0.5,1]`、`[0.5,0.5,0.5,1]`、`[4,4,4,4]`、`[4,4,4,4]`；renderer 已按两个 ID 和各自 UV scale 采样真实 detail diffuse/normal atlas。primary/multi debug sample 按 MeddleTools 的 `MultiBlendWeight` 语义使用 GetMultiValues + vertex-alpha mix，Single 固定 primary；Final 不消费未经证明的 detail tint/normal influence，`detailComposition` 显式标记该边界。完整 WeaponCatalog 扫描确认武器材质没有 bg family；在获得游戏 shader 或非武器 fixture 前不校准最终 influence 系数。
- 已完成：`g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_MultiEmissiveColor` 进入 `ModelMaterial`，默认值分别为白色、白色、黑色、黑色；renderer uniform 已进入 WGSL。该阶段的 generic MultiDiffuse/MultiEmissive mask 加权后来因无节点/通道证据被移除；当前 MultiEmissive 只保留解析/uniform，MultiDiffuse 仅在 verified BG secondary diffuse mix 中消费。
- 已完成：`g_OutlineColor`、`g_OutlineWidth`、`g_SpecularColorMask`、`g_SSAOMask`、`g_TextureMipBias`、`g_ShadowPosOffset` 进入 `ModelMaterial`，默认值分别为黑色、`0`、白色、`1`、`0`、`0`；renderer uniform 已进入 WGSL。Outline pipeline 保留但非默认输入由 `outlineComposition` 阻止自动提交；SpecularColorMask 与 SSAO/AO 只保留诊断。`g_TextureMipBias` 按 installed DXBC 只驱动 primary Base/Normal/Mask 及 dither Base/Normal，真实值域固定为 `0/+1/-1`。shadow offset 继续等待 shadow pass。
- 已完成：`g_GlassIOR`、`g_GlassThicknessMax` 进入 `ModelMaterial`，默认值分别为 `1`、`0.01`；installed characterglass pixel shader 不绑定它们，renderer 已移除无证据 uniform/lighting 消费，非默认值由 `glassShaderParameters` 显式诊断。opacity、折射与真实厚度传输仍待后续 shader-family 语义确认。
- 已完成：`g_AlphaAperture`、`g_AlphaOffset`、`g_ShadowAlphaThreshold` 进入 `ModelMaterial`，默认值分别为 `2`、`0`、`0.5`；renderer uniform 已进入 WGSL。aperture/offset 的无证据 shaping 已移除并由 `alphaShaping` 诊断替代；shadow alpha 与 transparency opacity 仍待后续 shader-family 语义确认。
- 已完成：`g_UVScrollTime` / `0x9A696A17` 进入 `ModelMaterial.uvScroll`，按 MeddleTools 映射转换为 `[-x, y, -z, w]`；`bguvscroll.shpk` 的 Map0/Map1 分别使用 UV0Scroll/UV1Scroll，`GetMultiValues` 以 vertex alpha 混合两套 color/alpha/normal/specular，其它 role 与 `characterscroll.shpk` 不继承该动画。
- 已完成：`lightshaft.shpk` 的 `g_Color`、`g_TexAnim`、`g_TexU`、`g_TexV`、`g_Ray` 进入 `ModelMaterial`；renderer 只消费节点已连接的 `g_Color`，`g_TexAnim/U/V/Ray` 保留审计但不再驱动无证据的 UV/强度近似。`g_Sampler0/1`、vertex B、emission strength/alpha 已按 MeddleTools 节点对齐；Type/AngleClip/NearClip 已结构化并通过 `lightshaftClip` 标明未消费。
- 已完成：`g_Transparency` 进入 `ModelMaterial.transparency`，material override 优先并 clamp 到 0..1；water prepared pass/alpha source 与 WGSL 已直接消费，character/glass 不受影响。`g_WaterDeepColor/g_RefractionColor/g_WhitecapColor` 及三种 water sampler role 也已结构化，primary wave 已复用 normal binding。

后续优先参数：

- `GlassBlendMode`、dither depth、water alpha/base/primary wave、Map1/UV1Scroll、stockings opaque alpha/pipeline 与 tattoo normal-Alpha 已完成；后续补 reflection/occlusion。water refraction/whitecap/WaveMap1 与 AlphaMulti variants 等待真实节点连接或游戏 shader 证据。

验证：

- 用合成 MTRL fixture 测 shader constant 解析。
- 已增加 normal scale focused tests，覆盖 primary/multi/detail normal scale 的 shader package default、material override 和 clamp；multi/detail normal scale fallback 的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 tile select focused tests，覆盖 `g_TileIndex`、`g_TileAlpha`、`g_TileScale` 的 shader package default、material override 和 renderer uniform 传递；native synthetic WGPU fixture 验证 ORB R/G/Alpha 不改变输出、ORB Blue 直接 darken base，以及 tile-normal Alpha × TileAlpha 控制 normal contribution。
- 已增加 toon/sheen/sphere focused tests，覆盖 shader package default、material override、非 finite fallback 和 renderer uniform 传递；native synthetic WGPU fixture 验证强 Toon override 与非零 Sheen/Sphere 都不改变 Final，同时分别显示独立 unsupported 诊断色。
- 已增加 detail focused tests，覆盖 `g_DetailID`、`g_MultiDetailID`、`g_DetailColor`、`g_MultiDetailColor`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 的 shader package default、material override、短数组 fallback、非 finite fallback、renderer uniform 传递和 `detailComposition` unsupported；native snapshot 同时验证 detail debug mix 与 Final 隔离。
- 已增加 shader color focused tests，覆盖 `g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_MultiEmissiveColor` 的 shader package default、material override、短数组 fallback、非 finite fallback 和 renderer uniform 传递；diffuse/multi diffuse 与 emissive/multi emissive 的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 outline/specular/occlusion focused tests，覆盖 `g_OutlineColor`、`g_OutlineWidth`、`g_SpecularColorMask`、`g_SSAOMask`、`g_TextureMipBias`、`g_ShadowPosOffset` 的 shader package default、material override、短数组 fallback、非 finite fallback 和 renderer uniform 传递；后续证据审计已把 outline、SpecularColorMask 与 SSAO/AO 的无证据 Final 消费替换为 structured unsupported 和 native boundary fixture。mip focused tests另覆盖奇数尺寸完整链、sRGB/data/normal downsample 和 linear/nearest mip sampler；最新 DXBC scope fixture 以高频纹理验证 Base 随 `-8/+4` 变化而 Specular/Emissive debug 保持一致。
- 已增加 glass params focused tests，覆盖 `g_GlassIOR`、`g_GlassThicknessMax` 的 shader package default、material override、非 finite fallback 和 renderer uniform 传递；glass IOR/thickness 的 WGSL 消费通过 native snapshot 编译验证。
- 已增加 alpha params focused tests，覆盖 `g_AlphaAperture`、`g_AlphaOffset`、`g_ShadowAlphaThreshold` 的 shader package default、material override、clamp、非 finite fallback、renderer uniform 传递和 `alphaShaping` unsupported；native snapshot 验证 override 不改变 Final 并显示独立诊断。
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
- `PreparedMaterial` 已包含第一版 `MaterialShaderFamily` 分类，覆盖 MeddleTools 映射中的 character/skin/glass/transparency/scroll/bg/lightshaft/water 常见包。
- `PreparedMaterial` 已包含第一版 `PreparedTextureBindings`，聚合 renderer 当前已知的材质贴图索引。
- `PreparedMaterial` 已包含第一版 `PreparedTextureSamplingSet`，把 texture role 的 sRGB/Non-Color、linear/nearest、repeat/clip 语义从 renderer 私有实现中拆出来；renderer 已消费 repeat/clamp/linear/nearest，`Clip` 目前在 sampler 层降级为 clamp，等待 shader 级 clip 逻辑。
- `PreparedMaterial` 已包含 `PreparedMaterialFeatureFlags`，聚合 `usesVertexColor`、`usesColorTable`、`usesTile`、`usesDetail`、`usesScroll`、`usesFlow` 与 `usesDye`；flow 已结合 material mode + mesh flow0，scroll 已结合非零 multiplier + per-role mask。
- `PreparedMaterial` 已包含 `PreparedMaterialUvSources` 与 `PreparedTextureScrollSet`；renderer 按 texture-role source 选择采样 UV，并只对 mask 明确启用的 role 应用对应 UV0/UV1 multiplier。
- `PreparedMaterial` 已包含第一版 `PreparedMaterialUnsupportedInputs`，会把 dye application、runtime ColorTable、decal/crest、runtime material change、tile/detail array 和特殊 shader family 行为缺口输出到 prepared summary。
- `PreparedMaterial` 已包含 `resourceAvailability` 和 `runtimeFallbacks`：共享数组是否成对完整、crest/decal 缺失时透明纹理 fallback、materialChange 的基础材质 fallback 均可审计。
- phantom `model-summary.json` 会在主 surface mesh 上输出 prepared material 决策，并通过第一版 `PreparedModel` 获得 mesh draw role / main pass 可见性。
- `PreparedModel` 仍是第一版：已包含 submesh attribute mask/name，并新增 `PreparedModelOptions.enabledAttributeMask` 与 `PreparedMeshVisibility`，可在显式提供运行时 enabled attribute mask 时按 Meddle composer 语义隐藏 disabled submesh。mesh-level shape influence 已进入 `ModelMesh` / `PreparedMesh`，sparse target 仅保留在 `ModelMesh` 作为几何 payload；shape value 按 mesh-relative index-buffer position 定位，submesh remap 保留 position/normal morph delta，`ModelRenderer::new_with_prepared_options` 会消费显式 shape mask，默认无 mask时不改变几何或 draw visibility。mesh-level flow presence 已进入 prepared material feature flags；第一版 UV source 已驱动 renderer 采样选择。model-level texture 访问现已把共享 array 的 missing texture、kind、layout、pair compatibility 与 layer count 前置到 prepared；尚未包含 runtime shape bit/name mapping、skinning 或 per-submesh prepared draw，sampler config、feature flags 与 shader-family-specific UV source 仍未完整驱动所有 runtime 绑定。
- 当前缺口：stain template/application 与 tile/detail array 已有真实数据入口和第一版 renderer 行为；runtime GPU ColorTable 仍无离线替代输入，真实 crest/decal 内容仍不可用，特殊 shader family 的 unsupported 标记仍主要是审计信息。

建议中间结构包含：

- mesh draw role：normal、glass、lightShaft、shadowOnly、ignored、materialChange、crestChange；已有第一版 `PreparedMesh`，并保留 submesh attribute mask/name、attribute visibility 决策与 shape influence active/inactive 状态；shape target 已独立保留 sparse output vertex + position/normal delta，不混入 draw visibility
- material shader family：character、characterStockings、characterGlass、characterReflection、characterTransparency、characterScroll、characterTattoo、characterOcclusion、bg、bgUvScroll、crystal、lightShaft、water、unknown；已有第一版分类，后续逐个补 shader-family-specific 行为
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
- `materialChange`、`crestChange` 已拆为独立 draw role并进入主 pass；prepared summary 分别标出基础材质与透明纹理 fallback，renderer final pass 会让 crest fallback discard、materialChange 使用基础材质，mesh-role debug 仍显示二者几何。
- submesh attribute mask/name 已进入 `ModelMesh` 与 `PreparedMesh`，并随 phantom summary 输出；`PreparedModelOptions.enabledAttributeMask` 已支持显式运行时 mask，按 `requiredMask & !enabledMask == 0` 判断 submesh 是否可见。mesh-level shape influence/target 已进入 `ModelMesh`，phantom summary 输出 target/delta count；`PreparedModelOptions.enabledShapeMask` 已支持 active/inactive 审计和 renderer creation 时的实际 morph。Web 离线默认仍不猜 mask。

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
- `colorset_sphere`: sphere index / sphere mask；Dawntrail raw SphereIndex 先从 half bits 解码再按 0..255 归一化，low-level debug 继续保留 raw `u16`
- `colorset_tile_matrix`: float channels 已以 `Rgba32Float` 上传，RGBA8 仅 debug/fallback

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
- multi map/detail map 的第二层颜色/法线影响；detail color/UV scale、tile index/scale、multi/detail normal scale 与 bguvscroll Map0/Map1 已进入第一版组合；ORB/tile-normal 通道已按节点证据对齐，后续重点是校准 detail 最终 influence 和补 multi map mask
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

当前仍保持近似视觉行为：fragment shader 已按 prepared UV source 与 per-role mask 选择静态/滚动 UV，bguvscroll Map1 会消费 UV1Scroll 与 secondary tangent frame，Flow 模式会消费 `flow0` tangent；其它 source 规则仍基本默认 `uv0`，primary normal/bitangent 与 `color0` 仍是主要输入。后续重点是其它 `uv1-uv3` 用途、`color1` 与 `flow1`。

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

当前仍未完成的是让已独立分派的 cutout/glass/lightshaft pipeline 具备更完整的 shader-family-specific 行为：

- opaque pass：写 depth
- cutout pass：已有独立 pipeline，写 depth，alpha test discard；尚未有 shader-family-specific cutout 行为
- transparent pass：不写 depth，mesh-level sorted
- glass pass：已有独立 pipeline，不写 depth，参与 mesh-level sorted，并支持显式 Mul/Add scene option；当前 Mul 仍是 alpha-blend 近似，尚无真实乘法、折射与厚度传输
- additive/lightshaft pass：加法混合、不写 depth；已按 MeddleTools 当前连接消费 Sampler0/Sampler1、vertex B、`g_Color`、emission strength/alpha，`ApplyAlphaTest/g_AlphaThreshold` 会在 additive 输出前 discard；Type/AngleClip/NearClip 与未连接 constants 继续只做可审计保留

shadow、terrainShadow、verticalFog 在主预览中默认不画，避免错误 surface；lightShaft 不作为普通 surface，但会通过 additive pass 绘制。

验证：

- 已增加 prepared material / render pass 单元测试，覆盖 opaque、cutout、transparent、glass、mesh glass override 和 culling policy。
- 已有 water transparency、tattoo dither alpha、GlassBlend override、outline、lightshaft 双纹理与 alpha-test threshold 编译/画面验证；后续仍需针对其它 family-specific cutout、真实 glass composition 和未获证据的 lightshaft clip 语义补 fixture。
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
- tile alpha 与 tile-normal Alpha 相乘，只控制 tile normal contribution，不作为材质透明度或高光参数。
- sheen/sphere 保留 binding 与 debug view，但不再进入无证据的额外高光/rim 近似；Blender `tile_select` 证明 tile-matrix 只参与 tile UV，旧 matrix-delta specular 已删除。
- renderer debug view 可直接预览 tile、sheen、sphere、tile-matrix 四张烘焙 ramp。
- 已增加 focused test，确认 extra map flags 只在 texture index 实际存在时启用。

仍建议的后续顺序：

1. TileMatrix identity delta 对 specular 的无证据调制已删除，uniform/patterned tile arrays 已分别验证“只改变 UV 不改变材质光照”；后续用更多真实 character 样本确认 TileMatrix 与 `g_TileScale` 的组合边界，并核对 ORB Blue darkening 的资源色彩空间。
2. 已完成 SheenProperties/SphereProperties 的 RGBA8 量化修正：保留 bake 后 float ramp、以 `Rgba16Float` 上传；focused fixture 覆盖 `SheenAperture=4.0`，真实 45059 regression 也确认 float payload 保留 4.0。
3. 已完成 WeaponCatalog 级 shader-family 扫描：修正 equipment-style 拳套、PAP hash collision 候选提前终止与 stale 副手材质回退后，8281 个武器条目、7365 个唯一模型、8112 个可解析材质中没有 bg/bguvscroll；分类为 8091 character、15 skin、6 characterGlass、0 unknown，审计 failures 为 0。另有 2 个 collision 与 2 个客户端悬空引用作为独立诊断保留。因此 weapon preview 内无真实 bg detail 样本，MeddleTools 最终 influence 仍标为 borked，在取得游戏 shader 或非武器 fixture 前不校准猜测权重。
4. sphere 作为环境/反射输入仍缺可采信公式：MeddleTools 只把 `SphereIndex/SphereMask` 暴露为 `PackedColorTableRampLookup`，没有 reflection/sphere 混合节点；Meddle 只确认字段与 `g_SphereMapIndex` 适用 family。现有经验 rim 已删除，在取得游戏 shader/节点证据前保持 unsupported。

这些贴图已经进入 shader binding并提供 debug view；tile/ORB 通道已按节点证据收敛，sheen/sphere ramp 用 synthetic WGPU fixture 证明数据与诊断链路存在、但 Final 不被未经验证的公式改变。detail 最终 influence 与 reflection/sphere 的真实游戏公式仍需更多证据。

验证：

- tile 已有真实/synthetic 选层、TileMatrix 和 ORB 通道验证；sheen/sphere 则由 synthetic fixture 锁定 active/neutral Final 相同且 unsupported 诊断可区分。SphereIndex focused test 已覆盖 raw `0x4000 -> semantic 2.0 -> baked R=2/255`，45059 真实回归也确认不再把该行烘焙成 R=1。
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

当前进度：数据层 `PreparedTextureSamplingSet` 已由 renderer 逐 role 执行。material bind group 的 15 个现有 texture binding 均有独立 sampler：base、normal、mask、emissive、material-properties、specular、tile/sheen/sphere/tile-matrix、index、material/multi map、tile/detail pair。WebGPU 基线每 shader stage 至少支持 16 samplers，当前使用 15 并保留一个预算；TileMatrix 使用 non-filtering sampler，bguvscroll 复用 extra binding 时 sampler policy 随 secondary color/normal/specular role 切换。skin face 的 base clamp 不再影响 emissive。

仍需要按 texture role 完整落地：

- base/emissive: sRGB；specular: Non-Color。当前 specular 已按 MeddleTools `g_SamplerSpecular` / `g_SamplerSpecularMap0` 配置使用 `Rgba8Unorm` mip 与 data sampler
- normal/mask/material/multi/material-properties: Non-Color；当前 renderer 已用 data sampler 采 normal/mask/material-properties，并在 material/multi debug view 中用 data sampler 预览 material/multi
- index: nearest/closest，renderer 已绑定 `_id.tex`，正常着色通过 row index 选择 ColorTable diffuse/specular/material/tile/sheen/sphere/tile-matrix extra properties，debug view 也使用同一 nearest policy
- tile/detail arrays 与 ColorTable extra maps: nearest 或 shader-family-specific；当前 renderer 已用 nearest sampler 消费 ColorTable extra maps 和两个 pair atlas，共享 arrays 按 Non-Color + nearest + repeat 采样
- decal: clip/extend 语义；decal/crest 已确认为 runtime-only on-render texture，并有透明 fallback 元数据，当前尚无显式 runtime texture 输入和独立 GPU binding

`Clip` address policy 仍只能在 sampler descriptor 层降级为 clamp，后续 decal 或 face override 仍需要 shader 级 UV clip/extend 行为。runtime decal 会需要第 16 个 sampler 或 layout 重排，当前未提前占用。

验证：

- 已有 prepared texture sampling 测试覆盖 `_id.tex` nearest policy，避免颜色边界被 linear 混合污染。
- renderer 已用 `Rgba8Unorm` 创建 normal/mask/material-properties；ColorTable extra maps 使用各自 nearest sampler 并提供 debug view；全部 15 个 sampler descriptor 均由对应 prepared role policy 派生。focused test 固定 base clamp、emissive repeat 和 index nearest，实际 Vulkan WGPU snapshot 验证 15-sampler pipeline 可创建和渲染。

### P2: 视觉验证和调试视图

UI 和 snapshot/test render options 已加入第一版 debug render mode：

- base color
- normal
- mask/material properties
- specular
- emissive
- alpha
- UV set preview
- vertex color preview，包括 color0 与 color1
- secondary normal、flow0、flow1 preview；四种 secondary/flow debug mode 已贯通 renderer、Web 选择器和 URL round-trip。synthetic WGPU fixture 为 `color1`、secondary normal、flow0、flow1 固定不同值并逐对验证最终 PNG RGB difference `>10,000`。Meddle 只证明这些是 usage-indexed 顶点数组，MeddleTools 没有足够节点证据把 `color1/flow1` 接入最终公式，因此当前只用于直接审计，不伪造材质行为
- mesh category / draw role colors
- ColorTable row index preview
- material map / multi map preview
- ColorTable tile / sheen / sphere / tile-matrix ramp preview

仍待补：

- per-texture independent sampler binding 已完成；sampler policy debug preview 后续按需要补充
- 已完成 tile normal/ORB、detail diffuse/normal array preview；detail 仍需真实 bg 武器样本校准

这些视图能显著缩短后续对照 Meddle/MeddleTools 的定位时间。

## 建议实施顺序

### 当前下一批工作队列

从当前状态继续推进时，优先级应按依赖关系排：

1. shape morph 已完成显式离线路线：MDL shape value 已提升为 sparse position/normal delta；submesh remap 会按 index-buffer position/target signature 拆点；多个 active shape 按 glTF/Meddle 权重 1 语义相加；renderer 新入口接受显式 `PreparedModelOptions`，默认入口继续无 mask/base geometry。focused tests 覆盖单 index occurrence 拆点和双 target additive delta；全 WeaponCatalog 7365 个模型中有 33 个含 shape，`42697 新生王国指虎` 的 `shp_arm`/1364 raw values 已通过真实 loader 回归。标准 native snapshot harness 现也接受 preparation options；42697 base/bit0 同视角 PNG 的 RGB difference 为 18,550，并以 `>5,000` 断言验证 GPU 输出。Web canvas 已在模型含 shape 时显示 Base/shape-name 下拉选项，选择后按 table-order mask 重建 renderer；资源请求/染色缓存不包含 shape，切换物品时旧 mask 会按 item id 与 available mask 自动失效。helper tests 覆盖名称去重/排序、mask validation 和 canvas key 变化。后续只在调用方能提供 runtime `ShapeMasks` name/id 表时替换当前明确标注的 table-order bit 约定。

2. 数据解析：WeaponCatalog shader-family audit 已确认当前武器材质为 character/skin/characterGlass；23 个 equipment-style 拳套已通过默认 c0101 glove + human body material fallback 恢复，并保留 race/skeleton/SkinColor 缺失诊断。`w0371/v0201` 的 PAP hash collision 已通过 typed candidate validation 绕过；`w3054 -> mt_w3103` 已确认为客户端悬空引用，并只复用同一请求中已加载主模型的同 index 材质。完整审计 failures 已归零，collision/unresolved reference 保持独立可审计；下一步继续 shader-family-specific texture role/UV 规则。`characterscroll.shpk` 专属 material key `0xF886E10E` 已完成结构化保留：Meddle 只确认默认值 `0x69EB4AE0` 与另一值 `0x9A8A46F5`，两者都没有名称；MeddleTools 则把该 family 直接映射到普通 `character.shpk`，没有可采信的 scroll 节点或公式。同步/异步 loader 现都会从 composed SHPK default/MTRL override 解析两种已知 hash，并为未知值保留 raw `u32`；phantom material summary 会输出 variant/raw，prepared unsupported 也有独立 `characterScrollVariant` 标记。`GetDecalColor` mode/raw、Skin runtime decal diagnostics、`GetValues` raw 与 AlphaMulti 显式 unsupported 都已完成同样的数据闭环；focused tests 覆盖 absent/default/override/unknown 和 prepared 传播。通用 incomplete-family 标记继续保留，WGSL/UV 动画和 decal/AlphaMulti 混色没有无证据改动。runtime GPU ColorTable 继续只作为 unsupported 输入标记。
3. 结果处理：Crystal/Environment 的“已解析、未渲染”状态已进入明确 prepared family/feature/unsupported 字段；`multiMapInterpretation` 也已区分共享 detail array 缺失与 MultiMap 通道未实现。共享 array 的 binding/layout/pair status 与 layer count 已从 renderer 前置到 prepared，非法资源不再只静默回退；Dawntrail TileIndex/SphereIndex 均已从 raw half bits 解码后进入语义 bake，TileIndex 的 `half * 64` 与本仓 `/64` bake 继续证明 WGSL 的 `TileProperties.r * 64`，SphereIndex 则由 45059 的 `0x4000 -> 2.0 -> R=2/255` 验证。TileMatrix 已按 UU/UV/VU/VV 顺序使用未 clamp float GPU texture，并只保留节点证明的 UV 变换；detail/multi-detail 已从固定叠加改为节点证明的 MultiBlendWeight A/B mix，ORB Blue darkening 与 normal-alpha × TileAlpha 权重也已按 `chara_detail_blend` 对齐。普通材质纹理 mip chain 与 character `g_TextureMipBias` 已贯通。`g_AmbientOcclusionMask (0x575ABFB2)` 也已独立保留：Meddle 显示它只属于部分 character family 且无默认值，不能与跨 family、默认 1 的 `g_SSAOMask` 合并；MeddleTools 没有对应节点。同步/异步 loader 现解析可选 finite scalar，phantom summary 输出原值，prepared unsupported 仅在值实际存在时标记；focused test 覆盖 absent/SHPK default/MTRL override/non-finite。进一步 installed audit 确认 `g_SSAOMask` 有 4 个非默认资源/9 次引用，但仍无 runtime SSAO buffer 或节点公式；旧的 `mix(0.45, 1.0, ssao)` ambient/environment 经验乘法已删除，两个 AO 字段现在都只进入结构化 unsupported 诊断。完整 MultiMap influence 继续等待证据。
4. 渲染器：characterTransparency/glass、dither depth、GlassBlend、outline、toon、bguvscroll Map0/Map1、Flow、stockings opaque alpha/pipeline、tattoo normal-A alpha、water direct alpha/deep color/primary wave 与 lightshaft 双纹理 emission/alpha 已完成第一版。tattoo 的 OptionColor/DecalColor 混色仍不猜测。secondary color/normal/specular 在 `BgUvScroll + GetMultiValues` 中复用 tile/sheen/sphere 物理 binding，按 UV1Scroll 和 vertex alpha 混合；lightshaft 只复用 secondary color binding，并由 vertex color B 驱动 Multiply blend，不启用 Map1/scroll 规则；sampled texture 数仍为 15。Map1 normal 现已使用 secondary tangent frame：WGSL vertex output 会传递 `normal1/bitangent1`，fragment 分别把 Map0/Map1 normal 转到各自世界空间后再按 vertex alpha 混合；缺失通道沿用既有 CPU primary fallback。synthetic WGPU fixtures 已覆盖 secondary frame 与 lightshaft Sampler1/alpha-test 最终像素差异。Meddle 按 usage index 成组保留 `Normals[]/Binormals[]`；`color1/flow1` 继续等待 family-specific 证据。lightshaft Type/AngleClip/NearClip 已完成数据与诊断闭环。LightShaft Final rendering 现使用独立 `fs_lightshaft` 及双面/剔除 additive pipelines，只执行双纹理、emission、alpha-test 路径；非 Final debug mode 继续复用 `fs_main`，保持全部诊断视图。两组 synthetic WGPU lightshaft fixtures 已在专用 entrypoint 上通过。characterReflection generic approximation 与 stockings/tattoo/occlusion runtime 输入均已有独立 diagnostic；后续再寻找真实 reflection 节点/样本。
5. runtime 输入：默认 crest/decal 透明 fallback 与 materialChange 基础材质 fallback 已执行；后续只在调用方能提供真实 on-render texture 时增加显式输入，不从静态 MTRL 伪造。
6. 验证：45052 第二通道/metallic 染色 case 已补齐，并与 baseline/stain0 做 application row count 和像素差异断言；weapon scope 已确认无 bg detail 样本，相关权重等待游戏 shader 或非武器 fixture。继续为 transparency/glass/scroll 等行为增加 synthetic 与真实 snapshot，并用 catalog audit 防止真实加载缺口被小型 phantom 集合漏掉。

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

1. 已完成 GPU 顶点格式扩展；prepared UV source、per-role scroll、Map1/UV1Scroll、Flow primary tangent，以及 bguvscroll Map1 的 secondary normal/bitangent frame 已进入 WGSL。后续让 shader-family 逻辑继续消费其它 `uv1-uv3` 用途、`color1` 和 `flow1`。
2. 已完成 per-material texture/sampler config，renderer 已为 15 个现有 texture binding 派生独立 sampler；共享 tile/detail arrays 已进入两个 GPU pair atlas、选层采样与 debug view。后续补 shader 级 clip/extend 和 runtime decal binding。
3. ColorTable diffuse/specular/material-properties/tile/sheen/sphere/tile-matrix 与共享 tile/detail arrays 已进入 renderer；`g_DiffuseColor`、verified BG secondary diffuse、`g_EmissiveColor`、tile normal/ORB 已按现有证据消费。Outline、`g_SpecularColorMask`、alpha aperture/offset、`g_SSAOMask`、Sheen/Sphere 和 glass IOR/thickness 保留数据/uniform/debug/diagnostic，但不再使用无证据 Final 公式。ORB/tile-normal 与 `g_TextureMipBias` sampler scope 已按节点或 installed DXBC 证据收敛；后续补 shader-family-specific source/scroll 和 detail/multi-map 的准确 influence。
4. 已完成第一版 shader family 分类和 alpha policy/prepared pass 分类；lightshaft Final 已拆成独立 `fs_lightshaft` pipeline，debug 继续复用主 shader。后续继续把 character/glass/transparency/scroll/reflection 等 family 的关键节点拆成明确 WGSL 函数块，而不是扩大单个主 shader 分支。

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
## 2026-07-21 Alpha aperture/offset DXBC 结构化审计

- `g_AlphaAperture/g_AlphaOffset` 不再被笼统描述为“没有公式”。installed SHPK 审计现在锁定 `log → mul aperture → exp → mul offset` 指数链，并把后续 `mul_sat(..., 3.333333)`、`mad_sat`、offset 符号 gate 与 `alpha < 1` gate 分开计数。
- `character.shpk` 的 768 条指数链和 `characterglass.shpk` 的 34 条全部进入上述 alpha 组合。最初固定 18 行的诊断窗口曾把 72 条 Character 链误分成其它 consumer；下游寄存器追踪证明这些 permutation 只是在指数链和 `mad_sat` 之间插入了更长的 Table/材质处理，不是不同语义。正式审计现在保留未分类/死链计数并锁定二者均为 0。
- `characterlegacy.shpk` 和 `skin.shpk` 没有 aperture consumer，但仍分别有 2/3 条 offset use，不能仅凭 offset 出现就泛化该 alpha 公式。
- 对 shaping dot 的来源追踪显示，Character 768/768 条、Glass 34/34 条均可直接追溯到归一化的 `-v6` 输入。先前未分类的 6 条 Character permutation 使用 `mov_sat |dot|`，与其余 shader 的 `min(|dot|, 1)` 等价，不是不同视线来源。当前 renderer 的 `resolve_view_direction` 使用 world position 与 preview camera position；在未审计对应 vertex shader 输出前，仍不能仅凭 `-v6` 寄存器名宣称空间完全等价。
- Glass 的 `mad_sat` destination 也不总是指数链所在 register，Character 的 `3.333333` 缩放输入和最终 alpha product 亦存在不同寄存器形态。这些事实证明当前 preview 的 normal/view/alpha source 还不能声称逐 permutation 等价。因此 WGSL 保持不消费 aperture/offset，`alphaShaping` unsupported 与“override 不改变 Final”的 native boundary fixture 继续有效。
