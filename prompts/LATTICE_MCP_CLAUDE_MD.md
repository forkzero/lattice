# Lattice Integration (MCP)

This project uses [Lattice](https://github.com/forkzero/lattice) for knowledge coordination via MCP.

## MCP Tools

| Tool | Purpose |
|------|---------|
| `lattice_summary` | Status overview — start here |
| `lattice_search` | Find nodes by criteria |
| `lattice_get` | Full node details with edges |
| `lattice_list` | List nodes by type |
| `lattice_resolve` | Mark requirement status |
| `lattice_add_requirement` | Create new requirement |
| `lattice_drift` | Check for stale edge bindings |

## Workflow

1. **Start**: `lattice_summary` for current state
2. **Find work**: `lattice_search` with `resolution: "unresolved"`
3. **Get details**: `lattice_get` for full specification
4. **Complete**: `lattice_resolve` with `status: "verified"`

Reference requirement IDs in commits: `Implements REQ-XXX-001`
