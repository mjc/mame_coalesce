# Release Checklist

This checklist is for GitHub handoff readiness. It does not push, tag, create a
GitHub release, or publish to crates.io.

## 1. Confirm A Clean Tree

```sh
git status --short --branch --ignored
```

Expected tracked state: no modified, added, or deleted tracked files.

## 2. Run The Local Gate

```sh
nix develop -c shellcheck scripts/fetch_public_domain_test_data.sh
nix develop -c cargo fmt --check
nix develop -c cargo test
nix develop -c cargo clippy --all-targets --all-features -- -D warnings
nix develop -c cargo package
```

## 3. Run Maintenance Checks

```sh
nix develop -c cargo audit
nix develop -c cargo deny check
nix develop -c cargo-udeps udeps --all-targets
```

`cargo deny check` may print duplicate dependency warnings under the current
policy. The release gate requires a zero exit code.

## 4. Optional External Smoke Test

This command downloads public-domain test data from archive.org, so it is not
part of the default local gate.

```sh
nix develop -c bash scripts/fetch_public_domain_test_data.sh --catalog-tier metadata --dry-run
```

## 5. Before Pushing

```sh
git log --oneline origin/main..main
git status --short --branch --ignored
```

Confirm that generated data remains untracked under ignored paths such as
`tmp/`, `target/`, `.direnv/`, or `result`.
