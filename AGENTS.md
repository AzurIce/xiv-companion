# Repository Guidance

This project has local reference repositories that are useful when changing FFXIV model, material,
or render data handling:

- `E:\repos\Meddle`
  - Dalamud/runtime-oriented exporter.
  - Key references: `Meddle\Meddle.Utils\Export\Model.cs`,
    `Meddle\Meddle.Utils\Export\Mesh.cs`,
    `Meddle\Meddle.Utils\Export\Vertex.cs`,
    `Meddle\Meddle.Utils\Files\Structs\Model\Model.cs`,
    `Meddle\Meddle.Utils\Files\Structs\Material\ColorTableRow.cs`.
- `E:\repos\MeddleTools`
  - Blender import/material node/bake tooling for Meddle glTF output.
  - Key references: `MeddleTools\node_setup\node_configs.py`,
    `MeddleTools\node_setup\node_mappings.py`,
    `MeddleTools\bake\bake_utils.py`.

When modifying `xiv-companion-data` parsing or texture/material baking, compare field semantics
against these references before changing assumptions. Keep fixes small and add focused tests for
each semantic correction.

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
