{
  description = "xiv companion";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    # Keep Notist's own toolchain pins: its Book CLI uses a newer nightly.
    notist.url = "github:AzurIce/notist";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      crane,
      notist,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
            "clippy"
            "rustfmt"
          ];
          targets = [
            "wasm32-unknown-unknown"
          ];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
      in
      {
        packages = pkgs.lib.optionalAttrs (builtins.hasAttr system notist.packages) {
          notist = notist.packages.${system}.default;
        };

        devShells.default = craneLib.devShell {
          packages =
            with pkgs;
            [
              dioxus-cli
              binaryen
              just
              lld
              tailwindcss_4
              vulkan-loader
            ]
            ++ [
              (pkgs.callPackage ./wasm-bindgen-cli.nix { })
            ]
            # Upstream currently publishes Linux and aarch64-darwin packages.
            ++ pkgs.lib.optionals (builtins.hasAttr system notist.packages) [
              notist.packages.${system}.default
            ];

          # native wgpu render tests dlopen libvulkan at runtime
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.vulkan-loader ];
        };
      }
    );
}
