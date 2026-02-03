.PHONY: install build test lint typecheck format ci clean dev

# Default target
all: ci

# Install dependencies
install:
	npm install

# Build the project
build:
	npm run build

# Run tests
test:
	npm test

# Run tests with coverage
test-coverage:
	npm run test:coverage

# Run linter
lint:
	npm run lint

# Run type checker
typecheck:
	npm run typecheck

# Format code
format:
	npm run format

# Full CI pipeline (what GitHub Actions runs)
ci: install lint typecheck test build
	@echo "✓ CI passed"

# Clean build artifacts
clean:
	rm -rf dist node_modules coverage

# Development mode with watch
dev:
	npm run dev

# Run CLI in development
cli:
	npm run cli -- $(ARGS)

# Validate lattice structure (once implemented)
validate:
	./bin/lattice validate

# Check for drift (once implemented)
drift:
	./bin/lattice drift --check

# Export requirements to markdown (once implemented)
export-requirements:
	./bin/lattice export requirements --format markdown > docs/REQUIREMENTS.md

# Help target
help:
	@echo "Lattice Development Commands"
	@echo ""
	@echo "  make install    - Install dependencies"
	@echo "  make build      - Build the project"
	@echo "  make test       - Run tests"
	@echo "  make lint       - Run linter"
	@echo "  make typecheck  - Run type checker"
	@echo "  make format     - Format code"
	@echo "  make ci         - Run full CI pipeline"
	@echo "  make clean      - Remove build artifacts"
	@echo "  make dev        - Run in development mode"
	@echo "  make cli ARGS=  - Run CLI with arguments"
	@echo ""
