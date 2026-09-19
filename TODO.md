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
- [ ] 身高/胸围等体型缩放（RGSP）未实现
  - `CharacterCustomize` height(第 3 字节)/bust(第 23 字节)已解析未消费；游戏按 human.cmp
    尾部 32×140 字节 RacialScalingParameter 表对骨骼缩放，身体与装备同骨架联动。接入点在
    rest-pose 构建（skeleton.rs rest_pose/world_matrices），须在种族变形烘焙之前应用。
    参照 xivModdingFramework `CharaMakeParameter.cs`。
- [ ] `smallclothes_model_race_candidates`（chara_assemble.rs:80）与 EQDP+PBD 变形树语义不一致
  - EQDP set 1 + 变形树给出 c1601/c1801→c0201、c1701→c0101，而现表为 c1001/c0701/c0801
    （仅 c1501→c0901 一致）。现有裸装快照以现表为准，改动需重验全部角色装配快照。

## 幻化套装

- [ ] 幻化 3D 预览支持主手/副手武器（挂手骨）
  - 武器是未蒙皮独立模型，游戏运行时挂到手部骨骼；渲染侧尚无"未蒙皮子模型绑定到关节矩阵"的
    变换路径。需要在 renderer 增加 bone-attach 机制（关节矩阵 × 绑定偏移），dressed loader
    的槽位数据已就绪（武器槽位当前在 UI 层被过滤，见 glamour.rs 的武器提示文案）。
- [ ] 头部装备的耳饰/耳朵显隐（EQP bits 47-53）与跨槽规则 BodyShowHead(10)、ShowTail(13) 未实现
  - 当前 met 槽只处理头发遮蔽（bits 41-44）；耳饰件始终显示。触发时
    `apply_dressed_visibility` 会输出诊断 eprintln（crates/xiv-companion-data/src/weapon_models.rs）。
- [ ] 套装级角色形象参数保存（当前预览形象是页面级 `previewCustomize`，见 glamour_state.rs）
- [ ] 从游戏导入套装（幻化柜/投影台）：inventory bridge 已知 GlamourDresser 容器，但桥协议
    `InventoryItemEntry` 只有 item_id/quantity/hq，无染色字段，需先扩展协议。

## Resolved

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
