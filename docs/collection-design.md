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

套装关系不使用模型 ID 推断。模型复用属于外观语义，不能代表游戏里的多部位装备套装。

套装按以下优先级生成：

1. `FittingShopItemSet` 中有正式名称的商城/试穿套装。
2. `MirageStoreSetItem` 中游戏定义的套装幻影化组合。
3. 其余普通副本、团队副本、点数和制作装备，按装备等级、品级、职业限制、稀有度、强化/颜色变体与中文名称族跨部位归组。
4. 武器、无法形成至少两个不同部位的条目保持单件，不伪装成套装。

自动推导组限制在合理套装规模内，并对“过期”“风化”等通用前缀做排除。图鉴页面不提供模型分组。

### 2.4 版本来源

当前 SqPack 只能提供当前快照，不能直接给出物品首次加入的具体补丁。Builtin 生成时会结合 Garland Tools 的历史补丁表、本地 `ffxiv-datamining-cn` 的 `ExVersion.csv` 与 `Item.csv` 历史：

- 后续物品按 `Item.csv` Git 历史标记首次出现的 patch/build。
- 固定在仓库内的 Garland Tools `Supplemental/patches.json` 仅补充 4.45 以前的精确物品版本，不覆盖较新的 datamining 结果。
- Garland 中标记为 1.0 的记录视为未细分的旧版 1.x 数据，显示为“旧版遗留 / 1.x（具体版本未知）”，不宣称其精确发布于 1.0。
- `ExVersion` 的起始 Item ID 用于确定 Garland 未收录条目的资料片，并回退到 2.x、3.x 或 4.45 及以前。
- Browser local 更新复用 builtin 已知版本；只存在于本地的新条目使用当前本地游戏 build。

## 3. UI 信息架构

### 3.1 图鉴

顶层使用固定类型 Tab。资料片作为所有图鉴类型共享的单选筛选，切换装备、乐谱、坐骑等类型时保留当前资料片。装备在当前资料片下完整列出全部 patch 折叠分组。

至少包含两个不同部位的套装使用默认展开的套装块，显示装备等级、品级范围、职业、部位数、获得进度和各装备部位。无法组成套装的装备直接以独立物品卡显示，不再套一层“单件”容器。搜索、职业、部位与获得状态筛选作用于当前资料片。所有类型均直接显示当前筛选结果，不再提供“继续加载”，并使用稳定 key 复用已有组件。

获得状态使用按条目寻址的 Dioxus Store。勾选时只更新对应条目及相关套装进度，并在 IndexedDB 中单条增删，不再克隆、序列化和重绘整份获得集合。

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
  --game-dir 'G:\最终幻想XIV\game' `
  --datamining-repo E:\_ff14\ffxiv-datamining-cn
```

Windows 可能因为可执行文件名含 `update` 触发提权启发式，此时可将构建后的 exe 复制为普通名称再执行。
