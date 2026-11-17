# Data model

Core entities:

- `Workspace`, `WorkspaceMember`
- `Project`
- `Locale`
- `Namespace`
- `TranslationKey`, `TranslationValue`
- `Environment`, `ConfigVariable`
- `AuditEvent`
- `SyncDocument`, `SyncCheckpoint`

Identifiers are UUID v4. Timestamps are RFC 3339 UTC strings. Environment
variables store `visibility` as `public` or `local`; `is_secret` is pinned to 0
until the encrypted vault exists.
