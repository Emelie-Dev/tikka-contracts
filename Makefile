.PHONY: build check test test-doc lint fmt-check clippy orphan-check wasm-size \
        docs-errors docs-events shellcheck \
        ruff-lint ruff-fmt-check python-tests \
        fuzz clean deploy-testnet deploy-mainnet verify reproducible \
        oracle-build oracle-test oracle-test-ci oracle-lint oracle-fmt-check oracle-typecheck \
        all ci

# ---------------------------------------------------------------------------
# Rust / contract targets
# ---------------------------------------------------------------------------

# `stellar contract build` targets wasm32v1-none — the same target the deploy
# scripts and CI use. Keep every build path going through it (issue #841); the
# artifact paths are defined once in scripts/common.sh.
build:
	stellar contract build

# Fast compile gate (mirrors the `check` CI job).
check:
	cargo check --workspace --all-targets --all-features --exclude raffle-fuzz

# Run all workspace unit + integration tests.
test:
	cargo test --workspace

# Run workspace doc-tests separately (cargo test --workspace does not run them).
test-doc:
	cargo test --workspace --doc

# fmt-check and clippy are split so each can be called independently; `lint`
# keeps the original convenience alias that runs both together.
fmt-check:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# Derive fuzz targets from fuzz/Cargo.toml so this list never drifts from what
# is actually declared.  The shell snippet greps every [[bin]] name = "…" line,
# strips the surrounding quotes and whitespace, then joins the results.
FUZZ_TARGETS := $(shell grep -A1 '^\[\[bin\]\]' fuzz/Cargo.toml \
                  | grep 'name\s*=' \
                  | sed 's/.*name\s*=\s*"\([^"]*\)".*/\1/')
FUZZ_TIME ?= 300

fuzz:
	@for target in $(FUZZ_TARGETS); do \
		echo "==> fuzzing $target (${FUZZ_TIME}s)"; \
		cargo fuzz run $target -- -max_total_time=$(FUZZ_TIME); \
	done

# ---------------------------------------------------------------------------
# Deploy / utility
# ---------------------------------------------------------------------------

deploy-testnet:
	./scripts/deploy-testnet.sh

deploy-mainnet:
	./scripts/deploy-mainnet.sh

verify:
	./scripts/verify.sh

reproducible:
	./scripts/build-reproducible.sh

clean:
	cargo clean

# ---------------------------------------------------------------------------
# Convenience aliases
# ---------------------------------------------------------------------------

# Original convenience target — kept for existing muscle memory.
all: lint test build

# ---------------------------------------------------------------------------
# ci — mirrors .github/workflows/ci.yml build_and_test + shellcheck +
#       oracle_check jobs exactly and in the same order.
#
# Run this before every push to catch CI failures locally.
#
# Steps omitted here because they cannot run without CI infrastructure:
#   - Rust/Node cache warm-up       (Swatinem/rust-cache, actions/setup-node)
#   - WASM size PR summary          (writes to $GITHUB_STEP_SUMMARY, PR-only)
#   - Coverage ratchet              (needs cargo-llvm-cov; run separately)
#   - Upload artifact steps
# ---------------------------------------------------------------------------
ci: check fmt-check orphan-check build wasm-size docs-errors docs-events \
    clippy test test-doc shellcheck \
    ruff-lint ruff-fmt-check python-tests \
    oracle-fmt-check oracle-lint oracle-typecheck oracle-test-ci
