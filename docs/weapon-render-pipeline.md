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

候选读取使用静默探测：缺失候选不输出 warn，只有全部失败才返回错误。

## 3. MDL → Mesh

使用 Physis 解析 MDL：

- 取 LOD0。
- 每个 part 转成 `WeaponModelMesh`。
- 保留：position、normal、uv0、uv1、bitangent、vertex color、index、material_index。

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

当前实现：先解析 MTRL sampler usage，路径后缀 `_id` 强制覆盖为 `Index`。

## 6. Dawntrail ColorTable + `_id.tex` 烘焙

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
3. 烘焙出 sRGB RGBA base texture。
4. 若 ColorTable 有 emissive，则额外烘焙 emissive texture。

只对 Dawntrail 32 行 ColorTable 启用；旧 16 行格式先保守回退，避免误烘焙。

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
- ColorTable `TileAlpha` 全部为 1.0。
- 透明度并不直接由 TileAlpha 给出，而是 glass shader 语义控制。

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
   - normal texture
   - mask texture
   - emissive texture
3. scene pass：
   - opaque pipeline：写 depth，不透明材质先画。
   - transparent pipeline：alpha blending，不写 depth，透明/glass 后画。
4. bloom pass：从 bright attachment 提取高亮并 blur。
5. compose pass：scene + bloom 输出到 canvas。

透明排序目前只做到“透明材质在不透明之后”，还没有逐三角/逐 mesh 按深度排序。

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
- `characterglass` 需要单独模式。

## 10. 已知限制与后续计划

当前实现目标是“武器模型预览可辨识、颜色/透明大体正确”，仍非完整 FFXIV shader 复刻。

后续优先级：

1. 透明排序：对透明 mesh 按相机距离排序，至少 mesh-level back-to-front。
2. Glass：参考 Meddle/Penumbra shader key 和 material params，解析更多 glass 参数，而不是固定 0.28。
3. Mask/物理参数：从 ColorTable 烘焙 roughness/metalness/specular，而不是大量使用材质平均值。
4. Legacy ColorTable：补旧 16 行格式的正确索引路径。
5. 染色：接入 ColorDyeTable + `chara/base_material/stainingtemplate.stm`。
6. 特殊 shader：emissive、scroll、reflection、transparency、stockings 等按 shader package 分类实现。
