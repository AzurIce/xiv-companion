# 武器模型渲染管线调研与设计

本文记录 `xiv-companion` Web 武器预览从游戏数据到 GPU 渲染的完整管线、已知坑点、当前实现策略和后续扩展方向。调研参考了本地游戏 SqPack、Physis、Meddle 运行时导出逻辑，以及实际问题样本：逗猫之幻梦、浪漫之幻梦、冬雪之幻梦、绝境系列双手武器等。

## 1. 数据入口：Item EXD → WeaponCatalog

武器目录来自 `Item` EXD：

- `EquipSlotCategory` 判断是否武器：`1` 主手、`2` 副手、`13` 双手主手、`14` 双持主手。
- `Model{Main}` / `Model{Sub}` 是武器模型 ID。

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
- 数据层保留：position、blend weights/indices、normal、uv0-uv3、bitangent、secondary normal/bitangent、color0/color1、flow0/flow1、index、material_index、mesh category、submesh info、bone table。
- mesh category 会映射成 `ModelMeshDrawRole`，并和 submesh attribute metadata 一起进入第一版 `PreparedModel` / `PreparedMesh`，作为 renderer-friendly 的第一步 prepared draw role。若调用方显式提供 `PreparedModelOptions.enabledAttributeMask`，prepared 阶段会按 submesh 所需 attribute mask 计算 visibility；默认离线模式不猜运行时 enabled mask。
- ignored phantom snapshot 的 `model-summary.json` 会输出 mesh category、submesh attributes、bone table、shape 影响摘要，并链接 full MDL metadata JSON。

注意：renderer 当前 GPU 顶点格式已上传 position、normal、uv0-uv3、bitangent、color0/color1、secondary normal/bitangent、flow0/flow1，WGSL `VertexInput` 也已声明对应 location；`PreparedMaterial` 已记录第一版 UV source，`PreparedModel` 会把 mesh-level flow presence 汇总到 `usesFlow`，但 fragment shader 目前仍主要消费 uv0、primary normal/bitangent 和 color0。

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
- 路径后缀 `_id` / `g_SamplerIndex` 会识别为 `Index`，不会当 mask 或 diffuse 直接采样。
- material debug 会输出 sampler 的 `textureUsageName` 和 `kindSource`，用于判断来源是 `.shpk` resource name、known CRC 还是 unknown。

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
   - tile/sheen/sphere/tile-matrix 与 MeddleTools 的 extra ramps 对齐，tile-matrix 同时保留 float channels。
4. 若 ColorTable 有 emissive，则额外启用 emissive texture。

当前实现同时支持 Dawntrail 32 行和 Legacy 16 行 ColorTable。renderer 已消费 diffuse/base、specular、material-properties、emissive，并已把 tile、sheen、sphere、tile-matrix extra maps 绑定进 WGSL，用 nearest sampler 做第一版高光/反射调制；完整 tile array 与 MeddleTools 节点图仍未复刻。

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
`g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale` 也已解析进材质数据和 renderer uniform，当前预留给后续 multi/detail normal 组合，尚未改变实际 shader 采样。
`g_TileIndex`、`g_TileAlpha`、`g_TileScale` 已解析进材质数据和 renderer uniform，当前预留给后续 tile array 选择与 UV repeat 逻辑；`TileAlpha` 仍不作为材质透明度。
`g_DetailID`、`g_MultiDetailID`、`g_DetailColor`、`g_MultiDetailColor`、`g_DetailColorUvScale`、`g_DetailNormalUvScale` 已解析进材质数据和 renderer uniform，当前预留给后续 detail color/normal 采样；detail color 目前只作为结构化输入和 summary 字段，不直接改变 fragment 输出。
`g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_MultiEmissiveColor` 已解析进材质数据、phantom summary 和 renderer uniform，当前只作为后续 shader-family color/emissive 节点输入，不覆盖现有 preview diffuse/emissive。
`g_GlassIOR`、`g_GlassThicknessMax` 已解析进材质数据、phantom summary 和 renderer uniform，当前只作为后续 glass shader 输入，不改变固定 glass opacity 或 tint。
`g_UVScrollTime` / `0x9A696A17` 已按 MeddleTools `UvScrollMapping` 转换成 UV0/UV1 scroll multiplier 并进入 renderer uniform，Web 渲染循环会用 RAF 时间驱动保守滚动采样，native snapshot 默认时间为 0。
`lightshaft.shpk` 的 `g_Color`、`g_TexAnim`、`g_TexU`、`g_TexV`、`g_Ray` 已解析进材质数据和 renderer uniform；LightShaft draw role 会启用保守 additive tint、`g_TexAnim.xy` UV 动画、`g_TexU/V` 仿射 UV 与 `g_Ray` 强度近似，尚未复刻完整节点语义。
`g_Transparency` 已解析进材质数据和 phantom summary，当前只作为后续 transparency/glass alpha 行为的稳定输入，尚未直接改变 opacity。

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
- opacity 设为 `0.28`。
- `g_GlassIOR` / `g_GlassThicknessMax` 已解析并进入 uniform，但尚未驱动 opacity、折射或厚度效果。
- WGSL 中降低 diffuse、增加蓝白 tint、增强 fresnel/specular。
- 进入透明 pass。

