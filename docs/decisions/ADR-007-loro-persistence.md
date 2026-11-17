# ADR-007: Durable Loro persistence

Status: Accepted

Decision: Persist Loro snapshots, pending updates and append-only update logs in
SQLite before treating them as durable.

Consequence: Peers can restart and recover without losing confirmed operations.
EOF
