# XIV Companion

XIV Companion is a Dioxus web app for Final Fantasy XIV utilities.

The app currently includes:

- crafting search with recipe trees, material summaries, source choices, market estimates, and Raphael macro solving
- notes with local storage, folders/pages, crafting summary cards, material planning, and item details

## Stack

- Rust
- Dioxus 0.7
- Tailwind CSS v4 through Dioxus assets
- Cargo xtask workspace for game-data export

## Development

Install the web toolchain once:

```bash
cargo install cargo-binstall
cargo binstall dioxus-cli@0.7.9 --force
cargo binstall wasm-bindgen-cli@0.2.121 --force
rustup target add wasm32-unknown-unknown
```

Generate or refresh crafting data:

```bash
cargo update-craft-data --game-dir ~/Files/_ffxiv/XIVLauncherGamePath/game/
```

Run the Dioxus dev server:

```bash
dx serve --web --features web --addr 127.0.0.1 --port 5174 --open false
```

Build for production:

```bash
dx build --web --release --features web --package xiv-companion --bin xiv-companion
```

The production bundle is written to `target/dx/xiv-companion/release/web/public`.

`cargo update-craft-data` reads the game `sqpack` data directly through the
`xtask/xtask-update-craft-data` package. Pass either the install directory or
the inner `game` directory with `--game-dir`.

The command writes `assets/craft-data.json` and `assets/version.json`, then
audits the generated exchange data by default.
