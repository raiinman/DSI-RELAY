# Purpose

Own production RELAY Core deterministic local business logic and durable state.

# Ownership

- Governs `crates/relay-core/`.
- Must remain transport/UI/provider/tool agnostic.

# Local Contracts

- Depend on `relay-contracts` for command registry, validation, and versioned envelopes; own project/result/job state, idempotency boundary, provenance/trust metadata, diagnostics service, and storage migrations.
- No Windows IPC, dashboard HTTP, MCP, UEFN, or external-tool process code belongs here.
- Storage corruption and diagnostics failure must have independent health states.
- Phase 1 schema-1 SQLite stores must migrate safely and remain readable.
- Validation happens before business logic; incompatible command versions fail explicitly.
- Durable result/job outputs carry provenance/trust metadata.

# Verification

- Unit tests for registry, storage migration, idempotency, diagnostics, malformed input, and damaged storage.
- Integration tests through the public Core service surface.

# Child DOX Index

- No child DOX files currently exist.
