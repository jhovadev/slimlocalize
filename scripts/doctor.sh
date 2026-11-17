#!/usr/bin/env bash
set -euo pipefail

pass=0
warn=0
fail=0
os="$(uname -s)"
arch="$(uname -m)"

check() {
  local label="$1"
  local status="$2"
  local detail="$3"
  printf '  [%s] %s: %s\n' "$status" "$label" "$detail"
  case "$status" in
    ok) pass=$((pass + 1)) ;;
    warn) warn=$((warn + 1)) ;;
    error) fail=$((fail + 1)) ;;
  esac
}

echo "SlimLocalize doctor"
echo "OS: $os ($arch)"
echo

for tool in rustc cargo mise bun just; do
  if command -v "$tool" >/dev/null 2>&1; then
    version="$("$tool" --version 2>/dev/null | head -n1)"
    check "$tool" ok "$version"
  else
    check "$tool" error "not found"
  fi
done

if command -v rustup >/dev/null 2>&1; then
  check "rustup toolchain" ok "$(rustup show active-toolchain 2>/dev/null | head -n1)"
else
  check "rustup toolchain" warn "rustup not found; rustc is available directly"
fi

if command -v cargo-clippy >/dev/null 2>&1; then
  check "cargo-clippy" ok "$(cargo clippy --version | head -n1)"
else
  check "cargo-clippy" warn "clippy unavailable; Rust linting will be skipped"
fi

if command -v docker >/dev/null 2>&1; then
  check "docker" ok "$(docker --version)"
else
  check "docker" warn "optional: required only for compose/server images"
fi

if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
  check "docker compose" ok "$(docker compose version)"
else
  check "docker compose" warn "optional: required only for compose"
fi

case "$os" in
  Linux)
    if pkg-config --exists webkit2gtk-4.1 gtk+-3.0 2>/dev/null; then
      check "Tauri native deps" ok "webkit2gtk-4.1 and gtk+-3.0"
    else
      check "Tauri native deps" warn "install libwebkit2gtk-4.1-dev libgtk-3-dev"
    fi
    ;;
  Darwin)
    if command -v xcodebuild >/dev/null 2>&1; then
      check "Xcode" ok "$(xcodebuild -version | head -n1)"
    else
      check "Xcode" warn "optional: required for iOS builds"
    fi
    ;;
  MINGW*|MSYS*|CYGWIN*)
    check "Windows native deps" warn "install WebView2 and the MSVC toolchain"
    ;;
esac

if [ -n "${ANDROID_HOME:-}" ] || [ -n "${ANDROID_SDK_ROOT:-}" ]; then
  check "Android SDK" ok "${ANDROID_HOME:-$ANDROID_SDK_ROOT}"
else
  check "Android SDK" warn "optional: set ANDROID_HOME for Android builds"
fi

echo
echo "Summary: $pass ok, $warn warnings, $fail blockers"
if [ "$fail" -gt 0 ]; then
  exit 1
fi
