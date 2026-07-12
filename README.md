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

Generate or refresh game data:

```bash
cargo run -p xtask-update-craft-data -- \
  --game-dir ~/Files/_ffxiv/XIVLauncherGamePath/game/ \
  --datamining-repo /path/to/ffxiv-datamining-cn
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

The exporter reads game `sqpack` data directly through the
`xtask/xtask-update-craft-data` package. Pass either the install directory or
the inner `game` directory with `--game-dir`. The optional
`--datamining-repo` argument adds first-seen patch metadata from a local
`ffxiv-datamining-cn` checkout.

The command writes the generated resource JSON files and audits exchange data
by default.

## Data Sources And Acknowledgements

XIV Companion derives its primary game data from the user's local FINAL
FANTASY XIV installation. Release metadata that is not present directly in the
current EXD tables is supplemented from these community projects:

- [ffxiv-datamining-cn](https://github.com/thewakingsands/ffxiv-datamining-cn)
  provides `Item.csv` history for first-seen patch detection and `ExVersion`
  boundaries for expansion-level fallback.
- [GarlandTools](https://github.com/ufx/GarlandTools) provides historical item
  patch metadata for releases before patch 4.45. The source is pinned to commit
  `04cadd2e1e0de86c20aa9303faa082c7971f8d8b`; newer release metadata is not
  taken from GarlandTools.

The pinned GarlandTools `patches.json`, its original MIT license, and source
notes are preserved under [`third_party/garland-tools`](third_party/garland-tools/README.md).
Thanks to the maintainers and contributors of both projects for preserving and
publishing this historical data.
