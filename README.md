# XIV Companion

XIV Companion is a Solid.js app-style toolbox for Final Fantasy XIV utilities.

The first module is a crafting search workspace extracted from Tomestone. The shell is designed for additional modules such as glamour preview, housing preview, and data browsing.

## Stack

- Bun workspace
- Vite
- Solid.js
- Tailwind CSS v4
- Kobalte Core
- lucide-solid

## Development

```powershell
bun install
bun run update-craft-data
bun run dev
```

The dev server uses `http://127.0.0.1:5174` by default to avoid colliding with adjacent FFXIV projects.

`update-craft-data` reads `..\ffxiv-datamining-cn` by default. Override with:

```powershell
$env:DATAMINING_DIR="E:\_ff14\ffxiv-datamining-cn"
bun run update-craft-data
```
