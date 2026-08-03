.DEFAULT_GOAL := help

MSRV := 1.95.0

.PHONY: help fmt fmt-check check lint test test-doc feature-check msrv-check docs examples
.PHONY: audit deny semver package ci clean

help: ## Show available commands
	@awk 'BEGIN {FS = ":.*##"; printf "Usage: make <target>\n\n"} /^[a-zA-Z_-]+:.*##/ {printf "  %-18s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

fmt: ## Format Rust sources
	cargo fmt --all

fmt-check: ## Check Rust formatting
	cargo fmt --all -- --check

check: ## Check default, memory-only, and all-feature builds
	cargo check --all-targets --locked
	cargo check --all-targets --no-default-features --locked
	cargo check --all-targets --all-features --locked

lint: ## Run strict Clippy checks
	cargo clippy --all-targets --all-features --locked -- -D warnings
	cargo clippy --all-targets --no-default-features --locked -- -D warnings

test: ## Run default, memory-only, and all-feature tests
	cargo test --locked
	cargo test --no-default-features --locked
	cargo test --all-features --locked

test-doc: ## Run documentation tests
	cargo test --doc --all-features --locked
	cargo test --doc --no-default-features --locked

feature-check: ## Check all feature combinations with cargo-hack
	cargo hack check --feature-powerset --depth 2 --locked

msrv-check: ## Check all targets with the minimum Rust version
	cargo +$(MSRV) check --all-targets --all-features --locked

docs: ## Build documentation with warnings denied
	RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps --locked

examples: ## Build all examples
	cargo build --examples --all-features --locked
	cargo build --examples --no-default-features --locked

audit: ## Check RustSec advisories
	cargo audit --deny warnings

deny: ## Check dependency and license policy
	cargo deny check

semver: ## Compare the public API with the latest release
	cargo semver-checks check-release --all-features

package: ## Build and verify the crates.io package
	cargo package --locked

ci: fmt-check check lint test test-doc feature-check msrv-check docs examples audit deny ## Run the complete local CI matrix

clean: ## Remove Cargo build output
	cargo clean
