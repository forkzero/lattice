# Adopting Lattice in Your Repo

A step-by-step guide for adding Lattice to an existing project.

## 1. Install

```bash
curl -fsSL https://forkzero.ai/lattice/install.sh | sh
```

Or build from source:

```bash
cargo install --git https://github.com/forkzero/lattice
```

## 2. Initialize

```bash
lattice init --skill
```

This creates the `.lattice/` directory with default config and installs the Claude Code skill and agent definitions under `.claude/`.

> **Note:** `.lattice/` is tracked in git by default. Every source, thesis, and requirement body is visible to anyone who can read the repo. For public repos, keep sensitive specifics (credentials, PII, financial detail) out of node bodies, or use a private repo for sensitive theses and requirements.

## 3. MCP Server (optional)

If your agent platform supports MCP, add Lattice as a tool server so agents can query the graph directly.

Add to `.mcp.json` (or your platform's equivalent):

```json
{
  "mcpServers": {
    "lattice": {
      "command": "lattice",
      "args": ["mcp"]
    }
  }
}
```

## 4. CI Gate

Add `lattice health --check` to your CI pipeline. It combines freshness, change pressure, and structural validation into a single PASS/WARN/FAIL verdict and exits non-zero on FAIL.

```yaml
- name: Lattice gate
  run: |
    lattice lint
    lattice health --check
```

## 5. Pre-commit and Makefile

Catch issues locally before CI runs:

```makefile
lattice-check:
	lattice lint
	lattice health --check

pre-commit: format lint lattice-check
```

## 6. Commit Convention

When a PR implements a requirement:

1. Reference the requirement in the commit message: `Implements REQ-XXX-NNN`
2. Record the implementation:
   ```bash
   lattice verify IMP-XXX-NNN satisfies REQ-XXX-NNN --tests-pass
   ```
3. Resolve the requirement:
   ```bash
   lattice resolve REQ-XXX-NNN --verified
   ```
4. Confirm no drift:
   ```bash
   lattice drift
   ```

## 7. Hosted Reader (optional)

For public graphs, export a static viewer suitable for GitHub Pages:

```bash
lattice export -f pages -o _site
```

See the [forkzero/lattice CI workflow](https://github.com/forkzero/lattice/blob/main/.github/workflows/release.yml) for a reference GitHub Pages deployment.

## 8. Data Hygiene

- `.lattice/` is tracked in git — all node bodies are visible to anyone who can read the repo
- Review `.lattice/` diffs before pushing, just as you would review code diffs
- For public repos, keep credentials, PII, and financial details out of node bodies
- Use a private repo for sensitive theses or requirements
- `lattice init` prints a reminder about this on every initialization
