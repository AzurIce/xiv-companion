# 武器模型渲染管线调研与设计

本文记录 `xiv-companion` Web 武器预览从游戏数据到 GPU 渲染的完整管线、已知坑点、当前实现策略和后续扩展方向。调研参考了本地游戏 SqPack、Physis、Meddle 运行时导出逻辑，以及实际问题样本：逗猫之幻梦、浪漫之幻梦、冬雪之幻梦、绝境系列双手武器等。

> 2026-07-22 alpha shaping update: installed SHPK pass pairing proves that all
> Character 64/64 and CharacterGlass 8/8 vertex shaders feeding aperture/offset
> pixel shaders write `TEXCOORD4` from the same pre-projection position used by
> the 22..25 clip-position matrix rows. Pixel shader `normalized(-v6)` therefore
> represents the surface-to-camera direction. Aperture/offset remain diagnostic
> only until the shaping normal and both alpha operands are mapped exactly.

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

`PreparedMaterial.runtimeInputRequirements` 把缺失输入按真实 owner 分组：`characterInstanceState` 覆盖 customize/instance 的 Skin/Option/Decal/SubColor，`onRenderMaterialOutput` 覆盖 decal/crest 与 slot skin-material resolution，`gpuColorTableTexture` 表示静态 MTRL/stain bake 之外的 CharacterBase GPU override，`resolvedResourceHandles` 表示 on-render texture/material handle。当前 local/Web JSON 调用链没有这些 live provider，因此该结构只建立可审计契约，不增加空值 GPU binding；现有 unsupported 细项以及 transparent/base fallback 保持权威。

`PreparedModel.runtimeGeometryRequirements` 同样区分静态 payload 与 live state：含 shape 时始终报告缺失 runtime name/id mapping，未提供 option 时分别报告 enabled shape/attribute mask；bone table 与 blend weights/indices 同时存在时报告 skeleton pose 和 skinning matrices，`chara/equipment/` skinned mesh 还报告 race deformer。显式 mask 会满足对应 requirement，但不会把 table-order shape bit 宣称为 runtime id；renderer 在没有 live bone/PBD provider 时仍保持静态 base pose。

注意：renderer 当前 GPU 顶点格式已上传 position、normal、uv0-uv3、bitangent、color0/color1、secondary normal/bitangent、flow0/flow1，WGSL `VertexInput` 也已声明对应 location；`PreparedMaterial` 已记录 UV source、per-role scroll mask 和结构化 flow mode。只有 `CategoryFlowMapType=Flow` 且 mesh 有 `flow0` 时 `usesFlow` 才启用，fragment shader 会把 `flow0.xyz` 正交化后作为 primary normal tangent；`BgUvScroll + GetMultiValues` 的 Map1 normal 会使用 `normal1/bitangent1` 构造 secondary frame，缺失时 CPU 顶点 fallback 回落到 primary frame；`characterstockings.shpk` 普通 surface 会按最终 alpha=1 选择 opaque pass，但 runtime skin material 仍不可用并由 `runtimeSkinMaterial` 标记；`charactertattoo.shpk` 按 MeddleTools 节点从 normal texture Alpha 取透明度，颜色所需的 OptionColor/DecalColor 仍分别由 `runtimeOptionColor`/`runtimeDecalColor` 标记；`skin.shpk` 的 `GetDecalColor` 已保留 Off/Alpha/RGBA/Unknown mode 与 raw value，任意非 Off mode 会标记缺失的 runtime DecalColor/DecalTexture 和未实现 mode，均不使用静态猜测。installed 武器中唯一 Skin 资源为 Body + DecalOff 且仅绑定主 Diffuse/Normal/Mask；reflection/occlusion/scroll/stockings/tattoo 均无武器样本，相关输入继续作为 runtime/evidence boundary，而不是通用 character 公式的隐式别名；`flow1` 和 color1 仍待后续 family-specific 逻辑。

注意：顶点色在 FFXIV shader 里不一定是纯颜色，常参与遮罩、alpha 或 family-specific 材质调制。MeddleTools 只证明 `ApplyVertexColor` 的 On/Off mapping，没有证明通用 RGB 乘法；因此 renderer 保留 color0/color1、`usesVertexColor`、uniform 和直接 debug，但不再让 `base * color0.rgb` 静默改变 Final。启用该 key 时 prepared 报告 `vertexColorComposition` unsupported。已验证的专用路径保持独立：BG/BGUvScroll `GetMultiValues` 使用 color0 Alpha，LightShaft 使用 color0 Blue/Alpha。

同样，`g_SpecularColorMask` 只保留解析、uniform 和 debug。Meddle/MeddleTools 没有证明它在当前通用 surface 中的 RGB 或额外 scalar 组合；非默认值由 `specularColorMaskComposition` 标记，不再静默乘入 Final specular。installed Character/CharacterLegacy/CharacterGlass/Skin 资源全部为默认 `[1,1,1]`。

`characterglass.shpk` 的 installed DXBC 不是只有一个 alpha 开关：38 个 pixel shader 中 34 个实际采样 Index/Normal/Table/TileNormal/TileOrb，24 个采样 Mask/ReflectionArray/SphereMap，19 个采样 Dissolve/Dissolve1，8 个采样 DepthWithWater/ViewPosition/Sky；31 个 shader 含 discard，32 个写 output alpha，其中 16 个固定写 1、16 个使用动态 `mad o0.w`，normal sample 均未请求 `.w` destination。45059 的静态 MTRL 只能固定三项 material key，其余 scene/subview key 属于 runtime permutation，离线数据无法唯一选择最终 PS。MeddleTools 将该 package 映射到 character 节点组，没有 glass-specific downstream node，也没有证明这些额外采样如何组成反射、溶解、深度或折射。因此 renderer 继续使用明确标注为 preview fallback 的基础 alpha/glass pass 与 `glassShaderParameters` unsupported 诊断，不把这些资源静默接入 generic PBR。

`g_OutlineColor` / `g_OutlineWidth` 也只由 Meddle 提供名称与零默认值，MeddleTools 没有 outline node 或几何公式。prepared 继续保留 `usesOutline` 能力和 outline uniform，renderer 也保留独立 front-cull pipeline 供未来有证据的调用方使用；静态材质出现非默认值时会设置 `outlineComposition`，正常 Final 不提交当前 `normal * width` 世界空间外扩。installed 四个 SHPK 全部为零、零 override。

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

renderer 会把 tile normal/ORB 横向合并成一张 GPU pair atlas，把 detail diffuse/normal 合并成另一张，避免四个独立 binding 使 fragment sampled texture 数超过常见 WebGPU 16 张限制。每级 mip 都按 half、array layer 独立缩小，禁止 normal/ORB、diffuse/normal 或相邻 layer 互相平均；normal half 从 RG 重建 Z、平均并重新编码 RG，B/A 作为独立线性 payload 保留。WGSL 保持 nearest texel 与 nearest mip，并在 `fract` 层内 wrap 前计算 UV 梯度，再按 atlas half/layer 比例缩放后交给 `textureSampleGrad`，避免高频 UV 已混叠后才隐式求导。选层规则如下：

- character tile：ColorTable 路径按每个 index texel 相邻保存 A/B 两套 TileIndex、TileAlpha 和 TileMatrix，并保留原始 index texture G；installed character/legacy DXBC 先采样偶数 A 行、再采样奇数 B 行，并使用 `1-G` 作为从 A 到 B 的基础插值因子。A/B 分别按 MeddleTools `tile_select` 的 `FLOOR` 离散到 layer，分别结合 TileMatrix 与 `g_TileScale` 生成层内 UV、计算 LOD、采样 Tile Normal/ORB 并旋转 tile normal，最后才混合采样结果。modern `character.shpk` 的已审计 permutation 会再用 ColorTable material-properties roughness 与 primary world-normal/view 项整形该 weight；renderer 只在 generic A/B ramp、Tile A/B ramp 和 modern package gate 同时成立时启用该分支。没有 ColorTable tile map 时回退 `g_TileIndex` / `g_TileAlpha` 与 identity TileMatrix。

modern shaping 的 native regression 使用斜视角平面和相同的 Tile A/B 数据，只改变 baked specular alpha（ColorTable Anisotropy）。installed audit 的 768/768 个 shaping permutation 都从偶数 A 行、Table texel 4 的 W 通道读取值；Meddle 的布局将该通道定义为 Anisotropy，因此非对称 A/B fixture 还会固定 B anisotropy 不影响 shaping。`character.shpk` 的 low/high A anisotropy 输出必须有方向性差异，而 `characterlegacy.shpk` 对照输出必须一致。该测试验证的是已安装 DXBC 公式的权重消费，不代表 renderer 已复刻完整角色 TBN/flow 输入。
- bg detail：`g_DetailID` / `g_MultiDetailID` 取整并 clamp 到数组范围，color/normal 分别使用对应 UV scale；GetMultiValues 按 vertex alpha 在 primary/multi 层之间插值，Single 固定 primary。该采样结果只进入 `DetailDiffuseArray` / `DetailNormalArray` debug view。

