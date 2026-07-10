# Collection（图鉴）与资源库设计

本文档描述当前已经落地的数据资源架构、图鉴领域模型和 UI 信息架构。

## 1. ResourceHub

### 1.1 概念边界

- `ResourceSource` 表示读取/存储位置，例如 IndexedDB、内置资源或用户目录。
- `ResourceOrigin` 表示数据内容的来源，例如 builtin 或 local SqPack。
- `ResourceMetadata` 保存实际游戏版本、revision、保存时间和条目数。
- `ResourceStatus` 是资源库 UI 的统一状态对象。
- `ResourceSnapshot` 由 `LoadedResource<T>` 表达，包含数据、存储位置和来源元数据。

可序列化目录统一存入 IndexedDB。WeaponModel 等大体积运行时资源继续从本地目录按需读取，并显式依赖 WeaponCatalog。

### 1.2 管理接口

页面只通过 ResourceHub 调用：

```rust
load_with_source<R>()
status<R>()
refresh<R>(ResourceOrigin::UserLocal)
reset<R>()
```

UI 不直接调用或识别 `IndexedDbCachedProvider`。

### 1.3 builtin 更新规则

1. IndexedDB 为空时写入 builtin。
2. 缓存来源为 builtin 时，schema revision 提升或 bundled 游戏版本更新会自动替换。
3. 缓存来源为 local 时，不使用较新的 builtin 静默覆盖。
4. local schema 已不兼容时回退 builtin，并允许用户重新执行本地更新。
5. 版本比较按数字段进行，不使用普通字符串排序。

## 2. 图鉴数据模型

### 2.1 类型与主键

图鉴类型为：

- 装备
- 乐谱
- 坐骑
- 宠物
- 时尚配饰
- 情感动作
- 传习录

稳定主键是 `CollectionEntryKey { kind, row_id }`，持久化格式如 `mount:123`。旧版只保存 Item ID 的状态会在首次读取新版目录时迁移。

### 2.2 分类语义

类型优先通过 ItemAction 类型判断：

| 类型 | ItemAction Type |
|---|---:|
| 宠物 | 853 |
| 坐骑 | 1322 |
| 情感动作 | 2633 |
| 传习录 | 4107 |
| 时尚配饰 | 20086 |
| 乐谱 | 25183 |

装备通过 `EquipSlotCategory != 0` 判断，灵魂水晶不进入装备图鉴。ItemUICategory 只作为原始字段保留，不承担顶层信息架构。

### 2.3 装备分组

每件装备导出：

- `expansion` / `patch`
- `item_series` / `set_id` / `set_name`
- `class_job_category`
- `level_equip` / `level_item`
- `slot_name` / `slot_order`
- `appearance_key`

套装优先使用 `ItemSeries`。没有 ItemSeries 时，按装备模型的 domain、model id 和 variant 分组。武器、普通装备和首饰使用不同 domain，避免相同 packed model 数值跨模型命名空间碰撞。

同模提示只在装备内部按 `appearance_key` 判断。

### 2.4 版本来源

当前 SqPack 只能提供当前快照，不能直接给出物品首次加入版本。Builtin 生成时会读取本地 `ffxiv-datamining-cn` 的 Item.csv 历史：

- 每个物品标记首次出现的 patch/build。
- 仓库最早快照已有的物品归入“历史版本 / 4.45 及以前”。
- Browser local 更新复用 builtin 已知版本；只存在于本地的新条目使用当前本地游戏 build。

## 3. UI 信息架构

### 3.1 图鉴

顶层使用固定类型 Tab。装备 Tab 有两个分组模式：

- `版本·套装`：资料片 -> patch -> 套装卡片。
- `同模`：按 appearance key 展示同模型条目。

套装卡片显示职业、最高品级、获得进度和各装备部位。搜索、职业、部位与获得状态筛选作用于当前 Tab。结果按 80 组/条分页追加，不再静默截断。

### 3.2 资源库

资源库只保留一份资源状态表。每行显示：

- 中文资源名称和用途
- 当前来源、游戏版本、条目数
- 更新中、重置中、成功或错误状态
- 本地更新与恢复 builtin 操作

WeaponModel 显示为“本地按需读取”，并说明其本地目录与武器索引依赖。

## 4. 生成命令

```powershell
cargo run -p xtask-update-craft-data -- `
  --game-dir E:\_ff14\game `
  --datamining-repo E:\_ff14\ffxiv-datamining-cn
```

Windows 可能因为可执行文件名含 `update` 触发提权启发式，此时可将构建后的 exe 复制为普通名称再执行。
