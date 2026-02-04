.PHONY: fmt lint test check build clean install

# Format code
fmt:
	cargo fmt

# Run clippy lints
lint:
	cargo clippy --all-targets --all-features -- -D warnings

# Run tests
test:
	cargo test

# Pre-commit check: format, lint, test
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

# Watch for changes and rebuild (requires cargo-watch)
watch:
	cargo watch -x 'build'

# Help
help:
	@echo "Available targets:"
	@echo "  make fmt      - Format code with cargo fmt"
	@echo "  make lint     - Run clippy lints"
	@echo "  make test     - Run tests"
	@echo "  make check    - Run fmt, lint, test (pre-commit)"
	@echo "  make build    - Build release binary"
	@echo "  make clean    - Clean build artifacts"
	@echo "  make install  - Install to ~/.cargo/bin"
	@echo "  make smoke    - Run CLI smoke tests"
	@echo "  make watch    - Watch and rebuild on changes"
