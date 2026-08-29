ARTIFACT_DIR ?= artifacts
SBOM ?= $(ARTIFACT_DIR)/sbom.cdx.json
SBOM_SHA ?= $(SBOM).sha256
CARGO_AUDIT_VERSION ?= 0.22.2

.PHONY: all build test test-all clippy fmt check ci sbom sbom-check license-check vuln-check security-check

# Static-analysis-clean is a hard requirement: clippy must be 0 warnings.
all: check

build:
	cargo build --all-targets

test:
	cargo test

test-all:
	cargo test --all-targets --all-features

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt --all -- --check

sbom:
	python3 scripts/sbom.py generate --output $(SBOM) --checksum $(SBOM_SHA)

sbom-check:
	python3 scripts/sbom.py check --output $(SBOM) --checksum $(SBOM_SHA)

license-check:
	python3 scripts/license_check.py

vuln-check:
	python3 scripts/vuln_check.py

security-check: sbom sbom-check license-check vuln-check

# Full gate: fails on any clippy warning, test failure, malformed SBOM,
# license issue, or high/critical RustSec advisory.
check: fmt build clippy test-all security-check

# CI entrypoint (mirrors .github/workflows/ci.yml).
ci: check
