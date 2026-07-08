# 幻梦武器渲染测试列表

这份清单用于后续调试 `xx之幻梦` 系列武器的渲染正确性。条目来自
`assets/craft-data.json` 中物品名包含 `之幻梦` 的记录，共 23 件。

机器可读版本见 `tests/fixtures/phantom_weapons.json`。后续 native snapshot
runner 可以按 `itemId` 从 WeaponCatalog / SqPack 解析模型并批量出图。

## 调试顺序

P0 先看当前已知问题：

- `45052 奶油之幻梦`：深度/遮挡问题。
- `45053 茶歇之幻梦`：透明、颜色、背面偏灰问题。

P1 再看文档中已经暴露过路径或材质特殊性的样本：

- `45050 逗猫之幻梦`：材质路径解析样本。
- `45058 浪漫之幻梦`：packed model id 的 body/variant 顺序样本。
- `45059 冬雪之幻梦`：glass / 透明外壳样本。

P2 最后跑全量覆盖，确认其它幻梦武器没有被局部修复带偏。

## 检查项

- 模型能加载成功，`loaded_paths` 与预期武器路径一致。
- 截图非空，模型居中且比例合理。
- base color / ColorTable 烘焙结果可辨识。
- 透明或 glass 材质不会错误遮挡内部结构。
- 背面、薄片、双面几何不会出现明显灰黑异常。
- bloom / emissive 不应吞掉主体颜色。

## 列表

| 优先级 | Item ID | 名称 | ItemUICategory | Icon | 页面 hash | 关注点 |
| --- | ---: | --- | ---: | ---: | --- | --- |
| P2 | 45047 | 烹饪之幻梦 | 2 | 30689 | `#/weapon-models?item=45047` | baseline |
| P2 | 45048 | 鲨滩之幻梦 | 1 | 30853 | `#/weapon-models?item=45048` | baseline |
| P2 | 45049 | 刺球之幻梦 | 3 | 31263 | `#/weapon-models?item=45049` | baseline |
| P1 | 45050 | 逗猫之幻梦 | 5 | 31665 | `#/weapon-models?item=45050` | material path resolution |
| P2 | 45051 | 丛林之幻梦 | 4 | 32053 | `#/weapon-models?item=45051` | baseline |
| P0 | 45052 | 奶油之幻梦 | 9 | 32484 | `#/weapon-models?item=45052` | depth / occlusion |
| P0 | 45053 | 茶歇之幻梦 | 7 | 32899 | `#/weapon-models?item=45053` | transparency / color / gray backfaces |
| P2 | 45054 | 旅途之幻梦 | 10 | 37828 | `#/weapon-models?item=45054` | baseline |
| P2 | 45055 | 仙韵之幻梦 | 98 | 37829 | `#/weapon-models?item=45055` | baseline |
| P2 | 45056 | 聚餐之幻梦 | 84 | 33639 | `#/weapon-models?item=45056` | baseline |
| P2 | 45057 | 春意之幻梦 | 87 | 34018 | `#/weapon-models?item=45057` | baseline |
| P1 | 45058 | 浪漫之幻梦 | 88 | 34512 | `#/weapon-models?item=45058` | packed id body/variant order |
| P1 | 45059 | 冬雪之幻梦 | 89 | 34711 | `#/weapon-models?item=45059` | glass transparency / inner depth |
| P2 | 45060 | 花伞之幻梦 | 96 | 36567 | `#/weapon-models?item=45060` | thin / broad surface |
| P2 | 45061 | 雨滴之幻梦 | 97 | 36866 | `#/weapon-models?item=45061` | baseline |
| P2 | 45062 | 深海之幻梦 | 106 | 36109 | `#/weapon-models?item=45062` | baseline |
| P2 | 45063 | 夏火之幻梦 | 107 | 36308 | `#/weapon-models?item=45063` | baseline |
| P2 | 45064 | 秋收之幻梦 | 108 | 37279 | `#/weapon-models?item=45064` | baseline |
| P2 | 45065 | 盛宴之幻梦 | 109 | 37075 | `#/weapon-models?item=45065` | baseline |
| P2 | 45066 | 馋扠之幻梦 | 110 | 37442 | `#/weapon-models?item=45066` | baseline |
| P2 | 45067 | 艺术之幻梦 | 111 | 37652 | `#/weapon-models?item=45067` | baseline |
| P2 | 45068 | 菜蔬之幻梦 | 11 | 30278 | `#/weapon-models?item=45068` | off-hand / shield |
| P2 | 45069 | 好戏之幻梦 | 105 | 39909 | `#/weapon-models?item=45069` | baseline |
