# CLAUDE.md

Instructions for Claude Code when working in this repository.

## Project Overview

**Lattice** — A knowledge coordination protocol for human-agent collaboration. TypeScript CLI and library. Connects research → strategy → requirements → implementation into a traversable, version-aware graph. File-based storage (YAML in `.lattice/`), Git-native.

## Repository Structure

```
/
├── src/
│   ├── cli/                 # CLI commands (init, add, query, drift, verify)
│   ├── core/                # Core data model (nodes, edges, versions)
│   ├── storage/             # File-based storage layer
│   ├── graph/               # Graph traversal and queries
│   └── export/              # Export to markdown, bibtex, etc.
├── tests/                   # Test files
├── .lattice/                # Lattice describing itself (self-hosting)
│   ├── config.yaml
│   ├── sources/             # Research backing theses
│   ├── theses/              # Strategic claims
│   ├── requirements/        # Specifications for Lattice
│   └── implementations/     # Code bindings
├── docs/
│   └── STRATEGIC_VISION.md  # Plain English vision document
└── bin/
    └── lattice              # CLI entrypoint
```

## Development Commands

```bash
# Install dependencies
npm install

# Build
npm run build

# Run tests
npm test

# Run tests with coverage
npm run test:coverage

# Lint
npm run lint

# Type check
npm run typecheck

# Format
npm run format

# Full CI check
make ci

# Run CLI locally
npm run cli -- <command>
# or after build:
./bin/lattice <command>
```

## Key Concepts

### Node Types
- **Source**: Primary research (papers, articles, URLs)
- **Thesis**: Strategic claims derived from research
- **Requirement**: Testable specifications derived from theses
- **Implementation**: Code that satisfies requirements

### Edge Types
- `supports`: Source → Thesis
- `derives`: Thesis → Requirement
- `satisfies`: Implementation → Requirement
- `depends_on`: Requirement → Requirement
- `reveals_gap_in`: Implementation → Requirement/Thesis (feedback)
- `challenges`: Any → Thesis (contradictory evidence)
- `validates`: Implementation → Thesis (confirming evidence)

### Version-Bound Edges
Edges record the version of both source and target nodes. When a node changes, edges bound to the old version are flagged as "potentially stale" — this enables drift detection.

### File-Based Storage
The `.lattice/` directory is the source of truth. YAML files organized by node type. Git provides versioning. The API (when built) reads/writes these files directly.

## Testing

TDD approach. Write tests first.

```bash
npm test                      # All tests
npm test -- --watch           # Watch mode
npm run test:coverage         # With coverage (80% threshold)
```

Test framework: Vitest
Coverage: 80% minimum

## Self-Describing

This repository uses Lattice to describe itself. The `.lattice/` directory contains:
- 6 sources (research)
- 6 theses (strategic claims)
- 18 requirements (specifications)

When implementing features, verify against the self-hosted requirements:
```bash
./bin/lattice list requirements --category CORE
./bin/lattice graph REQ-CORE-001 --direction upstream
```

## Git Checkpoint

When told **"checkpoint"** or **"commit and push"**:
1. `git status` to check changes
2. `git add` relevant files
3. Commit with descriptive message + `Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>`
4. Push to `origin/main`

## Requirements Traceability

Every feature links back to requirements in `.lattice/requirements/`:
- Reference requirement IDs in commit messages: `Implements REQ-CORE-001`
- Reference requirement IDs in test descriptions
- Update implementation nodes when code changes

## Architecture Rules

1. **Files are source of truth** — No database, no separate state
2. **Prose is primary** — Every node has human-readable title and body
3. **Edges are version-bound** — Always record target version for drift detection
4. **Feedback is first-class** — Support `reveals_gap_in`, `challenges`, `validates` edges
5. **Attribution required** — Every node/edge has `created_by` field
6. **Semver for nodes** — MAJOR.MINOR.PATCH versioning on all nodes

## CI/CD

GitHub Actions runs on every push:
- Lint (ESLint)
- Type check (TypeScript)
- Test (Vitest)
- Build

Local CI check: `make ci`
