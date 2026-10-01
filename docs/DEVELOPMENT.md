# Development Guide

This guide covers the repository workflow. The current checkout is part of a
build-repair and hardening pass, so commands and feature descriptions below do
not imply that the workspace currently compiles.

## Prerequisites

- Rust and `rustup`
- The `wasm32-unknown-unknown` Rust target
- Stellar CLI compatible with the deployment scripts
- Node.js 20 or newer for `oracle/`

Install the WebAssembly target with:

```bash
rustup target add wasm32-unknown-unknown
```

## Local Checks

Before opening a pull request, run the full CI suite with a single command:

```bash
make ci
```

This mirrors `.github/workflows/ci.yml` exactly: orphan check, formatting,
contract build, WASM size gate, docs sync, clippy, tests, doc-tests,
shellcheck, and the complete oracle pipeline. If `make ci` is green, CI will
be green.

Individual targets are also available when you want a faster focused check:

```bash
make fmt-check      # cargo fmt --all -- --check
make clippy         # cargo clippy --all-targets --all-features
make test           # cargo test --workspace
make test-doc       # cargo test --workspace --doc
make build          # stellar contract build  (wasm32v1-none)
make orphan-check   # python3 scripts/check_orphan_modules.py
make wasm-size      # python3 scripts/check_wasm_sizes.py
make docs-errors    # regenerate + git diff docs/ERRORS.md
make docs-events    # regenerate + git diff docs/EVENTS.md
make shellcheck     # shellcheck scripts/*.sh
```

For the oracle service:

```bash
make oracle-fmt-check   # Prettier formatting check
make oracle-lint        # ESLint
make oracle-typecheck   # TypeScript compile check
make oracle-test-ci     # Jest tests (CI mode, with coverage)
```

The workspace currently has known build issues, so record any failure and
consult the relevant issue before claiming a green build. See [TESTING.md](TESTING.md)
for the test layout and [FAQ.md](FAQ.md) for common environment and toolchain
problems. Do not treat stale implementation plans or status notes as evidence
that a feature works.

## Build Targets

The two contract packages are `raffle-factory` and `raffle-instance`. Contracts
are built with the Stellar CLI, which targets `wasm32v1-none` — the same target
the deploy scripts use (issue #841):

```bash
make build
```

Or to build individual packages:

```bash
cargo build --target wasm32v1-none --release -p raffle-factory
cargo build --target wasm32v1-none --release -p raffle-instance
```

Deployment and verification instructions are maintained in
[DEPLOYMENT.md](DEPLOYMENT.md). Storage tiers and TTL policy are maintained in
[STORAGE.md](STORAGE.md).

## Contribution Conventions

Use a descriptive branch prefix such as `feat/`, `fix/`, `docs/`, `test/`, or
`chore/`. Keep pull requests focused, document externally visible changes, and
update the relevant document in this directory rather than adding a temporary
root-level status or plan file.

See the root [CONTRIBUTING.md](../CONTRIBUTING.md) for the contribution and
review process.
