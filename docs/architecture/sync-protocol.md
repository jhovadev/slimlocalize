# Sync protocol

SlimLocalize uses the official Loro WebSocket protocol for optional relay
synchronization.

## Offline-first exchange

For local development, peers exchange versioned JSON envelopes:

- `format`: `slimlocalize-sync`
- `protocolVersion`
- `schemaVersion`
- `projectId`
- `documentKey`
- `peerId`
- `versionVector` (base64)
- `payload` (base64 Loro update)
- `sha256` (hex)

The payload is treated as untrusted. Import validates project identity, schema,
size and checksum before applying it.

## Room identity

Rooms are derived from `project:<project-id>`. The relay does not trust arbitrary
client room names.

## Modes

- Development: loopback by default, writes allowed for local testing.
- Production: requires `SLIM_SYNC_TOKEN`; unauthenticated joins are denied.
  EOF
