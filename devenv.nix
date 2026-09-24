{
  config,
  lib,
  pkgs,
  ...
}: let
  scriptCheck = "${pkgs.shellcheck}/bin/shellcheck scripts/fetch_public_domain_test_data.sh scripts/profile_flamegraph.sh scripts/benchmark_run.sh scripts/generate_synthetic_benchmark_corpus.sh scripts/parse_flamegraph scripts/parse_perfdata";
in {
  languages.rust = {
    enable = true;
    channel = "stable";
    version = "latest";
    components = ["rustc" "cargo" "clippy" "rustfmt" "rust-src" "rust-analyzer"];
    mold.enable = pkgs.stdenv.isLinux;
  };

  packages = with pkgs; [
    cmake
    pkg-config
    sqlite
    openssl
    zlib
    sccache
    shellcheck
    alejandra
    p7zip
    git
    curl
    jq
    cargo-nextest
  ];

  env = {
    CC = "${pkgs.stdenv.cc}/bin/cc";
    CXX = "${pkgs.stdenv.cc}/bin/c++";
    RUSTC_WRAPPER = "${pkgs.sccache}/bin/sccache";
    PKG_CONFIG_PATH = lib.makeSearchPath "lib/pkgconfig" [pkgs.sqlite.dev pkgs.openssl.dev pkgs.zlib.dev];
    GH_PAGER = "cat";
  };

  # The CLI selects its own cache; never inject a shared DATABASE_URL from .env.
  dotenv.disableHint = true;

  scripts = {
    build.exec = ''cargo build --locked "$@"'';
  };

  tasks = {
    "project:format" = {
      description = "Check Rust and Nix formatting";
      exec = "cargo fmt --all -- --check && alejandra --check devenv.nix";
    };
    "project:scripts" = {
      description = "Check all maintained shell scripts";
      exec = scriptCheck;
    };
    "project:test" = {
      description = "Run unit, property, integration and doc tests";
      showOutput = true;
      after = ["project:format"];
      exec = ''
        cargo test --locked &&
        cargo test --locked --test integration p7zip_extracts_r7z_builder_archive -- --ignored --exact
      '';
    };
    "project:clippy" = {
      description = "Check all targets and features with the existing strict lint policy";
      showOutput = true;
      after = ["project:test"];
      exec = "cargo clippy --locked --all-targets --all-features -- -D warnings";
    };
    "project:check" = {
      description = "Run the complete local verification gate";
      after = ["project:clippy" "project:scripts"];
      # Keep verification out of ordinary shell activation.
      before = lib.optionals config.devenv.isTesting ["devenv:enterTest"];
    };
  };

  enterTest = ''
    cargo run --locked -- --help > /dev/null
  '';

  git-hooks.hooks = {
    rust-format = {
      enable = true;
      name = "Rust formatting";
      entry = "${pkgs.coreutils}/bin/env RUSTFMT=${config.languages.rust.toolchain.rustfmt}/bin/rustfmt ${config.languages.rust.toolchain.cargo}/bin/cargo fmt --all -- --check";
      files = "\\.rs$";
      pass_filenames = false;
    };
    alejandra = {
      enable = true;
      files = "^devenv\\.nix$";
    };
    scripts = {
      enable = true;
      name = "ShellCheck";
      entry = scriptCheck;
      files = "^scripts/";
      pass_filenames = false;
    };
  };

  profiles = {
    profiling.module = {
      languages.rust.components = ["llvm-tools-preview"];
      packages = with pkgs;
        [cargo-flamegraph hyperfine]
        ++ lib.optionals stdenv.isLinux [perf];
    };
    maintenance.module = {
      packages = with pkgs; [cargo-audit cargo-deny cargo-outdated cargo-machete];
    };
  };
}
