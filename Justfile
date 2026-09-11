serve:
    dx serve --web --features web --addr 127.0.0.1 --port 5174 --open false

build:
    dx build --web --release --features web --package xiv-companion --bin xiv-companion

check-web:
    cargo check --target wasm32-unknown-unknown --features web

# headless native wgpu render snapshots (synthetic fixtures; needs nix dev shell for vulkan)
render-test:
    nix develop --command cargo test --release -j8 --features render-test-support --test native_weapon_snapshot -- --ignored --test-threads=4
