# Lattice Integration

This project uses [Lattice](https://github.com/forkzero/lattice) for requirements tracking. The `.lattice/` directory contains the knowledge graph.

## Quick Reference

```bash
lattice summary              # Status overview
lattice list requirements    # All requirements
lattice list requirements --status unresolved  # Open work
lattice get REQ-XXX-001      # Full details
lattice plan                 # What to work on next
lattice drift                # Check for stale edges
```

## When Working on Features

1. **Before starting**: `lattice plan` to see prioritized work
2. **Reference requirements** in commits: `Implements REQ-XXX-001`
3. **After completing**: `lattice resolve REQ-XXX-001 verified`

## Adding New Requirements

```bash
lattice add requirement \
  --id REQ-FEAT-001 \
  --title "Short description" \
  --body "Detailed specification" \
  --priority P1 \
  --category FEAT \
  --derives-from THX-XXX
```

## Node Types

| Type | Purpose | ID Pattern |
|------|---------|------------|
| Source | Research/references | `SRC-XXX` |
| Thesis | Strategic claims | `THX-XXX` |
| Requirement | Specifications | `REQ-XXX-NNN` |
| Implementation | Code bindings | `IMP-XXX-NNN` |

## Resolution States

- `verified` - Implemented and tested
- `blocked` - Waiting on dependency
- `deferred` - Postponed to later milestone
- `wontfix` - Rejected/out of scope

---

> **Using Claude Code?** Run `lattice init --skill` to install the `/lattice` skill instead of pasting this manually.
