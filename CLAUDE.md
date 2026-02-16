# CLAUDE.md

Instructions for Claude Code when working in this repository.

## Project Overview

**Lattice** — A knowledge coordination protocol for human-agent collaboration. Rust CLI and library. Connects research → strategy → requirements → implementation into a traversable, version-aware graph. File-based storage (YAML in `.lattice/`), Git-native.

## Repository Structure

```
/
├── src/
│   ├── main.rs             # CLI entry point (clap)
│   ├── lib.rs              # Library exports
│   ├── types.rs            # Core data model (nodes, edges, versions)
│   ├── storage.rs          # File-based storage layer
│   ├── graph.rs            # Graph traversal and drift detection
│   └── export.rs           # Export to markdown narrative
├── tests/                  # Integration tests
├── .lattice/               # Lattice describing itself (self-hosting)
│   ├── config.yaml
│   ├── sources/            # Research backing theses
│   ├── theses/             # Strategic claims
│   ├── requirements/       # Specifications for Lattice
│   └── implementations/    # Code bindings
├── docs/
│   └── STRATEGIC_VISION.md # Plain English vision document
├── Cargo.toml
└── Cargo.lock
```

## Development Commands

```bash
# Build
cargo build

# Build release (optimized)
cargo build --release

# Run tests
cargo test

# Run linter
cargo clippy

# Format code
cargo fmt

# Run CLI locally
cargo run -- <command>
# or after build:
./target/debug/lattice <command>
./target/release/lattice <command>

# Pre-commit gate (ALWAYS run before committing)
make pre-commit

# Pre-push gate (ALWAYS run before pushing)
make pre-push
```

**Important**: Always run `make pre-commit` before any commit and `make pre-push` before any push. This matches what CI checks and prevents push-then-fix cycles.

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
The `.lattice/` directory is the source of truth. YAML files organized by node type. Git provides versioning. The CLI reads/writes these files directly.

## CLI Commands

```bash
lattice init                    # Initialize lattice (stub)
lattice list <type>             # List nodes (sources, theses, requirements, implementations)
lattice get <id>                # Get a specific node
lattice drift                   # Check for version drift
lattice drift --check           # Exit non-zero if drift detected
lattice add requirement ...     # Add a requirement
lattice add thesis ...          # Add a thesis
lattice add source ...          # Add a source
lattice export                  # Export narrative (overview)
lattice export -a investor      # Export for investors
lattice export -a contributor   # Export for contributors
lattice export -f json          # Export as JSON
lattice export -f pages -o _site  # Export for GitHub Pages
```

## Self-Describing

This repository uses Lattice to describe itself. The `.lattice/` directory contains:
- 6 sources (research)
- 6 theses (strategic claims)
- 27 requirements (specifications)

When implementing features, verify against the self-hosted requirements:
```bash
cargo run -- list requirements
cargo run -- get REQ-CORE-001
```

## Git Checkpoint

When told **"checkpoint"** or **"commit and push"**:
1. `git status` to check changes
2. `git add` relevant files
3. Commit with descriptive message + `Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>`
4. Push to `origin/main`

## Requirements Traceability

Every feature links back to requirements in `.lattice/requirements/`:
- Reference requirement IDs in commit messages: `Implements REQ-CORE-001`
- Reference requirement IDs in source file doc comments
- Update implementation nodes when code changes

## Agent Workflows

See `.lattice/prompts/` for agent workflows (e.g., `lattice next` planning).

## Architecture Rules

1. **Files are source of truth** — No database, no separate state
2. **Prose is primary** — Every node has human-readable title and body
3. **Edges are version-bound** — Always record target version for drift detection
4. **Feedback is first-class** — Support `reveals_gap_in`, `challenges`, `validates` edges
5. **Attribution required** — Every node/edge has `created_by` field
6. **Semver for nodes** — MAJOR.MINOR.PATCH versioning on all nodes

## Releasing

When told **"release"**, **"bump the release"**, or **"cut a release"**:

1. **Bump version** in `Cargo.toml` (follow semver: patch for fixes, minor for features, major for breaking changes)
2. **Commit** the version bump: `Bump version to vX.Y.Z`
3. **Push** to `origin/main`
4. **Wait for CI** to pass (use ci-monitor agent)
5. **Create the release** with `gh release create` using good release notes (see below)
6. **Monitor the release workflow** (use ci-monitor agent) — it runs CI again, cross-builds 4 platform binaries, deploys pages + install script, and runs e2e verification

### Release Notes

Write meaningful release notes — not just a changelog link. Include:

- **Summary**: 1-2 sentence overview of what this release brings
- **What's New / Changed / Fixed**: Grouped bullet points describing user-visible changes
- **Full Changelog link**: Append at the end for completeness

Example format:
```bash
gh release create vX.Y.Z --title "vX.Y.Z" --notes "$(cat <<'EOF'
## Summary

Brief description of the release theme.

## What's New

- Feature A: short description
- Feature B: short description

## What's Changed

- Improvement to X
- Refactored Y for better Z

## Fixes

- Fixed issue with W

**Full Changelog**: https://github.com/forkzero/lattice/compare/vPREV...vX.Y.Z
EOF
)"
```

### Release Workflow

The `release.yml` workflow triggers on `release: published` and runs:
1. Full CI (reuses `ci.yml` via `workflow_call`)
2. Cross-compile for 4 targets (aarch64/x86_64 × macOS/Linux)
3. Upload binaries + checksums to the GitHub Release
4. Deploy documentation to GitHub Pages
5. Deploy install script to S3 (`forkzero.ai/lattice/install.sh`)
6. E2E verification of the published release via install script

## CI/CD

GitHub Actions (`ci.yml`) runs on every push and as part of releases:
- Format check (rustfmt)
- Lint (clippy)
- Test (cargo test)
- Build (debug + release)
- CLI smoke tests against `.lattice/`
- E2E integration tests
- Code coverage (40% threshold)

## CI/CD Monitoring

After any `git push`, automatically spawn the `ci-monitor` agent to track the GitHub Actions workflow and alert on failures.

### Custom Agents

Custom agents are defined in `.claude/agents/`:

| Agent | Purpose |
|-------|---------|
| `ci-monitor` | Monitor GitHub Actions after push, alert on failures |

### Manual Monitoring

```bash
gh run list --limit 5              # Recent runs
gh run watch <RUN_ID>              # Watch live
gh run view <RUN_ID> --log-failed  # Failed logs
gh run rerun <RUN_ID> --failed     # Re-run failures
```