tile normal 会与 primary tangent-space normal 组合，贡献权重为采样 normal Alpha × `TileAlpha`。Tile ORB 先按 `neutral + TileAlpha * (sample - neutral)` 混合，其中 neutral 为 `(1, 0.5, 1)`；因此 TileAlpha 会把 ORB Blue 从中性 1 推向采样值，但仍不是材质透明度。MeddleTools `chara_detail_blend` 的最终已连接消费者只使用有效 ORB Blue，将其作为黑色到 base color 的直接 darkening factor；ORB R/G/Alpha 不参与着色，不再猜测 AO、roughness 或 specular。detail normal 使用 RG 重建 Z，再按 detail/multi-detail normal scale 处理，随后按 MultiBlendWeight 选择 primary/multi，仅用于直接 debug。detail 对 base/normal 的最终 influence 未被 MeddleTools 证明，BG/BgUvScroll 的 prepared 结果会设置 `unsupportedInputs.detailComposition`；Final 使用中性 detail，不再引入固定权重或程序化波形。model-level prepared 只有在数组实际验证为 Ready 时才清除 `unsupportedInputs.tileArray/detailArray`；renderer uniform 直接消费 prepared layer count/ready flag，GPU pair atlas 创建仍保留防御性验证并在失败时使用中性纹理。45052 的真实 summary 为 tile Ready/64 layers、detail MissingBindings。

验证边界：CPU focused test 会固定每个 half/layer 的 mip 像素并验证 normal B/A；synthetic native WGPU checker 与预平均 atlas 的最终像素完全一致。另一组 native fixture 固定 layer 21/22 差异，验证 RGBA8 TileProperties `86 / 255 * 64 ≈ 21.58` 与 `g_TileIndex=21.75` 均 floor 到 layer 21。detail synthetic fixture 现在要求 primary/multi debug 输出不同、Final 输出逐像素一致，并检查 `detailComposition` 诊断色。45053/45068 已重跑 final/tile-normal，45068 的前景高频分别下降约 53%/94%，45053 tile-normal 下降约 71%；45050 已重跑 packed normal alpha 路径，45052 继续用 final/tile-normal/tile-ORB snapshot 验证逐像素选层。当前没有真实 bg 武器样本，detail 最终 influence 继续等待样本校准。

本地或带 SqPack 的 self-hosted runner 可执行 `pwsh -NoProfile -File scripts/verify-weapon-render.ps1 -GameDir <path>`，统一运行 full installed audit、fixture 中动态选择的 P0/P1 phantom、workspace、wasm32、fmt 与 diff。脚本限制 Cargo 为单 job 以控制 Windows WGPU/Web 链接内存峰值；普通 hosted CI 没有游戏数据时不伪造或静默跳过真实资源门禁。

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
   - canonical baked diffuse 同时保留兼容 sRGB RGBA8 与未 clamp 的线性 float A/B payload，Alpha 固定不透明；GPU 优先上传 `Rgba16Float`，避免 installed Diffuse 最大值 `6.7929688` 被截断。Compatibility 的 `base × ColorTable diffuse` 会先将 source base 从 sRGB 解码，在线性空间乘以 float colorset，再同时保存 HDR float 结果与 byte fallback。`TileAlpha` 是 tile 属性，不作为材质透明度。
   - canonical baked specular ramp 同时保留兼容 RGBA8 和 float payload；GPU 优先上传 `Rgba16Float`，RGB 直接保存线性 ColorTable Specular，Alpha 保存未 clamp 的 `Anisotropy`。这避免 installed 最大值 `7.0` 被 RGBA8 UNORM 截成 `1.0`，并让 modern ColorTable blend shaping 读取真实 A 行指数。原始打包 `g_SamplerSpecularMap0` 仍按 MeddleTools 作为 `NonColor` RGBA8，且其 Alpha 不会被误用为 anisotropy。普通 PBR anisotropy 在 BRDF 使用处仍夹到 `[0,1]`。
   - canonical baked emissive ramp 同样保留兼容 sRGB RGBA8 和线性 float payload；GPU 优先上传 `Rgba16Float`，避免 installed ColorTable Emissive 最大值 `61.46875` 在进入 HDR scene 前被 UNORM 截断。普通 byte-only/source emissive 仍走原有 sRGB mip 路径。
   - native test-support 可直接回读 pre-compose HDR scene；BaseColor、Specular、Emissive 与 SheenProperties debug 不在 scene pass 把 HDR 数值夹到 1，因此测试可将 GPU 采样结果与源 ColorTable 线性值逐像素比较，最终 canvas 仍正常经过 tone mapping。installed audit 还会拒绝所有进入 `Rgba16Float` ramp 的非有限或绝对值超过 half-float 最大有限值 `65504` 的字段。ColorTable A/B ramp 的主路径使用 `textureLoad(..., mip 0)` 分别取相邻 A/B texel 再显式混合，因此不会把 A/B 边界交给隐式过滤/mip；float ramp 保持单级是这一语义的刻意结果。
- material-properties 同时保留兼容线性 unorm 与未 clamp 的 float A/B payload，通道为 metalness / roughness / gloss strength / specular strength；canonical baked ramp 以 `Rgba16Float` 上传。installed 值域中 Metalness/Roughness 为 `0..1`，GlossStrength 最大 `193.375`、SpecularStrength 最大 `100`，因此 RGBA8 不能作为真实数据源。
   - tile/sheen/sphere/tile-matrix 与 MeddleTools 的 extra ramps 对齐；其中 sheen ramp 第三通道是 ColorTable `SheenAptitude`，与材质常量 `g_SheenAperture` 不同。Dawntrail raw TileIndex/SphereIndex 均先从 half bits 解码，再分别按 0..64/0..255 写入 UNORM。low-level material debug 仍保留原始 `u16`。tile properties 与 tile-matrix 为每个源 index texel 相邻保存 A/B 两套值；tile-matrix 的 UU/UV/VU/VV 同时保留 float channels，并以双倍宽度 `Rgba32Float` 上传，避免 RGBA8 截断负 skew 和大于 1 的 repeat。
4. 若 ColorTable 有 emissive，则额外启用 emissive texture；canonical baked 路径优先使用上述 float payload。
5. MeddleTools 的 character 节点只在当前 `GetValues=GetValuesCompatibility` 或旧式 `GetValuesTextureType=Compatibility` 时启用 `g_SamplerDiffuse × ColorTable diffuse`；同步/异步 loader 因此仅对映射到 character 节点的 family 在该 gate 命中时生成 `#base-times-colorset`。MultiMaterial 使用 `#colorset-diffuse`，原 diffuse 仍保留在 raw texture indices。installed `character.shpk` 的成对 node/FXC 审计进一步确认代表 surface PS：MultiMaterial 不声明 Diffuse resource，Compatibility 在相同 ColorTable/normal/mask 管线上新增并采样 Diffuse。武器实际覆盖没有 AlphaMulti、MultiMap sampler 或 bg/detail family，因此这些未连接/未知公式继续只保留 raw 与 unsupported。

当前实现同时支持 Dawntrail 32 行和 Legacy 16 行 ColorTable。renderer 已消费 diffuse/base、specular、material-properties、emissive，并已把 tile、sheen、sphere、tile-matrix extra maps 绑定进 WGSL；tile properties 还会驱动共享 tile normal/ORB atlas 的逐像素 layer selection。specular-ramp Alpha 现在按 MeddleTools 的 Principled Anisotropic 连接进入 tangent-oriented anisotropic GGX NDF；零值严格回退现有 isotropic GGX，缺失 tangent frame 时也保持 isotropic。该分布是预览 PBR 近似，FFXIV 的精确 lobe、rotation 与 visibility 项仍未知。installed audit 覆盖 6394 个带 ColorTable 的资源，其中 113 个资源/170 次引用含非零 anisotropy，原始值域为 `0..7`；canonical baked float payload 会完整保留该值，普通 PBR 分支只在 BRDF 使用处夹到 `[0,1]`，modern shaping 则读取原始 A 行指数。native fixture 会旋转同一表面的 tangent frame，验证零值旋转不变、非零值产生方向响应。45059 真实材质包含 raw `SphereIndex=0x4000`，语义 half 值为 `2.0`；修正后的 sphere-properties 贴图平均 R 为精确的 `2/255`，验证不再把 raw bits 错误烘焙为饱和 1。TileMatrix binding 使用 unfilterable `Rgba32Float` 与 non-filtering nearest sampler，float payload 无效时逐通道回退 RGBA8/identity；Blender `tile_select` 证明它只形成 tile UV vector，WGSL 已删除旧的 matrix-delta specular。互补 synthetic fixtures 证明 patterned tile 在 scale 1/2 下输出不同，而 uniform tile 在同样矩阵变化下逐像素一致。MeddleTools character 图中的 Sheen/Sphere ramp 只进入无下游消费的 mix-group 接口，因此 renderer 保留它们的 float payload、binding 和独立 debug view，但不再让经验高光/rim 公式改变 Final。installed audit 锁定非零 SheenRate 为 857 个资源/1335 次引用、非零 SphereMask 为 121 个资源/183 次引用；prepared 分别输出 `sheenLighting` / `sphereLighting` unsupported，native fixture 验证 active/neutral Final 完全一致且诊断色可区分。

## 7. 材质模式

当前抽象为三类：

