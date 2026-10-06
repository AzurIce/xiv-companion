# TODO

## Verification Infrastructure

- [ ] Fix `cargo test --features render-test-support` failing to build example
  `dump_gear.rs` without `game-data`.
  - `examples/dump_gear.rs` calls `physis::savedata::gearsets` but `physis` is an optional
    dependency gated behind `game-data`, so any cargo invocation that builds examples without
    `game-data` fails. Found during the 2026-09-15 todo fix pass (out of scope there).
  - Either gate the example's body behind `#![cfg(feature = "game-data")]` with a `main` stub,
    or declare the example with `required-features = ["game-data"]` in Cargo.toml.

## App UI

## 渲染

- [ ] 装备跨种族变形目前是骨架替换近似，不是游戏的 PBD 矩阵
  - 游戏用 `chara/xls/boneDeformer/human.pbd` 的 deform 矩阵（physis 有
    `PreBoneDeformer::get_deform_matrices` 可解析）；当前实现按目标骨架静止姿态烘焙。
    同性且仅一步回退时差异很小，追求完全对齐需接 PBD 矩阵。

## 幻化套装

- [ ] 从游戏导入套装（幻化柜/投影台）：inventory bridge 已知 GlamourDresser 容器，但桥协议
    `InventoryItemEntry` 只有 item_id/quantity/hq，无染色字段，需先扩展协议。

## Resolved

- [x] `smallclothes_model_race_candidates`/`bare_limb_model_race_candidates` 与 EQDP+PBD
  变形树语义不一致（小衣/裸肤回退来源族错误）。
  (resolved: 2026-09-22)
  - 真实数据探针（`probe_smallclothes_bare_limb_race_fallback_eqdp`）验证：EQDP set 0/1
    的 HasModel 与骨变形树祖先链上文件存在性 18 族 × 5 槽一一对应；e0000 文件仅
    中原男/女、鲁加男、拉拉男有（含 glv/sho），e0001 sho 仅此四族（拉拉女除外）。
  - 两表改为派生自装备同一骨变形树函数（`equipment_model_race_candidates`），不再
    手写：e0001 小衣 硌狮男→鲁加男 c0901、硌狮女/维埃拉女→中原女 c0201、维埃拉男→
    中原男 c0101（旧表错用鲁加女/猫魅男/猫魅女身体）；e0000 裸肤手 硌狮男→鲁加男
    c0901（旧表错用中原男），其余族映射不变。
  - 无 e0001 sho 的族同样沿树回退小衣鞋（如敖龙女→中原女）——游戏裸装无赤脚，
    移除 e0000 sho 裸足补件；裸装/着装快照按修正后来源重验（c1601/c1701/c1801 身体
    比例变化为游戏正确结果）。
- [x] 套装级角色形象参数保存（此前预览形象仅为页面级 `previewCustomize`）。
  (resolved: 2026-09-22)
  - `GlamourSet` 新增可选 `customize`（52 字符 hex，normalize/导入与页面级同校验）：
    非空时编辑器形象控件与 3D 预览作用于套装并刷新 updated_at，为空时跟随页面级预览形象；
    复制套装保留绑定，导出/导入随套装携带（schemaVersion 仍为 1）。
- [x] 身高/胸围/尾巴长度等体型缩放（RGSP）未实现。
  (resolved: 2026-09-22)
  - 方案：human.cmp 尾部缩放参数表（80 × 56 字节 `RacialScalingParameter`，
    xivModdingFramework `CharaMakeParameter.cs` 布局——条目下标 base_race×10+subrace，
    仅 16 条有效位；此前文档猜的 32×140 是错的）在装配加载时经本地游戏目录实时解析
    （回退恒等），捏脸字节（0x03 身高 / 0x15 尾长 / 0x17 胸围）线性插值为 `BodyScaling`
    附着到 `ModelSkeleton.body_scaling`。应用为姿势层合成（不烘顶点、不改 inverse bind
    与种族变形烘焙）：`scaled_rest_pose()` 与 `sample_animation_pose` 输出前对
    `n_root`（身高，全模型均匀缩放）/ 尾链根 `n_sippo_a`（尾长，仅猫魅敖龙）/
    `j_mune_l/r`（胸围逐轴，仅女性）局部 scale 相乘，rest 与动画路径一致；装备与武器
    挂接经同一缩放 joint / 挂点骨世界矩阵自动跟随。渲染快照锚定身高 0/50/100 前景
    包围盒递增、胸围极值可见差异与武器挂点随身高比缩放（tests/native_dressed_character.rs）。
  - 已知限制：模型 bounds 保持未缩放值（取景按原始体型）；中部落滑条 50 ≠ scale 1.0
    （骨架本身是基准，如敖龙女 50→0.97）；0x15 对非尾种族是肌肉/耳语义（肌肉走皮肤
    材质 alpha，不做骨缩放）。

