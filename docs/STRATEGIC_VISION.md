# Lattice: Strategic Vision

**Version**: 0.1.0
**Last Updated**: 2026-02-03
**Status**: Draft - Capturing Initial Vision

---

## The Core Insight

In the age of AI agents, **requirements become the most durable artifact** in a software system—more durable than code, which can be regenerated, and more precise than prose documentation, which drifts from reality.

But requirements don't exist in isolation. They're part of a **knowledge lattice**:

```
Primary Research (papers, data, citations)
    ↓ supports
Strategic Theses (claims about the world, market, technology)
    ↓ derives
Requirements (testable specifications)
    ↓ satisfied by
Implementations (code, config, infrastructure)
```

Current tools treat these layers as disconnected:
- Research lives in Google Docs or Notion
- Strategy lives in slide decks
- Requirements live in Jira or markdown files
- Code lives in git

**Lattice connects them into a single, traversable, version-aware knowledge graph.**

---

## Why Now?

### The Agent Revolution Changes Everything

AI agents are increasingly capable of:
- Reading and understanding requirements
- Generating implementations from specifications
- Verifying that code satisfies requirements
- Proposing new requirements based on implementation experience

But agents need **structured knowledge** to work effectively. They can't reliably extract requirements from a 50-page PRD or a Confluence wiki. They need:
- Clear, atomic, testable requirements
- Explicit relationships (what depends on what)
- Version-aware traceability (what changed since I last looked)
- A protocol for recording discoveries (gaps, challenges, validations)

### Current Tools Were Built for Humans

Jira, Notion, Confluence, Google Docs—all designed for human authors and human readers. They optimize for:
- Rich formatting and media
- Free-form collaboration
- Search and tagging

They don't optimize for:
- Machine-parseable structure
- Explicit relationship graphs
- Bidirectional traceability
- Drift detection

**Lattice is designed for the human-agent collaboration era.**

### Requirements Are Only Part of the Story

Most "requirements management" tools stop at requirements. But requirements don't justify themselves. They derive from strategic theses ("we believe X"), which are supported by research ("evidence shows Y").

When an agent implements a requirement, it should be able to ask:
- "Why does this requirement exist?" (trace to thesis)
- "What evidence supports this approach?" (trace to research)
- "What else depends on this?" (trace to downstream requirements and implementations)

And when an agent discovers something during implementation:
- "This requirement has a gap" (record feedback)
- "This evidence contradicts the thesis" (challenge upstream)
- "This implementation validates the thesis" (strengthen upstream)

**The lattice is bidirectional. Information flows up and down.**

---

## The Problem with Existing Approaches

### Problem 1: Requirements Drift from Reality

Requirements are written, then forgotten. Code evolves. The requirements document becomes a historical artifact, not a living specification.

**Lattice solution**: Bidirectional traceability with drift detection. When code changes, affected requirements are flagged. When requirements change, affected implementations are flagged.

### Problem 2: No Justification Chain

Why does REQ-AUTH-005 exist? Who knows. It's in Jira. Someone wrote it.

**Lattice solution**: Every requirement explicitly derives from a thesis. Every thesis is supported by sources. You can always answer "why?"

### Problem 3: Implicit Dependencies

REQ-API-003 depends on REQ-AUTH-001, but this is only in the developer's head. When AUTH-001 changes, no one remembers to check API-003.

**Lattice solution**: Explicit dependency edges. Change impact analysis is automatic.

### Problem 4: No Feedback Loop

Implementation reveals that the requirement was incomplete. Developer fixes the code, doesn't update the requirement. Knowledge is lost.

**Lattice solution**: First-class "reveals_gap_in" and "challenges" edges. Agents are expected to record what they learn during implementation.

### Problem 5: Research Disconnected from Decisions

The team read 20 papers before designing the system. Those papers live in someone's Zotero. Six months later, no one remembers why certain decisions were made.

**Lattice solution**: Sources are first-class nodes. Theses explicitly cite their supporting sources. The research is always one query away.

---