```rust
WeaponMaterialRenderMode::Opaque
WeaponMaterialRenderMode::Transparent
WeaponMaterialRenderMode::Glass
```

### 7.1 Opaque

普通角色/武器材质。使用 base/ColorTable、normal、mask、emissive，经相机感知的 GGX/Fresnel、半球环境光与程序化 studio environment 做预览近似；这不是游戏 shader 的逐式复刻。
`g_NormalScale` 会从 shader package default 与 material override 中解析为 `normalScale`，并用于缩放 tangent-space normal map 强度。
`g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale` 也已解析进材质数据和 renderer uniform；detail pair atlas 仍可在 debug view 中采样真实 detail normal，但由于 MeddleTools 未证明其最终 influence，detail scale 不再改变 Final primary normal。
双面材质的 shading normal 朝向不依赖三角形 winding（`front_facing`）：WGSL 用 vertex normal 与相机视线的几何关系把几何法线翻向观察者（`orient_geometric_normal_toward_viewer`），并据此重建 tangent frame，保持翻转时 tangent-space handedness 一致；几何、primary/secondary TBN、tile array 与 flow-based Final 路径都继承该朝向。这修复了导入顶点法线与 winding 不一致时双面材质两面都发灰的问题；synthetic WGPU fixture 会从正反两个方向渲染同一双面平面，与 winding/法线一致的参照面比较 luminance 与 specular 响应。
`g_TileIndex`、`g_TileAlpha`、`g_TileScale` 已解析进材质数据和 renderer uniform；WGSL 已绑定 tile pair atlas，按 ColorTable A/B tile properties 或 `g_TileIndex` 选层，并结合各自 TileMatrix/TileScale 采样。ColorTable 的有效 A/B weight 为 `1-index.G`；`TileAlpha` 与 tile-normal Alpha 相乘控制法线贡献；ORB 则从中性 `(1, 0.5, 1)` 按 TileAlpha 混向采样值，因此会影响 ORB Blue 的 base-color darkening，但不作为材质透明度。modern `character.shpk` 的 roughness/view-dependent shaping 已在离线 renderer 实现，并通过独立的 generic-ramp/tile-ramp 能力位避免影响 legacy 或 tile-only fallback。

character/legacy/glass 的 surface alpha 还消费 material constant `0xAD94E254`。Meddle 将其列为 unknown，因此数据模型使用描述性字段 `vertexAlphaToOne`，不声称这是游戏正式名称。installed DXBC 对所有消费者都执行 `adjustedVertexAlpha = mix(vertexAlpha, 1, value)`，随后乘入 texture/surface alpha；代表 character permutation 明确是 Normal Blue × adjusted vertex alpha 并最终写入 `o0.w`。renderer 只对 `character.shpk`、`characterlegacy.shpk`、`characterglass.shpk` 启用该 remap。45050 透明毛的值为 `1`，默认材质为 `0`；tattoo、skin 等没有相同 DXBC 证明的 family 不继承该行为。
`g_ToonIndex`、`g_ToonLightScale`、`g_ToonLightSpecAperture`、`g_ToonReflectionScale`、`g_ToonSpecIndex`、`g_SheenRate`、`g_SheenTintRate`、`g_SheenAperture`、`g_SphereMapIndex` 已解析进材质数据、phantom summary 和 renderer uniform；默认值分别为 `0/2/50/2.5/约 0/0/0/1/0`。Meddle 只证明 Toon 常量名称、适用 family 与默认值；MeddleTools 源码和 `shaders.blend` character group 没有 Toon socket、node、texture 或 mapping。installed audit 对 character/legacy/glass/skin 的五个 Toon 常量覆盖 6399 个材质资源，所有 MTRL non-default override 都为 0。因此 WGSL 已删除黄金比例 phase、35% diffuse band、40% spec band 和 reflection scale 等经验公式；通用 PBR 直接消费一次 NdotL，并使用稳定的 `PREVIEW_DIRECT_SPECULAR_SCALE`。默认 Toon 值只保留在数据/uniform 中，未来非默认值触发 `toonLighting` unsupported，不改变 Final。Sheen/Sphere 常量同样只保留供审计，非零值分别触发结构化 unsupported。
`g_DetailID`、`g_MultiDetailID`、`g_DetailColor`、`g_MultiDetailColor`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 已解析进材质数据和 renderer uniform；WGSL 已绑定 detail pair atlas，按两个 ID 选层并分别采样 diffuse/normal。`detailParams.z` 记录 bg GetMultiValues，启用时用 vertex alpha 统一 mix primary/multi debug sample；synthetic WGPU fixture 已验证 alpha 0/1 切层。由于 MeddleTools 明确标记 terrain detail influence 为 borked，prepared 会设置 `detailComposition`，Final 不消费 detail tint/normal。
`g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_MultiEmissiveColor` 已解析进材质数据、phantom summary 和 renderer uniform。WGSL 把 `g_DiffuseColor` 作为 generic base tint；只有 `BgUvScroll + GetMultiValues + secondary base` 使用 MeddleTools 已证明的 vertex-alpha Map0/Map1 mix 消费 `g_MultiDiffuseColor`。其它非默认 MultiDiffuse 与所有非默认 MultiEmissive 输出 `multiColorComposition` unsupported，不再使用 generic mask-R/固定权重公式。ColorTable emissive 使用逐像素 baked texture，`emissiveColor` 行摘要只在贴图缺失时作为 fallback，避免同源数据重复相加；`g_EmissiveColor` 直接作为附加发光。installed audit 当前没有 BG/Crystal multi-color 常量覆盖；ColorTable emissive 覆盖 6397 个资源，其中 2811 个资源/5482 次引用含非零值，最大通道值为 `61.46875`。
SHPK semantic debug 会原样保留 exact package 的 material/system/scene key defaults，以及每个 material parameter 的 byte offset/size 和可选 default array；MTRL override 之后再形成 resolved value。WeaponCatalog audit 对唯一 MTRL 资源与 catalog item-material 引用双计数，并单列 unknown、malformed、non-finite、unresolved、shader flags 和代表样本。无 SHPK defaults 的参数不会被误判为 override-only，duplicate MTRL constant ID 以最后一个可解析记录作为有效值，同时保留 malformed 原始诊断。

Sampler semantic 同样以 exact SHPK resource 为准：debug 会输出 resource name/CRC/slot/size、MTRL flags、logical role 和底层 texture kind；WeaponCatalog audit 按 `SHPK + resource CRC + flags` 对唯一 MTRL 与 catalog 引用双计数。主 BaseColor/Normal/Mask/Index 保持 UV0，bguvscroll Map1 保持 UV1Scroll。`g_SamplerSkinDiffuse/Normal/Mask` 不再折叠进主槽，而是保存为独立 `ModelMaterial`/prepared binding，sampling policy 分别沿用 sRGB base、non-color normal/mask，UV source 固定 UV2；由于没有已证明的组合公式且武器全量审计 6399 个唯一 MTRL 中没有实际使用记录，renderer 不新增 GPU binding，并以 `skinSamplerComposition` unsupported 暴露该边界。Decal UV1 与 runtime texture 仍等待显式运行时接口。

`CategorySpecularType` 已结构化为 Default/Mask/Unknown。只有 `characterlegacy.shpk + GetValuesCompatibility` 有可验证的分支：CPU 将 `MaterialUniform.properties.w` 编码为 Default=`1`、Mask=`2`；WGSL 对普通 ColorTable 路径使用 `SpecularStrength * mask.r`，legacy Compatibility Default 使用 `SpecularStrength`，legacy Compatibility Mask 使用 `SpecularStrength * mask.r²`，该分支不因 ColorTable material-properties texture 存在而失效。无 ColorTable properties 的普通路径已在 property fallback 中消费一次 mask R，不再追加经验式 `1.35` 放大。依据是 installed Compatibility FXC permutation，MultiMaterial Default/Mask pass 相同，因此不扩展到无证据路径。`g_TileMipBiasOffset`、`g_VertexMovementScale`、`g_VertexMovementMaxLength` 已进入 model/prepared/phantom summary；前者已按 installed DXBC 驱动 Tile Normal/ORB LOD，后两者在 `ApplyVertexMovement` scene key 全量为 Off 时只报告 `vertexMovementParameters`，不驱动顶点动画。

installed audit 会直接从 SqPack 读取 SHPK 本体并现场调用 `D3DCompiler_47.dll` 反汇编，不依赖预生成 probe 文件。`character.shpk` 的 1038 个 pixel shader 中有 1024 个声明 `g_TileMipBiasOffset`、600 个实际消费；`characterlegacy.shpk` 的 1740 个中有 1728 个声明并消费。所有 consumer 都恰好执行两次 bias，锁定 A/B 两条 tile 路径：character 1200 次、legacy 3456 次，共 4656 次；每次均计算 `max(log2(min(length(TileMatrix.xz), length(TileMatrix.yw)) / 128), 0) + g_TileMipBiasOffset`，随后只用于 Tile ORB，或同时用于 Tile ORB/Normal 的 `sample_b`。consumer 分布为 character ORB-only 128 次、ORB+Normal 1072 次，legacy ORB-only 1152 次、ORB+Normal 2304 次。pair-atlas 路径以 `exp2(bias)` 缩放显式梯度实现等价 LOD，detail arrays 与普通 texture mip bias 不消费该值。installed MTRL 中仅 character 有 3 个非零资源/4 次引用，值域为 `-1/+1`；offset 276、size 4、default 0 已由审计锁定。ColorTable bake 现在保留 A/B 两套 TileIndex/TileAlpha/TileMatrix，游戏的 A/B 独立 layer/matrix/LOD 采样后混合顺序已进入 WGSL。