- [x] 头部装备的耳饰/耳朵显隐（EQP bits 46-53）、跨槽规则 BodyShowHead(10) 与遮尾
  ShowTail(13)/LegShowTail(22)。
  (resolved: 2026-09-22)
  - 耳饰位修正为按种族分组（中原/鲁加、精灵/拉拉、猫魅/硌狮/维埃拉、敖龙，Penumbra
    EqpEntry，真实 EQP 表部分置位组合证实）；猫魅耳在脸部基础网格内无法按网格隔离，
    不隐藏仅记诊断；尾部 top/腿任一关闭即藏；真实数据探针 + 合并/逐件两路径一致性
    锚定（tests/native_dressed_character.rs 渲染快照目验通过）。

- [x] 幻化 3D 预览支持主手/副手武器（挂手骨）。
  (resolved: 2026-09-21)
  - 方案：武器 MDL（原点在握把、顶点在挂点骨局部空间的刚性模型）在逐件着装加载时烘焙为挂点骨
    单骨蒙皮（bone table 覆写为 [挂点骨]、顶点强制关节 0 权重 1.0），主手槽位（1/13/14）挂
    `n_buki_r`、副手（2）挂 `n_buki_l`，次模型（刀鞘/双持副手）挂 `n_buki_l`；关节矩阵 =
    挂点骨姿势世界 × 常量校正（恒等即可，native 快照目验剑/盾/双手武器落位正确），不走
    inverse bind。场景画布动画驱动按件携带挂接规则（rest 与播放一致），遮蔽计划跳过武器件，
    幻化页移除武器过滤与提示文案。武器不做种族骨变形（刚性件不随体型变化；拉拉肥等种族的
    武器尺寸差异未验证）。
  - 已知限制：带内部可动部件的罕见武器（如部分书）强制单骨后丢失内部动画（记入加载诊断）；
    武器实例包围盒是挂点局部空间值，场景取景并集对超长武器略有偏差（可用轨道相机调整）。

- [x] Fix collapsed-sidebar nav tooltips being clipped by the nav scroll container.
  (resolved: 2026-09-15)
  - Symptom: with the sidebar collapsed (72px), hovering a nav icon shows the label tooltip cut
    off at the sidebar's right edge, appearing covered by the main content.
  - Root cause: `IconTooltip` renders the tooltip as `absolute left-full ... z-50` inside the nav
    item wrapper (`src/app/shell.rs:121-130`), but the nav lives inside
    `div.flex-1.overflow-y-auto` (`src/app/shell.rs:264`). Per CSS Overflow, `overflow-y: auto`
    computes `overflow-x` to `auto`, so the scroller clips in both axes; the tooltip is clipped
    at the scroller's padding box and `z-50` cannot escape an ancestor overflow clip. The
    `-right-3` collapse toggle is unaffected because it sits directly under the
    `overflow-visible` aside, outside the scroller.
  - Fix direction: take the tooltip out of the scroll container — render it with
    `position: fixed` anchored to the trigger's `getBoundingClientRect` on hover (portal at
    shell root), or use the top layer (`popover` attribute); check for new stacking conflicts
    with page modals (`fixed inset-0 z-50` in `src/app/pages/{crafting,collection,notes}.rs`).
  - Relevant code: `src/app/shell.rs` (`IconTooltip`, `DesktopSidebar` nav scroller).

- [x] Fix the deterministic SIGSEGV when running the ignored native GPU snapshot suites with
  parallel test threads. (resolved: 2026-09-15)
  - `cargo test --features render-test-support --test native_weapon_snapshot -- --ignored
    --test-threads=4` crashes with SIGSEGV; `--test-threads=1` passes. Suspected cause: concurrent
    wgpu instances across tests.
  - Until fixed, run these suites single-threaded or add a serialized harness.

- [x] Fix compilation of `tests/weapon_shader_family_audit.rs` under the documented
  `--features web` verification command. (resolved: 2026-09-15)
  - Whole-workspace `cargo test --features web` fails to build: the test references
    `disassemble_dxbc` and related helpers that are not available without additional features.
  - Either gate the test behind the required features or document the full verification command.

## Reference Files

Local clones for checking FFXIV model/material/texture semantics:

- `~/Files/repos/MeddleTools` — clone of [PassiveModding/MeddleTools](https://github.com/PassiveModding/MeddleTools),
  Blender import/material-node tooling for Meddle glTF output
  (`MeddleTools/node_setup/node_configs.py`, `MeddleTools/node_setup/node_mappings.py`,
  `MeddleTools/shaders.blend`).
- `third_party/Meddle` — shallow clone of [PassiveModding/Meddle](https://github.com/PassiveModding/Meddle),
  Dalamud/runtime-oriented exporter
  (`Meddle/Meddle.Utils/Export/Model.cs`, `Meddle/Meddle.Utils/Export/Mesh.cs`,
  `Meddle/Meddle.Utils/Export/Vertex.cs`, `Meddle/Meddle.Formats/Files/MtrlFile/ColorTableRow.cs`).
