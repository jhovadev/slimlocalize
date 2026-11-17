#!/usr/bin/env bash
set -euo pipefail

fail=0
warn=0

echo "SlimLocalize git audit"

tracked="$(git ls-files)"

if printf '%s\n' "$tracked" | grep -E '(^|/)(target|node_modules|dist|coverage|test-results|playwright-report)/' >/dev/null; then
  echo "  [error] generated directories are tracked"
  fail=$((fail + 1))
else
  echo "  [ok] no generated directories tracked"
fi

if printf '%s\n' "$tracked" | grep -E '(^|/)\.env($|\.)' | grep -v '\.env\.example$' >/dev/null; then
  echo "  [error] real environment files are tracked"
  fail=$((fail + 1))
else
  echo "  [ok] no real .env files tracked"
fi

if printf '%s\n' "$tracked" | grep -E '\.(sqlite|sqlite3|db)$' >/dev/null; then
  echo "  [error] local databases are tracked"
  fail=$((fail + 1))
else
  echo "  [ok] no local databases tracked"
fi

if printf '%s\n' "$tracked" | grep -E '\.(AppImage|deb|rpm|dmg|msi|exe)$' >/dev/null; then
  echo "  [error] build artifacts are tracked"
  fail=$((fail + 1))
else
  echo "  [ok] no build artifacts tracked"
fi

large="$(git ls-files -z | xargs -0 -r du -b 2>/dev/null | awk '$1 > 5242880 {print}' || true)"
if [ -n "$large" ]; then
  echo "  [error] tracked files larger than 5 MiB:"
  printf '%s\n' "$large"
  fail=$((fail + 1))
else
  echo "  [ok] no tracked files larger than 5 MiB"
fi

secret_pattern='(AKIA[0-9A-Z]{16}|ghp_[A-Za-z0-9]{36}|sk-[A-Za-z0-9]{20,}|-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----)'
matches="$(git grep -nE "$secret_pattern" -- . 2>/dev/null || true)"
if [ -n "$matches" ]; then
  echo "  [error] possible secret material detected"
  printf '%s\n' "$matches" | sed 's/:.*/:<redacted>/'
  fail=$((fail + 1))
else
  echo "  [ok] no obvious secret material detected"
fi

echo
echo "Summary: $fail blockers"
if [ "$fail" -gt 0 ]; then
  exit 1
fi
