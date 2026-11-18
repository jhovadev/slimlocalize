#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
printf '%s\n' 'Checking UX-01 source files and assets...'
test -f packages/ui/src/QuickAddKey.tsx
test -f packages/ui/src/InlineValue.tsx
grep -q 'showQuickAdd' packages/ui/src/AppShell.tsx
! grep -q 'autoComplete=' packages/ui/src/QuickAddKey.tsx
! grep -Eq 'LOCAL-FIRST|Local data|Local and ready|SQLite \(local, WAL\)' packages/ui/src/AppShell.tsx
if grep -q '<select' packages/ui/src/AppShell.tsx; then
  echo 'Unexpected native select in AppShell' >&2; exit 1
fi
grep -q 'translation-layout--details' packages/ui/src/styles/app.css
if command -v bun >/dev/null 2>&1; then
  bun run typecheck
  bun run test
  bun run build:web
else
  echo 'Bun unavailable: frontend tests/build NOT RUN.' >&2
fi
