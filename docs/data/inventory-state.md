# 本地物品状态

物品页面通过 XIV Local Bridge 的 `inventory.containers`、`inventory.container` 和对应变化事件读取当前角色的逻辑物品容器。

## 状态作用域

库存状态同时保存在页面内存和独立的 IndexedDB 数据库 `xiv-companion-inventory-state` 中。数据库当前保存一份 `latest` 完整快照：

- 目录、容器描述符、完整容器内容、revision 和保存时间一起持久化。
- 页面启动时先恢复上次快照，即使 bridge 未连接也可以查看。
- Bridge URL 和验证状态仍单独保存在 `localStorage`。
- `cached` 内容来自游戏自己的 ItemFinder 本地缓存，可能已经过期。
- 雇员 `ownerId` 只在生成该快照的登录会话内有意义，不能作为跨快照身份。
- 当前没有角色作用域 ID，因此持久化快照必须显示为“最后已知数据”，不能宣称属于当前登录角色。

物品页的“全量刷新”会重新连接 bridge、读取完整目录以及所有 `live`/`cached` 容器。全部预期容器响应完成后，才整体写入 IndexedDB，避免保存半套刷新结果。

## 连接流程

1. 连接成功后请求 `inventory.containers`。
2. 对目录中 `live` 和 `cached` 的容器并发请求 `inventory.container`。
3. 目录响应和 `inventory.containers.changed` 都按完整目录处理，并删除已经不存在的容器描述符。
4. `inventory.container.changed` 携带完整容器快照；只有 revision 不小于当前值时才覆盖。
5. change 事件更新内存后防抖写入 IndexedDB。
6. `session.logout` 保留最后快照用于离线查看，并将连接状态改为等待登录。
7. `session.login` 自动重新请求目录。

页面不维护事件历史，也不尝试从漏失的增量补丁恢复，因为容器变化事件本身就是完整快照。

## 展示语义

- `live`：内容仍由当前客户端已加载的内存结构提供，不代表对应游戏窗口仍然打开。
- `cached`：游戏保存的上次可见内容；页面显示缓存提示。
- `notLoaded`：容器存在但没有可报告内容，不能显示为空库存。

### 窗口关闭与换区

投影台和收藏柜等按需数据需要先在游戏中打开相应界面，客户端才会向服务器请求并加载内容。界面关闭后，游戏通常不会立即清除已经加载的内存，因此“窗口已关闭”和“数据已卸载”不能视为同一个事件。

以投影台为例，bridge 按以下优先级判断状态：

1. `MirageManager.PrismBoxLoaded` 仍为真时，从 `PrismBoxItemIds` 读取内容并报告 `live`。
2. 客户端内存已经卸载，但 `ItemFinderModule.IsGlamourDresserCached` 可用时，从游戏缓存读取并报告 `cached`。
3. 实时内存和游戏缓存都不可用时报告 `notLoaded`。

因此常见的状态变化是：

```text
尚未访问投影台                         notLoaded
打开投影台并完成加载                   notLoaded -> live
关闭投影台，但 PrismBox 数据仍在内存    保持 live
换区后 PrismBox 数据被游戏清空          live -> cached
换区后没有可用的 ItemFinder 缓存        live -> notLoaded
再次打开投影台                         cached/notLoaded -> live
```

Bridge 每秒重新捕获一次容器状态。可用性变化属于目录变化，会发送完整的 `inventory.containers.changed`；对应内容快照也会通过 `inventory.container.changed` 更新。前端只消费 bridge 报告的状态，不根据窗口是否可见自行降级。

换区后的具体结果由游戏当前保留的缓存决定，不能保证一定从 `live` 变成 `cached`。无论变成 `cached` 还是 `notLoaded`，客户端都不能把快照中缺失的物品解释为玩家已经失去该物品。

全量刷新遇到 `notLoaded` 容器时会保留其上次持久化内容作为“最后已知的正向记录”，但页面不会把这些条目显示成当前实时库存。图鉴从物品数据更新只做正向合并，因此可以继续利用这些历史可见 Item ID，不会据此删除图鉴记录。

物品名称和图标来自 companion 自己的 CraftData/ItemIcon 资源。Bridge 只传 Item Row ID、数量、HQ 和容器内逻辑槽位，避免重复传输静态游戏数据。