`g_OutlineColor`、`g_OutlineWidth`、`g_SpecularColorMask`、`g_SSAOMask`、`g_TextureMipBias`、`g_ShadowPosOffset` 已解析进材质数据、phantom summary 和 renderer uniform。Meddle/MeddleTools 没有证明 `g_SpecularColorMask` 的 RGB 或补齐 Alpha 如何进入 Final，因此非默认值由 `specularColorMaskComposition` 标记，Final 不消费它。Outline 参数同样缺少空间单位和外扩公式；独立 inverted-hull pipeline 保留，但 `outlineComposition` 会阻止正常 prepared batch 自动提交该 pass。`g_SSAOMask` 与可选 `g_AmbientOcclusionMask` 是不同字段：前者跨多个 family、默认 1，installed character 中有 4 个非默认资源/9 次引用（约 `0.90/0.98/0.99`）；后者只有部分 character family 且无默认值，覆盖 198 个资源/251 次引用、值为 `0.25`。MeddleTools 均没有可验证的 runtime SSAO/occlusion 组合节点，离线 renderer 也没有屏幕空间 AO buffer，因此两者只保留数据和 `UnsupportedInputs` 诊断，不再用经验常量静默压暗 ambient/environment。

普通 base/emissive、normal、mask/specular/material-properties 上传会生成完整 mip chain：sRGB RGB 在线性空间平均，normal 只从 RG 重建/平均/归一化并重新编码 RG，B/A 独立线性平均，其余 data/alpha 线性平均；linear sampler 使用 trilinear mip filtering。installed audit 对 SHPK 本体现场反汇编：`character.shpk` 的 1038 个 pixel shader 中有 1032 个 `g_TextureMipBias` consumer，`characterlegacy.shpk` 的 1740 个中有 1734 个，`characterglass.shpk` 的 38 个中有 34 个。每个 consumer 都恰好一次同号计算 `material bias + g_PbrParameterCommon.m_MipBias`，没有取反或缩放。直接消费者仅为 family 实际绑定的主 Diffuse、Normal、Mask；Index/Table、baked ColorTable ramps、secondary maps、emissive、lightshaft、tile arrays 和环境采样不使用该值。预览器没有全局 scene mip bias，因此只把 clamp 到 `[-16,15.99]` 的 material bias 应用于 primary Base/Normal/Mask；dither depth 对 primary Base/Normal 使用相同采样。installed MTRL 中只有 `character.shpk` 有非默认值：6 个资源/9 次引用，`+1` 为 3 个资源/6 次引用，`-1` 为 3 个资源/3 次引用；其余 package 全为 0。ColorTable extra maps、tile/detail pair atlas 与 TileMatrix 使用各自采样语义，不消费该参数。shadow offset 仍未实现。
`g_GlassIOR`、`g_GlassThicknessMax` 已解析进材质数据和 phantom summary，但不再进入 renderer uniform。installed `characterglass.shpk` 的 38 个 pixel shader 均不绑定这两个参数，MeddleTools 通用 character group 也不读取它们；prepared 因而以 `glassShaderParameters` 显式标记未消费的 raw 输入，不再用无证据公式调节 tint/specular/rim。
`g_AlphaAperture`、`g_AlphaOffset`、`g_ShadowAlphaThreshold` 已解析进材质数据、phantom summary 和 renderer uniform。Meddle 只证明名称、适用 family 与默认值，MeddleTools 没有对应节点或 shaping 公式；installed character 有 7 个非默认 aperture 资源/10 次引用、3 个非默认 offset 资源/4 次引用。installed DXBC 现已结构化审计：character 的 768 条 aperture 指数链和 glass 的 34 条全部最终进入 `mul_sat(..., 3.333333)`、`mad_sat`、offset 符号 gate 与 `alpha < 1` gate；部分 character permutation 在指数链与组合之间还会执行 Table/材质采样，因此审计按寄存器活跃范围向后追踪，而不是依赖固定的短指令窗口。Character 768/768、Glass 34/34 条 shaping dot 均可直接追溯到归一化的 `-v6` 视线输入；其中 6 条 Character permutation 只把 `min(abs(dot), 1)` 编译成等价的 `mov_sat |dot|`。node/pass 配对进一步确认这些 PS 只连接 Character 64 个、Glass 8 个 VS，所有 paired VS 均把 `TEXCOORD4` 写到 `o6`，且 `o6.xyz` 的源位置同时作为 22..25 四行矩阵的投影输入生成 `SV_Position`；因此 `-v6` 的相机方向空间已闭环。legacy/skin 没有 aperture consumer，只有少量其它 offset use。由于 shaping normal 来源、两个 alpha 乘数的来源及与当前 WGSL alpha source 的一一对应尚未完全证明，aperture/offset 继续只触发 `alphaShaping` unsupported，不改变 Final alpha；`g_ShadowAlphaThreshold` 仍未驱动 shadow pass。

cutout 只由支持 `ApplyAlphaTest` material key 的 family 进入 Mask pass，并使用 `g_AlphaThreshold` discard。installed 武器 material-key coverage 没有 `ApplyAlphaTest`，24 组 phantom 也没有 Mask/Cutout；character 系列的 scene-level `ApplyAlphaClip` 在四个实际 SHPK 中全部为默认 Off、0 override，不能当作静态 MTRL cutout 开关。audit 已为 AlphaClip 补齐 known label并固定该边界；现有 Cutout pipeline/synthetic tests 继续覆盖未来 bg/crystal 等非武器输入。

`PreparedAlphaSource` 会区分普通 character 的 `NormalBlue` 与 tattoo 的 `NormalAlpha`。renderer uniform 分别编码为 `2` 和 `4`，主颜色 pass 与 `DrawDepthMode_Dither` depth pass 都采样 normal texture 的 B/A 并按 prepared source 选择；两种 normal-channel alpha 都使用中性 vertex alpha=1，不再把 `GetValuesMultiMaterial` 等 vertex A 分区信号误乘为 opacity。tattoo Alpha 直接使用 normal A。CharacterGlass 当前也复用 `NormalBlue`，但这只是符合 45059 静态 payload 的 preview fallback：installed glass DXBC 的 16 个动态 alpha permutation 并非统一 normal-B 输出，且最终 scene/subview key 无法从 MTRL 恢复。generic BaseColorAlpha、crest fallback 与 lightshaft 的 vertex-alpha 行为保持原样。
`g_UVScrollTime` / `0x9A696A17` 已按 MeddleTools `UvScrollMapping` 转换成 UV0/UV1 scroll multiplier 并进入 renderer uniform。`bguvscroll.shpk` 已单独分类：Color/Normal/Specular Map0 使用 UV0Scroll，独立 Map1 roles 使用 UV1Scroll；`GetMultiValues` 下 WGSL 按 vertex alpha 混合两套 color/alpha/normal/specular。`GetValues` mode/raw 会进入 model/prepared/phantom summary；AlphaMulti/2/3 因 MeddleTools 节点输入未连接而设置 `alphaMultiValues` unsupported，含 Map1 时同时设置 `secondaryMapBlend`。三张 Map1 仅在该 family 复用 tile/sheen/sphere 物理 binding，sampled texture 数保持 15。Web RAF 驱动和 native snapshot 时间 0 的稳定行为保持不变。
`lightshaft.shpk` 的 `g_Color`、`g_TexAnim`、`g_TexU`、`g_TexV`、`g_Ray` 已解析进材质数据和 renderer uniform。`g_Sampler0/1` 会分别进入 base/secondary-base sRGB binding；LightShaft draw role 以 vertex color B 计算 `mix(Sampler0, Sampler0 * Sampler1)`，结果乘 `g_Color`，颜色 scalar 作为 emission strength，并与 vertex alpha 相乘得到 additive alpha。`ApplyAlphaTestOn` 会保留 AdditiveLightShaft pass，同时在输出前按 `g_AlphaThreshold` discard；synthetic WGPU 已覆盖 visible/clipped 差异。Final mode 使用独立 `fs_lightshaft` 与双面/剔除 additive pipelines，避免执行通用 PBR；任一 debug mode 会选择原 additive `fs_main` pipeline，以保留 base/UV/vertex/mesh-role 等视图。MeddleTools 节点图未连接 `g_TexAnim/U/V/Ray`，因此这些值不再产生推测的 UV/强度效果，只保留用于审计。`Type0/Type1/Unknown`、raw type、`g_AngleClip` 与 `g_NearClip` 已进入材质/phantom/prepared summary；prepared `lightshaftClip` 会明确标出它们尚未参与 WGSL。
`g_Transparency` 已解析进材质数据和 phantom summary；MeddleTools water group 证明它直接连接 Alpha。prepared 对 water 使用 `MaterialTransparency` alpha source，小于 1 时选择 Transparent pass；WGSL 直接输出该值，不乘 vertex/base alpha 或 character shaping。`g_WaterDeepColor` 以有限线性 float 直接作为 water base，不再经过 preview-only `0..4` clamp；native HDR BaseColor readback 固定值 12 在 `Rgba16Float` scene 中保真。primary `g_SamplerWaveMap` 复用 normal binding并按 R/G 解码；refraction/whitecap colors 与 WaveMap1/WhitecapMap 已保留在 model/prepared，但因 MeddleTools 当前没有输出连接而不参与着色。三种 water sampler 已脱离 `Other`，不会再被 fallback 当成 base texture。

