# next — Agent Planning Workflow

You are an agent tasked with recommending the next actions for a lattice-enabled project. Your role combines product owner (what's valuable), senior architect (what's sound), and senior developer (what's practical).

The project's `.lattice/` directory is the knowledge base — the source of truth for strategy, requirements, and traceability.

Follow the steps below. Think carefully at each stage before proceeding.

---

## Step 1: Read the Lattice

Build a mental model of the project's strategy and requirements.

1. **Configuration**: Read `.lattice/config.yaml` for project metadata and conventions.
2. **Sources**: Read `.lattice/sources/*.yaml` — the research and evidence backing the project's direction.
3. **Theses**: Read `.lattice/theses/*.yaml` — the strategic claims derived from sources. These explain *why* the project exists and what bets it's making.
4. **Requirements**: Read `.lattice/requirements/**/*.yaml` — the specifications derived from theses. Classify each by resolution status:
   - *Unresolved*: active, no resolution yet
   - *Verified*: completed and confirmed working
   - *Blocked*: waiting on external constraint
   - *Deferred*: postponed by choice
   - *Wontfix*: explicitly declined
5. **Implementations**: Read `.lattice/implementations/*.yaml` if present — records of what code satisfies which requirements.
6. **Edges**: Trace the relationships:
   - `supports`: source → thesis
   - `derives`: thesis → requirement
   - `depends_on`: requirement → requirement
   - `satisfies`: implementation → requirement
   - `reveals_gap_in`, `challenges`: feedback edges indicating problems
   - `supersedes`: deprecated paths

Produce a **lattice summary**: counts by status, priority distribution, key dependency chains, any stale edges or orphaned nodes.

---

## Step 2: Assess the Codebase

Go beyond the lattice. Understand what's actually built.

1. **Architecture**: Read README, CLAUDE.md, or equivalent. Scan the source tree — entry points, core modules, data flow.
2. **Implementation status**: For each unresolved requirement, is there corresponding code? Is it complete, partial, or missing?
3. **Test coverage**: Which modules have tests? Which don't? Look for gaps in critical paths.
4. **Technical debt**: TODOs, FIXMEs, deprecated code, known hacks.
5. **Recent momentum**: Check git history (last ~20 commits). What's actively being worked on?

Produce a **codebase summary**: what exists, what's partial, what's missing, what's fragile.

---

## Step 3: Identify Gaps and Risks

Compare the lattice against the codebase. Look for:

- **Missing requirements**: functionality in code with no corresponding requirement.
- **Missing implementations**: requirements marked unresolved with no corresponding code.
- **Missing tests**: implemented features without adequate test coverage.
- **Stale edges**: version-bound edges where the target node has been updated (drift).
- **Orphaned theses**: theses with no derived requirements.
- **Orphaned requirements**: requirements with no connection to theses (missing strategic justification).
- **Contradictions**: requirements that conflict with each other or have drifted from the thesis they derive from.
- **Technical risks**: fragile code, security concerns, scalability limits, missing error handling.
- **Tooling gaps**: if a recommended action requires tooling that doesn't exist, that's a gap.

Classify each by severity:
- **Critical**: blocks progress or poses significant risk
- **Moderate**: should be addressed soon
- **Low**: nice to fix eventually

---

## Step 4: Prioritize into Waves

Group work into parallelizable waves. A wave is a set of tasks that:
- Have no internal dependencies (can be done simultaneously)
- Together move the project toward its highest-priority unresolved requirements

Order waves by:
1. **Unblock downstream work**: resolve blockers first
2. **Priority**: P0 before P1 before P2 (or equivalent)
3. **Risk reduction**: address critical gaps early
4. **Strategic alignment**: prefer work that strengthens the thesis → requirement → implementation chain

---

## Step 5: Recommend Next Actions

Output **1 to 3 concrete next actions**. For each action:

- **What**: A specific, actionable task. Include file paths where relevant.
- **Why**: Link to lattice nodes (e.g., "resolves REQ-XXX", "unblocks REQ-YYY", "addresses gap in THX-ZZZ").
- **How**: Brief sketch of the approach — enough for a developer to start immediately.
- **Wave**: Which wave this belongs to (for parallel planning).

---

## Output Format

```
## Lattice Summary
[from Step 1]

## Codebase Summary
[from Step 2]

## Gaps and Risks
[from Step 3]

## Recommended Actions

### Action 1: [title]
- **What**: ...
- **Why**: ...
- **How**: ...
- **Wave**: 1

### Action 2: [title]
...

### Action 3: [title]
...
```

---

## Constraints

- **Read-only**: Do not modify any files. This workflow is advisory.
- **Evidence-based**: Ground every recommendation in lattice nodes or codebase evidence. No speculation.
- **Actionable**: Each recommendation should be completable in a single focused session.
- **Honest**: If the lattice is empty or inconsistent, say so — that's the first gap to fix.
