# XIV Companion Design Notes

XIV Companion uses the same restrained, shadcn-like visual language as XIV Market, but the layout behaves like a multi-module application instead of a market website.

## App Shell

- Persistent left sidebar for modules.
- The root route is a lightweight home page for announcements, onboarding, and recent changes.
- Tool pages keep their own compact context and workflow controls.
- Main area supports dense tool layouts, split panes, and future preview canvases.
- Mobile collapses navigation into a horizontal module strip above the current page.
- Settings owns connection, local game directory, resource source, cache, refresh, reset, and diagnostics controls.

## Capability Communication

- `Web 可用` means the core workflow works directly in the browser without an extension.
- `本地数据增强` means local SqPack access adds fresher or richer data but is not required for the page's core workflow.
- `需要本地数据` means the page's core workflow requires an authorized FFXIV game directory.
- `API Bridge 增强` means the page works on its own and can optionally consume saved runtime character data.
- `需要 API Bridge` means the page's core workflow requires the Dalamud API Bridge plugin.
- The sidebar only shows module lifecycle labels such as `实验`; capability labels belong in page headers.
- The home page reports current integration status and explains what each integration adds. Settings remains the source of truth for configuration and diagnostics.

## Visual Language

- White canvas and neutral gray hierarchy.
- 4px spacing base.
- 6px controls and 8px cards.
- Icons identify modules and compact actions.
- Colored accents are semantic and sparse.

## Initial Modules

- Crafting search
- Notes
- Weapon model preview
- Inventory
- Collection
- Settings

