# XIV Companion

XIV Companion is a Solid.js app-style toolbox for Final Fantasy XIV utilities.

The first module is a crafting search workspace extracted from Tomestone. The shell is designed for additional modules such as glamour preview, housing preview, and data browsing.

## Stack

- Bun workspace
- Cargo xtask workspace
- Vite
- Solid.js
- Tailwind CSS v4
- Kobalte Core
- lucide-solid

## Development

With Nix flakes:

```bash
nix develop
bun install
cargo update-craft-data --game-dir ~/Files/_ffxiv/XIVLauncherGamePath/game/
bun run dev
```

Without Nix:

```powershell
bun install
cargo update-craft-data --game-dir ~/Files/_ffxiv/XIVLauncherGamePath/game/
bun run dev
```

The dev server uses `http://127.0.0.1:5174` by default to avoid colliding with adjacent FFXIV projects.

`cargo update-craft-data` reads the game `sqpack` data directly through the
`xtask/xtask-update-craft-data` package. Pass either the install directory or
the inner `game` directory with `--game-dir`.

The command writes `app/public/craft-data.json` and `app/public/version.json`,
then audits the generated exchange data by default.
