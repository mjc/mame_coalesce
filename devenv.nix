{
  config,
  lib,
  pkgs,
  ...
}: let
  # Scan-status builds enable per-opcode cycle accounting on every connection.
  # Keep EXPLAIN and normal SQLite integrity checks without timing every opcode.
  catalogSqlite = pkgs.sqlite.overrideAttrs (previous: let
    compileFlags = previous.env.NIX_CFLAGS_COMPILE or previous.NIX_CFLAGS_COMPILE or "";
  in {
    env =
      (previous.env or {})
      // {
        NIX_CFLAGS_COMPILE = lib.replaceStrings ["-DSQLITE_ENABLE_STMT_SCANSTATUS"] [""] compileFlags;
      };
  });
  scriptCheck = "${pkgs.shellcheck}/bin/shellcheck scripts/fetch_public_domain_test_data.sh scripts/profile_flamegraph.sh scripts/profile_catalog_imports.sh scripts/render_catalog_flamegraph.sh scripts/test_filter_perf_user_stacks.sh scripts/benchmark_run.sh scripts/generate_synthetic_benchmark_corpus.sh scripts/generate_xml_import_benchmark_corpus.sh scripts/benchmark_xml_import.sh scripts/parse_flamegraph scripts/parse_perfdata";
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
    catalogSqlite
    openssl
    zlib
    sccache
    shellcheck
    alejandra
    git
    curl
    jq
    gawk
    cargo-nextest
    time
  ];

  env = {
    CC = "${pkgs.stdenv.cc}/bin/cc";
    CXX = "${pkgs.stdenv.cc}/bin/c++";
    RUSTC_WRAPPER = "${pkgs.sccache}/bin/sccache";
    PKG_CONFIG_PATH = lib.makeSearchPath "lib/pkgconfig" [catalogSqlite.dev pkgs.openssl.dev pkgs.zlib.dev];
    GH_PAGER = "cat";
  };

  # The CLI selects its own cache; never inject a shared DATABASE_URL from .env.
  dotenv.disableHint = true;

  scripts = {
    build.exec = ''cargo build --locked "$@"'';
  };

  tasks = {
    "project:sqlite" = {
      description = "Reject native SQLite builds with per-opcode timing enabled";
      exec = ''
        case "$(sqlite3 :memory: "SELECT sqlite_compileoption_used('ENABLE_STMT_SCANSTATUS');")" in
          0) ;;
          *) echo "SQLite scan-status instrumentation must be disabled" >&2; exit 1 ;;
        esac
      '';
    };
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
      exec = "cargo test --locked && cargo test --locked --examples";
    };
    "project:clippy" = {
      description = "Check all targets and features with the existing strict lint policy";
      showOutput = true;
      after = ["project:test"];
      exec = "cargo clippy --locked --all-targets --all-features -- -D warnings";
    };
    "project:check" = {
      description = "Run the complete local verification gate";
      after = ["project:clippy" "project:scripts" "project:sqlite"];
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
      tasks."project:profiling-renderer" = {
        description = "Check lossless perf-stack rendering and kernel-symbol boundaries";
        exec = "bash scripts/test_filter_perf_user_stacks.sh";
        before = ["project:scripts"];
      };
      packages = with pkgs;
        [cargo-flamegraph hyperfine inferno]
        ++ lib.optionals stdenv.isLinux [perf heaptrack];
    };
    maintenance.module = {
      packages = with pkgs; [cargo-audit cargo-deny cargo-outdated cargo-machete];
    };
  };
}
