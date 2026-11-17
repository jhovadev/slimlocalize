---
name: translation-auditor
description: Inspect and validate a SlimLocalize workspace using the localize CLI. Use when an agent needs to check missing translations, placeholders, export deterministic JSON, or summarize project health without mutating data.
license: MIT
---

# Translation auditor

Use this skill to inspect a SlimLocalize workspace read-only.

## Prerequisites

- The `localize` binary must be available.
- A local database path may be supplied with `--db`.

## Read-only workflow

1. List workspaces:
   `localize --json workspace list`
2. Resolve the target project:
   `localize --json project list --workspace <slug>`
3. Validate:
   `localize --json check --workspace <slug> --project <slug>`
4. Export deterministic files:
   `localize --json export --workspace <slug> --project <slug> --out ./audit-export --nested`

## Rules

- Prefer `--json` for machine parsing.
- Do not run mutating commands unless the user explicitly asks.
- Treat `hasIssues` as the gating result.
- Report missing keys and placeholder mismatches separately.
