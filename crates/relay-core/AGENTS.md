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
- Phase 3 project baselines and change rows are project-scoped, derived state. Canonical roots stay local; machine-facing index paths are project-relative. Reconciliation treats watcher paths as hints and hashes changed candidates after authoritative metadata enumeration.
- Schema-5 delta reads and dependency edges are bounded, project-scoped derived state. Edge replacement requires the current index generation and source content hash; source/target changes invalidate edges, and rebuild resets the delta boundary.
- Hint-only index updates read and hash only named files, commit `stale` state, and never claim complete continuity. Authoritative reconciliation restores `ready`; delta and dependency-edge reads/writes require ready state.
- Normal reconciliation checks all filesystem metadata and hashes changed candidates. Explicit `verify_content` reconciliation hashes all files for uncertain continuity and detects same-size/same-timestamp edits; schedule this expensive recovery around foreground work.
- Trusted daemon watcher input can query local indexed roots and durably mark index status `stale` without changing generation. These methods never expose canonical roots through command results.

# Verification

- Unit tests for registry, storage migration, idempotency, diagnostics, malformed input, and damaged storage.
- Integration tests through the public Core service surface.
- Schema-3-to-4 migration preserves Phase 2 authority state; project index tests cover generation guards, isolation, missed hints, and rebuildability.
- Schema-4-to-5 migration preserves index records and marks a conservative baseline generation; delta and edge tests cover stale input, bounds, restart, and cross-project authority.
- Hint-update tests cover targeted work counts, rename/directory hints, missed-event recovery, stale restart, and cross-project denial.
- Content-verification tests cover same-size/same-timestamp changes missed by metadata-only reconciliation and the full-hash cost on a 1,000-file fixture.

# Child DOX Index

- No child DOX files currently exist.
