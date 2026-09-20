# Repository Guidance

This project has local reference repositories that are useful when changing FFXIV model, material,
or render data handling:

- `third_party/Meddle` (shallow clone of [PassiveModding/Meddle](https://github.com/PassiveModding/Meddle), gitignored)
  - Dalamud/runtime-oriented exporter.
  - Key references: `Meddle/Meddle.Utils/Export/Model.cs`,
    `Meddle/Meddle.Utils/Export/Mesh.cs`,
    `Meddle/Meddle.Utils/Export/Vertex.cs`,
    `Meddle/Meddle.Formats/Files/MtrlFile/ColorTableRow.cs`.
- `~/Files/repos/MeddleTools` (clone of [PassiveModding/MeddleTools](https://github.com/PassiveModding/MeddleTools))
  - Blender import/material-node tooling for Meddle glTF output.
  - Key references: `MeddleTools/node_setup/node_configs.py`,
    `MeddleTools/node_setup/node_mappings.py`,
    `MeddleTools/shaders.blend`.

When modifying `xiv-companion-data` parsing or texture/material baking, compare field semantics
against these references before changing assumptions. Keep fixes small and add focused tests for
each semantic correction.

Additional references:

- VFX（`.avfx`）字段与运行时语义的权威参考是
  [Dalamud-VFXEditor](https://github.com/0ceal0t/Dalamud-VFXEditor)（格式层：
  `VFXEditor/Formats/AvfxFormat/`，含曲线容器/发射器/粒子 Data 块布局）与
  [AVFXTools](https://github.com/0ceal0t/AVFXTools)（旧查看器，含运行时模拟与
  shader 合成实现：`AVFXTools/Graphics/`）。按需浅克隆到 /tmp 即可，不要签入仓库。
- `third_party/Meddle` is gitignored, so it must be cloned on first use:
  `git clone --depth 1 https://github.com/PassiveModding/Meddle.git third_party/Meddle`.
  Note that Meddle resolves model paths at runtime
  and does not construct them; for SqPack path construction semantics (equipment/accessory
  `chara/equipment|accessory`, monster/demihuman `chara/monster|demihuman`, housing
  `bgcommon/hou` SGB) the authority is
  [xivModdingFramework](https://github.com/TexTools/xivModdingFramework) (`Models/FileTypes/Mdl.cs`,
  `Materials/FileTypes/Mtrl.cs`, `Items/Categories/{Gear,Housing,Companions}.cs`,
  `General/XivModelChara.cs`).
- EXD table shapes without a game install: `ffxiv-datamining-cn` CSV exports (see README data
  sources).

## Local Development Servers

Do not start a local development server unless the user explicitly asks for one. This includes
`dx serve`, HTTP preview servers, and background server processes. For normal implementation and
verification, use build, check, and test commands only. Do not infer permission to start a server
from a request to change or verify frontend code.

## Dialog Keyboard Interaction

All modal dialogs must provide keyboard behavior in addition to pointer controls. `Escape` closes or
cancels the dialog, and `Enter` activates its enabled primary action. Informational dialogs without a
distinct primary action may treat `Enter` as close. Keyboard handling must work while focus is inside
the dialog's form controls and must not submit or activate the action twice.

## Changelog

Keep `CHANGELOG.md` in reverse chronological order. The `开发中` section must remain at the top,
followed by the newest release date and then progressively older entries.

Keep the changelog synchronized with repository changes throughout development. Update it for each
change, but describe user-visible features, important optimizations, and meaningful behavioral
changes rather than implementation details or minor fixes. Merge related work into a concise entry
whenever possible instead of creating one entry per commit or edit.

Record uncommitted and still-adjustable work under `开发中`. Before pushing, move work that is fully
complete into the appropriate dated section. Consolidate the final entries around completed
features, key improvements, or significant changes, and avoid duplicating development entries in
the dated history.
