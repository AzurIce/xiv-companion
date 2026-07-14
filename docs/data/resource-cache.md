# 资源缓存数据库

## 数据库

| 属性 | 值 |
|---|---|
| 名称 | `xiv-companion-resource-cache` |
| 版本 | `1` |
| 实现 | `src/app/indexed_db_cache.rs` |

该数据库保存 ResourceHub 使用的可恢复数据资源。资源内容统一序列化为字节，不在 IndexedDB value 内展开其领域结构。

## `resources`

### Key

Key 是资源类型的稳定字符串。目前存在：

| Key | 字节内容 |
|---|---|
| `craft-data` | `CraftDataPackage` JSON |
| `weapon-catalog` | `WeaponCatalogPackage` JSON |
| `collection-catalog` | `CollectionCatalogPackage` JSON |

### Value

```ts
interface CachedResourceRecord {
  fingerprint: string;
  sourceTag: "builtin" | "local" | string;
  gameVersion: string;
  schemaRevision: number;
  recordCount: number;
  savedAt: string;
  bytes: Uint8Array;
}
```

| 字段 | 存储类型 | 含义 |
|---|---|---|
| `fingerprint` | `string` | 数据内容指纹，当前通常由游戏版本、资源 revision 和 schema revision 组成 |
| `sourceTag` | `string` | 缓存内容来源；当前主要是 `builtin` 或 `local` |
| `gameVersion` | `string` | 资源对应的游戏版本 |
| `schemaRevision` | `number` | 解码该资源所需的 schema 版本 |
| `recordCount` | `number` | 资源中的逻辑记录数 |
| `savedAt` | `string` | 写入缓存的 ISO 8601 时间 |
| `bytes` | `Uint8Array` | 序列化后的完整资源内容 |

### 操作语义

- 首次读取且缓存不存在时，从 bundled asset 写入。
- builtin 缓存落后于 bundled manifest 时，可以自动替换。
- local 缓存不会仅因为 builtin 游戏版本更新而被静默覆盖。
- schema 不兼容的 local 缓存会被丢弃并回退到 builtin。
- “恢复 builtin”会删除对应 key，再写入当前 bundled 资源。

资源是单条整体替换，不对 `bytes` 内部的物品或配方进行 IndexedDB 级别的增量更新。
