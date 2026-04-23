{
  description = "mame_coalesce - merge MAME ROMs into 1G1Z format";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [(import rust-overlay)];
      pkgs = import nixpkgs {
        inherit system overlays;
      };

      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = ["rust-src" "rust-analyzer" "llvm-tools-preview"];
      };

      # Nightly toolchain for tools that require it (cargo-udeps)
      rustNightlyForUdeps = pkgs.rust-bin.nightly.latest.default;

      # Wrapper for cargo-udeps that uses nightly
      cargo-udeps-wrapped = pkgs.writeShellScriptBin "cargo-udeps" ''
        export RUSTC="${rustNightlyForUdeps}/bin/rustc"
        export CARGO="${rustNightlyForUdeps}/bin/cargo"
        exec ${pkgs.cargo-udeps}/bin/cargo-udeps "$@"
      '';

      nativeBuildInputs = with pkgs;
        [
          rustToolchain
          pkg-config
          cmake  # Required for zlib-ng feature in flate2

          # Code quality & linting
          cargo-deny
          cargo-audit
          shellcheck

          # Testing & coverage
          cargo-nextest
          cargo-tarpaulin
          cargo-mutants

          # Build & dependencies
          cargo-outdated
          cargo-bloat
          cargo-udeps-wrapped

          # Utilities
          curl
          jq
          p7zip
          tokei
          gh

          # Performance profiling
          cargo-flamegraph

          # Build acceleration
          sccache
        ]
        ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          perf
          cargo-llvm-cov
          mold  # Fast linker (Linux only)
        ];

      buildInputs = with pkgs; [
        openssl
        zlib
        sqlite
      ];

      pkgConfigPath = with pkgs; lib.concatStringsSep ":" [
        "${openssl.dev}/lib/pkgconfig"
        "${zlib.dev}/lib/pkgconfig"
        "${sqlite.dev}/lib/pkgconfig"
      ];
    in {
      devShells.default = pkgs.mkShell {
        inherit nativeBuildInputs buildInputs;

        shellHook = ''
          export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/library"
          export PKG_CONFIG_PATH="${pkgConfigPath}"
          export RUSTC_WRAPPER="sccache"
          export LIBRARY_PATH="${pkgs.sqlite}/lib:${pkgs.zlib}/lib:${pkgs.openssl.out}/lib"

          ${pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="clang"
            export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=-fuse-ld=mold -C target-cpu=native"
          ''}

          ${pkgs.lib.optionalString pkgs.stdenv.isDarwin ''
            export CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS="-C target-cpu=native"
          ''}

          echo "mame_coalesce dev environment"
          echo "  rustc $(rustc --version)"
        '';

        GH_PAGER = "cat";

        OPENSSL_DIR = "${pkgs.openssl.dev}";
        OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
        PKG_CONFIG_PATH = pkgConfigPath;
      };

      packages.default = pkgs.rustPlatform.buildRustPackage {
        pname = "mame_coalesce";
        version = "0.1.0";
        src = ./.;

        cargoLock = {
          lockFile = ./Cargo.lock;
        };

        inherit nativeBuildInputs buildInputs;

        OPENSSL_DIR = "${pkgs.openssl.dev}";
        OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
        PKG_CONFIG_PATH = pkgConfigPath;

        meta = with pkgs.lib; {
          description = "Merge MAME ROMs into 1G1Z format";
          license = licenses.mit;
          platforms = platforms.linux ++ platforms.darwin;
        };
      };
    });
}
