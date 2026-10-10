PROJECT_NAME := rs-ai
# Resolve before redirecting child TMPDIR; retain invalid-override failures.
export PROJECT_ORIGINAL_TMPDIR := $(if $(filter undefined,$(origin PROJECT_ORIGINAL_TMPDIR)),$(TMPDIR),$(PROJECT_ORIGINAL_TMPDIR))
PROJECT_TMP_PATHS := $(shell PROJECT=$(PROJECT_NAME) bash scripts/project-tmp.sh paths)
ifneq ($(.SHELLSTATUS),0)
$(error Unable to resolve PROJECT_TMP_ROOT)
endif
PROJECT_TMP_ROOT := $(patsubst PROJECT_TMP_ROOT=%,%,$(filter PROJECT_TMP_ROOT=%,$(PROJECT_TMP_PATHS)))
CACHE_ROOT ?= $(PROJECT_TMP_ROOT)/cache
BUILD_ROOT ?= $(PROJECT_TMP_ROOT)/build
TEST_ROOT ?= $(PROJECT_TMP_ROOT)/tests
LOG_ROOT ?= $(PROJECT_TMP_ROOT)/logs
RUN_ROOT ?= $(PROJECT_TMP_ROOT)/runs
RUN_PURPOSE ?= make
RUN_ID := $(if $(RUN_ID),$(RUN_ID),$(shell date -u +%Y%m%dT%H%M%S)-$(shell od -An -N8 -tx1 /dev/urandom | tr -d ' \n'))
RUN_DIR := $(if $(RUN_DIR),$(RUN_DIR),$(RUN_ROOT)/$(RUN_PURPOSE)/$(RUN_ID))

export PROJECT_TMP_ROOT := $(PROJECT_TMP_ROOT)
export RS_AI_TMP_ROOT := $(PROJECT_TMP_ROOT)
export RS_AI_RUN_DIR := $(RUN_DIR)
export CARGO_HOME := $(CACHE_ROOT)/cargo
export CARGO_TARGET_DIR := $(BUILD_ROOT)/cargo-target
export PYTHONPYCACHEPREFIX := $(CACHE_ROOT)/python/pycache
export npm_config_cache := $(CACHE_ROOT)/npm
export BUN_INSTALL_CACHE_DIR := $(CACHE_ROOT)/bun
export XDG_CACHE_HOME := $(CACHE_ROOT)/xdg
export TMPDIR := $(RUN_DIR)/tmp
export TMP := $(TMPDIR)
export TEMP := $(TMPDIR)
export PATH := $(CARGO_HOME)/bin:$(PATH)

.DEFAULT_GOAL := all

ARTIFACT_DIR ?= $(BUILD_ROOT)/artifacts
SBOM ?= $(ARTIFACT_DIR)/sbom.cdx.json
SBOM_SHA ?= $(SBOM).sha256
CARGO_AUDIT_VERSION ?= 0.22.2

.PHONY: all tmp-init build test test-all clippy fmt check ci sbom sbom-check license-check license-check-selftest vuln-check vuln-check-selftest security-check

# Host builds use the canonical project-owned hierarchy. This target creates only
# disposable cache/build/run directories and refuses symlinked or foreign roots.
tmp-init:
	@PROJECT=$(PROJECT_NAME) PROJECT_TMP_ROOT="$(PROJECT_TMP_ROOT)" scripts/project-tmp.sh init >/dev/null
	@set -eu; \
	for path in "$(PROJECT_TMP_ROOT)" "$(CACHE_ROOT)" "$(BUILD_ROOT)" "$(TEST_ROOT)" "$(LOG_ROOT)" "$(RUN_ROOT)" "$(RUN_DIR)" "$(TMPDIR)" "$(CARGO_HOME)" "$(PYTHONPYCACHEPREFIX)" "$(npm_config_cache)" "$(BUN_INSTALL_CACHE_DIR)" "$(XDG_CACHE_HOME)"; do \
		test ! -L "$$path" || { echo "Refusing symlink scratch path: $$path" >&2; exit 1; }; \
		if test -e "$$path"; then test -d "$$path" && test -O "$$path" || { echo "Scratch path must be an owned directory: $$path" >&2; exit 1; }; fi; \
	done; \
	mkdir -p "$(CARGO_HOME)" "$(CARGO_TARGET_DIR)" "$(PYTHONPYCACHEPREFIX)" "$(npm_config_cache)" "$(BUN_INSTALL_CACHE_DIR)" "$(XDG_CACHE_HOME)" "$(TMPDIR)"

# Static-analysis-clean is a hard requirement: clippy must be 0 warnings.
all: check

build: tmp-init
	cargo build --all-targets

test: tmp-init
	cargo test

test-all: tmp-init
	cargo test --all-targets --all-features

clippy: tmp-init
	cargo clippy --all-targets --all-features -- -D warnings

fmt: tmp-init
	cargo fmt --all -- --check

sbom: tmp-init
	python3 scripts/sbom.py generate --output $(SBOM) --checksum $(SBOM_SHA)

sbom-check: tmp-init
	python3 scripts/sbom.py check --output $(SBOM) --checksum $(SBOM_SHA)

license-check: tmp-init
	python3 scripts/license_check.py

license-check-selftest: tmp-init
	python3 scripts/license_check_selftest.py

vuln-check: tmp-init
	python3 scripts/vuln_check.py

vuln-check-selftest: tmp-init
	python3 scripts/vuln_check_selftest.py

security-check: sbom sbom-check license-check-selftest license-check vuln-check-selftest vuln-check

# Full gate: fails on any clippy warning, test failure, malformed SBOM,
# license issue, or high/critical RustSec advisory.
check: fmt build clippy test-all security-check

# CI entrypoint (mirrors .github/workflows/ci.yml).
ci: check
