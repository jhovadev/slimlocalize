#!/usr/bin/env bash
set -euo pipefail
if ! command -v "$1" >/dev/null 2>&1; then
  echo "Missing required tool: $1" >&2
  exit 1
fi