## Design Principles

### 1. Prose is Primary

Natural language descriptions capture intent and context that formal specifications cannot. Lattice doesn't deprecate prose—it structures it.

Every node has a `title` and `body` in plain English. The structured metadata (edges, versions, acceptance tests) augments the prose, doesn't replace it.

### 2. Files are the Source of Truth

Lattice stores knowledge in files (YAML/JSON), not a database. This means:
- Git provides versioning, history, blame, branching for free
- Works offline
- Diffable in PRs ("this PR changes thesis X")
- Portable across projects
- No vendor lock-in

The API server is a view over the files, not a separate source of truth.

### 3. The Graph is Explicit

Relationships between nodes are first-class, typed, and versioned. Edge types have semantics:
- `supports`: Research backs a claim
- `derives`: Requirement follows from thesis
- `satisfies`: Code implements requirement
- `reveals_gap_in`: Implementation found missing pieces
- `challenges`: Evidence contradicts a claim
- `conflicts_with`: Two things cannot both be true

### 4. Versions Matter

Edges are anchored to specific versions. When the upstream node changes, the edge becomes "potentially stale." This enables:
- Drift detection ("requirement changed since you verified")
- Impact analysis ("if I change this thesis, what's affected")
- Safe evolution ("old implementations still reference old requirement version")

### 5. Agents are First-Class Citizens

Lattice is designed for agent interaction:
- Structured API for queries and mutations
- Protocol for recording discoveries during implementation
- Drift alerts via webhooks or polling
- Clear attribution ("created_by: agent:claude-opus")

### 6. Humans Remain in Control

Agents can propose, but humans approve. Key operations require human confirmation:
- Creating new theses
- Deprecating requirements
- Resolving conflicts
- Promoting draft → active

### 7. Incremental Adoption

Lattice can be added to existing projects:
- Import existing REQUIREMENTS.md files
- Gradually add upstream traceability (theses, sources)
- Start with just requirements → implementations, add depth over time

---

## The Lattice Model

### Node Types

| Type | Description | Example |
|------|-------------|---------|
| **Source** | Primary research—papers, articles, data | "RLHF incentivizes sycophancy" (Anthropic, 2024) |
| **Thesis** | Strategic claim derived from research | "Multi-agent debate reduces sycophancy" |
| **Requirement** | Testable specification | "REQ-DEBATE-001: Seven Specialized Agents" |
| **Implementation** | Code that satisfies requirements | "IMP-DEBATE-ENGINE" |

### Edge Types

| Edge | From → To | Meaning |
|------|-----------|---------|
| `supports` | Source → Thesis | Research backs this claim |
| `derives` | Thesis → Requirement | Requirement follows from thesis |
| `satisfies` | Implementation → Requirement | Code implements spec |
| `depends_on` | Requirement → Requirement | Can't do X without Y |
| `reveals_gap_in` | Implementation → Requirement/Thesis | Found missing pieces |
| `challenges` | Any → Thesis | Evidence contradicts claim |
| `validates` | Implementation → Thesis | Evidence confirms claim |
| `conflicts_with` | Any → Any | Cannot both be true |
| `supersedes` | Any → Any | This replaces that |

### The Ecosystem is Cyclic

The graph isn't strictly hierarchical. Information flows in all directions:

```
Source ←───────────────────────────────────┐
   ↓ supports                              │ needs_research
Thesis ←────────────────────────────────────┤
   ↓ derives                               │ challenges
Requirement ←───────────────────────────────┤
   ↓ satisfied_by                          │ reveals_gap_in
Implementation ────────────────────────────┘
```

- Implementation reveals that a requirement is incomplete → `reveals_gap_in`
- Implementation shows that a thesis is wrong → `challenges`
- Thesis needs more evidence → `needs_research` (creates research task)
- New research strengthens or weakens existing theses → updates `supports` edges

---

## How It Works

### Directory Structure

