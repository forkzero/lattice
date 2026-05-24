# Lattice

[![CI](https://github.com/forkzero/lattice/actions/workflows/ci.yml/badge.svg)](https://github.com/forkzero/lattice/actions/workflows/ci.yml)
[![Release](https://github.com/forkzero/lattice/actions/workflows/release.yml/badge.svg)](https://github.com/forkzero/lattice/actions/workflows/release.yml)
[![Latest Release](https://img.shields.io/github/v/release/forkzero/lattice)](https://github.com/forkzero/lattice/releases/latest)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

**A knowledge coordination protocol for the human-agent era.**

Built by [Forkzero](https://forkzero.ai).

Your AI agents write code. But do they know *why*? Lattice connects research, strategy, requirements, and implementation into a traversable knowledge graph — so agents (and humans) can trace any decision back to its source.

```
Sources (research, papers, data)
    ↓ supports
Theses (strategic claims)          ←─ rebuts/concedes (adversarial debate)
    ↓ derives                       ←─ grounded_in (messages)
Requirements (testable specifications)
    ↓ satisfied by
Implementations (code)              ──→ reveals_gap_in (feedback)
```

## The Problem

Traditional tools fragment knowledge:
- **Research** lives in docs, wikis, or someone's head
- **Strategy** is implicit or buried in meetings
- **Requirements** are in Jira/Notion without traceability
- **Code** exists without knowing why it was built

AI agents make this worse. They implement requirements without understanding the reasoning. When requirements change, nobody knows what code is affected.

## How Lattice Helps

| Capability | What it means |
|------------|---------------|
| **Traceability** | Every requirement links to strategic theses. Every thesis links to research. |
| **Drift detection** | When requirements change, implementations bound to old versions are flagged. |
| **Bidirectional feedback** | Implementations can `challenge` or `validate` theses. Gaps flow upstream. |
| **Adversarial debate** | Theses can `rebut` and `concede` to each other. Confidence tracked over time. |
| **Messaging** | Persona-specific claims `grounded_in` theses. Drift-detected when theses weaken. |
| **Health checks** | Unified `lattice health` — freshness, change pressure, and code impact in one verdict. |
| **Agent-native** | MCP server, structured queries, JSON output. Agents can reason about the graph. |
| **Git-native** | YAML files in `.lattice/`. No database. Branch, merge, version control. |

## Quick Look

```bash
# What should I work on?
$ lattice plan REQ-AUTH-001 REQ-AUTH-002
Ready to implement:
  REQ-AUTH-001  JWT Authentication  (P0, all deps verified)

Blocked:
  REQ-AUTH-002  OAuth Integration   (depends on REQ-AUTH-001)

# Why does this requirement exist?
$ lattice get REQ-AUTH-001
REQ-AUTH-001: JWT Authentication
Derives from: THX-SECURITY-FIRST (v1.0.0)
Body: Implement JWT-based authentication with refresh tokens...

# Has anything drifted?
$ lattice drift
DRIFT DETECTED:
  REQ-AUTH-001 changed: 1.0.0 → 1.1.0
    ↳ IMP-AUTH-JWT bound to 1.0.0 — NEEDS RE-VERIFICATION
```

## Comparison

| | Jira/Linear | Notion/Confluence | Beads | Spec Kit | **Lattice** |
|---|:-----------:|:-----------------:|:-----:|:--------:|:-----------:|
| Tracks requirements | ✓ | ✓ | ✓ | ✓ | ✓ |
| Links to research/strategy | | | | ~ | ✓ |
| Version-bound edges | | | | | ✓ |
| Drift detection | | | | | ✓ |
| Bidirectional feedback | | | | | ✓ |
| Adversarial debate | | | | | ✓ |
| Persona messaging | | | | | ✓ |
| Unified health checks | | | | | ✓ |
| Git-native | | | ✓ | ✓ | ✓ |
| MCP server | | | | | ✓ |

## Core Concepts

**Nodes** — Five artifact types:
- **Source** (SRC-*): Research (papers, data, citations)
- **Thesis** (THX-*): Strategic claims derived from research
- **Requirement** (REQ-*): Testable specifications derived from theses
- **Implementation** (IMP-*): Code that satisfies requirements
- **Message** (MSG-*): Persona-specific claims grounded in theses

**Edges** — Typed, version-bound relationships:
- `supported_by`, `derives_from`, `satisfies`, `depends_on` (traceability)
- `reveals_gap_in`, `challenges`, `validates` (feedback flows upstream)
- `rebuts`, `concedes` (adversarial debate between theses)
- `grounded_in` (message → thesis traceability)

**Status** — Theses can be `draft`, `active`, `contested`, `deprecated`, or `superseded`.

**Resolution** — Requirements track status:
- `verified` (implemented + tested)
- `blocked` (waiting on dependency)
- `deferred` (postponed)
- `wontfix` (rejected)

## Installation

**macOS / Linux:**
```bash
curl -fsSL https://forkzero.ai/lattice/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://forkzero.ai/lattice/install.ps1 | iex
```

Or download from [GitHub Releases](https://github.com/forkzero/lattice/releases).

<details>
<summary>Platform-specific binaries</summary>

| Platform | Binary |
|----------|--------|
| macOS Apple Silicon | `lattice-VERSION-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `lattice-VERSION-x86_64-apple-darwin.tar.gz` |
| Linux x86_64 | `lattice-VERSION-x86_64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 | `lattice-VERSION-aarch64-unknown-linux-gnu.tar.gz` |
| Windows x86_64 | `lattice-VERSION-x86_64-pc-windows-msvc.tar.gz` |

</details>

## Getting Started

```bash
# Initialize in your project
lattice init

# Or initialize with Claude Code skill + agent support
lattice init --skill

# Add a requirement
lattice add requirement \
  --id REQ-AUTH-001 \
  --title "JWT Authentication" \
  --body "Implement JWT-based auth with refresh tokens" \
  --priority P0 \
  --category AUTH

# Query the lattice
lattice summary              # Overview
lattice list requirements    # All requirements
lattice get REQ-AUTH-001     # Full details

# Search with filters
lattice search -q "auth"                        # Keyword search (ranked by relevance)
lattice search -q "auth" --priority P0           # Filter by priority
lattice search --tag mvp --resolution unresolved # Unresolved MVP items

# Health checks
lattice health                  # Unified PASS/WARN/FAIL verdict
lattice health --check          # CI gate — exits 2 on FAIL
lattice assess                  # Change pressure (contested theses, drift)
lattice freshness               # Time gap between code and lattice updates

# Export
lattice export --format json > lattice-data.json
lattice export --format pages --output _site
lattice export --audience investor

# Learn the domain model
lattice help concepts           # Node types, edge semantics, versioning
lattice help workflows          # Common command sequences
```

## Publishing Documentation

Lattice can publish an interactive dashboard to GitHub Pages with a single command. Add this to your CI:

```yaml
# .github/workflows/pages.yml
- run: curl -fsSL https://forkzero.ai/lattice/install.sh | sh
- run: lattice export --format pages --output _site
```

The `pages` format exports `lattice-data.json` and a redirect `index.html` that points to the [hosted reader](https://forkzero.ai/reader) on forkzero.ai. The GitHub Pages URL is derived automatically from your git remote.

The reader displays stats, coverage, resolution status, priority breakdown, traceability tree, and filterable requirements — all from the JSON export.

You can also use `lattice export --format html` for a self-contained HTML dashboard.

## For AI Agents

### Claude Code (recommended)

```bash
# Install the /lattice skill and product-owner agent
lattice init --skill
```

This creates `.claude/skills/lattice/SKILL.md` (a `/lattice` skill with commands, workflow, and node/edge reference) and `.claude/agents/product-owner.md` (a product owner agent for backlog triage and planning).

### MCP Server

Lattice includes an MCP server for broader AI integration:

```bash
# Run as MCP server
lattice mcp
```

Or add to your project's `.mcp.json`:

```json
{
  "mcpServers": {
    "lattice": {
      "command": "lattice",
      "args": ["mcp"]
    }
  }
}
```

**MCP Tools**: `lattice_summary`, `lattice_search`, `lattice_get`, `lattice_list`, `lattice_resolve`, `lattice_add_requirement`, `lattice_drift`

### Manual CLAUDE.md snippet

```bash
# Generate CLAUDE.md integration snippet
lattice prompt >> CLAUDE.md
lattice prompt --mcp >> CLAUDE.md
```

## Self-Describing

Lattice is built with Lattice. The `.lattice/` directory contains sources, theses, and requirements for Lattice itself.

**[View Live Documentation](https://forkzero.ai/reader?url=https://forkzero.github.io/lattice/lattice-data.json)**

```bash
lattice list requirements
lattice export --audience overview
```

## Status

**v0.2.1** — Adversarial debate (rebuts/concedes edges, contested status, confidence history), messages (persona-specific claims grounded in theses), unified health check, change pressure assessment, schema versioning.

See [docs/STRATEGIC_VISION.md](docs/STRATEGIC_VISION.md) for the full vision.

## License

Copyright (c) 2026 Forkzero. All rights reserved.
See [LICENSE](LICENSE) for details.
