# ADR-008: Sync ownership

Status: Accepted

Decision: Use an independent optional relay based on the official Loro WebSocket
protocol instead of embedding public write endpoints in the Axum server.

Consequence: Offline-first operation remains possible and the main server stays
read-only.
EOF