```
project/
├── .lattice/
│   ├── config.yaml           # Lattice configuration
│   ├── sources/              # Research artifacts
│   │   └── sycophancy-anthropic.yaml
│   ├── theses/               # Strategic theses
│   │   └── multi-agent-value.yaml
│   ├── requirements/         # Requirements by category
│   │   ├── debate/
│   │   │   └── 001-seven-agents.yaml
│   │   └── llm/
│   │       └── 001-multi-provider.yaml
│   └── implementations/      # Implementation bindings
│       └── debate-engine.yaml
```

### A Requirement Node

```yaml
id: REQ-DEBATE-001
type: requirement

title: "Seven Specialized Agents"
body: |
  The system includes 7 specialized agents: Risk Analyst, Market Strategist,
  Financial Advisor, Technical Architect, Chief Scientist, Devil's Advocate,
  and Synthesis Builder.

priority: P0
category: DEBATE

acceptance:
  - given: "a debate is started"
    when: "agents are invoked"
    then: "7 distinct agents participate AND each produces an AnnotationLayer"

edges:
  derives_from:
    - target: THX-MULTI-AGENT-VALUE
      version: "1.0.0"
  depends_on:
    - target: REQ-ANNO-003  # Multi-layer architecture

status: active
version: "1.0.0"
created_by: "human:george"
```

### Drift Detection

When you change a requirement:
1. Its version increments
2. All `satisfies` edges pointing to the old version are flagged as "potentially stale"
3. Agents/humans are notified to re-verify

```bash
$ lattice drift
DRIFT DETECTED:

REQ-DEBATE-001 changed: 1.0.0 → 1.1.0
  ↳ IMP-DEBATE-ENGINE bound to 1.0.0 — NEEDS RE-VERIFICATION

REQ-LLM-003 changed: 1.2.0 → 2.0.0 (MAJOR)
  ↳ IMP-COMPETENCY-ROUTER bound to 1.2.0 — NEEDS RE-VERIFICATION
  ↳ REQ-LLM-005 depends_on 1.2.0 — REVIEW DEPENDENCY
```

### Agent Workflow

```markdown
1. Agent receives task: "Implement REQ-DEBATE-001"

2. Agent queries lattice:
   GET /lattice/nodes/REQ-DEBATE-001
   GET /lattice/graph/REQ-DEBATE-001?direction=upstream

   → Understands requirement + why it exists (thesis, research)

3. Agent implements, discovers gap:
   POST /lattice/edges
   { type: "reveals_gap_in", from: "IMP-DEBATE-ENGINE", to: "REQ-DEBATE-001",
     rationale: "Requirement doesn't specify agent failure handling" }

4. Agent completes, registers implementation:
   POST /lattice/verify
   { implementation: "IMP-DEBATE-ENGINE", requirement: "REQ-DEBATE-001",
     evidence: { tests_pass: true, coverage: 0.94 } }

5. Later, requirement changes. Agent is notified:
   GET /lattice/drift?scope=IMP-DEBATE-ENGINE
   → Re-verifies or updates implementation
```

---

## Views and Exports

Lattice is the source of truth. Human-readable documents are generated views:

```bash
# Generate traditional REQUIREMENTS.md
lattice export requirements --format markdown > docs/REQUIREMENTS.md

# Generate traceability matrix
lattice export matrix > docs/TRACEABILITY.md

# Generate research bibliography
lattice export sources --format bibtex > docs/references.bib

# Generate strategic overview
lattice export theses --with-sources > docs/STRATEGY.md
```

Generated documents include a header warning not to edit directly.

---

## What Lattice Enables

### For Humans

- **"Why does this requirement exist?"** → Trace to thesis and research
- **"What breaks if I change this?"** → Automatic impact analysis
- **"Is our implementation complete?"** → Coverage reports
- **"What did we learn during implementation?"** → Query feedback edges

### For Agents

- **"What should I implement?"** → Query uncovered requirements
- **"What context do I need?"** → Traverse upstream to theses and research
- **"What constraints apply?"** → Query conflicts and dependencies
- **"How do I record what I learned?"** → Post feedback edges

