# Lattice Integration

This project uses [Lattice](https://github.com/forkzero/lattice) for knowledge coordination. The `.lattice/` directory contains the knowledge graph — sources, theses, requirements, implementations, and messages connected by version-tracked edges.

## Quick Reference

```bash
lattice summary              # Status overview (nodes, resolution, drift)
lattice health               # Unified PASS/WARN/FAIL health check
lattice list requirements    # All requirements
lattice list messages        # All messages
lattice get REQ-XXX-001      # Full details with edges
lattice search -q "keyword"  # Text search
lattice plan REQ-A REQ-B     # Implementation order
lattice drift                # Check for stale edge bindings
lattice help concepts        # Node types, edge semantics, versioning
```

## When Working on Features

1. **Before starting**: `lattice summary` and `lattice plan` to see prioritized work
2. **Reference requirements** in commits: `Implements REQ-XXX-001`
3. **After completing**: `lattice resolve REQ-XXX-001 --verified`
4. **Verify**: `lattice drift` to confirm no stale edges

## Node Types

| Type | Purpose | ID Pattern |
|------|---------|------------|
| Source | Research/references | `SRC-XXX` |
| Thesis | Strategic claims (can be `contested`) | `THX-XXX` |
| Requirement | Specifications | `REQ-XXX-NNN` |
| Implementation | Code bindings | `IMP-XXX-NNN` |
| Message | Persona-specific claims | `MSG-XXX-NNN` |

## Key Edge Types

- `supported_by`, `derives_from`, `satisfies`, `depends_on` (traceability)
- `reveals_gap_in`, `challenges`, `validates` (feedback)
- `rebuts`, `concedes` (adversarial debate)
- `grounded_in` (message → thesis)

## Resolution States

- `verified` - Implemented and tested
- `blocked` - Waiting on dependency
- `deferred` - Postponed to later milestone
- `wontfix` - Rejected/out of scope

---

> **Using Claude Code?** Run `lattice init --skill` to install the `/lattice` skill instead of pasting this manually.