installed 武器审计没有 water/river/crystal family、water LOD0 range、water/environment sampler 或对应 package constant coverage。MeddleTools 的 water 节点只证明上述 deep color、primary wave、direct alpha 三条连接，crystal 的 EnvMap 以及 water refraction/whitecap/WaveMap1 均未连接；因此这些 structured inputs 继续报告 unsupported，不进入 WGSL。专用 installed assertion 会在真实覆盖出现时要求重新审计，而不是让零样本边界静默变化。

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
- prepared alpha source 暂用 normal texture Blue 作为 preview fallback；ColorTable baked base 保持 opaque，不再写入固定 glass alpha。
- `g_GlassIOR` / `g_GlassThicknessMax` 只保留 parsed/raw/summary，installed PS resource binding 证明它们不应驱动当前离线 surface；prepared 输出 `glassShaderParameters=true`。
- WGSL 使用较亮的 transmission tint、normal-B preview alpha 与 fresnel/specular；`EnableLightingOff` 的 character transparency 可绕过 surface lighting。
- 进入透明 pass。

真实 45059 `characterglass.shpk` 样本只有 normal/mask/index 与 ColorTable 派生 base，没有独立 base texture；MTRL 覆盖 `DrawDepthMode_Dither`，`g_GlassIOR=1`、`g_GlassThicknessMax=0`，normal Alpha 恒为 255、Blue 实测范围为 `57..255`。normal-B fallback 能让雪景玻璃罩保持内部可见和表面变化，但它不等价于游戏的动态 alpha permutation。

45050 `_b.mtrl` 是 `character.shpk + Blend + GetValuesMultiMaterial`，毛发 mesh 的 807 个顶点中 445 个 A=0；旧实现因此把 400/848 个三角形强制为完全透明。修正后 final 前景像素由 29,559 增至 33,358；`XIV_PHANTOM_ALPHA_DEBUG=1` 输出的 `debug-alpha.png` 显示主体为 opaque，normal B 仍保留毛束边缘和内部缝隙透明。synthetic native WGPU fixture 还验证 character normal-B 与 tattoo normal-A 在 vertex A=0/1 时逐像素一致，同时 normal 通道变化继续产生显著输出差异。

`DrawDepthMode` 与 `EnableLighting` 已进入 material/prepared policy。renderer 会在 opaque/cutout 后、透明颜色 pass 前重绘 `DrawDepthMode_Dither` 的 Transparent/Glass batch，使用与颜色 pass 相同的 prepared alpha fallback 和稳定 4x4 屏幕空间有序阈值，只写 depth、不写两个颜色 target；透明颜色 pass 则逐帧按相机方向对所有 Transparent/Glass 三角形中心做全局 back-to-front 排序，将排序后的索引写入独立动态 index buffer，并合并相邻同 batch draw run，继续 alpha blend 且不写 depth。Meddle `Names.cs` 只能确认该 material key 适用于 `characterglass.shpk` / `charactertransparency.shpk`，没有暴露游戏使用的抖动矩阵或噪声公式；MeddleTools 也不实现运行时 depth pass，因此当前阈值公式是保守近似。该行为与覆盖更多 shader family 的 scene key `ApplyDitherClip` 分开处理。

`GlassBlendMode` 是 scene key而非 MTRL material key。Meddle 只提供默认 `GlassBlendMode_Mul` 与可选 `GlassBlendMode_Add` 的名字/CRC，MeddleTools 没有对应节点或 bake 行为，因此不能从静态材质恢复场景选择，也没有证据把 Mul 直接解释为某个硬件 blend equation。renderer 的默认管线实际使用 WebGPU `ALPHA_BLENDING`，现准确暴露为 `ModelGlassBlendMode::Alpha` 和 Web `Alpha`；旧 `multiply`/`mul` 输入仍兼容解析为 Alpha。`Additive` 只让 Glass batch 选择 additive pipeline，继续作为显式预览近似。真实 multiply/scene-color composition 仍需游戏 shader 或运行时捕获证据。

这不是完整游戏 glass shader，目前只能显示内部模型并提供近似透明外壳。

## 8. GPU 渲染管线

当前 WebGPU/WGSL 渲染流程：

通用 surface fragment 采用显式三阶段数据流：`resolve_surface_samples` 解析逐 role UV 并采样 primary/secondary base、normal、specular、emissive、mask/material-properties；`resolve_surface_state` 合成 normal、base、alpha、material/specular、emissive 和 debug 中间值；`resolve_surface_output` 只负责 opaque/glass lighting、extra lighting 与 bright attachment。`fs_main` 仅调度这三个阶段，并保留 debug dispatch、crest/cutout/alpha discard 和 lightshaft fallback，避免 family 条件重新与采样/lighting 交叉。MeddleTools 的 character variants 复用共同 surface group也是该边界的依据；这不是新增 shader 语义，uniform/binding ABI 与公式保持不变。

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

   纹理 GPU 格式统一由 prepared `textureSampling.<role>.colorSpace` 经单一映射（`mip_semantic_for_color_space` / `texture_format_for_color_space`）决定：`Srgb` → `Rgba8UnormSrgb`（RGB 硬件解码、alpha 保持线性），`NonColor` → `Rgba8Unorm`。审计结论：base/emissive/baked specular ramp 走 `Srgb`，原始打包 specular、mask、material-properties、material/multi map、index 与 ColorTable extra ramps 走 `NonColor`；packed normal（RG 重建 Z）、tile matrix（`Rgba32Float`）、sheen/sphere float ramp、tile/detail pair atlas 与中性 fallback 保持显式语义，不从名称推断。environment binding 继续保留 prepared Non-Color policy，但因为 MeddleTools 没有可验证的下游映射而显式保持 unsupported；water primary wave 已按 packed-normal policy 进入共享 normal binding，`wave1`/`whitecap` 则保留为可审计的 unsupported 输入。
3. scene pass：
   - renderer 先计算第一版 `PreparedModel`，为每个 mesh 记录 draw role、main-pass 可见性、submesh attribute metadata、attribute visibility 和 `PreparedMaterial`；`PreparedMaterial` 会把材质 alpha/render mode 与 mesh draw role 合成 `Opaque`、`Cutout`、`Transparent`、`Glass`、`AdditiveLightShaft` 五类 prepared render pass，并记录第一版 shader family 分类、texture bindings、texture sampling policy、material feature flags、UV source 和 unsupported/runtime-only 输入摘要。
   - opaque pipeline：写 depth，绘制 `Opaque` batch。
   - cutout pipeline：写 depth，绘制 `Cutout` batch；当前仍由 WGSL alpha test discard。
   - transparent pipeline：alpha blending，不写 depth；将 `Transparent` 三角形按当前相机方向排序后绘制。
   - glass pipeline：alpha blending，不写 depth；与 transparent 三角形全局排序后绘制，仍沿用现有 glass 近似参数。
   - dither depth prepass：opaque/cutout 后、transparent/glass 颜色 pass 前，仅重绘 `DrawDepthMode_Dither` batch；独立双面/背面剔除 pipeline 使用与颜色 pass 一致的 base-alpha/normal-B prepared source 和稳定 4x4 屏幕空间阈值，开启 depth write，并把两个 MRT 的 color write mask 设为空。Meddle/MeddleTools 没有提供游戏公式，因此该 pass 只解决透明表面的深度覆盖近似，不代表复刻 `ApplyDitherClip` scene key。
   - additive/lightshaft pipeline：additive blending，不写 depth；Final 的 `AdditiveLightShaft` 使用专用 `fs_lightshaft`，debug 使用通用 `fs_main`，Additive Glass 仍在透明排序阶段使用通用 additive pipeline。
   - opaque/cutout/transparent/glass/additive 各有 backface 与 culled pipeline，按材质 `render_backfaces` 选择。
   - `PreparedMesh` 先过滤非 surface：shadow、terrainShadow、verticalFog 不进入当前渲染；lightShaft 不作为普通 surface，但会分类为 `AdditiveLightShaft` 并保留到 additive pass；materialChange/crestChange 已拆为独立 draw role 并暂时保留在主 pass；mesh category glass 会强制进入 `Glass` prepared pass。
