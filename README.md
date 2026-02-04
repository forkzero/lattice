# Lattice

A knowledge coordination protocol for the human-agent era.

## Installation

```bash
curl -fsSL https://raw.githubusercontent.com/forkzero/lattice/main/install.sh | sh
```

The installer automatically detects your OS and architecture (macOS/Linux, x86_64/ARM64).

### Verify Installation

```bash
lattice --version
```

### Install Specific Version

```bash
VERSION=0.0.1 curl -fsSL https://raw.githubusercontent.com/forkzero/lattice/main/install.sh | sh
```

### Manual Installation

Download binaries directly from [GitHub Releases](https://github.com/forkzero/lattice/releases):

| Platform | Binary |
|----------|--------|
| macOS Apple Silicon | `lattice-VERSION-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `lattice-VERSION-x86_64-apple-darwin.tar.gz` |
| Linux x86_64 | `lattice-VERSION-x86_64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 | `lattice-VERSION-aarch64-unknown-linux-gnu.tar.gz` |

```bash
# Example manual install (macOS Apple Silicon)
curl -fsSL https://github.com/forkzero/lattice/releases/download/v0.0.1/lattice-0.0.1-aarch64-apple-darwin.tar.gz | tar -xz
sudo mv lattice-0.0.1-aarch64-apple-darwin/lattice /usr/local/bin/
```

### Uninstall

```bash
sudo rm /usr/local/bin/lattice
```

## What is Lattice?

Lattice connects research, strategy, requirements, and implementation into a single, traversable, version-aware knowledge graph.

```
Primary Research (papers, data, citations)
    ↓ supports
Strategic Theses (claims about the world)
    ↓ derives
Requirements (testable specifications)
    ↓ satisfied by
Implementations (code that does the work)
```

Unlike traditional requirements tools (Jira, Notion, Confluence), Lattice is designed for **human-agent collaboration**:

- **Structured for machines**: Typed nodes, explicit edges, queryable graph
- **Readable by humans**: Prose-first, with structure as augmentation
- **Version-aware**: Drift detection when artifacts diverge
- **Bidirectional**: Information flows up (feedback) and down (justification)
- **File-native**: Git provides versioning; no database required

## Quick Start

```bash
# Initialize lattice in your project
lattice init

# Add a requirement
lattice add requirement \
  --id REQ-AUTH-001 \
  --title "JWT Authentication" \
  --body "Implement JWT-based authentication" \
  --priority P0 \
  --category AUTH

# Query the lattice
lattice list requirements
lattice get REQ-AUTH-001

# Check for drift
lattice drift

# Export narrative
lattice export --audience overview
lattice export --audience investor
lattice export --audience contributor
```

## Directory Structure

```
your-project/
├── .lattice/
│   ├── config.yaml           # Lattice configuration
│   ├── sources/              # Research artifacts
│   │   └── *.yaml
│   ├── theses/               # Strategic claims
│   │   └── *.yaml
│   ├── requirements/         # Specifications
│   │   ├── core/
│   │   ├── api/
│   │   └── *.yaml
│   └── implementations/      # Code bindings
│       └── *.yaml
├── src/                      # Your code
└── docs/
    └── REQUIREMENTS.md       # Generated view
```

## Core Concepts

### Nodes

Four types of knowledge artifacts:

| Type | Description | Example |
|------|-------------|---------|
| **Source** | Primary research | "RLHF incentivizes sycophancy" (Anthropic, 2024) |
| **Thesis** | Strategic claim | "Multi-agent debate reduces sycophancy" |
| **Requirement** | Testable spec | "REQ-DEBATE-001: Seven Specialized Agents" |
| **Implementation** | Code binding | "IMP-DEBATE-ENGINE" → src/debate/*.ts |

### Edges

Typed relationships with version binding:

| Edge | Meaning |
|------|---------|
| `supports` | Research backs a thesis |
| `derives` | Requirement follows from thesis |
| `satisfies` | Code implements requirement |
| `depends_on` | Prerequisite relationship |
| `reveals_gap_in` | Implementation found missing pieces |
| `challenges` | Evidence contradicts a thesis |
| `validates` | Evidence confirms a thesis |

### Drift Detection

When a requirement changes, implementations bound to the old version are flagged:

```bash
$ lattice drift
DRIFT DETECTED:

REQ-AUTH-001 changed: 1.0.0 → 1.1.0
  ↳ IMP-AUTH-JWT bound to 1.0.0 — NEEDS RE-VERIFICATION
```

## For Agents

Lattice provides a structured protocol for agent interaction:

1. **Before implementing**: Query requirement and traverse upstream
2. **During**: Record gaps and challenges via feedback edges
3. **After**: Register implementation and verify
4. **On drift**: Re-verify or update

## Self-Describing

This repository uses Lattice to describe itself. The `.lattice/` directory contains:

- **Sources**: Research on requirements engineering, knowledge graphs, agent systems
- **Theses**: Why Lattice should exist and how it should work
- **Requirements**: Specifications for Lattice itself (36 requirements)

```bash
# See the lattice for Lattice
lattice list requirements
lattice export --audience overview
```

## Status

**v0.0.1** - Early development. Core CLI implemented, API in progress.

See [docs/STRATEGIC_VISION.md](docs/STRATEGIC_VISION.md) for the full vision.

## License

Copyright (c) 2026 ForkZero. All rights reserved.

This source code is provided for reference and evaluation purposes only.
See [LICENSE](LICENSE) for details.
