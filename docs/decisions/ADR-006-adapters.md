# ADR-006: CLI and MCP as domain consumers

Status: Accepted

Decision: CLI and MCP call the same application service instead of reimplementing
domain behavior.

Consequence: Behavior stays consistent across adapters.