4. bloom pass：第一级 blur 直接从 HDR scene 做 bright-pass 提取，阈值为场景线性空间的 1.0（display white）+ 0.5 knee 平滑，第二级纯 blur；材质 shader 不再内嵌高亮提取常量（旧的 0.72 阈值、`emissive * 1.15`、`highlight * 0.65` 已删除），也不再有独立 bright MRT attachment。
5. compose pass：scene + bloom 输出到 canvas。scene/bloom 中间目标均为 `Rgba16Float`（core WebGPU 可渲染/混合/过滤），HDR 高光与 >1.0 的 emissive 在合成前不再被 `Rgba8Unorm` 截断；compose 先乘 exposure（默认 1.0），再做 Khronos PBR Neutral tone mapping（0.76 以下近似恒等，以上平滑压缩并轻度去饱和），最后按目标格式决定是否输出编码：sRGB surface 由硬件编码一次，非 sRGB surface 由 shader 编码一次。synthetic WGPU fixture 验证 2.0/8.0 两档红色 emissive 在 tone map 后仍可区分（去饱和使 G/B 通道分离），LDR 截断下两者会撞成同一白色；bloom fixture 分别验证 >1.0 emissive 越过轮廓发光、0.85 sub-threshold emissive 与 dielectric/metallic 不发光。

### 8.1 稳定预览灯光契约

预览灯光是 presentation policy，不是 FFXIV 材质输入。材质、法线、ColorTable 或纹理色彩空间修复不得通过同步修改灯光来补偿。所有参数集中为 Rust/WGSL 中命名的 `PREVIEW_*` 常量，并由结构测试锁定：

- 主光方向使用相机基向量，固定为 `-0.45 * right + 0.65 * up + 0.65 * view` 后归一化；旋转相机会让主光与观察方向一起旋转，保证武器预览角度变化时保持一致的工作室布光关系。
- 主光颜色固定为线性 RGB `(1.0, 0.95, 0.88)`；direct diffuse scale 为 `2.20`，direct specular scale 按 toon 项在 `1.15..1.75` 间变化。
- ambient ground/sky 固定为 `(0.12, 0.10, 0.085)` / `(0.30, 0.35, 0.42)`，view fill 为 `(0.24, 0.215, 0.19)`，ambient scale 为 `0.42`。
- 程序化 environment 的 ground/sky、horizon、warm key/cool fill lobe 和 rim 强度同样集中命名；它只作为缺少已验证 environment-map 语义时的预览环境，不代表 crystal/environment shader 行为。
- exposure 固定为 `1.0`，scene-linear bloom threshold 固定为 `1.0`；输出继续只经过一次 tone map 与一次 sRGB encoding。

固定相机 synthetic WGPU fixtures 已覆盖 matte/dielectric、glossy、metallic、roughness sweep、emissive、transparent、two-sided 与 bloom threshold。测试比较数值探针和材质之间的稳定关系，避免依赖人工观察或用灯光漂移掩盖材质回归。

相机使用 45° 透视投影，因此 fragment lighting 不再把模型中心 `viewDir` 当成整屏视线。
camera uniform 同时传递世界空间 eye position；vertex shader 输出插值 world position，fragment
按 `cameraPosition - worldPosition` 计算逐像素 view vector。GGX half-vector、Fresnel、程序化
environment reflection、rim、ambient view fill 和双面法线方向共用该向量；中心 `viewDir` 只保留
为相机相对主光契约和退化位置 fallback。宽平面 metallic fixture 会断言画面中心与透视边缘的
高光/Fresnel 响应不同，防止重新退化为模型级常量视线。

GPU/offscreen 回归还覆盖 water primary wave/alpha、tile normal/ORB、tile/detail arrays、
Sheen/Sphere unsupported 边界、透明三角形排序、character/tattoo alpha 和 LightShaft
sampler/alpha-test。由于 MeddleTools 的 crystal EnvMap 与 Sheen/Sphere 都没有最终下游连接，
这些输入不伪造采样/光照贡献；对应 fixture 会在 `UnsupportedInputs` debug 中断言独立诊断色，
并为 Sheen/Sphere 额外断言 active/neutral Final 像素一致，锁定“已解析但无可靠公式”的边界。

### 8.2 Runtime on-render fallback

Meddle `OnRenderMaterialUtil` 表明 weapon decal 与 FC crest 来自运行时 `OnRenderMaterial`，不是静态 MTRL sampler。运行时没有 decal/crest 时，游戏路径使用透明纹理；materialChange 则应回落到基础材质。

当前 prepared 层已表达：

- `CrestChange`：`TransparentTexture` fallback，同时标记 `decalOrCrest` unsupported。
- `MaterialChange`：`BaseMaterial` fallback；renderer 继续使用基础材质，不再标记 `runtimeMaterialChange` unsupported。

renderer final 模式会让 `CrestChange` 进入 transparent pass 并 discard，避免透明 fallback 写 depth；mesh-role debug 模式仍显示该几何。`MaterialChange` 继续使用基础材质。真实运行时 crest/decal 内容仍不可用，因此 `decalOrCrest` 保持 unsupported。

透明排序已做到逐三角形：静态 index buffer 继续供 opaque、dither depth、outline 和 additive pass 使用；Transparent/Glass 每帧生成按三角形中心全局 back-to-front 排序的动态 index buffer。45050 的 848 个透明三角形和 45059 的 320 个透明三角形已在 snapshot harness 中验证没有共享顶点之外的 proper intersection，因此当前真实样本不存在循环遮挡，排序可保持精确 alpha composition；weighted blended OIT 会引入权重近似，等待真实相交样本再评估。

WeaponCatalog audit 同时统计 LOD0 mesh ranges：7365 个唯一模型只有 normal category，共 8114 个 mesh；shadow/terrainShadow model 与 mesh 均为 0。`g_ShadowAlphaThreshold` 在四个实际 SHPK 中全部为默认 0.5，`g_ShadowPosOffset` 只有 5 个非零资源。Meddle 证明这些字段和通用 MDL range 存在，但没有提供离线 light matrix、bias、shadow sampling 或 alpha 公式；当前不以 normal geometry 自创 shadow map，参数继续保留 parsed/raw/uniform 诊断。

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

### 9.1 ColorTable 光照字段组合（MeddleTools 节点拓扑验证）

对 `shaders.blend` 的 `meddle character.shpk` 组及其宿主材质树做二进制解析后，以下组合关系得到验证（WGSL 已对齐）：

