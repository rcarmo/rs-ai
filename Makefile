.PHONY: all build test clippy fmt check ci

# Static-analysis-clean is a hard requirement: clippy must be 0 warnings.
all: check

build:
	cargo build --all-targets

test:
	cargo test

clippy:
	cargo clippy --all-targets -- -D warnings

fmt:
	cargo fmt --all -- --check

# Full gate: fails on any clippy warning or test failure.
check: clippy test

# CI entrypoint (mirrors .github/workflows/ci.yml).
ci: build clippy test
