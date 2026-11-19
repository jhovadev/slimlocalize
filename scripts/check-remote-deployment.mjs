// Structural regression smoke. Complement with cargo test, Bun CI and two-browser E2E.
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
const text = (p) => readFileSync(new URL(`../${p}`, import.meta.url), "utf8");
const compose = text("compose.dokploy.yaml");
assert.match(compose, /SLIM_SYNC_MODE:\s*"production"/);
assert.match(compose, /SLIM_SYNC_TOKEN:\s*"\$\{SLIM_SYNC_TOKEN:\?/);
assert.match(compose, /SLIM_MCP_TOKEN:\s*"\$\{SLIM_MCP_TOKEN:\?/);
assert.equal(
  (compose.match(/ports:/g) || []).length,
  1,
  "Only web should bind a host port",
);
assert.match(compose, /"127\.0\.0\.1:\$\{SLIM_WEB_PORT:-8080\}:8080"/);
const nginx = text("infra/docker/nginx.web.conf");
assert.match(nginx, /location \^~ \/sync\//);
assert.match(nginx, /location = \/mcp/);
assert.match(nginx, /location \^~ \/api\/ \{ return 404;/);
const client = text("packages/platform/src/web-sync.ts");
assert.match(client, /Remote collaboration requires wss:\/\//);
assert.match(client, /new TextEncoder\(\)\.encode\(relayKey\)/);
assert.doesNotMatch(client, /url:\s*assertLocalRelay\(DEFAULT_RELAY\)/);
console.log(
  "PASS deployment config / WSS / access-boundaries structural smoke",
);
