# Lattice Integration (MCP)

This project uses [Lattice](https://github.com/forkzero/lattice) for requirements tracking via MCP.

## MCP Tools

| Tool | Purpose |
|------|---------|
| `lattice_summary` | Status overview (start here) |
| `lattice_search` | Find nodes by criteria |
| `lattice_get` | Full node details |
| `lattice_list` | List nodes by type |
| `lattice_resolve` | Mark requirement status |
| `lattice_add_requirement` | Create new requirement |
| `lattice_drift` | Check for stale edges |

## Common Searches

```json
// Open P0 requirements
{"resolution": "unresolved", "priority": "P0"}

// All API requirements
{"id_prefix": "REQ-API"}

// Requirements related to a node
{"related_to": "REQ-CORE-001"}

// Tagged items
{"tags": ["mvp", "core"]}
```

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
