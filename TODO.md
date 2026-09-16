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
