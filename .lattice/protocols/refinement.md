# Decision Refinement Protocol

When implementing a requirement, you may discover ambiguities, gaps, or contradictions not covered by the specification. Rather than making implicit decisions in code, use `lattice_refine` to create a sub-requirement that captures the decision explicitly.

## Gap Types

| Type | When to use | Agent action | Escalation |
|------|-------------|--------------|------------|
| `clarification` | Requirement is ambiguous but decision is low-stakes | Resolve with proposed answer, continue | Auto-approved in trust mode |
| `design_decision` | Multiple valid approaches with meaningful tradeoffs | Propose preferred option, flag for review | Always requires review |
| `missing_requirement` | Entirely new capability needed that wasn't anticipated | Create requirement, may block implementation | Requires PO or human review |
| `contradiction` | Requirements conflict with each other or with reality | Create `challenges` edge, stop and escalate | Always escalates to human |

## ID Convention

Sub-requirements use the parent ID with a letter suffix:

- Parent: `REQ-CORE-005`
- First refinement: `REQ-CORE-005-A`
- Second refinement: `REQ-CORE-005-B`

If a refinement reveals an entirely new concern (not a sub-decision of the parent), use a new top-level ID instead.

## Edge Wiring

The `lattice_refine` tool automatically creates:

1. **Sub-requirement node** with `derives_from` pointing to the parent's thesis
2. **`depends_on` edge** from parent to sub-requirement (parent can't be fully satisfied until sub-req is resolved)
3. **`reveals_gap_in` edge** from implementation to parent requirement (if an implementation ID is provided)

## Context to Capture

Every refinement should include:

- **The specific ambiguity**: What exactly is underspecified?
- **Why it matters**: What implementation decision depends on this?
- **Proposed resolution**: Your recommended answer (even for design decisions — always propose)
- **Alternatives considered**: For design decisions, note what you rejected and why

## Proposal Mode Behavior

| Mode | Clarification | Design Decision | Missing Req | Contradiction |
|------|--------------|-----------------|-------------|---------------|
| `trust` | Auto-resolve, continue | Create as draft, continue | Create as active, may block | Stop, escalate |
| `interactive` | Create as draft, wait | Create as draft, wait | Create as draft, wait | Stop, escalate |
| `batch` | Collect in branch | Collect in branch | Collect in branch | Stop, escalate |

## Example

```
Agent is implementing REQ-CORE-005 (Drift Detection).

Ambiguity: "The requirement says 'flag edges as stale' but doesn't
define what severity levels drift should have or how to triage."

lattice_refine(
  parent: "REQ-CORE-005",
  gap_type: "design_decision",
  title: "Drift severity classification levels",
  description: "Requirement says 'flag as stale' but doesn't define
    triage levels. Implementation needs to know whether to treat all
    drift equally or classify by impact.",
  proposed: "Three levels: info (patch version drift, likely cosmetic),
    warning (minor version drift, may affect behavior), error (major
    version drift, likely breaking). Based on semver semantics.",
  implementation: "IMP-DRIFT-001"
)
```

This creates `REQ-CORE-005-A` with the proposed resolution, flags it for review, and links `IMP-DRIFT-001 --reveals_gap_in--> REQ-CORE-005`.