### For CI/CD

- **Block merges** if implementations drift from requirements
- **Auto-verify** when tests pass
- **Generate compliance reports** showing full traceability
- **Alert on coverage gaps** (requirements without implementations)

### For Compliance

- **Full audit trail** of who created/changed what and when
- **Justification chain** from implementation → requirement → thesis → research
- **Version history** with semantic versioning
- **Conflict documentation** showing how contradictions were resolved

---

## Competitive Landscape

### Traditional Requirements Tools (Jira, Azure DevOps, IBM DOORS)

- Designed for human workflows
- No native agent integration
- Weak traceability (manual links, not enforced)
- No upstream connection to strategy/research

### Documentation Tools (Notion, Confluence)

- Free-form, not structured
- No version-aware relationships
- No drift detection
- Requires humans to maintain consistency

### Formal Methods Tools (TLA+, Alloy, Z)

- Powerful but steep learning curve
- Don't integrate with code verification
- No strategic layer (pure specification)
- Overkill for most requirements

### Lattice

- **Agent-native**: Designed for human-agent collaboration
- **Full stack**: Research → Strategy → Requirements → Implementation
- **File-based**: Git-native, no database required
- **Pragmatic**: Prose-first with optional formalization
- **Bidirectional**: Feedback flows up, justification flows down

---

## Risks and Mitigations

### Risk: Overhead kills adoption

**Mitigation**: Minimal viable lattice is just requirements with acceptance tests. Add theses and sources incrementally. CLI makes common operations fast.

### Risk: Agents can't actually use it effectively

**Mitigation**: Simple REST API. Clear protocol documentation. Start with Lattice-aware Claude Code integration as proof point.

### Risk: Files get out of sync with reality

**Mitigation**: CI integration checks drift. Pre-commit hooks validate structure. Generated views have "do not edit" warnings.

### Risk: Too complex for small projects

**Mitigation**: Lattice scales down. A 10-requirement project can use a single `requirements.yaml` file. The full directory structure is optional.

---

## Roadmap

### Phase 1: Foundation (Current)

- [ ] Core data model (nodes, edges, versions)
- [ ] File-based storage format (YAML)
- [ ] CLI for basic operations (add, query, verify)
- [ ] Drift detection algorithm
- [ ] Export to markdown

### Phase 2: API & Integration

- [ ] REST API server
- [ ] WebSocket for real-time updates
- [ ] Git hooks for validation
- [ ] CI integration (GitHub Actions)

### Phase 3: Agent Protocol

- [ ] Claude Code integration
- [ ] Agent workflow documentation
- [ ] Feedback edge types
- [ ] Automated verification from test results

### Phase 4: Intelligence

- [ ] Impact analysis queries
- [ ] Coverage gap detection
- [ ] Conflict resolution workflow
- [ ] Source auto-extraction from URLs

### Phase 5: Ecosystem

- [ ] VS Code extension
- [ ] Import from Jira/Notion
- [ ] Multi-repo lattice federation
- [ ] Lattice-as-a-service (hosted API)

---

## The Meta Question: Lattice for Lattice

This project will use Lattice to specify itself. The `.lattice/` directory in this repo contains:
- Sources: Research on requirements engineering, agent systems, knowledge graphs
- Theses: Claims about why Lattice should exist and how it should work
- Requirements: Testable specifications for Lattice itself
- Implementations: The code that satisfies those requirements

This is the ultimate test: if Lattice can't describe itself, it can't describe anything.

---

## Summary

**Lattice is a knowledge coordination protocol for the human-agent era.**

It connects research → strategy → requirements → implementation into a single, traversable, version-aware graph. It's designed for:
- Agents that need structured knowledge to work effectively
- Humans who need to understand why things exist and what depends on what
- Teams that need traceability without the overhead of enterprise tools
- Projects that evolve, where drift detection matters

The source of truth is files. The API is a view. Markdown is generated. Agents and humans collaborate through a shared protocol.

**Better requirements → Better implementations → Better systems.**
