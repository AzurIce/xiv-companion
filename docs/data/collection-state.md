# 图鉴状态数据库

## 数据库

| 属性 | 值 |
|---|---|
| 名称 | `xiv-companion-collection-state` |
| 版本 | `3` |
| 实现 | `src/app/collection_state.rs` |

该数据库只保存用户主动标记为已获得的 Item ID，与图鉴目录、分类和版本数据分离。

## `collections`

每个已获得物品对应一条 record，使用 out-of-line key。

### Key

Item 表的 `u32` Row ID，以十进制字符串保存：

```text
12345
33041
45678
```

分类不属于持久化 key。同一个物品在分类规则调整后仍使用相同的获得状态。

### Value

```ts
true
```

Value 是占位值。record 存在表示已获得；未获得不保存 record，取消获得时直接删除对应 key。因此该 store 在逻辑上是一个稀疏的 `Set<ItemId>`。

### 读写方式

- 页面启动：一次 `getAllKeys()`，在内存中构造 `HashSet<u32>`。
- 页面查询：只查询内存 Set，不逐项访问 IndexedDB。
- 单项勾选：`put(itemId, true)`。
- 单项取消：`delete(itemId)`。
- JSON 导入：在一个读写事务中执行 `clear()` 和逐项 `put()`。

数据库从 v2 升级到 v3 时，会将 `entries` 中的 `{collection-kind}:{item-id}` key 转换为纯 Item ID，然后删除 `entries`。旧 `state` store 不再参与兼容读取，会在升级时删除。

## 导入导出

JSON 格式带独立 schema 版本：

```json
{
  "schemaVersion": 1,
  "items": [12345, 33041, 45678]
}
```

导出时 Item ID 按数字升序排列。导入会校验 schema 版本和 Item ID，并自动去重；导入是整体替换，不与现有状态合并。

## 内存表示

```rust
HashSet<u32>
```

图鉴目录定义当前可展示的物品，用户 Set 可以保留暂时不在当前目录中的 ID，避免目录或分类变化丢失用户状态。
