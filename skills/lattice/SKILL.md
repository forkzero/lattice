---
name: lattice
description: "Lattice knowledge graph integration. Use when working in a project with a .lattice/ directory — for requirements, theses, sources, implementations, messages, drift detection, or lattice CLI commands."
allowed-tools: Bash(lattice *), Bash(./target/release/lattice *), Bash(./target/debug/lattice *), Bash(gh issue *), Read, Grep, Glob
---

# Lattice Skill

You have access to a **Lattice** knowledge graph in this project. The `.lattice/` directory contains the structured knowledge graph (YAML files). Use the `lattice` CLI to query and modify it.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY in this document are to be interpreted as described in RFC 2119.

## Discovery

```bash
lattice help                    # Grouped command list
lattice help concepts           # Node types, edge semantics, versioning, ID conventions
lattice help workflows          # Common task-oriented command sequences
lattice <command> --help        # Help for any specific command
lattice --json --compact        # Machine-readable command schema
```

## Editing Rules

Agents MUST use the `lattice` CLI for all supported operations.
The CLI handles timestamps, version bumps, edge wiring, and ID validation.

If the CLI does not support a required operation, agents MAY edit `.lattice/` YAML files directly, but MUST run `lattice lint` immediately after to verify correctness.

If a CLI gap is found, agents SHOULD file an issue on `forkzero/lattice`:
```bash
gh issue create --repo forkzero/lattice \
  --title "CLI gap: <what's missing>" \
  --body "Encountered while working on <context>. Had to edit YAML directly because <reason>." \
  --label "cli-gap"
```

## Workflow

### Before Starting Work
1. Run `lattice summary` to understand the current state
2. Run `lattice health` to check overall lattice health
3. Run `lattice plan` or `lattice search --priority P0 --resolution unresolved` to find what to work on
4. Run `lattice get REQ-XXX` to read full requirement details before implementing

### While Working
- Reference requirement IDs in commits: `Implements REQ-XXX-001`
- Use `lattice get <ID> --format json` for structured data when you need to parse output

### After Completing Work
1. `lattice resolve REQ-XXX --verified` to mark requirements as done
2. `lattice verify IMP-XXX satisfies REQ-XXX --tests-pass` to record satisfaction evidence
3. `lattice edit IMP-XXX --files "src/new.rs" --test-command "cargo test"` to update implementation nodes
4. `lattice add edge --from IMP-XXX --type satisfies --to REQ-XXX` to wire new edges
5. Run `lattice drift` to confirm no unresolved drift

### If Gaps Are Found
- `lattice refine REQ-XXX --gap-type missing_requirement --title "..." --description "..."` to create sub-requirements
- `lattice add edge --from IMP-XXX --type reveals_gap_in --to REQ-XXX --rationale "..."` to record feedback

## JSON Output

All read and write commands accept `--format json` for structured output. Use this when you need to parse results programmatically.

## Product Owner Agent

For backlog triage, strategic critique, and planning work, use the **product-owner** agent (`/product-owner`). It manages the lattice as persistent working memory for product strategy.
