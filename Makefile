.PHONY: fmt lint test check build clean install docker-e2e pre-commit pre-push

# Format code
fmt:
	cargo fmt

# Check formatting without modifying files
fmt-check:
	cargo fmt --check

# Run clippy lints
lint:
	cargo clippy --all-targets --all-features -- -D warnings

# Run tests
test:
	cargo test

# Pre-commit gate: fast checks (format + lint)
pre-commit: fmt-check lint
	@echo "Pre-commit checks passed."

# Pre-push gate: full checks (format + lint + test + build)
pre-push: pre-commit test build
	@echo "Pre-push checks passed."

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
	@echo "  make test       - Run tests"
	@echo "  make build      - Build release binary"
	@echo "  make clean      - Clean build artifacts"
	@echo "  make install  - Install to ~/.cargo/bin"
	@echo "  make smoke      - Run CLI smoke tests"
	@echo "  make docker-e2e - Run Docker end-to-end integration test"
	@echo "  make watch      - Watch and rebuild on changes"
