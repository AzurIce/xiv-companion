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

Do not start local development servers or preview servers for this repository. The user will start
and manage them when needed. Build and test commands that terminate on completion are still allowed.
