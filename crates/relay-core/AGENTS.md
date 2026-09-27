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
- Result payload reads verify SHA-256 against the original stored JSON bytes before decoding; a changed payload with a stale digest fails closed, while historical JSON ordering remains valid.
- Result descriptions query stored metadata and payload byte size without loading or serializing the payload; the same project-scope rule as full result retrieval applies.
- Bounded result listing reads descriptions only; scoped clients must name an authorized project.
- Bounded job listing reads descriptions only, never checkpoint/provenance, with the same explicit project-scope rule.
- Compact result context preserves exact eligible scalar facts under an explicit serialized byte limit, excludes path/secret-keyed and unsafe-key content, reports omissions, and retains the full result ID. Bounded literal focus terms rank already eligible facts; repeated equivalent array observations are deduplicated. Bounded required JSON Pointers take precedence and fail if unavailable or over budget. Source timestamp, schema/producer versions, trust, and payload digest remain visible for freshness/provenance assessment without exposing stored provenance actor details. It is presentation filtering, not data-egress authorization.
- Bounded task focus terms may reprioritize safe context facts but cannot change eligibility, required-fact guarantees, or project scope. Diagnostic summaries expose fixed health, aggregate counts, and at most 12 retained local start/failure/recovery event codes; only registry-known commands and failure codes or fixed host-parser codes may enter recent summaries.
- Multi-result context compilation reuses the exact scalar allowlist and source digests, checks every result against the requested project, reports timestamp-relative age without asserting authority, and compares conflicts only within the same result kind and pointer. It fails rather than dropping conflicts or required facts. Its source set and input bytes are bounded.
- `context.task.compile@1` reads project registration revision, index continuity and bounded last-reconciled timestamp, declared adapter configuration, selected digest-checked results, and at most four explicitly selected removal-approval records in one SQLite read snapshot. It labels absent/stale index and result currentness uncertainty, keeps selected decisions as local-confirmation evidence with human presence unverified, and never includes project roots or raw actor/client IDs. Fixed task kind adds deterministic ranking terms; caller focus terms add to that ranking. Neither selects sources, expands fact eligibility, or grants authority. Existing exact-fact, conflict, and byte-budget guarantees still apply. General durable decision memory and source-to-index generation links remain open.
- Successful `context.compile` views may be reused from an eight-entry in-memory FIFO cache only after each call rechecks project authority, reloads every selected result, and verifies stored JSON payload digests. Retained compiled output is capped at eight times the registry's 32 KiB output budget. The key includes project, ordered source IDs/digests and output metadata, required pointers, focus terms, and byte budget. Cache counters report actual hits, misses, and skipped per-source fact collections; runtime and token savings remain unmeasured.
- Trusted host extensions execute only after shared command validation and Core authority checks; Core validates extension result schemas. The generic ready-index snapshot returns project-relative names only and requires ready continuity. Core contains no UEFN-specific parser.
- The project-independent `tools.local.discover` command also routes through the trusted host extension after read authority checks; Core contains no installation scanning logic.
- Local-user authority grants the distinct `project_write` effect/permission for explicit project-file actions such as Krita export; these actions use the existing transaction record path and remain project scoped.
- Trusted project-root lookup serves authorized local adapters only; public results and diagnostics do not include that path.
- Generic registered-project checks let host adapters bind caller-supplied capture analysis to an authorized project without adding Verse logic to Core.
- Phase 3 project baselines and change rows are project-scoped, derived state. Canonical roots stay local; machine-facing index paths are project-relative. Reconciliation treats watcher paths as hints and hashes changed candidates after authoritative metadata enumeration.
- Schema-5 delta reads and dependency edges are bounded, project-scoped derived state. Edge replacement requires the current index generation and source content hash; source/target changes invalidate edges, and rebuild resets the delta boundary.
- Schema-9 stores revisioned project check catalogs, guarded parser coverage even for zero-edge observations, and edges invalidated at a specific index generation. Reconciliation records invalidated edges before deletion and clears affected coverage. Configuration changes, baseline rebuilds, and continuity loss invalidate coverage. Existing schema-8 stores migrate with no assumed coverage.
- Check planning reads catalog, index, files, changes, edges, invalidation history, and coverage in one database snapshot. Catalog format 1 may set per-check `dependency_mode: "direct"`; these checks select from changed declared roots/leaves and once for the catalog's exact current ready-index generation, even when that generation follows the original baseline and has no later delta. The stable plan ID makes repeated execution at that same boundary replay safely. Direct checks do not need parser configuration or coverage. An omitted mode keeps legacy transitive traversal and requires matching guarded parser coverage at each source. A mixed catalog falls back for all checks if any transitive check lacks that basis; other uncertainty also plans the full catalog with an explicit reason. Planned entries always say `planned_not_run` and contain no result ID; planning never executes a check.
- `automation.checks.execute@1` accepts the exact current selective plan ID and evaluates only catalog-declared indexed-file presence/digest assertions against that complete ready index snapshot. Assertion paths must be declared roots/leaves. Changed generation/catalog/configuration or incomplete coverage/inventory fails before execution. One SQLite transaction rechecks the plan boundary, inserts bounded privacy-safe hashed-path result payloads, and writes a deterministic job replay marker; retries cannot duplicate results. Checks without supported assertions remain `untested` with null result IDs. A passing assertion describes indexed state only, not live filesystem/native creator behavior.
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
- Schema-8-to-9 migration leaves historical parser coverage unknown; focused tests cover zero-edge recording, generation-tagged invalidation history, stale/rebuild clearing, revisioned catalog integrity, stable selective plans, and full-catalog fallback.
- Schema 10 stores bounded project-removal approvals and a project registration/lifecycle revision. Removal execution checks that revision and the plan expiry, then commits the tombstone and single-use approval state atomically. Plan output never includes the project root. A crash after this commit but before command idempotency or transaction finalization can yield an uncertain outer response; inspect the durable approval state before retrying.
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
