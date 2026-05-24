# PRFAQ: Lattice Continuous Learning

## Press Release

**Lattice Enables Software Projects to Continuously Self-Improve Through Autonomous Research and Adversarial Debate**

*Projects that use Lattice can now wake on a schedule, discover new evidence, stress-test their own strategic assumptions, and propose changes — all while humans sleep.*

Today, Forkzero announces Lattice Continuous Learning — a capability that turns any Lattice-managed project into a self-improving system. By combining Lattice's version-tracked knowledge graph with scheduled autonomous agents, projects can continuously discover new research, evaluate it against existing strategic claims, refine requirements, and surface implementation proposals as pull requests for human review.

"Most software projects make strategic decisions once and never revisit them," said George Moon, founder of Forkzero. "Requirements rot. Theses go unchallenged. The research that informed a decision six months ago may have been contradicted by three papers since. Lattice Continuous Learning closes that loop."

**How it works:**

A project maintainer configures a learning schedule and research scope in their `.lattice/config.yaml`. On each cycle, Lattice orchestrates a multi-phase pipeline:

1. **Survey** — For each thesis in the lattice, an agent searches for new sources (papers, articles, industry reports) that either support or contradict the claim. New sources are added to the graph with `supports` or `challenges` edges.

2. **Debate** — When contradicting evidence is found, a second agent generates the strongest possible counterargument as a competing thesis linked via a `rebuts` edge. A third agent defends the original position. The debate is recorded in the graph as a series of thesis nodes with typed edges — not discarded after the session.

3. **Assess** — Thesis confidence scores are updated based on the debate outcome. Theses whose confidence drops below a configurable threshold are flagged as `contested`. Requirements that derive from contested theses are automatically flagged for review.

4. **Refine** — If enough requirements are affected (configurable "change pressure" threshold), Lattice generates a planning cycle: new or revised requirements, updated priorities, and a proposed implementation order.

5. **Propose** — For requirements that are implementable, agents write code and open pull requests. Each PR links back to the thesis, the new evidence, and the debate that motivated the change.

Humans review the PRs, the updated theses, and the new evidence. They can accept, reject, or redirect. The graph records everything — every source found, every argument made, every confidence change — creating a complete audit trail of how the project's strategy evolved.

**Availability:**

Lattice Continuous Learning will be available as a premium feature for teams using Lattice Cloud. The core lattice primitives (adversarial edges, confidence tracking, contested status) ship in the open-source CLI.

---

## Frequently Asked Questions

### Customer FAQ

**Q: What actually triggers a learning cycle? Is it a cron job?**

A: Yes — you configure a schedule in `.lattice/config.yaml` (e.g., `research_cadence: weekly`). A scheduled agent wakes up, reads the lattice state, and runs the pipeline. The agent uses the lattice graph itself to decide what to do: which theses haven't been researched recently, which sources are stale, where confidence is lowest. The lattice is both the state and the instruction set.

**Q: How does the agent know what to research? I don't want it randomly googling.**

A: The agent's research is scoped by the theses in your lattice. Each thesis is a specific, falsifiable claim (e.g., "File-based YAML storage scales to 10,000 nodes without performance degradation"). The agent searches for evidence relevant to that claim — not general browsing. You can further scope research by adding `research_scope` metadata to theses (e.g., "arxiv, HN, specific blogs") or by limiting which theses are eligible for autonomous research.

**Q: What if the agent finds something that invalidates a core thesis? Does it just change my project direction?**

A: No. The agent records evidence and updates confidence scores, but it never unilaterally changes project direction. When a thesis becomes `contested` (confidence drops below threshold), it surfaces to humans for review. The agent proposes — you decide. The debate record in the graph shows exactly why confidence changed, so you can evaluate the reasoning yourself.

**Q: How is this different from just having an LLM summarize papers for me?**

A: Three ways. First, the knowledge compounds across cycles — each research session builds on what was found before, not starting from scratch. Second, the adversarial debate structure means the agent actively tries to disprove your theses, not just confirm them. Third, everything is version-tracked with typed edges, so you can trace any requirement change back to the specific evidence and debate that motivated it.

**Q: What does "change pressure" mean? How does it decide when to act?**

A: Change pressure is a metric computed from the graph: how many theses have shifted confidence, how many requirements are affected downstream, and how much time has passed since the last planning cycle. You configure a threshold (e.g., "plan when 3+ theses shift by 0.2+ confidence affecting 5+ requirements"). Below the threshold, the agent just records evidence and waits. Above it, it triggers a planning cycle.

**Q: Can I run a learning cycle manually instead of waiting for the schedule?**

A: Yes. `lattice research` runs the survey phase on demand. `lattice debate <thesis-id>` triggers adversarial evaluation of a specific thesis. `lattice assess` recomputes confidence across the graph. You can run any phase independently or the full pipeline.

