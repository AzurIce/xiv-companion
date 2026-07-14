# IndexedDB 数据设计

本文档目录记录 Web 端当前对 IndexedDB 的全部使用。IndexedDB 数据分为资源缓存、用户状态和浏览器授权三类，分别使用独立数据库，避免缓存清理影响用户数据或目录授权。

## 数据库总览

| 数据库 | 版本 | ObjectStore | 用途 | 详细文档 |
|---|---:|---|---|---|
| `xiv-companion-resource-cache` | 1 | `resources` | 缓存内置或本地 SqPack 生成的数据资源 | [resource-cache.md](resource-cache.md) |
| `xiv-companion-collection-state` | 3 | `collections` | 以 Item ID 保存用户已获得的图鉴条目 | [collection-state.md](collection-state.md) |
| `xiv-companion-local-source` | 2 | `state` | 保存目录句柄及后续应用状态 | [local-source.md](local-source.md) |

当前所有 ObjectStore 都使用显式 key，不设置 `keyPath`、自增主键或二级索引。

## 生命周期与所有权

### 资源缓存

`xiv-companion-resource-cache` 中的数据可以从 builtin 资源或用户选择的本地 SqPack 重新生成，属于可恢复缓存。应用会根据游戏版本、schema revision 和来源决定是否更新或替换缓存。

### 用户状态

`xiv-companion-collection-state` 是用户拥有的数据，不能跟随资源缓存一起清理。图鉴目录定义“有哪些条目”，该数据库只定义用户标记了哪些条目为已获得。

### 浏览器授权

`xiv-companion-local-source` 保存浏览器原生 `FileSystemDirectoryHandle`。句柄可被持久化，但读取权限仍由浏览器控制；恢复后应用必须重新查询权限和目录布局。

## 类型约定

文档中的类型分为两层：

- 逻辑类型：Rust 或领域模型中的含义，例如 `u32`、`usize`、ISO 8601 时间。
- 存储类型：IndexedDB 中实际保存的 JavaScript 值，例如 `string`、`number`、`Uint8Array`、`FileSystemDirectoryHandle`。

修改数据库名、版本、ObjectStore、key 格式或 value 结构时，应同步更新本目录文档。需要结构迁移时必须提升对应数据库版本，并在 upgrade 回调中完成兼容处理。
