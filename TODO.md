# Rendering Pipeline TODO

This document tracks rendering-fidelity gaps found by comparing the current pipeline with the
field and texture semantics used by Meddle and MeddleTools.

MeddleTools is a behavioral reference, not an authoritative copy of the in-game shaders. Its
`shaders.blend` materials are hand-crafted approximations and the project is licensed under
AGPL-3.0-or-later. Keep WGSL implementations independent and use MeddleTools primarily to verify
texture roles, UV routing, material keys, ColorTable fields, and expected feature composition.

## Verification Infrastructure

- [ ] Fix the deterministic SIGSEGV when running the ignored native GPU snapshot suites with
  parallel test threads.
  - `cargo test --features render-test-support --test native_weapon_snapshot -- --ignored
    --test-threads=4` crashes with SIGSEGV; `--test-threads=1` passes. Reproduced on the baseline
    commit, so it is independent of the colorset composition change. Suspected cause: concurrent
    wgpu instances across tests.
  - Until fixed, run these suites single-threaded or add a serialized harness.

- [ ] Fix compilation of `tests/weapon_shader_family_audit.rs` under the documented
  `--features web` verification command.
  - Whole-workspace `cargo test --features web` fails to build: the test references
    `disassemble_dxbc` and related helpers that are not available without additional features.
    Reproduced on the baseline commit.
  - Either gate the test behind the required features or document the full verification command.

## App UI

- [ ] Fix collapsed-sidebar nav tooltips being clipped by the nav scroll container.
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

## Reference Files

- `E:\repos\Meddle\Meddle\Meddle.Utils\Export\Model.cs`
- `E:\repos\Meddle\Meddle\Meddle.Utils\Export\Mesh.cs`
- `E:\repos\Meddle\Meddle\Meddle.Utils\Export\Vertex.cs`
- `E:\repos\Meddle\Meddle.Utils\Files\Structs\Material\ColorTableRow.cs`
- `E:\repos\MeddleTools\MeddleTools\node_setup\node_configs.py`
- `E:\repos\MeddleTools\MeddleTools\node_setup\node_mappings.py`
- `E:\repos\MeddleTools\MeddleTools\bake\bake_utils.py`
- `E:\repos\MeddleTools\MeddleTools\shaders.blend`