**Q: What if I disagree with the agent's confidence assessment?**

A: Override it. `lattice edit THX-FILE-OVER-DB --confidence 0.9` sets the confidence back. The confidence history records both the agent's assessment and your override, preserving the audit trail. The agent will continue researching in future cycles, but your manual override stands until new evidence warrants another assessment.

**Q: How do I prevent the agent from going down rabbit holes?**

A: Three controls. (1) Research scope on theses — limit which sources the agent searches. (2) Time budget per cycle — cap how long the agent spends researching. (3) Change pressure threshold — require significant evidence accumulation before acting. The agent is also constrained by the graph structure: it can only research theses that exist and can only propose changes to requirements that derive from those theses.

### Internal FAQ

**Q: What's the orchestration layer? Where does the "wake up and run" logic live?**

A: This is the key architectural question. Three options under consideration:

*Option A: External scheduler + CLI.* A cron job or GitHub Actions workflow runs `lattice research && lattice debate && lattice assess && lattice refine`. The lattice CLI does the work; the scheduler just triggers it. Simple, composable, but requires the customer to set up infrastructure.

*Option B: Lattice Cloud scheduler.* Lattice Cloud runs the learning cycle as a managed service. Customer configures the schedule in `.lattice/config.yaml`; we run the agents on their behalf. Higher-value, but requires cloud infrastructure.

*Option C: Claude Code triggers.* For teams already using Claude Code, a scheduled trigger (like the existing `schedule` skill) runs the learning cycle as a background agent. No new infrastructure needed — just a Claude Code subscription.

Current leaning: **A for open-source, C for Claude Code users, B for enterprise.** The lattice CLI primitives (`research`, `debate`, `assess`, `refine`) are the same regardless of orchestration layer.

**Q: Where do the LLM calls happen? Does the lattice CLI call an LLM API?**

A: No. The lattice CLI remains a state layer — it reads/writes the graph, computes drift, tracks confidence. The LLM calls happen in the agent that invokes the CLI. This separation means:
- The CLI works without API keys (pure graph operations)
- Any LLM can drive the loop (Claude, GPT, Gemini, local models)
- The customer controls which model is used and at what cost
- The CLI is testable without mocking LLM APIs

The lattice CLI provides structured output (`--format json`) that agents parse, and structured input (CLI flags) that agents invoke. The agent is the brain; lattice is the memory.

**Q: How do we prevent the graph from growing unboundedly?**

A: Several mechanisms: (1) Source deduplication — if a source URL already exists, the agent updates it rather than adding a duplicate. (2) Thesis consolidation — when a debate resolves, the losing thesis is marked `superseded` with a `supersedes` edge from the winner. (3) Staleness pruning — sources older than a configurable threshold and not recently cited can be archived. (4) The graph's typed structure naturally constrains growth — there are only so many theses in a project, and each thesis only generates research on its specific claim.

**Q: What's the MVP? What ships first?**

A: Layer 1 (lattice primitives): `rebuts` and `concedes` edge types, confidence history on theses, `contested` thesis status, source freshness tracking. These ship in the open-source CLI and are useful even without automation — humans can manually record debates and track evidence.

Layer 2 (agent commands): `lattice research`, `lattice debate`, `lattice assess`. These are structured CLI commands that output/accept JSON, designed to be called by any agent. They don't call LLMs themselves.

Layer 3 (orchestration): Scheduled execution via cron, Claude Code triggers, or Lattice Cloud. This is where the "continuous" in continuous learning comes from.

**Q: How does this relate to Karpathy's autoresearch pattern?**

A: Autoresearch uses three components: a modifiable workspace (`train.py`), a frozen evaluator (`prepare.py`), and human instructions (`program.md`). Lattice maps to all three: theses are the modifiable workspace (confidence changes, new evidence), requirements are the frozen evaluator (acceptance criteria don't change unless explicitly revised), and the graph structure + node bodies are the instructions (they tell the agent what to research and what to build). The key difference is that autoresearch optimizes a single scalar metric, while Lattice optimizes a multi-dimensional knowledge graph. The keep-or-revert pattern maps to Lattice's version-bound edges and drift detection.

**Q: What's the competitive moat?**

A: Three things competitors lack: (1) The graph structure — typed nodes and version-bound edges create an audit trail that flat document stores can't match. (2) Human-in-the-loop by design — the graph captures both autonomous and human decisions, with clear provenance. Every commercial auto-research tool today is either fully autonomous (no human oversight) or fully manual (no automation). Lattice sits in the middle. (3) The prompt layer insight — the graph isn't just memory, it's the agent's operating instructions. Changing a thesis changes what the agent researches. Changing a requirement changes what the agent builds. The graph programs the agent.
