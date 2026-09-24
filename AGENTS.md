# Development environment

Use the repository-owned devenv environment and the commands in README.md.
It supersedes the parent instruction to use `nix develop` for project work.
Run `devenv test` for the complete gate; all Cargo dependency resolution stays locked.

For shell activation and environment checks, consult the canonical
[NixOS repository operating guide](https://lific.mjc.lol/NIXOS/pages/40).
If `DEVENV_ROOT` already matches this checkout, run project commands directly.
Do not nest `nix develop` inside devenv or bypass Nix to fix native dependencies.