这不是完整游戏 glass shader，但能避免“实心灰球”，并显示内部模型。

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
3. scene pass：
   - renderer 先计算第一版 `PreparedModel`，为每个 mesh 记录 draw role、main-pass 可见性、submesh attribute metadata、attribute visibility 和 `PreparedMaterial`；`PreparedMaterial` 会把材质 alpha/render mode 与 mesh draw role 合成 `Opaque`、`Cutout`、`Transparent`、`Glass`、`AdditiveLightShaft` 五类 prepared render pass，并记录第一版 shader family 分类、texture bindings、texture sampling policy、material feature flags 和 UV source。
   - opaque pipeline：写 depth，绘制 `Opaque` batch。
   - cutout pipeline：写 depth，绘制 `Cutout` batch；当前仍由 WGSL alpha test discard。
   - transparent pipeline：alpha blending，不写 depth，绘制 `Transparent` batch。
   - glass pipeline：alpha blending，不写 depth，绘制 `Glass` batch；仍沿用现有 glass 近似参数。
   - additive pipeline：additive blending，不写 depth，绘制 `AdditiveLightShaft` batch。
   - opaque/cutout/transparent/glass/additive 各有 backface 与 culled pipeline，按材质 `render_backfaces` 选择。
   - `PreparedMesh` 先过滤非 surface：shadow、terrainShadow、verticalFog 不进入当前渲染；lightShaft 不作为普通 surface，但会分类为 `AdditiveLightShaft` 并保留到 additive pass；materialChange/crestChange 暂作为 debugVisible 绘制；mesh category glass 会强制进入 `Glass` prepared pass。
4. bloom pass：从 bright attachment 提取高亮并 blur。
5. compose pass：scene + bloom 输出到 canvas。

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

1. Prepared draw role / pass：`PreparedModel` / `PreparedMesh` 已完成第一步主 pass 过滤并保留 submesh attribute metadata；显式 `enabledAttributeMask` 输入已可隐藏 disabled submesh，默认离线模式仍保持不过滤；renderer 内部已有 `Opaque/Cutout/Transparent/Glass/AdditiveLightShaft` prepared pass 分类，Cutout/Glass 已有独立 wgpu pipeline 入口，`AdditiveLightShaft` 已进入最小 additive wgpu pipeline 并消费第一组 lightshaft 参数；后续还需要 runtime shape visibility、更完整的 cutout/glass shader 行为和 lightshaft 节点语义。
2. GPU 顶点格式：uv1-uv3、color1、secondary normal/bitangent、flow 已进入 GPU 顶点输入；PreparedMaterial 已有第一版 feature flags 和 UV source，其中 `usesFlow` 已由 `PreparedModel` 按 mesh 顶点属性汇总；下一步是按 shader family、UV source 与 flags 实际消费这些通道。
3. Glass：参考 Meddle/Penumbra shader key 和 material params，解析更多 glass 参数，而不是固定 0.28。
4. Material params：`g_AlphaThreshold`、`g_Transparency`、`g_GlassIOR`、`g_GlassThicknessMax`、`g_NormalScale`、`g_MultiNormalScale`、`g_DetailNormalScale`、`g_MultiDetailNormalScale`、`g_TileIndex`、`g_TileAlpha`、`g_TileScale`、`g_DetailID`、`g_MultiDetailID`、`g_DetailColor`、`g_MultiDetailColor`、`g_DiffuseColor`、`g_MultiDiffuseColor`、`g_EmissiveColor`、`g_MultiEmissiveColor`、`g_DetailColorUvScale`、`g_DetailNormalUvScale`、`g_UVScrollTime` 以及 `lightshaft.shpk` 的 `g_Color/g_TexAnim/g_TexU/g_TexV/g_Ray` 已进入结构化材质字段；后续要继续接入其它 shader family 参数，并让 transparency alpha、glass IOR/thickness、multi/detail normal、tile select、detail tint/UV、shader diffuse/emissive 与 shader-family-specific UV scroll 更完整地参与 shader。
5. Tile/Sphere/Sheen：renderer 已消费 ColorTable extra maps 做第一版近似；PreparedMaterial 已保留第一版 UV source；后续要接入 tile array、shader-family-specific UV source 和更接近 MeddleTools 的 reflection/sphere 规则。
6. 纹理采样配置：数据层已有第一版 role policy，renderer 已从 prepared policy 派生 color/data/nearest-data sampler，并已绑定 `_id.tex` 与 ColorTable extra maps；后续还要让真实 tile/detail array 等 nearest 资源进入 runtime 绑定。
7. 染色：接入 ColorDyeTable + `chara/base_material/stainingtemplate.stm`。
8. 特殊 shader：lightshaft 已有最小 additive 参数消费；transparency、scroll、reflection、stockings、tattoo、occlusion 等已先进入 shader package 分类；后续还要补 emissive 与各 family 的实际 shader 行为。
