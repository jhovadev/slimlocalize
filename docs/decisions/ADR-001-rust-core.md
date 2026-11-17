# ADR-001: Framework-independent Rust core

Status: Accepted

Decision: Domain logic lives in `crates/core` and has no dependency on Tauri,
Axum, SQLite or CLI.

Consequence: Tauri, HTTP and CLI adapters share one implementation and can be
tested without UI or network.