- Roughness（非 legacy）来自 ColorTable Roughness，与 GlossStrength 独立。分量级 DXBC 数据流审计确认 modern `character.shpk` 的 976 次 Roughness 采样中有 768 次传播到 `o1.y`；Legacy 的 1712 次 Gloss 采样中只有 272 次传播到同一输出，恰好覆盖 `Gloss-1` 的 88 次 compatibility/mask 特例与 `Gloss*-0.066667` 的 184 次直接路径。后者写出 `exp2(-Gloss/15)`；特例会先按 mask 条件在 `1` 与 Gloss 之间选择，再执行同一变换。renderer 现在用独立的 exact `characterlegacy.shpk` uniform 能力位，在采样染色后的 float material-properties ramp 后执行该变换，作为 environment/GGX preview 的 Legacy roughness 参数；modern、glass、无 ColorTable fallback 均不启用。raw GlossStrength 和 MaterialProperties debug 保持不变。其余 1440 次 Legacy sample 与上述 272 次形成严格互斥分区：1440/1440 都构造 `V=normalize(-camera-relative position)`、`L=normalize(-position+(0,0.2,0))`、`R=reflect(-V,N)`，并将 `min(3-3(1-saturate(N·L))²,1) × saturate(R·L)^Gloss` 传播到 `o0.rgb`；其中一个 permutation 使用等价的 `mad_sat`，其余使用 `mad+min`。renderer 已把这条完整 direct-specular lobe 作为 exact Legacy 分支接入，不再把孤立 `pow` 塞进 GGX 分母。同一批 permutation 的 environment LOD 也已闭环：864 个 forward permutation 使用由 vertex position Y、`g_InstanceParameter[4]` 与 `g_ModelParameter[0]` 形成的 `TEXCOORD4.w` wetness height control，576 个 deferred permutation 使用 `g_SamplerGBuffer1.w`；正 control `h` 令 `effectiveGloss=mix(Gloss,10-Gloss,h²)`，最终两次 cube-array sample 的 LOD 均为 `6*(1-(1-8/(effectiveGloss+9))²)`，合计 2880 次且无未分类。16/16 个配对 VS 的 reflection 将这两个来源精确命名为 `InstanceParameter.m_Wetness`（offset 64）与 `ModelParameter.m_Params.x`（offset 0），而非静态 bounds。由于 preview 当前没有游戏 ReflectionArray、对应 runtime wetness provider 或 GBuffer pass，这条公式尚不能同构接入 procedural studio environment；后续 environment color accumulation 也仍未完整复原。`characterglass.shpk` 的 32 次 Roughness 采样同样没有传播到 `o1.y`。因此 `legacyGlossComposition` 继续标记剩余 pass-specific Final 差异。
- Legacy ReflectionArray 的基础环境处理也已由 installed DXBC 全量固定：每个主颜色 Gloss shader 分别以 `g_AmbientParam.envLocationIndex` 和 `envLocationIndexPrev` 采样一次，共 1440+1440；2880/2880 个结果都按 `rgb²/(alpha+0.0001)` 解码并传播到 `o0.rgb`，1440/1440 shader 使用 `envLocationInterpRate` 混合当前/上一环境、应用 `reflectionScale/reflectionOffset`，并通过 `bakeLightRate` 组合带 `2.356194` 系数的 ambient 分支。这仍不等于完整 Final：后续 sphere harmonics、direct/scene light 与 material specular 的最终累加尚在审计，preview 也没有游戏 ReflectionArray/runtime Ambient provider。
- Legacy SpecularStrength 与 environment 不是全 package 的单一公式，但 pass 边界现已闭环：864 个同时含 cube/Gloss/SpecularStrength 的 Final shader 全部把第一次 strength product 传播到 `o0.rgb`，并只属于 `0xc885bbd3/0xf21a038f`；128 个 Gloss+SpecularStrength non-cube shader 与 16 个不采样 Gloss 的 shader 则全部属于 producer pass `0x03ac862e`，两类 product 均不进入 `o0.rgb`，合计 144 个 shader，恰好等于全部 `o1.x=mul` producer。代表 assembly 直接写 `mul o1.x, otherTableComposite, SpecularStrength`。因此 128+16 不是遗漏的另一套 Final lighting，renderer 不需要为它们创建独立 surface 分支。576 个 Legacy deferred cube permutation 都从 `g_SamplerGBuffer1.X/W` 取值，两个 lane 均传播到 Final `o0.rgb`；X 的第一段为 `log/max/mad/movc` 复合链，不能等同命名为 raw SpecularStrength，W 则是已闭环的 wetness environment control。配对的 288 个 producer 均写 `o1.xyzw`，其中 `o1.y` 均为 `exp` 结果，来自 272 个固定及 16 个动态 ColorTable Gloss/W sample；`o1.x` 另有 48 个 `mov` 与 96 个 `movc` runtime/decal 写入。故 deferred 仍必须作为独立 material/GBuffer path 复原。modern `character.shpk` 另有 64 个独立 deferred shader 读取 GBuffer1.Y/Z/W，也不应套用 Legacy X/W 结论。
- 864 个 Legacy Final shader 的第一次 strength product 还可按首个后继 opcode 分成 144 个 `first-log` 与 720 个 `first-mul`，但这不是两套不同的非线性/线性材质模型。前者在 SpecularStrength 与完整的 Index/Table/TileOrb（其中 96 个还含 Decal）复合值相乘后直接取 log；后者先让 SpecularStrength 与局部因子相乘，再补乘其余因子。全量分量 taint 审计确认两组最终 864/864 都进入 `log(value) → ×0.2 → exp`，即 `value^0.2`，随后进入同一 wetness 加权的 `max/mad/min/movc` 整形。它与 deferred GBuffer1.X consumer 的开头同构，说明 forward 与 deferred 共享的是 pass-specific composite，而不是可直接命名为线性 F0 的 raw SpecularStrength。当前统一 GGX 中 `SpecularStrength → F0` 仍只是 preview mapping；缺少 runtime wetness、ReflectionArray 和 scene accumulation 时，不能宣称覆盖这 864 个官方 Final path。
- forward terminal boundary 现也已按物理 destination swizzle 闭环：864/864 个整形 composite 都作为 replicated scalar 乘入三分量 RGB 支路并到达 `o0.rgb`，864/864 的首个后继合成都是 `mad`，0 unclassified。乘法另一侧始终含 ReflectionArray、Index/Table/Tile、Normal/Mask/Occlusion；576 个 permutation 还含 LightDiffuse/LightSpecular。后继 `mad` 独立项不含 ReflectionArray/LightSpecular，并仅在 576 个 permutation 新增 Diffuse。这直接否定了 raw Legacy SpecularStrength 作为局部 GGX/F0 标量的 preview 接法：renderer 现对 exact `characterlegacy.shpk` ColorTable 分支保留 raw lane、MaterialProperties debug 与 `legacyGlossComposition` 诊断，但在完整 runtime composite 可实现前用中性权重跳过 `SpecularStrength → F0`；modern/general preview 仍保留原有映射。
- 对上述 576 个 Legacy deferred X consumer 的按分量审计现已越过 packed `movc`：576/576 都把整形后的物理 X 以 replicated scalar 乘入 RGB，且该乘法结果全部到达 `o0.rgb`；其第一个后继颜色合成 576/576 都是 `mad`。乘法另一侧 576/576 已含 ReflectionArray、GBuffer、Index、Mask、Normal、Occlusion、Table 与 TileOrb，288 个 permutation 还含 LightDiffuse/LightSpecular；乘法后的 `mad` 非乘法项不再含 ReflectionArray/LightSpecular，却新增 384 个 Diffuse sample，并在 288 个 permutation 保留 LightDiffuse。该分区证明 X 位于包含游戏 environment/scene-light 的复合支路边界，而不是可搬到当前 preview GGX 的单一 F0、roughness 或 SpecularStrength 因子。审计使用按 swizzle 的分量传播和 `if/else/endif` 状态合并；旧的 packed-register “first join” 统计已删除，避免把同一 `movc rN.xy` 的 Y lane 来源误归给 X。
- SHPK provenance 进一步固定了 `o1.x` 的 pass/runtime 分区：144 个 `mul` PS 全部属于 pass `0x03ac862e`；48 个 `mov` 与 96 个 `movc` 全部属于 `0x6006067f`。后两者由 `GetDecalColor` key 精确分开：Off 写常量 `0.172549`，Alpha/RGBA 则比较 `g_SamplerDecal.a * g_DecalColor.w >= 0.75`，在 `0.125490` 与 `0.172549` 间选择。`mul` 横跨全部 decal mode，继续组合 SpecularStrength 与其它 Table lane。因此 deferred X 还携带 runtime decal/pass control；静态 loader 即便拥有 ColorTable 和 MTRL key，也缺少 `g_DecalColor`/DecalTexture provider，不能生成同构值。
- prepared runtime requirements 现把 Final 的外部所有者单列为 `materialDynamicEmissiveColor`、`instanceMulColor`、`instanceCameraLightParameters`、`modelWetnessParameters`、`sceneAmbientParameters` 和 `reflectionArrayTexture`。前三项适用于 exact Character/Legacy ColorTable Final，后三项是 Legacy environment 特有依赖。它们分别对应独立 16-byte `g_MaterialParameterDynamic.m_EmissiveColor`、`InstanceParameter.m_MulColor`、`m_CameraLight.m_DiffuseSpecular/m_Rim`、上述 `m_Wetness/m_Params.x`、160-byte `g_AmbientParam` 与由 current/previous location index 选择的 cube array。Legacy dynamic emissive、MulColor 和两个 CameraLight vector 在 864/864 个 forward SpecularStrength Final shader 中都传播到 `o0.rgb`，而 `m_EnvParameter` 为 0/864；128+16 个 producer class 对这些字段均为 0。864/864 的 dynamic emissive 乘法另一侧精确追溯到 `g_SamplerTable` texel 2.5；随后 864/864 再乘 `max(dot(preEmissiveLighting, vec3(0.298910, 0.586610, 0.114480)), 1)` 并到达 `o0.rgb`，短路径以 216 个 `mad` 合成，长路径以 648 个 `mul` 后继续合成。该 luma 源不是只为 emissive 构造的辅助量：864/864 还通过独立 `mad` 作为 RGB 照明项进入 Final；modern Character 的对应边界也为 256/256，分成 64 `mad` / 192 `mul`，且 luma 源 256/256 独立进入 RGB。其 provenance 全覆盖 Ambient、Instance MulColor/CameraLight、material 与 Normal/Occlusion/Table/Tile 等输入，部分 permutation 再包含 Camera、Diffuse/Light/GBuffer/Decal；Meddle/MeddleTools 不导出这些 runtime buffer。WGSL 现以当前 `lit` 作为离线 pre-emissive lighting，按完全相同的 Rec.601/max 公式只缩放 exact Character/Legacy 的 ColorTable emissive；静态 `g_EmissiveColor`/shader emissive 不缩放。`ModelRenderOptions.dynamic_emissive_color` 已作为 per-render runtime provider 接入 camera uniform，默认 `[1,1,1]` 保持既有输出，非有限分量回退到 1；native exact/control fixture 同时锁定 Final 增强、Emissive debug 不变及非 exact Final 不受影响。其余 preview lighting 仍不冒充游戏 Ambient/CameraLight/ReflectionArray。
- Metallic（非 legacy）= ColorTable Metalness 直接输入；legacy 近似为 `1 - mask.B`。
- SpecularStrength × mask.R → Principled `Specular IOR Level`；specular ramp 颜色 → Principled `Specular Tint`，这是 MeddleTools/通用 GGX preview 的连接。installed DXBC 确认 character 的 256/256、legacy 的 1008/1008 个 Specular/W consumer 首先执行 raw `mul`，没有提前 saturate，因此 float payload 不 clamp；但 Legacy 的 864 个 Final consumer 已进一步证明该值进入 runtime wetness/environment composite，而不是局部 F0。WGSL 只在 modern/general preview 执行 `0.08 × strength × mask.R × tint` 并于最终 dielectric F0 夹到 `[0,1]`；exact Legacy ColorTable 分支保留 raw/debug，但以中性 strength 跳过该矛盾映射。
- IOR 近似 = `1 + roughness`；Anisotropy → Principled `Anisotropic`（specular ramp alpha）。WGSL 保留该输入连接，并用 tangent-oriented anisotropic GGX NDF 做预览近似；精确 FFXIV 分布与 rotation 未由参考证明。
- Sheen（Rate/Tint/Aptitude）与 Sphere（Index/Mask）ramp 在该版本材质树中只汇入混合组、未连接任何下游消费，即 MeddleTools 不验证其最终着色行为。
- bg 树中 specular map（Map0/1 混合后）的 G 通道 → `Roughness`、B 通道 → `Metallic`，即现有 bg specular channels 实现（properties.x=specular.b、properties.y=specular.g）升级为 verified。
- crystal 材质树把 `g_SamplerEnvMap` 接入组接口，但组内没有任何下游消费（dead-end）；bg 树中也不存在 EnvMap。MeddleTools 因此不提供任何 environment mapping 语义。
- water/river 树中 WaveMap 经 `NTNormal_Fix`（R/G 重建 Z）→ Normal、DeepColor → Base Color、`g_Transparency` → Alpha（另有接口默认 IOR/Transmission Weight）；WaveMap1 与 WhitecapMap 同样只接入组接口而无下游消费。

