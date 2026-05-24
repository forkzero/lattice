# Lattice Integration

This project uses [Lattice](https://github.com/forkzero/lattice) for knowledge coordination. The `.lattice/` directory contains the knowledge graph — sources, theses, requirements, implementations, and messages connected by version-tracked edges.

## Quick Reference

```bash
lattice summary              # Status overview
lattice health               # Unified PASS/WARN/FAIL health check
lattice help concepts        # Node types, edge semantics, versioning
lattice help workflows       # Common task-oriented command sequences
lattice help                 # Full grouped command list
```

## When Working on Features

1. **Before starting**: `lattice summary` and `lattice plan` to see prioritized work
2. **Reference requirements** in commits: `Implements REQ-XXX-001`
3. **After completing**: `lattice resolve REQ-XXX-001 --verified`
4. **Verify**: `lattice drift` to confirm no stale edges

## Resolution States

- `verified` - Implemented and tested
- `blocked` - Waiting on dependency
- `deferred` - Postponed to later milestone
- `wontfix` - Rejected/out of scope

---

> **Using Claude Code?** Run `lattice init --skill` to install the `/lattice` skill instead of pasting this manually.
