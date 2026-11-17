# Security Policy

## Supported versions

Only the latest development release is supported. This project is currently F0/F1.

## Reporting a vulnerability

Please open a private security advisory or email the maintainers. Do not disclose
publicly until a fix is available.

## Security model in F1

- All data is local by default.
- The optional Axum server is read-only and binds loopback unless explicitly
  overridden.
- Remote MCP is not implemented.
- Secret variables are rejected (`visibility=secret`) until the encrypted vault
  exists.
- SQLite is opened with foreign keys enabled.
- Import/export paths are canonicalized and checked against traversal.
- No secrets are written to logs by the core; adapters redact sensitive keys.

## Not yet implemented

- Encrypted vault and key management.
- Authentication and authorization.
- Remote sync.
- MCP write operations.
