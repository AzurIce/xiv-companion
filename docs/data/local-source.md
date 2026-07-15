# 应用状态数据库

## 数据库

| 属性 | 值 |
|---|---|
| 名称 | `xiv-companion-local-source` |
| 版本 | `2` |
| 实现 | `src/app/user_local_directory.rs` |

该数据库保存需要跨页面生命周期恢复的浏览器应用状态。当前保存用户通过 File System Access API 选择的游戏目录句柄，后续可增加设置等状态键。

## `state`

当前记录：

| Key | Value |
|---|---|
| `user-local-game` | `FileSystemDirectoryHandle` |

目录句柄是浏览器原生的结构化克隆对象，不是路径字符串，也不是应用定义的 JSON。出于浏览器安全模型，应用无法通过该记录直接获得任意文件系统路径。

数据库从 v1 升级到 v2 时，会将 `directories/user-local-game` 原子迁移到 `state/user-local-game`，然后删除旧 `directories` store。

## 恢复流程

1. 从 `state/user-local-game` 读取目录句柄。
2. 查询句柄的读取权限，结果可能为 `granted`、`prompt`、`denied` 或未知。
3. 检查目录是 `game` 目录、安装根目录，还是缺少 `sqpack` 布局。
4. 将本次页面生命周期内可用的句柄保存到 `window.__xivCompanionUserLocalDirectory`。

IndexedDB 中存在句柄不代表浏览器仍授予读取权限。权限被撤销或浏览器不允许静默恢复时，用户需要重新选择目录。
