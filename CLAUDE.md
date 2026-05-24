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
│   ├── implementations/    # Code bindings
│   └── messages/           # Persona-specific messaging
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

**Code review**: Before non-trivial commits, run `/simplify` to review changed code for reuse, quality, and efficiency issues. Skip for version bumps, doc-only changes, or single-line fixes.

## Key Concepts

### Node Types
- **Source** (SRC-*): Primary research (papers, articles, URLs)
- **Thesis** (THX-*): Strategic claims derived from research
- **Requirement** (REQ-*): Testable specifications derived from theses
- **Implementation** (IMP-*): Code that satisfies requirements
- **Message** (MSG-*): Persona-specific claims grounded in theses

### Edge Types
- `supported_by`: Thesis → Source (research backing)
- `derives_from`: Requirement → Thesis (specification from strategy)
- `satisfies`: Implementation → Requirement (code fulfills spec)
- `depends_on`: Requirement → Requirement (dependency)
- `reveals_gap_in`: Implementation → Requirement/Thesis (feedback)
- `challenges`: Any → Thesis (contradictory evidence)
- `validates`: Implementation → Thesis (confirming evidence)
- `rebuts`: Thesis → Thesis (adversarial debate)
- `concedes`: Thesis → Thesis (partial agreement in debate)
- `grounded_in`: Message → Thesis (messaging traceability)
- `extends`, `conflicts_with`, `supersedes`: General-purpose edges

### Thesis Status
Theses can be `draft`, `active`, `contested`, `deprecated`, or `superseded`. The `contested` status signals a thesis is under active adversarial challenge — requirements downstream of contested theses are flagged by `lattice assess`.

### Version-Bound Edges
Edges record the version of both source and target nodes. When a node changes, edges bound to the old version are flagged as "potentially stale" — this enables drift detection.

### File-Based Storage
The `.lattice/` directory is the source of truth. YAML files organized by node type. Git provides versioning. The CLI reads/writes these files directly.

## CLI Commands

```bash
# Knowledge Graph
lattice add source ...          # Add research (papers, articles, data)
lattice add thesis ...          # Add a strategic claim backed by sources
lattice add requirement ...     # Add a testable specification
lattice add implementation ...  # Add code that satisfies requirements
lattice add message ...         # Add persona-specific messaging grounded in theses
lattice add edge ...            # Add an edge between nodes
lattice get <id>                # Get a specific node with full details
lattice list <type>             # List nodes (sources, theses, requirements, implementations, messages)
lattice search -q <query>       # Search with filters (text, priority, resolution, tags)
lattice edit <id> ...           # Edit a node (auto-bumps version)
lattice resolve <id> --verified # Resolve a requirement
lattice verify IMP... satisfies REQ...  # Record implementation satisfaction
lattice refine <req-id> ...     # Create sub-requirement from discovered gap
lattice remove edge ...         # Remove an edge
lattice replace edge ...        # Retarget an edge

# Health & Analysis
lattice summary                 # Status overview (nodes, resolution, drift)
lattice drift                   # Check for version drift in edge bindings
lattice freshness               # Check if lattice is updated alongside code
lattice assess                  # Assess change pressure (contested theses, drift)
lattice health                  # Unified health check (PASS/WARN/FAIL verdict)
lattice health --check          # CI gate — exits 2 on FAIL
lattice lint                    # Check for structural issues
lattice diff                    # Show changes since a git ref
lattice plan REQ-001 REQ-002    # Plan implementation order
lattice export                  # Export narrative (overview)
lattice export -a investor      # Export for investors
lattice export -f json          # Export as JSON
lattice export -f pages -o _site  # Export for GitHub Pages

# Setup
lattice init                    # Initialize lattice
lattice init --skill            # Initialize + install Claude Code skill + agents
lattice update                  # Self-update to latest version
lattice help                    # Show grouped command list
lattice help concepts           # Node types, edge semantics, versioning
lattice help workflows          # Common task-oriented command sequences
lattice --json                  # Machine-readable command catalog for LLMs
lattice --json --compact        # Compact schema (signatures only, no examples)
```

## Self-Describing

This repository uses Lattice to describe itself. The `.lattice/` directory contains:
- 19 sources (research)
- 10 theses (strategic claims)
- 81 requirements (specifications)
- 12 implementations (code bindings)

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

## Lattice Integration (Mandatory)

This project uses Lattice to describe itself. The `.lattice/` directory contains the knowledge graph.

**NEVER read or write `.lattice/` files directly. ALL lattice operations MUST use the `lattice` CLI.**

### Requirements Traceability

Every feature links back to requirements in `.lattice/requirements/`:
- Reference requirement IDs in commit messages: `Implements REQ-CORE-001`
- Reference requirement IDs in source file doc comments
- Update implementation nodes when code changes

### Working Tickets

When asked to work on a ticket (GitHub issue, etc.), follow this workflow:

**1. Assess** — Before any code, evaluate the ticket against the lattice:
   - Run `lattice summary` and `lattice plan` to understand current state
   - Determine if the ticket requires new or modified **requirements**, **theses**, or **sources**
   - Identify if additional research is needed (new sources)
   - Flag any existing requirements that will be affected

**2. Comment on the ticket** — Add a comment to the ticket listing:
   - Planned new requirements, theses, and sources
   - Existing lattice nodes that will be affected
   - Any research gaps that need to be filled

**3. Execute (strict order)**:
   1. **Lattice first** — Use the lattice CLI to create/update requirements, theses, and sources BEFORE writing any code
   2. **Code second** — Implement the feature or fix
   3. **Record & verify last** — Record implementations and run drift detection

**4. Close the ticket** — Include a lattice summary in the closing comment:
   - List all added/modified/resolved lattice nodes
   - Confirm `lattice drift` reports no unresolved drift

### Plan Mode Workflow

When entering plan mode, every plan MUST include a **Lattice Impact** section:

1. Run `lattice summary` and `lattice plan` to understand current state
2. Evaluate whether the planned work requires new or modified requirements, theses, or sources
3. Identify if additional research is needed
4. Flag any existing requirements that will be affected
5. List all lattice changes as part of the plan output for user review

### Recording Implementations

After completing coding tasks:

```bash
# Record what was implemented
lattice add implementation --id IMP-XXX-NNN --requires REQ-XXX-NNN \
  --bind "file:function" --status pass

# Resolve the requirement
lattice resolve REQ-XXX-NNN verified

# Confirm no drift
lattice drift
```

A task is NOT complete until `lattice drift` reports no unresolved drift for affected requirements. Reference requirements in commits: `Implements REQ-XXX-001`.

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
