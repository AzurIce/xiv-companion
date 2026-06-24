serve:
    dx serve --web --features web --addr 127.0.0.1 --port 5174 --open false

build:
    dx build --web --release --features web --package xiv-companion --bin xiv-companion

check-web:
    cargo check --target wasm32-unknown-unknown --features web
