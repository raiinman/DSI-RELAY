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
- Result descriptions query stored metadata and payload byte size without loading or serializing the payload; the same project-scope rule as full result retrieval applies.
- Bounded result listing reads descriptions only; scoped clients must name an authorized project.
- Bounded job listing reads descriptions only, never checkpoint/provenance, with the same explicit project-scope rule.
- Compact result context preserves exact eligible scalar facts under an explicit serialized byte limit, excludes path/secret-keyed and unsafe-key content, reports omissions, and retains the full result ID. Bounded literal focus terms rank already eligible facts; repeated equivalent array observations are deduplicated. Bounded required JSON Pointers take precedence and fail if unavailable or over budget. Source timestamp, schema/producer versions, trust, and payload digest remain visible for freshness/provenance assessment without exposing stored provenance actor details. It is presentation filtering, not data-egress authorization.
- Bounded task focus terms may reprioritize safe context facts but cannot change eligibility, required-fact guarantees, or project scope. Diagnostic summaries expose only fixed health and aggregate counts.
- Multi-result context compilation reuses the exact scalar allowlist and source digests, checks every result against the requested project, reports timestamp-relative age without asserting authority, and compares conflicts only within the same result kind and pointer. It fails rather than dropping conflicts or required facts. Its source set and input bytes are bounded.
- Trusted host extensions execute only after shared command validation and Core authority checks; Core validates extension result schemas. The generic ready-index snapshot returns project-relative names only and requires ready continuity. Core contains no UEFN-specific parser.
- Trusted project-root lookup serves authorized local adapters only; public results and diagnostics do not include that path.
- Generic registered-project checks let host adapters bind caller-supplied capture analysis to an authorized project without adding Verse logic to Core.
- Phase 3 project baselines and change rows are project-scoped, derived state. Canonical roots stay local; machine-facing index paths are project-relative. Reconciliation treats watcher paths as hints and hashes changed candidates after authoritative metadata enumeration.
- Schema-5 delta reads and dependency edges are bounded, project-scoped derived state. Edge replacement requires the current index generation and source content hash; source/target changes invalidate edges, and rebuild resets the delta boundary.
- Schema-7 project configuration is separate from derived index state, versioned by format and revision, and limited to a project type plus an optional declared adapter ID/version pair. Core must not treat that declaration as installed capability or parser authority.
- Schema-8 project lifecycle keeps archived and removed rows and their history. Active discovery/watchers exclude inactive rows, restore invalidates the old ready index, and registration cannot reuse inactive IDs.
- Trusted daemon parser reads expose only ready indexed sources and recheck the selected configuration, generation, and source digest before source delivery. Automatic edge replacement supplies an additive configuration-selection guard, verified atomically with the existing index/source/target checks. Configuration updates invalidate derived edges.
- Generic runtime host-component health is forwarded through live status/doctor commands. Core diagnostics accept fixed host component event labels/codes only; parser source paths, content, and worker messages never enter those events.
- Core owns the current-process optional automation pause flag and host-reported watcher availability. Shared pause/resume commands change only that flag; live status and diagnostic summaries expose the mode. Restart resets pause state, and ordinary commands remain available while paused.
- Hint-only index updates read and hash only named files, commit `stale` state, and never claim complete continuity. Delta and dependency-edge reads/writes require ready state.
- Normal reconciliation checks all filesystem metadata and hashes changed candidates. Explicit `verify_content` reconciliation hashes all files for uncertain continuity and detects same-size/same-timestamp edits. After continuity loss, a durable schema-6 flag requires full content verification before `ready` can be restored.
- Trusted daemon watcher input can query local indexed roots, durably mark continuity loss, and run cooperatively cancellable reconciliation. These methods never expose canonical roots through command results.

# Verification

- Unit tests for registry, storage migration, idempotency, diagnostics, malformed input, and damaged storage.
- Integration tests through the public Core service surface.
- Schema-3-to-4 migration preserves Phase 2 authority state; project index tests cover generation guards, isolation, missed hints, and rebuildability.
- Schema-4-to-5 migration preserves index records and marks a conservative baseline generation; delta and edge tests cover stale input, bounds, restart, and cross-project authority.
- Hint-update tests cover targeted work counts, rename/directory hints, missed-event recovery, stale restart, and cross-project denial.
- Content-verification tests cover same-size/same-timestamp changes missed by metadata-only reconciliation and the full-hash cost on a 1,000-file fixture.
- Schema-5-to-6 migration and hard-restart tests preserve the verification requirement; a metadata-only commit cannot clear it. Guarded reconciliation abandons partial plans before a storage commit.
- Schema-6-to-7 migration preserves index continuity state. Configuration tests cover revision conflict, malformed bindings, cross-project scope denial, and restart persistence.
- Live installed-parser fixtures verify that a stale configuration guard cannot restore edges after a version change.
- Result-description tests cover no payload/provenance exposure, UTF-8 byte size, project-scope denial, and restart persistence.
- Result-context tests cover exact allowlisted facts, byte ceilings, deterministic ordering, unsafe path/token omission, full-payload size comparison, and project-scope denial.
- Required-pointer tests cover schema bounds and uniqueness, pointer syntax, exact retention under competing eligible facts, unsafe/missing fact denial, project scope, and all-required budget failure.
- Focused-context tests cover literal term bounds, relevance ordering, duplicate suppression, exact required facts, source metadata, privacy filtering, deterministic output, and byte ceilings.
- The deterministic Context Gauntlet integration fixture stores and re-reads results through Core commands, compares serialized payload/context body bytes, asserts required exact facts, reports retention of exploratory fields without treating current omissions as invariants, and checks repeat output.

# Child DOX Index

- No child DOX files currently exist.
