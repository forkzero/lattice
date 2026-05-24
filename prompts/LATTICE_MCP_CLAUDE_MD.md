# Lattice Integration (MCP)

This project uses [Lattice](https://github.com/forkzero/lattice) for knowledge coordination via MCP. The `.lattice/` directory contains sources, theses, requirements, implementations, and messages connected by version-tracked edges.

## MCP Tools

| Tool | Purpose |
|------|---------|
| `lattice_summary` | Status overview — nodes, resolution, drift, contested theses |
| `lattice_search` | Find nodes by criteria (text, priority, resolution, tags) |
| `lattice_get` | Full node details with edges |
| `lattice_list` | List nodes by type (sources, theses, requirements, implementations, messages) |
| `lattice_resolve` | Mark requirement status (verified, blocked, deferred, wontfix) |
| `lattice_add_requirement` | Create new requirement |
| `lattice_drift` | Check for stale edge bindings |

## Node Types

| Type | ID Pattern | Purpose |
|------|-----------|---------|
| Source | `SRC-*` | Research, papers, references |
| Thesis | `THX-*` | Strategic claims (can be `contested`) |
| Requirement | `REQ-*` | Testable specifications |
| Implementation | `IMP-*` | Code that satisfies requirements |
| Message | `MSG-*` | Persona-specific claims grounded in theses |

## Workflow

1. **Start**: `lattice_summary` for current state
2. **Find work**: `lattice_search` with `resolution: "unresolved"`
3. **Get details**: `lattice_get` for full specification
4. **Complete**: `lattice_resolve` with `status: "verified"`

## Reference Requirements

When implementing features, note the requirement ID in commits:
```
Implements REQ-XXX-001
```