按 family 的行为状态：

- **Character / CharacterLegacy**：上述组合为 verified（按 value mode 分 MultiMaterial/Compatibility 两条树）；默认 Toon 常量不改变 Final，非默认 Toon 输入显式 unsupported；sheen/sphere 因 MeddleTools 无下游连接而显式 unsupported；runtime ColorTable/染色应用、decal 为 unsupported。
- **CharacterStockings / Tattoo / Occlusion / Scroll / Reflection / Glass / Transparency**：基础 surface 组合同 Character；family 专属行为（stocking alpha、tattoo alpha、scroll 变体、reflection、glass 参数）为 approximated 或显式 unsupported，详见 prepared `unsupportedInputs` 与 `UnsupportedInputs` debug 视图。
- **Skin**：uv2 skin sampler 组合未验证，报告 `skinSamplerComposition` unsupported。
- **Bg / BgUvScroll**：Map0/Map1 与 UV scroll 为 verified（MeddleTools bg 组连接）；specular map G/B → roughness/metallic 为 verified（bg 树 `Separate Color` 直连输出）；detail layer selection、UV scale 和 primary/multi debug mix 已保留，但 detail 对 Final base/normal 的 influence 未证明，设置 `detailComposition` unsupported；AlphaMulti/2/3 显式 unsupported。
- **Crystal**：EnvMap 仅接入组接口、无下游消费（直接解析确认），environment mapping 显式 unsupported（preview 使用程序化 studio 环境近似）。
- **LightShaft**：Sampler0/1、vertex B、`g_Color`、emission strength 为 verified；Type/AngleClip/NearClip 显式 unsupported。
- **Water**：deep color、primary wave（`NTNormal_Fix` R/G 重建）、`g_Transparency` 直连为 verified；WaveMap1/whitecap/refraction 未连接，显式 unsupported。
- **Unknown**：generic surface 近似。

材质语义回归另有 checked-in fixture matrix：`tests/fixtures/material_fixture_matrix.json`。
它以 16/32 行分别代表 Legacy/Dawntrail ColorTable，并覆盖 single/multi、染色、metallic、
emissive、transparent 与 double-sided。`tests/material_fixture_matrix.rs` 会把 fixture row 送入
生产 `bake_color_table_maps`，核对 diffuse/specular 的 sRGB packed bytes、material/tile 的
linear packed bytes、emissive presence、Sheen/Sphere/TileMatrix float payload；同一 case 再构造
真实 `ModelMaterial` 并经过 `prepare_model_for_render`，核对 render pass、alpha source、shader
family 与 backface policy。这样基础语义不再只依赖 ignored PNG 或人工审图。

## 10. 已知限制与后续计划

当前实现目标是“武器模型预览可辨识、颜色/透明大体正确”，仍非完整 FFXIV shader 复刻。

后续优先级：

1. Prepared draw role / pass：`PreparedModel` / `PreparedMesh` 已完成第一步主 pass 过滤并保留 submesh attribute metadata；显式 `enabledAttributeMask` 输入已可隐藏 disabled submesh，显式 `enabledShapeMask` 已可在 renderer creation 时应用 sparse morph target，默认离线模式仍保持不过滤/base geometry；renderer 内部已有 `Opaque/Cutout/Transparent/Glass/AdditiveLightShaft` prepared pass 分类，Cutout/Glass 已有独立 wgpu pipeline 入口，`AdditiveLightShaft` 已对齐 MeddleTools 当前连接的 Sampler0/Sampler1、vertex B、`g_Color`、emission strength 和 alpha；后续还需要 runtime shape name/bit mapping、更完整的 cutout/glass shader 行为，以及 lightshaft `Type/AngleClip/NearClip` 等游戏语义。
2. GPU 顶点格式：uv1-uv3、color1、secondary normal/bitangent、flow 已进入 GPU 顶点输入；PreparedMaterial 已有 feature flags、UV source、per-role scroll mask、flow/value mode 和 unsupported/runtime-only 输入摘要。`usesFlow` 已收紧为 Flow mode + flow0，WGSL 已消费 primary flow tangent，Map1 已消费 UV1Scroll 与 secondary normal/bitangent frame。Web/native debug mode 已可分别预览 color1、secondary normal、flow0、flow1；flow1 与 color1 的最终 family-specific 公式仍待证据。
   renderer 现也在 material uniform 中携带由 `PreparedMaterialUnsupportedInputs` 派生的诊断色，并新增 `UnsupportedInputs` debug mode（web 下拉 `"unsupported"`）：lightshaft clip=琥珀、crystal 环境贴图=蓝、character reflection=品红、scroll variant=橙、glass=青、其它已知不完整 family=红；AlphaMulti=紫、Multi color=橙红、detail composition=淡紫、AO mask=黄绿、unsupported legacy specular=紫灰、vertex movement=绿；其余 runtime-only 输入=灰、完全支持=暗绿。Final 渲染路径仍按文档化的 generic 近似回退，诊断色只用于可视化审计，不改变着色语义。synthetic WGPU fixture 验证 supported、characterglass、crystal EnvMap 与 detail composition 的诊断输出。
3. Glass：normal-B alpha、`DrawDepthMode` / `EnableLighting` prepared policy、dither depth prepass 与显式 `GlassBlendMode` scene input 已接入；后续实现折射与真实厚度传输。
4. Material params：alpha/glass/normal/tile/toon/detail/color/outline/scroll/lightshaft 参数、`CategoryFlowMapType`、`CategorySpecularType`、`GetValues` mode/raw 与 `GetDecalColor` 已结构化；water colors/wave、tile mip bias、vertex movement constants 和 bguvscroll Map1 sampler roles 也已加入。legacy Compatibility specular Default/Mask、tile mip bias 与 ColorTable A/B tile 双采样已有 WGSL 消费；movement、AlphaMulti、detail composition 继续显式 unsupported。后续处理 multi map mask、runtime skin decal 输入、water refraction/whitecap 和其它 shader-family 行为。
5. Tile/Sphere/Sheen：renderer 已消费有节点证据的 tile/ORB 行为，并保留 Sheen/Sphere extra maps、float payload、binding 与 debug view；后两者当前只报告 unsupported。后续只在取得游戏 shader 或有下游连接的参考节点后实现 reflection/sphere 规则。
6. 纹理采样配置：数据层已有逐 role policy，renderer 已为现有 15 个 texture binding 分别派生 sampler，并已绑定 `_id.tex`、ColorTable extra maps 与共享 arrays；后续重点是 shader 级 clip/extend 和 runtime decal binding。
7. 染色：Legacy/Dawntrail `ColorDyeTable` 的 template、channel 和可染通道 flag 已结构化进 `ModelMaterial.colorDyeTable`；数据层已支持 Legacy `stainingtemplate.stm` 与 Dawntrail `stainingtemplate_gud.stm` 的 v1.1/v2.x 解析、1-based stain lookup、GUD template ID `-1000` 的 Legacy fallback 和逐 flag `ColorTableRowColors` override。`WeaponModelLoadRequest.stainIds` 已进入同步/异步加载，STM 按请求缓存并在 material summary/ColorTable bake 前应用；`WeaponModelData.stainIds`、`ModelMaterial.stainingApplication`、phantom summary 与 prepared unsupported 会记录结果。`WeaponCatalogPackage` 已导出 EXD stain UI metadata，Web 提供 stain0/stain1 色块选择器并把值写入 URL 和模型资源 key。默认 `[0,0]` 不加载 STM，EXD `Stain.Color` 仅用于 UI，不替代 STM 数据。phantom fixture 已支持 case-level `stainIds`，并用 `45052` 的 `[0,0]`/`[1,0]` 正式 snapshot 验证染色画面和 application report；后续扩展第二通道与 metallic 样本。
8. 特殊 shader：lightshaft、transparency/glass、bguvscroll、Flow 与 water 已有第一版消费；stockings 已对齐 opaque alpha/pipeline，tattoo 已按 normal Alpha 处理透明度。reflection 与 occlusion 仍主要停留在 package 分类/runtime diagnostics，后续补各 family 的实际 shader 行为。
