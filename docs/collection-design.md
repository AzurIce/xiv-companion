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

图鉴目录中的条目来自 Item 表，用户获得状态以 Item Row ID 作为稳定主键。分类属于可调整的目录视图，不进入用户状态主键。

### 2.2 分类语义

分类定义集中在 `collection_classification.rs`。同一份注册表负责分类顺序、中文名称、实验状态和是否按详细版本分组，生成器与 UI 不再各自维护分支。

分类采用“候选全集减法”模型：

1. 装备和已登记的永久解锁 `ItemAction.Type` 先成为图鉴候选。
2. 非装备候选默认归入“其他解锁”。
3. 有序规则按 first-match-wins 将候选认领到更具体的分类。
4. 新永久解锁类型只需加入候选 action type 表；在编写细分规则前也一定会显示在“其他解锁”。

不能把所有非零 ItemAction 都视为候选。装备箱、随机卡包、临时状态道具和普通消耗品仍由候选 action type 白名单排除。

当前主要 ItemAction 类型包括：

| 类型 | ItemAction Type |
|---|---:|
| 宠物 | 853 |
| 坐骑 | 1322 |
| 情感动作 | 2633 |
| 传习录 | 4107 |
| 时尚配饰 | 20086 |
| 乐谱 | 25183 |

每次生成都会执行守恒检查：候选数必须等于各分类条目数之和；并输出“其他解锁”按 `ItemAction.Type` 的分布，便于发现值得新增的分类规则。主要 action type 和通用解锁名称规则都有代表性测试。

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

顶层分类使用注册表驱动的单选 Tag 云。资料片作为所有图鉴类型共享的单选筛选，切换装备、乐谱、坐骑等类型时保留当前资料片。装备在当前资料片下完整列出全部 patch 折叠分组。

`ItemAction.Type == 2633` 是通用解锁动作，不等同于情感动作。生成时按中文物品语义拆分为情感动作、发型与面妆、详细地图、方城声援、肖像教材及其他解锁，避免把所有解锁型物品混入情感动作。

其他永久解锁动作还包括肖像教材旧类型、九宫幻卡、陆行鸟装甲、制作秘籍、战果/调查记录、面部配饰、新月岛辅助职业及成就证书。九宫幻卡、鸟甲、面部配饰、生产秘籍使用独立分类；青魔法图腾、建造许可证书、成就证书、战果/调查记录和辅助职业收入“其他解锁”。装备箱、随机卡包、临时状态道具等一次性消耗效果不属于收藏图鉴。

至少包含两个不同部位的套装使用默认展开的套装块，显示装备等级、品级范围、职业、部位数、获得进度和各装备部位。无法组成套装的装备直接以独立物品卡显示，不再套一层“单件”容器。搜索、职业、部位与获得状态筛选作用于当前资料片。所有类型均直接显示当前筛选结果，不再提供“继续加载”，并使用稳定 key 复用已有组件。

获得状态在内存中使用 `HashSet<u32>`。页面启动时通过一次 IndexedDB `getAllKeys()` 构造 Set，之后的查询只访问内存；勾选时乐观更新 Set，并在 `collections` store 中按 Item ID 单条增删。导入导出使用带 schema 版本的 JSON，导入在单个事务中整体替换。

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
