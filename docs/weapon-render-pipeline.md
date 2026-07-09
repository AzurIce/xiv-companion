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
- 每个 mesh/submesh range 转成 `WeaponModelMesh`。
- 数据层保留：position、blend weights/indices、normal、uv0-uv3、bitangent、secondary normal/bitangent、color0/color1、flow0/flow1、index、material_index、mesh category、bone table。
- mesh category 会映射成 `ModelMeshDrawRole`，作为 renderer-friendly 的第一步 prepared draw role。
- ignored phantom snapshot 的 `model-summary.json` 会输出 mesh category、submesh attributes、bone table、shape 影响摘要，并链接 full MDL metadata JSON。

注意：renderer 当前 GPU 顶点格式已上传 position、normal、uv0-uv3、bitangent、color0/color1、secondary normal/bitangent、flow0/flow1，WGSL `VertexInput` 也已声明对应 location；fragment shader 目前仍主要消费 uv0、primary normal/bitangent 和 color0。

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
   - renderer 先为每个 draw batch 计算 `PreparedMaterial`，把材质 alpha/render mode 与 mesh draw role 合成 `Opaque`、`Cutout`、`Transparent`、`Glass` 四类 prepared render pass，并记录第一版 shader family 分类、texture bindings 和 texture sampling policy。
   - opaque pipeline：写 depth，绘制 `Opaque` 与 `Cutout` batch；`Cutout` 仍由 WGSL alpha test discard。
   - transparent pipeline：alpha blending，不写 depth，绘制 `Transparent` 与 `Glass` batch。
   - opaque/transparent 各有 backface 与 culled pipeline，按材质 `render_backfaces` 选择。
   - `ModelMeshDrawRole` 先过滤非主 surface：shadow、terrainShadow、verticalFog、lightShaft 不进入当前主 pass；materialChange/crestChange 暂作为 debugVisible 绘制；mesh category glass 会强制进入 `Glass` prepared pass。
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

1. Prepared draw role / pass：`ModelMeshDrawRole` 已完成第一步主 pass 过滤，renderer 内部已有 `Opaque/Cutout/Transparent/Glass` prepared pass；后续还需要独立 cutout/glass/additive-lightshaft wgpu pipeline。
2. GPU 顶点格式：uv1-uv3、color1、secondary normal/bitangent、flow 已进入 GPU 顶点输入；下一步是按 shader family 实际消费这些通道。
3. Glass：参考 Meddle/Penumbra shader key 和 material params，解析更多 glass 参数，而不是固定 0.28。
4. Tile/Sphere/Sheen：renderer 已消费 ColorTable extra maps 做第一版近似；后续要接入 tile array、UV source 和更接近 MeddleTools 的 reflection/sphere 规则。
5. 纹理采样配置：数据层已有第一版 role policy，renderer 已区分 color/data/nearest-data sampler；后续还要让 index/tile array 等 nearest 采样进入 runtime 绑定，尤其 `_id.tex` 不应统一 linear 采样。
6. 染色：接入 ColorDyeTable + `chara/base_material/stainingtemplate.stm`。
7. 特殊 shader：emissive、scroll、reflection、transparency、stockings 等按 shader package 分类实现。
