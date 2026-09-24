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
devenv test
```

## 3. Optional Packaging Check

Run this only if crates.io packaging or publishing is part of the release goal.

```sh
devenv shell -- cargo package --locked
```

This currently fails because the project depends on the git-only `r7z` crate.

## 4. Run Maintenance Checks

```sh
devenv --profile maintenance shell -- cargo audit
devenv --profile maintenance shell -- cargo deny check
devenv --profile maintenance shell -- cargo machete
```

`cargo deny check` may print duplicate dependency warnings under the current
policy. The release gate requires a zero exit code.

## 5. Optional External Smoke Test

This command downloads public-domain test data from archive.org, so it is not
part of the default local gate.

```sh
devenv shell -- bash scripts/fetch_public_domain_test_data.sh --catalog-tier metadata --dry-run
```

## 6. Before Pushing

```sh
git log --oneline origin/main..main
git status --short --branch --ignored
```

Confirm that generated data remains untracked under ignored paths such as
`tmp/`, `target/`, `.direnv/`, or `result`.
