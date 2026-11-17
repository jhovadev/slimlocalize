set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load := true

default:
    @just --list

doctor:
    @./scripts/doctor.sh

setup:
    bun install
    cargo fetch

dev: dev-desktop

dev-desktop:
    bun run --cwd apps/client tauri dev

dev-web:
    bun run --cwd apps/web dev

dev-server:
    cargo run -q -p slimlocalize-server

dev-android:
    ./scripts/require-tool.sh cargo-ndk
    bun run --cwd apps/client tauri android dev

dev-ios:
    ./scripts/require-tool.sh xcodebuild
    bun run --cwd apps/client tauri ios dev

fmt:
    cargo fmt --all
    bun run format

fmt-check:
    cargo fmt --all -- --check
    bun run format:check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

typecheck:
    bun run typecheck

test: test-rust test-web

test-rust:
    cargo test --workspace

test-web:
    bun run test

test-e2e:
    bun run test:e2e

build: build-web build-cli

build-desktop:
    bun run build:desktop

build-web:
    bun run build:web

build-server:
    cargo build --release -p slimlocalize-server

build-cli:
    cargo build --release -p slimlocalize-cli

db-migrate:
    cargo run -q -p slimlocalize-cli --bin localize -- db migrate

db-status:
    cargo run -q -p slimlocalize-cli --bin localize -- db status

db-backup:
    mkdir -p backups
    cargo run -q -p slimlocalize-cli --bin localize -- db backup --out backups/slimlocalize-$(date +%Y%m%d-%H%M%S).sqlite3

docker-build:
    docker compose build

docker-up:
    docker compose up -d

docker-down:
    docker compose down

docker-logs:
    docker compose logs -f

docker-config:
    docker compose config

docker-ps:
    docker compose ps

sync-build:
    docker compose --profile collaboration build sync-relay

sync-up:
    docker compose --profile collaboration up -d sync-relay

sync-down:
    docker compose --profile collaboration down

sync-logs:
    docker compose --profile collaboration logs -f sync-relay

sync-ps:
    docker compose --profile collaboration ps

sync-test:
    cargo test -p slimlocalize-collaboration

sync-smoke:
    docker compose --profile collaboration up -d sync-relay
    @sleep 8
    curl -fsS http://127.0.0.1:8791/
    docker compose --profile collaboration down

sync-demo:
    @echo "Use two isolated SQLite databases and the CLI sync commands:"
    @echo "  localize --db peer-a.sqlite3 sync bootstrap --project <slug>"
    @echo "  localize --db peer-a.sqlite3 sync export --project <slug> --output a.loro"
    @echo "  localize --db peer-b.sqlite3 sync import --project <slug> a.loro"

ci: fmt-check lint typecheck test build docker-config

clean:
    @read -r -p "Remove target/, node_modules/, dist/, backups/? [y/N] " answer; \
    case "$$answer" in y|Y) rm -rf target node_modules apps/*/node_modules packages/*/node_modules apps/*/dist packages/*/dist apps/web/dist backups; echo "cleaned";; *) echo "aborted";; esac
