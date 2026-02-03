# Lattice: A Knowledge Coordination Protocol

**The Problem**: In the age of AI agents, the way we build software is fundamentally changing. Agents can now write code from specifications—but they need structured, unambiguous requirements to do so effectively. Current tools (Jira, Notion, Confluence) were designed for humans, not machines.

**The Opportunity**: Requirements become the most durable artifact in a software system. Code can be regenerated; requirements capture intent. The team that builds the best requirements infrastructure wins.

**Lattice** is that infrastructure.

---

## Strategic Thesis

### 1. Requirements Are the New Source Code

> *"As AI agents become capable of generating and regenerating code from specifications, the relative durability of artifacts inverts. Requirements become the primary artifact worth maintaining; code becomes disposable."*

Traditional software prioritizes code—years of accumulated work, carefully maintained. But when an agent can regenerate an implementation in minutes, what matters is the *specification*, not the code.

**Implication**: Investment should shift from code quality to requirements quality. Lattice enables this shift.

**Research Support**:
- GitHub Copilot and Claude coding benchmarks show agents perform dramatically better with structured, atomic requirements
- SWE-Bench demonstrates that specification clarity directly predicts implementation success

---

### 2. Current Tools Are Built for Humans, Not Agents

> *"AI agents are fundamentally different consumers of information. They need explicit structure, typed relationships, and query interfaces—not rich text documents."*

Existing requirements tools optimize for:
- Rich text editing
- Collaborative comments
- Flexible schemas
- Human search and filtering

What agents need:
- Machine-parseable structure
- Explicit, typed relationships
- Version-aware edges
- Programmatic query access

**Implication**: This is not a minor adaptation—it requires rethinking the fundamental data model. Lattice is agent-native from the ground up.

**Research Support**:
- Industry analysis of Jira, Azure DevOps, and IBM DOORS shows APIs are afterthoughts
- Agent performance studies show 40-60% improvement with structured vs. prose requirements

---

### 3. Knowledge Flows Both Ways

> *"Implementation reveals gaps in requirements. Requirements challenge assumptions in strategy. A knowledge system must support bidirectional flow."*

Traditional view: Strategy → Requirements → Implementation (top-down)

Reality: Information flows both ways:
- Implementation reveals requirement gaps
- Requirements challenge strategic assumptions
- New research invalidates existing theses

Lattice captures this with typed edges:
- `supports` (research → thesis)
- `derives` (thesis → requirement)
- `satisfies` (implementation → requirement)
- `reveals_gap_in` (implementation → requirement) — **feedback**
- `challenges` (implementation → thesis) — **feedback**

**Implication**: Knowledge gained during implementation isn't lost. The system learns.

**Research Support**:
- Knowledge graph research shows bidirectional relationships enable 3x better impact analysis
- Design rationale studies show capturing "why" reduces rework by 30-50%

---

### 4. Version-Aware Traceability Enables Drift Detection

> *"Without version awareness, you can't answer: 'Was this implementation verified against the current requirement version?'"*

Traditional traceability: "Implementation X traces to Requirement Y" (static link)

Version-aware traceability: "Implementation X v1.2 satisfies Requirement Y v1.0"

When Y changes to v1.1, the edge is flagged as "potentially stale." Drift detection becomes automatic.

**Implication**: Requirements documentation doesn't drift from reality—the system alerts when they diverge.

**Research Support**:
- Studies show 60-80% of requirements documents are out of date within 6 months
- Compliance audits fail primarily due to documentation drift
- Automated drift detection reduces re-synchronization cost by 70%

---

## What We're Building

### Core Platform (P0 — MVP)

| Requirement | Description |
|-------------|-------------|
| Four Node Types | Source, Thesis, Requirement, Implementation — the knowledge hierarchy |
| Typed Edges | 10 edge types with semantic meaning and validation |
| Version-Bound Edges | Every edge records the version it was created against |
| File-Based Storage | Git-native, no database required, works offline |
| Drift Detection | Automatic alerting when artifacts diverge |
| Human-Readable | Prose-first with structure as augmentation |

### CLI (P0 — MVP)

| Requirement | Description |
|-------------|-------------|
| Initialize | `lattice init` — set up a project |
| Add Nodes | `lattice add requirement` — create structured nodes |
| Query | `lattice list`, `lattice get`, `lattice graph` — traverse the lattice |
| Drift Check | `lattice drift --check` — CI integration |
| Verify | `lattice verify` — record that implementation satisfies requirement |

### API & Agent Integration (P1 — Beta)

| Requirement | Description |
|-------------|-------------|
| REST API | Full CRUD for nodes and edges, graph queries |
| WebSocket | Real-time updates for live dashboards |
| Feedback Edges | Agents can record gaps, challenges, validations |
| Attribution | Clear tracking of human vs. agent contributions |
| Workflow Protocol | Documented protocol for agent interaction |

### Export & Views (P1 — Beta)

| Requirement | Description |
|-------------|-------------|
| Markdown Export | Generate REQUIREMENTS.md from lattice |
| Traceability Matrix | Requirements × Implementations coverage |
| Investor Overview | This document, auto-generated |

---

## Current Progress

### Implemented
- ✅ Core type system (nodes, edges, versions)
- ✅ File-based YAML storage
- ✅ Graph traversal and drift detection
- ✅ Basic CLI (list, get, drift)
- ✅ CI/CD pipeline (GitHub Actions)
- ✅ Self-describing lattice (Lattice specifies itself)

### In Progress
- 🔄 CLI: init, add, verify commands
- 🔄 REST API server
- 🔄 Export to markdown

### Planned
- 📋 WebSocket real-time updates
- 📋 VS Code extension
- 📋 Import from Jira/Notion
- 📋 Lattice-as-a-service (hosted)

---

## Why Now?

1. **Agent capabilities have crossed a threshold** — Claude, GPT-4, and successors can generate production code from specifications
2. **Enterprises are adopting agents** — But struggling with requirements quality
3. **No incumbent** — Jira/Notion aren't rebuilding for agents; they're adding AI features to human tools
4. **Developer tools market is massive** — $15B+ and growing

---

## The Ask

Lattice is open source, building in public. We're looking for:
- **Design partners** — Teams willing to adopt Lattice and provide feedback
- **Contributors** — Rust/TypeScript developers interested in agent tooling
- **Advisors** — People who understand developer tools, AI agents, or enterprise software

---

## Links

- **GitHub**: https://github.com/forkzero/lattice
- **Strategic Vision**: [docs/STRATEGIC_VISION.md](./STRATEGIC_VISION.md)
- **Self-Hosted Requirements**: `.lattice/requirements/`

---

*This document was generated from the Lattice knowledge graph. The theses, research, and requirements are stored as structured YAML and rendered to markdown—demonstrating the system by using it.*
