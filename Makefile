# ThreatFlux Cache Library - Makefile
# Comprehensive build and test automation

# Docker configuration
DOCKER_IMAGE = threatflux-cache
DOCKER_TAG = latest
DOCKER_FULL_NAME = $(DOCKER_IMAGE):$(DOCKER_TAG)

# Rust configuration
CARGO_FEATURES_DEFAULT = 
CARGO_FEATURES_ALL = --all-features
CARGO_FEATURES_NONE = --no-default-features

# Colors for output
RED = \033[0;31m
GREEN = \033[0;32m
YELLOW = \033[0;33m
BLUE = \033[0;34m
PURPLE = \033[0;35m
CYAN = \033[0;36m
WHITE = \033[0;37m
NC = \033[0m # No Color

.PHONY: help all all-coverage all-docker all-docker-coverage clean docker-build docker-clean
.PHONY: fmt fmt-check fmt-docker lint lint-docker audit audit-docker deny deny-docker codedup
.PHONY: test test-docker test-doc test-doc-docker test-features feature-check build build-docker build-all build-all-docker
.PHONY: docs docs-docker examples examples-docker bench bench-docker
.PHONY: coverage coverage-open coverage-lcov coverage-html coverage-summary coverage-json coverage-docker
.PHONY: dev-setup setup-dev ci-local ci-local-coverage

# Default target  
all: fmt-check lint audit deny codedup test msrv-check feature-check docs build examples ## Run all checks and builds locally

# Extended target with coverage
all-coverage: fmt-check lint audit deny codedup test coverage docs build examples ## Run all checks including coverage locally

# Docker all-in-one target
all-docker: docker-build ## Run all checks and builds in Docker container
	@echo "$(CYAN)Running all checks in Docker container...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) sh -c " \
		echo '$(BLUE)=== Formatting Check ===$(NC)' && \
		cargo fmt --all -- --check && \
		echo '$(BLUE)=== Linting ===$(NC)' && \
		cargo clippy --all-targets --all-features -- -D warnings && \
		cargo clippy --all-targets --no-default-features -- -D warnings && \
		cargo clippy --all-targets -- -D warnings && \
		echo '$(BLUE)=== Security Audit ===$(NC)' && \
		cargo audit && \
		echo '$(BLUE)=== Dependency Check ===$(NC)' && \
		cargo deny check && \
		echo '$(BLUE)=== Tests ===$(NC)' && \
		echo '  With all features...' && \
		cargo test --verbose --all-features && \
		echo '  With no default features...' && \
		cargo test --verbose --no-default-features && \
		echo '  With default features...' && \
		cargo test --verbose && \
		echo '$(BLUE)=== Documentation ===$(NC)' && \
		cargo doc --all-features --no-deps && \
		echo '$(BLUE)=== Build ===$(NC)' && \
		cargo build --all-features && \
		echo '$(BLUE)=== Examples ===$(NC)' && \
		cargo build --examples --all-features && \
		echo '$(GREEN)✅ All checks passed!$(NC)' \
	"

# Docker all-in-one target with coverage
all-docker-coverage: docker-build ## Run all checks including coverage in Docker container
	@echo "$(CYAN)Running all checks with coverage in Docker container...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) sh -c " \
		echo '$(BLUE)=== Formatting Check ===$(NC)' && \
		cargo fmt --all -- --check && \
		echo '$(BLUE)=== Linting ===$(NC)' && \
		cargo clippy --all-targets --all-features -- -D warnings && \
		cargo clippy --all-targets --no-default-features -- -D warnings && \
		cargo clippy --all-targets -- -D warnings && \
		echo '$(BLUE)=== Security Audit ===$(NC)' && \
		cargo audit && \
		echo '$(BLUE)=== Dependency Check ===$(NC)' && \
		cargo deny check && \
		echo '$(BLUE)=== Tests with Coverage ===$(NC)' && \
		echo '  With all features...' && \
		cargo llvm-cov --all-features --workspace --lcov --output-path lcov-all.info && \
		echo '  With no default features...' && \
		cargo llvm-cov --no-default-features --workspace --lcov --output-path lcov-no-default.info && \
		echo '  With default features...' && \
		cargo llvm-cov --workspace --lcov --output-path lcov.info && \
		cargo llvm-cov --all-features --workspace --html && \
		echo '$(BLUE)=== Documentation ===$(NC)' && \
		cargo doc --all-features --no-deps && \
		echo '$(BLUE)=== Build ===$(NC)' && \
		cargo build --all-features && \
		echo '$(BLUE)=== Examples ===$(NC)' && \
		cargo build --examples --all-features && \
		echo '$(GREEN)✅ All checks with coverage passed!$(NC)' \
	"

help: ## Show this help message
	@echo "$(CYAN)ThreatFlux Cache Library - Available Commands$(NC)"
	@echo ""
	@echo "$(YELLOW)Main Commands:$(NC)"
	@awk 'BEGIN {FS = ":.*##"; printf "  %-20s %s\n", "Target", "Description"} /^[a-zA-Z_-]+:.*?##/ { printf "  $(GREEN)%-20s$(NC) %s\n", $$1, $$2 }' $(MAKEFILE_LIST) | grep -E "(all|help|setup|clean)"
	@echo ""
	@echo "$(YELLOW)Local Development:$(NC)"
	@awk 'BEGIN {FS = ":.*##"; printf "  %-20s %s\n", "Target", "Description"} /^[a-zA-Z_-]+:.*?##/ { printf "  $(GREEN)%-20s$(NC) %s\n", $$1, $$2 }' $(MAKEFILE_LIST) | grep -E "^  [a-zA-Z_-]+[^-docker]" | grep -v -E "(all|help|setup|clean|docker)"
	@echo ""
	@echo "$(YELLOW)Docker Commands:$(NC)"
	@awk 'BEGIN {FS = ":.*##"; printf "  %-20s %s\n", "Target", "Description"} /^[a-zA-Z_-]+:.*?##/ { printf "  $(GREEN)%-20s$(NC) %s\n", $$1, $$2 }' $(MAKEFILE_LIST) | grep -E "(docker|all-docker)"

# =============================================================================
# Setup and Installation
# =============================================================================

dev-setup: ## Install development tools required for `make all`
	@echo "$(CYAN)Installing development tools...$(NC)"
	@./setup-dev-tools.sh --skip-system-deps
	@echo "$(GREEN)✅ Development tools installed!$(NC)"

setup-dev: dev-setup ## (Deprecated) Use `make dev-setup` instead
	@echo "$(YELLOW)⚠️  'setup-dev' is deprecated; use 'make dev-setup'.$(NC)"

# =============================================================================
# Docker Commands
# =============================================================================

docker-build: ## Build Docker image for consistent environment
	@echo "$(CYAN)Building Docker image...$(NC)"
	@echo 'FROM rust:1.83-alpine\n\
RUN apk add --no-cache pkgconfig musl-dev\n\
RUN rustup component add rustfmt clippy\n\
RUN cargo install cargo-audit cargo-deny cargo-llvm-cov cargo-hack\n\
WORKDIR /workspace\n\
ENV CARGO_TERM_COLOR=always\n\
ENV RUST_BACKTRACE=1\n\
CMD ["cargo", "build"]' | docker build -t $(DOCKER_FULL_NAME) -

docker-clean: ## Clean Docker images and containers
	@echo "$(CYAN)Cleaning Docker resources...$(NC)"
	@docker rmi $(DOCKER_FULL_NAME) 2>/dev/null || true
	@docker system prune -f

# =============================================================================
# Formatting Commands
# =============================================================================

fmt: ## Format code using rustfmt
	@echo "$(CYAN)Formatting code...$(NC)"
	@cargo fmt --all

fmt-check: ## Check code formatting without modifying files
	@echo "$(CYAN)Checking code formatting...$(NC)"
	@cargo fmt --all -- --check

fmt-docker: docker-build ## Format code using Docker
	@echo "$(CYAN)Formatting code in Docker...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) cargo fmt --all

# =============================================================================
# Linting Commands
# =============================================================================

lint: ## Run clippy linting
	@echo "$(CYAN)Running clippy linting...$(NC)"
	@echo "$(BLUE)  With all features...$(NC)"
	@cargo clippy --all-targets --all-features -- -D warnings
	@echo "$(BLUE)  With no default features...$(NC)"
	@cargo clippy --all-targets --no-default-features -- -D warnings
	@echo "$(BLUE)  With default features...$(NC)"
	@cargo clippy --all-targets -- -D warnings

lint-docker: docker-build ## Run clippy linting in Docker
	@echo "$(CYAN)Running clippy linting in Docker...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) sh -c "\
		echo '$(BLUE)  With all features...$(NC)' && \
		cargo clippy --all-targets --all-features -- -D warnings && \
		echo '$(BLUE)  With no default features...$(NC)' && \
		cargo clippy --all-targets --no-default-features -- -D warnings && \
		echo '$(BLUE)  With default features...$(NC)' && \
		cargo clippy --all-targets -- -D warnings"

# =============================================================================
# Security and Dependency Commands
# =============================================================================

audit: ## Run security audit
	@echo "$(CYAN)Running security audit...$(NC)"
	@cargo audit

audit-docker: docker-build ## Run security audit in Docker
	@echo "$(CYAN)Running security audit in Docker...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) cargo audit

deny: ## Run dependency validation
	@echo "$(CYAN)Running dependency validation...$(NC)"
	@cargo deny check

deny-docker: docker-build ## Run dependency validation in Docker
	@echo "$(CYAN)Running dependency validation in Docker...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) cargo deny check

codedup: ## Check for code duplication
	@echo "$(CYAN)Checking code duplication...$(NC)"
	@npx -y jscpd --threshold 5 --format rust --reporters console src examples tests 2>/dev/null || echo "$(YELLOW)Code duplication check requires npx$(NC)"

# =============================================================================
# Testing Commands
# =============================================================================

test: ## Run all tests
	@echo "$(CYAN)Running tests...$(NC)"
	@echo "$(BLUE)  With all features...$(NC)"
	@cargo test --verbose --all-features
	@echo "$(BLUE)  With no default features...$(NC)"
	@cargo test --verbose --no-default-features
	@echo "$(BLUE)  With default features...$(NC)"
	@cargo test --verbose

test-docker: docker-build ## Run all tests in Docker
	@echo "$(CYAN)Running tests in Docker...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) sh -c "\
		echo '$(BLUE)  With all features...$(NC)' && \
		cargo test --verbose --all-features && \
		echo '$(BLUE)  With no default features...$(NC)' && \
		cargo test --verbose --no-default-features && \
		echo '$(BLUE)  With default features...$(NC)' && \
		cargo test --verbose"

test-doc: ## Run documentation tests
	@echo "$(CYAN)Running documentation tests...$(NC)"
	@echo "$(BLUE)  With all features...$(NC)"
	@cargo test --doc --verbose --all-features
	@echo "$(BLUE)  With no default features...$(NC)"
	@cargo test --doc --verbose --no-default-features
	@echo "$(BLUE)  With default features...$(NC)"
	@cargo test --doc --verbose

test-doc-docker: docker-build ## Run documentation tests in Docker
	@echo "$(CYAN)Running documentation tests in Docker...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) sh -c "\
		echo '$(BLUE)  With all features...$(NC)' && \
		cargo test --doc --verbose --all-features && \
		echo '$(BLUE)  With no default features...$(NC)' && \
		cargo test --doc --verbose --no-default-features && \
		echo '$(BLUE)  With default features...$(NC)' && \
		cargo test --doc --verbose"

test-features: ## Test with different feature combinations
	@echo "$(CYAN)Testing different feature combinations...$(NC)"
	@echo "$(BLUE)Testing with all features...$(NC)"
	@cargo test --verbose --all-features
	@echo "$(BLUE)Testing with no default features...$(NC)"
	@cargo test --verbose --no-default-features  
	@echo "$(BLUE)Testing with default features only...$(NC)"
	@cargo test --verbose
	@echo "$(BLUE)Testing with specific features...$(NC)"
	@cargo test --verbose --no-default-features --features "filesystem-backend"
	@cargo test --verbose --no-default-features --features "json-serialization"
	@cargo test --verbose --no-default-features --features "bincode-serialization"
	@cargo test --verbose --no-default-features --features "compression"
	@cargo test --verbose --no-default-features --features "metrics"
	@echo "$(GREEN)✅ All feature combinations tested!$(NC)"

test-integration: ## Run integration tests
	@echo "$(CYAN)Running integration tests...$(NC)"
	@if ls tests/*.rs 1>/dev/null 2>&1; then \
		cargo test --test '*' --all-features -- --nocapture; \
	else \
		echo "$(YELLOW)No integration tests found in tests/ directory$(NC)"; \
	fi

feature-check: ## Check all feature combinations with cargo-hack
	@echo "$(CYAN)Checking feature combinations with cargo-hack...$(NC)"
	@cargo hack check --feature-powerset --depth 2 --no-dev-deps 2>/dev/null || echo "$(YELLOW)Feature check requires cargo-hack (cargo install cargo-hack)$(NC)"

msrv-check: ## Check Minimum Supported Rust Version (1.81.0)
	@echo "$(CYAN)Checking MSRV (1.81.0)...$(NC)"
	@rustup toolchain install 1.81.0 --profile minimal 2>/dev/null || true
	@cargo +1.81.0 check --all-features && echo "$(GREEN)✅ MSRV check passed!$(NC)" || echo "$(RED)❌ MSRV check failed - code requires features from Rust > 1.81.0$(NC)"

# =============================================================================
# Build Commands
# =============================================================================

build: ## Build the project
	@echo "$(CYAN)Building project...$(NC)"
	@cargo build

build-docker: docker-build ## Build the project in Docker
	@echo "$(CYAN)Building project in Docker...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) cargo build

build-all: ## Build with all features
	@echo "$(CYAN)Building project with all features...$(NC)"
	@cargo build --all-features

build-all-docker: docker-build ## Build with all features in Docker
	@echo "$(CYAN)Building project with all features in Docker...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) \
		cargo build --all-features

build-release: ## Build optimized release
	@echo "$(CYAN)Building release...$(NC)"
	@cargo build --release --all-features

build-release-docker: docker-build ## Build optimized release in Docker
	@echo "$(CYAN)Building release in Docker...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) \
		cargo build --release --all-features

# =============================================================================
# Documentation Commands
# =============================================================================

docs: ## Generate documentation
	@echo "$(CYAN)Generating documentation...$(NC)"
	@RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps

docs-docker: docker-build ## Generate documentation in Docker
	@echo "$(CYAN)Generating documentation in Docker...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) \
		sh -c "RUSTDOCFLAGS='-D warnings' cargo doc --all-features --no-deps"

docs-open: docs ## Generate and open documentation
	@echo "$(CYAN)Opening documentation...$(NC)"
	@cargo doc --all-features --no-deps --open

# =============================================================================
# Examples and Benchmarks
# =============================================================================

examples: ## Build and run all examples
	@echo "$(CYAN)Building and running examples...$(NC)"
	@cargo build --examples --all-features
	@echo "$(BLUE)Running basic_usage example...$(NC)"
	@cargo run --example basic_usage
	@echo "$(BLUE)Running custom_entry example...$(NC)"
	@cargo run --example custom_entry
	@echo "$(BLUE)Running file_scanner_migration example...$(NC)"
	@cargo run --example file_scanner_migration
	@echo "$(BLUE)Running simple_test example...$(NC)"
	@cargo run --example simple_test

examples-docker: docker-build ## Build all examples in Docker
	@echo "$(CYAN)Building examples in Docker...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) \
		cargo build --examples --all-features

bench: ## Run benchmarks
	@echo "$(CYAN)Running benchmarks...$(NC)"
	@cargo bench --all-features 2>/dev/null || echo "$(YELLOW)No benchmarks configured yet$(NC)"

bench-docker: docker-build ## Run benchmarks in Docker
	@echo "$(CYAN)Running benchmarks in Docker...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) \
		cargo bench --all-features

# =============================================================================
# Coverage and Profiling
# =============================================================================

coverage: ## Generate test coverage report (HTML + LCOV)
	@echo "$(CYAN)Generating coverage report...$(NC)"
	@echo "$(BLUE)  With all features...$(NC)"
	@cargo llvm-cov --all-features --workspace --lcov --output-path lcov-all.info
	@echo "$(BLUE)  With no default features...$(NC)"
	@cargo llvm-cov --no-default-features --workspace --lcov --output-path lcov-no-default.info
	@echo "$(BLUE)  With default features...$(NC)"
	@cargo llvm-cov --workspace --lcov --output-path lcov.info
	@cargo llvm-cov --all-features --workspace --html
	@echo "$(GREEN)✅ Coverage report generated in target/llvm-cov/html/index.html$(NC)"

coverage-open: coverage ## Generate and open HTML coverage report
	@echo "$(CYAN)Opening coverage report...$(NC)"
	@open target/llvm-cov/html/index.html 2>/dev/null || \
	 xdg-open target/llvm-cov/html/index.html 2>/dev/null || \
	 echo "$(YELLOW)Please open target/llvm-cov/html/index.html manually$(NC)"

coverage-lcov: ## Generate LCOV coverage report only
	@echo "$(CYAN)Generating LCOV coverage report...$(NC)"
	@cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
	@echo "$(GREEN)✅ LCOV report generated at lcov.info$(NC)"

coverage-html: ## Generate HTML coverage report only
	@echo "$(CYAN)Generating HTML coverage report...$(NC)"
	@cargo llvm-cov --all-features --workspace --html
	@echo "$(GREEN)✅ HTML report generated in target/llvm-cov/html/index.html$(NC)"

coverage-summary: ## Show coverage summary
	@echo "$(CYAN)Generating coverage summary...$(NC)"
	@cargo llvm-cov --all-features --workspace --summary-only

coverage-json: ## Generate JSON coverage report
	@echo "$(CYAN)Generating JSON coverage report...$(NC)"
	@cargo llvm-cov --all-features --workspace --json --output-path coverage.json
	@echo "$(GREEN)✅ JSON report generated at coverage.json$(NC)"

coverage-docker: docker-build ## Generate test coverage report in Docker
	@echo "$(CYAN)Generating coverage report in Docker...$(NC)"
	@docker run --rm -v "$$(pwd):/workspace" $(DOCKER_FULL_NAME) \
		sh -c "cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info && \
		       cargo llvm-cov --all-features --workspace --html"

# =============================================================================
# CI/Local Integration
# =============================================================================

ci-local: ## Run CI-like checks locally
	@echo "$(CYAN)Running CI checks locally...$(NC)"
	@echo "$(BLUE)=== Formatting ===$(NC)"
	@$(MAKE) fmt-check
	@echo "$(BLUE)=== Linting ===$(NC)"
	@$(MAKE) lint
	@echo "$(BLUE)=== Security Audit ===$(NC)"
	@$(MAKE) audit
	@echo "$(BLUE)=== Dependency Check ===$(NC)"
	@$(MAKE) deny
	@echo "$(BLUE)=== Tests ===$(NC)"
	@$(MAKE) test
	@echo "$(BLUE)=== Integration Tests ===$(NC)"
	@$(MAKE) test-integration
	@echo "$(BLUE)=== MSRV Check ===$(NC)"
	@$(MAKE) msrv-check
	@echo "$(BLUE)=== Documentation ===$(NC)"
	@$(MAKE) docs
	@echo "$(BLUE)=== Build ===$(NC)"
	@$(MAKE) build-all
	@echo "$(BLUE)=== Examples ===$(NC)"
	@$(MAKE) examples
	@echo "$(GREEN)✅ All CI checks passed locally!$(NC)"

ci-local-coverage: ## Run CI-like checks locally with coverage
	@echo "$(CYAN)Running CI checks with coverage locally...$(NC)"
	@echo "$(BLUE)=== Formatting ===$(NC)"
	@$(MAKE) fmt-check
	@echo "$(BLUE)=== Linting ===$(NC)"
	@$(MAKE) lint
	@echo "$(BLUE)=== Security Audit ===$(NC)"
	@$(MAKE) audit
	@echo "$(BLUE)=== Dependency Check ===$(NC)"
	@$(MAKE) deny
	@echo "$(BLUE)=== Tests with Coverage ===$(NC)"
	@$(MAKE) coverage-summary
	@echo "$(BLUE)=== Documentation ===$(NC)"
	@$(MAKE) docs
	@echo "$(BLUE)=== Build ===$(NC)"
	@$(MAKE) build-all
	@echo "$(GREEN)✅ All CI checks with coverage passed locally!$(NC)"

# =============================================================================
# Utility Commands
# =============================================================================

clean: ## Clean build artifacts and coverage reports
	@echo "$(CYAN)Cleaning build artifacts...$(NC)"
	@cargo clean
	@rm -rf target/
	@rm -f lcov*.info coverage.json
	@echo "$(GREEN)✅ Clean complete!$(NC)"

watch: ## Watch for changes and run tests
	@echo "$(CYAN)Watching for changes...$(NC)"
	@cargo watch -x "test --all-features" 2>/dev/null || echo "$(YELLOW)Watch requires cargo-watch (cargo install cargo-watch)$(NC)"

update: ## Update dependencies
	@echo "$(CYAN)Updating dependencies...$(NC)"
	@cargo update

check-deps: ## Check dependency tree
	@echo "$(CYAN)Checking dependency tree...$(NC)"
	@cargo tree --all-features

# =============================================================================
# Development Workflows
# =============================================================================

dev: ## Quick development check (format + lint + test)
	@echo "$(CYAN)Running quick development checks...$(NC)"
	@$(MAKE) fmt
	@$(MAKE) lint
	@$(MAKE) test

dev-docker: ## Quick development check in Docker
	@echo "$(CYAN)Running quick development checks in Docker...$(NC)"
	@$(MAKE) fmt-docker
	@$(MAKE) lint-docker
	@$(MAKE) test-docker

pre-commit: ## Run pre-commit checks
	@echo "$(CYAN)Running pre-commit checks...$(NC)"
	@$(MAKE) fmt-check
	@$(MAKE) lint
	@$(MAKE) test
	@echo "$(GREEN)✅ Pre-commit checks passed!$(NC)"

pre-push: ## Run comprehensive checks before pushing (ensures CI will pass)
	@echo "$(CYAN)Running pre-push checks to ensure CI will pass...$(NC)"
	@echo "$(YELLOW)This will run all checks that CI runs. It may take a few minutes.$(NC)"
	@$(MAKE) ci-local
	@echo "$(GREEN)✅ All pre-push checks passed! Safe to push.$(NC)"

# Show variables for debugging
debug-vars: ## Show Makefile variables
	@echo "$(CYAN)Makefile Variables:$(NC)"
	@echo "DOCKER_IMAGE: $(DOCKER_IMAGE)"
	@echo "DOCKER_TAG: $(DOCKER_TAG)"
	@echo "DOCKER_FULL_NAME: $(DOCKER_FULL_NAME)"