# ADR-003: SQLite as local persistence

Status: Accepted

Decision: Use embedded SQLite with versioned migrations, WAL, constraints and
foreign keys.

Consequence: No database server is required for development or normal use.
