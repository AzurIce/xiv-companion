# XIV Companion

XIV Companion is a Dioxus web app for Final Fantasy XIV utilities.

The app currently includes:

- a home page with announcements, integration status, usage guidance, and recent changelog entries
- crafting search with recipe trees, material summaries, source choices, market estimates, and Raphael macro solving
- a crafting list with local storage, a draggable item tree, multi-selection summaries, material planning, and item details
- live and cached character inventory browsing through API Bridge
- collection browsing split into equipment possession and permanent unlocks, with bidirectional equipment progress updates from saved inventory snapshots
- local game directory, resource cache, and API Bridge management under Settings

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
cargo binstall wasm-bindgen-cli@0.2.126 --force
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

The furniture catalog does not require a game install; it joins the datamining
`HousingFurniture.csv` / `HousingYardObject.csv` ModelKey→item mapping with the
existing `craft-data.json`:

```bash
cargo run -p xtask-update-craft-data -- --furniture-catalog \
  --housing-furniture-csv /path/to/HousingFurniture.csv \
  --housing-yard-object-csv /path/to/HousingYardObject.csv
```

Without explicit CSV paths the files are read from `--datamining-repo` or
downloaded from ffxiv-datamining-cn with `curl`.

The chara catalog (minions and mounts) works the same way: it joins the
datamining `ItemAction` / `Companion` / `Mount` / `ModelChara` tables with the
item links already present in `collection-catalog.json`:

```bash
cargo run -p xtask-update-craft-data -- --chara-catalog \
  --item-action-csv /path/to/ItemAction.csv \
  --companion-csv /path/to/Companion.csv \
  --mount-csv /path/to/Mount.csv \
  --model-chara-csv /path/to/ModelChara.csv
```

## Data Sources And Acknowledgements

XIV Companion derives its primary game data from the user's local FINAL
FANTASY XIV installation. Release metadata that is not present directly in the
current EXD tables is supplemented from these community projects:

- [ffxiv-datamining-cn](https://github.com/thewakingsands/ffxiv-datamining-cn)
  provides `Item.csv` history for first-seen patch detection, `ExVersion`
  boundaries for expansion-level fallback, and the `HousingFurniture` /
  `HousingYardObject` / `ItemAction` / `Companion` / `Mount` / `ModelChara`
  tables behind the furniture and chara catalogs.
- [GarlandTools](https://github.com/ufx/GarlandTools) provides historical item
  patch metadata for releases before patch 4.45. The source is pinned to commit
  `04cadd2e1e0de86c20aa9303faa082c7971f8d8b`; newer release metadata is not
  taken from GarlandTools.

The pinned GarlandTools `patches.json`, its original MIT license, and source
notes are preserved under [`third_party/garland-tools`](third_party/garland-tools/README.md).
Thanks to the maintainers and contributors of both projects for preserving and
publishing this historical data.
