.PHONY: fmt fmt-check format format-check lint test test-all check build clean install docker-e2e pre-commit pre-push lattice-fresh hooks

# Format code
fmt:
	cargo fmt

# Check formatting without modifying files
fmt-check:
	cargo fmt --check

# Cross-repo verb aliases (every forkzero repo exposes format / format-check)
format: fmt
format-check: fmt-check

# Run clippy lints
lint:
	cargo clippy --all-targets --all-features -- -D warnings

# Run tests with JUnit XML output
test:
	@mkdir -p test-results
	cargo nextest run --workspace

# Alias for consistency with other repos
test-all: test

# Pre-commit gate: format + lint + lattice health
pre-commit: fmt-check lint
	@if command -v lattice >/dev/null 2>&1 && lattice health --help 2>&1 | grep -q strict; then \
		lattice health --strict --check; \
	else \
		echo "Note: lattice health skipped (install lattice >=0.2.2 to enable)"; \
	fi
	@echo "Pre-commit checks passed."

# Lattice staleness gate: hard-fail if .lattice is >72h behind code.
# Time-based threshold lives at push/CI altitude, NOT pre-commit (see docs/ENGINEERING.md).
lattice-fresh:
	@if command -v lattice >/dev/null 2>&1; then \
		lattice freshness --check || { echo "Lattice is stale (>72h behind code). Update .lattice before pushing."; exit 1; }; \
	else \
		echo "Note: lattice freshness skipped (lattice not installed)"; \
	fi

# Pre-push gate: full checks (format + lint + lattice freshness + test + build)
pre-push: pre-commit lattice-fresh test build
	@echo "Pre-push checks passed."

# Install git hooks (Rust repo has no husky — wire core.hooksPath to committed .githooks/)
hooks:
	git config core.hooksPath .githooks
	@echo "Git hooks installed (core.hooksPath=.githooks)."

# Legacy alias
check: fmt lint test
	@echo "All checks passed!"

# Build release binary
build:
	cargo build --release

# Clean build artifacts
clean:
	cargo clean

# Install to ~/.cargo/bin
install: build
	cp target/release/lattice ~/.cargo/bin/

# Run all smoke tests locally
smoke: build
	./target/release/lattice --version
	./target/release/lattice list requirements
	./target/release/lattice list theses
	./target/release/lattice list sources
	./target/release/lattice drift
	@echo "Smoke tests passed!"

# Run Docker end-to-end integration test (from GitHub release)
docker-e2e:
	docker build -t lattice-e2e tests/docker
	docker run --rm lattice-e2e

# Run Docker e2e with locally-built source (builds in container)
docker-e2e-local:
	docker build -f tests/docker/Dockerfile.local -t lattice-e2e-local .
	docker run --rm lattice-e2e-local

# Watch for changes and rebuild (requires cargo-watch)
watch:
	cargo watch -x 'build'

# Help
help:
	@echo "Available targets:"
	@echo "  make pre-commit - Check formatting + lint (run before commit)"
	@echo "  make pre-push   - Full check: format + lint + test + build (run before push)"
	@echo "  make fmt        - Format code with cargo fmt"
	@echo "  make lint       - Run clippy lints"
	@echo "  make test       - Run tests (JUnit XML output)"
	@echo "  make test-all   - Alias for test (consistency with other repos)"
	@echo "  make build      - Build release binary"
	@echo "  make clean      - Clean build artifacts"
	@echo "  make install  - Install to ~/.cargo/bin"
	@echo "  make smoke      - Run CLI smoke tests"
	@echo "  make docker-e2e - Run Docker end-to-end integration test"
	@echo "  make watch      - Watch and rebuild on changes"
