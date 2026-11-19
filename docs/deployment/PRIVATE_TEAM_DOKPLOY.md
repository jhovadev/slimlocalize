# SlimLocalize: private-team deployment on Dokploy (REMOTE-01)

This is a **small, single-team preview**, not a public multi-tenant service. Never expose `sync-relay` or `server` directly. Only expose `web` (port **8080**) through Dokploy's TLS-enabled Traefik domain. Require valid HTTPS certificates and WSS. The browser UI is served publicly, but relay room access requires the team key at every join.

## Deploy

1. Generate **two different** long random secrets. On your own machine:

   ```bash
   openssl rand -hex 32   # SLIM_SYNC_TOKEN — share only with your trusted teammates
   openssl rand -hex 32   # SLIM_MCP_TOKEN — give only to authorized MCP clients
   ```

2. Set `SLIM_SYNC_TOKEN` and `SLIM_MCP_TOKEN` as secret environment variables in Dokploy for this Compose stack. Leave `SLIM_MCP_ALLOWED_ORIGIN` empty for CLI-based MCP clients. Keep the values **out of Git, URLs and frontend builds**. Do not reuse secrets.
3. Configure the Compose file `compose.dokploy.yaml`; deploy its `web` service. Attach a domain with HTTPS (port **8080**) via Dokploy. Do **not** attach domains to the Rust API or relay containers.
4. Check the stack and the health endpoint:

   ```bash
   docker compose -f compose.dokploy.yaml config --quiet
   docker compose -f compose.dokploy.yaml up -d --build
   curl -fsS https://YOUR-APP-DOMAIN/healthz
   ```

5. In the web UI, open a project, go to Settings > Share this project. Put `wss://YOUR-APP-DOMAIN/sync/` as the server, paste `SLIM_SYNC_TOKEN` as the team key, and press Connect. Each trusted friend must also receive the _pairing code_ and the _team key_ through separate secure channels. The pairing code is not authentication.
6. Test with two browser profiles, new translations, public environment variables, offline/reconnect and visible presence. Existing projects and all other non-secret metadata are still browser-local; this is **not yet a full cloud workspace**.

For local testing use the existing `just sync-up` with the development relay and a `ws://127.0.0.1:8790` URL; it does not require a team key. **Never expose the local-development compose profile to the public internet.**

## Remote MCP (read only)

Once deployed, use `https://YOUR-APP-DOMAIN/mcp`. The endpoint supports a stateless JSON-RPC 2.0 subset of MCP Streamable HTTP (`2025-11-25`), with a Bearer key in the request header. There is no remote write tool.

```bash
curl -fsS https://YOUR-APP-DOMAIN/mcp \
  -H "Authorization: Bearer $SLIM_MCP_TOKEN" \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"smoke","version":"0.1"}}}'

curl -fsS https://YOUR-APP-DOMAIN/mcp \
  -H "Authorization: Bearer $SLIM_MCP_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
```

**Data boundary:** The original eight MCP tools (`list_workspaces`, `list_projects`, etc.) still read `SLIMLOCALIZE_DB`, the separate Rust API database, which may initially be empty. Additional read-only tools (`list_shared_projects`, `list_shared_translations`, `list_shared_environments`) query **persisted Loro snapshots** in the relay's `sync.sqlite`, mounted read-only at `/sync-data/sync.sqlite`. This is a snapshot view, so expect a short delay (the relay's save interval) and no presence/cursor data. The shared-environments tool deliberately returns names and kinds only—never configuration values. Neither group membership nor user identity is exposed as an authorization claim.

## Security and limits

- `SLIM_SYNC_MODE=production` enforces the same 32+ character team token **at every Loro join** (documents and ephemeral presence), including reconnections. URL query tokens are not used. The frontend never embeds the token in its JavaScript bundle.
- This preview is _team-level_ authorization. **Every holder of the team key has access to all rooms on this relay**. It does **not** implement user accounts, invitations, revocation of one person, per-project ACL or individual audit logs. If one member leaves, rotate `SLIM_SYNC_TOKEN` and redistribute it; old data already copied to clients cannot be revoked remotely.
- The proxy intentionally blocks `/api/` and forwards only `/mcp` to the Rust API. That MCP endpoint uses a **separate** Bearer token, no anonymous access, denies `Origin` by default, has a 64KiB request limit, and only calls the existing read-only tool registry.
- Use HTTPS/WSS only. Data on the server is **not end-to-end encrypted**. Browser storage is local, including translations and local-only variables. The team key is held in memory and briefly in tab-scoped `sessionStorage` during the _Join project_ reload (up to 2 minutes). It is never copied to pairing codes or `localStorage`.
- Secrets must NEVER be placed into _public_ environment variables. Changing a previously shared variable to Local does not erase the other peers' copies.
- Implement passwordless identity/OIDC, invitations, per-project ACL in the relay authentication callback, separate keys per workspace, full project metadata replication, token rotation flows, rate limiting and audited browser E2E **before calling the product multi-tenant or production-ready**.
- Take backups of both named Docker volumes and test restore. Keep the Rust and JS Loro versions compatible; test schema migrations and two-browser concurrency before each release.

## Troubleshooting

- `401` MCP: Bearer key missing or incorrect.
- `403` MCP: `Origin` header supplied but `SLIM_MCP_ALLOWED_ORIGIN` not configured to match exactly; CLI agents normally omit Origin.
- WebSocket join rejected: wrong team key; check both relay and browser console. No query-string token is accepted.
- WSS network failures: check HTTPS certificate and Traefik domain. Ensure only the `web` container is routed publicly.
- Blank MCP `list_workspaces`: correct for a fresh API DB. Use `list_shared_projects` to find project IDs stored by the relay, followed by `list_shared_translations` for a particular project ID. These tools reflect persisted CRDT data only.
- Disconnect/reconnect: refresh requires a fresh key unless coming directly from the Join-project reload; intentionally avoids long-term browser storage.
