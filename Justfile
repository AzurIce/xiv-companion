set shell := ["zsh", "-eu", "-o", "pipefail", "-c"]

wasm_bindgen := env_var_or_default("WASM_BINDGEN", "wasm-bindgen")

build-wasm:
    @if { ! command -v lld >/dev/null 2>&1 || ! command -v "{{ wasm_bindgen }}" >/dev/null 2>&1; } && [[ -z "${XIV_COMPANION_IN_NIX:-}" ]] && command -v nix >/dev/null 2>&1; then \
      exec nix develop --command env XIV_COMPANION_IN_NIX=1 just _build-wasm; \
    else \
      exec just _build-wasm; \
    fi

[private]
_build-wasm:
    cargo build --lib --target wasm32-unknown-unknown --features wasm
    "{{ wasm_bindgen }}" --target web --out-dir app/src/wasm --out-name xiv_companion target/wasm32-unknown-unknown/debug/xiv_companion.wasm
