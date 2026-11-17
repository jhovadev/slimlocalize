# Conflict semantics

Loro convergence is technical, not semantic.

- Concurrent edits to the same translation value converge deterministically.
- Distinct keys and locales merge.
- Convergence does not imply placeholder correctness.
- After import, SlimLocalize runs domain validation separately.
- Human review remains required for sensitive changes.
  EOF
