# Phase 1 Start Here

## Status

Phase 1 — Technical spike and stack selection — is active.

Phase 0 is closed as the approved architecture/research baseline. Phase 1 must test the assumptions rather than quietly replacing them.

Current prototype progress:

- Spike 1 complete: Node proved the first per-user host + CLI + structured Windows round trip; later runtime selection work supersedes Node as the intended shipping host.
- Spike 2 complete: Node first proved the project/result/checkpoint SQLite durability contract that later runtimes must preserve.
- Spike 3 complete: exact whole-blob dedupe, Zstandard cost curves, SQLite-BLOB versus metadata+file layout, log aggregation, telemetry downsampling, idle recompression, and lossless/reference image handling have benchmark evidence.
- Spike 4 complete: Windows recursive notifications are proven as fast hints but not authoritative under bursts; reconciliation + changed-only parsing is the least-privilege baseline, and USN journal reading is deferred to an optional privileged-helper experiment because non-elevated reads were denied.
- Spike 5 complete: real UEFN message-pump coexistence confirms foreground-safe deferral as the primary protection; soft Windows QoS remains available for work that must continue, hard CPU caps are rejected as a routine default, and 100 registered inactive projects remained effectively idle.
- Spike 6 complete: dashboard command-contract parity is proven; a standalone proxy costs another resident runtime, so the selected local shape is a dependency-free static/HTTP shell hosted inside `relayd`.
- Spike 7 complete: Rust materially beat the Node reference on host/dashboard RSS, startup, CLI launch, restart, and explicit Windows pipe security while remaining protocol-interoperable.
- Spike 8 complete: Rust reproduced the schema-1 SQLite durability/recovery contract, opened Node databases and produced databases Node could read, and retained a 92.23% post-storage idle-RSS reduction. D-153 selects Rust + bundled SQLite as the Phase 1 local core foundation.
- Spike 9 complete: one JSON command registry using a bounded JSON Schema 2020-12 profile now drives deterministic Rust validation plus CLI, dashboard, adapter, AI, and capability discovery metadata without another runtime dependency. D-154 selects it as the Phase 1 semantic command-contract source.
- Spike 10 complete: D-155 selects an on-demand out-of-process adapter broker/manifest foundation with command-registry binding, artifact/provenance checks, timeout/backoff/quarantine, and query-verified Windows Job Object lifecycle/resource containment. Job Objects are explicitly not treated as the security sandbox.
- Current next spike: Spike 11 — Windows adapter sandbox + egress/capability enforcement. Prove an OS-enforced boundary for synthetic untrusted workers before open third-party adapters can be considered strongly isolated.
- Benchmark artifacts live under `spikes/phase1/results/`; current Phase 1 stack consequences are recorded through D-155.

## Phase 1 goal

Choose the minimum durable implementation stack through prototypes and benchmarks.

Do not build the full product yet.

Phase 1 exists to answer:

- what process/runtime model should RELAY use on Windows?
- what storage/database/index design survives recovery and scale tests?
- what local IPC and schema/IDL should become the core machine contract?
- what dashboard stack preserves one command system?
- how should adapters be isolated and versioned?
- how should indexing use watchers/USN plus reconciliation?
- how should evidence storage use compression, deduplication, tiering, and retention?
- what resource controls keep RELAY invisible beside UEFN/Fortnite?
- what compatibility/security/privacy constraints materially affect the stack?

## Read first

1. `AGENTS.md`
2. `docs/AGENTS.md`
3. `docs/PROJECT_PLAN.md`
4. `docs/ROADMAP.md`
5. `docs/SYSTEM_ARCHITECTURE.md`
6. `docs/PERFORMANCE_AND_RESOURCE_ECONOMICS.md`
7. `docs/RESILIENCE_AND_RECOVERY.md`
8. `docs/VERSIONING_AND_COMPATIBILITY.md`
9. `docs/SECURITY_AND_PERMISSIONS.md`
10. `docs/DATA_BOUNDARY_AND_PRIVACY.md`
11. `docs/PHASE0_ADVERSARIAL_REVIEW.md`
12. `docs/RESEARCH_PLAN.md`

Use the specialized documents when a spike touches their domain.

## Phase 1 rule

Prototype first. Lock architecture second.

Any durable stack choice needs:

- a working prototype
- failure/recovery behavior
- performance/resource measurements
- compatibility/versioning implications
- security/privacy implications
- complexity/operability impact
- rationale recorded in `DECISION_LOG.md`

## Recommended first work order

### Spike 1 — Local host + CLI + command round trip

Prove:

- per-user RELAY host starts/stops/restarts cleanly
- CLI can discover/connect to it
- structured command request/response works
- machine mode is non-interactive
- version/capability handshake works
- basic diagnostics work

Deliver a minimal:

```
relayd
relay status
relay doctor
relay exec
```

No UEFN feature work yet beyond what is needed to validate process/integration assumptions.

### Spike 2 — Durable storage + result round trip

Prove:

- project registry
- durable result ID
- job/checkpoint record
- restart persistence
- integrity/recovery path
- schema versioning/migration skeleton

Do not treat derived indexes as authoritative project truth.

### Spike 3 — Evidence Storage Lifecycle

Benchmark a storage pipeline built around:

```
classify
 -> content hash
 -> project/workspace dedupe
 -> compress
 -> store metadata + blob
 -> retention/tiering
 -> prune/recompress/downsample
```

Compare at minimum:

- Zstandard compression levels
- whole-blob content-hash deduplication
- database BLOB versus file/blob-store layout
- lossless evidence-image storage versus reference-grade archival formats
- telemetry chunking/downsampling
- log/event aggregation
- idle-time recompression
- compression CPU cost versus storage saved
- decompression/read latency
- maintenance disk headroom

Later research may evaluate embedding quantization and content-defined chunking. Do not make either a v0.1 dependency before benchmarks justify the complexity.

### Spike 4 — Windows indexing

Compare:

- baseline full scan
- file-system notifications
- NTFS USN journal-assisted updates
- reconciliation after continuity loss
- changed-only parsing

Measure CPU, RAM, disk I/O, update latency, and missed-change recovery.

### Spike 5 — Resource coexistence

Benchmark RELAY while UEFN/Fortnite is active.

Evaluate:

- EcoQoS
- process/thread priority
- Job Objects
- foreground-safe backoff
- local AI defer/pause behavior
- inactive-project idle cost

### Spike 6 — Dashboard shell

Only after the command/storage contracts are usable.

Dashboard must invoke the same underlying command system as CLI.

### Spike 7 — Runtime + IPC challenger

The provisional Node host remains too memory-heavy to become final by default.

Compare a lower-footprint Windows candidate, beginning with Rust because the target fixture already has a current Rust toolchain, against the same narrow contracts:

- per-user process startup/restart
- structured request/response
- version/capability negotiation
- explicit Windows local-IPC access control
- `status` / `doctor`
- embedded static/HTTP dashboard feasibility
- cold/warm startup
- idle CPU/RAM
- command p50/p95/p99 latency
- binary/package size and build/dependency complexity
- crash/recovery and mixed-version implications

Do not port all storage/index/evidence work merely to create a language bake-off. The challenger must first beat or materially improve the host/IPC resource-security trade-off before deeper porting is justified.

### Spike 8 — Rust operational-state parity + dependency economics

Rust earned deeper parity work in Spike 7. Challenge the operational-state layer next, not the entire product.

Prove the Rust candidate can preserve the Spike 2 contracts:

- project registry
- durable result IDs
- job/checkpoint records
- schema migration metadata
- SQLite integrity / `quick_check`
- degraded startup on deliberately damaged storage
- blocked writes while storage is unavailable
- graceful and hard-kill restart persistence
- compatibility/migration implications for the existing prototype schema

Measure:

- SQLite dependency/package strategy and license/build implications
- release binary-size growth
- clean/incremental build cost
- host idle RSS after storage is loaded
- durable write/read/checkpoint latency
- startup/recovery cost
- database/WAL growth
- crash behavior and integrity checks

Do not port indexing, evidence compression, dashboard features, or UEFN adapters in Spike 8 unless a tiny change is strictly required to exercise the storage contract.

### Spike 9 — Schema/IDL + command-registry source of truth

With runtime, IPC, dashboard transport, and operational storage selected, prove one machine-readable source of truth can describe RELAY commands without duplicating contracts across Rust, CLI help, dashboard metadata, adapters, and AI skills.

At minimum prove:

- stable command/capability identifiers and per-command contract versions
- argument/result/error schemas for the current shared command subset
- one generated or validated command registry consumed by Rust
- CLI/dashboard/adapter discovery metadata derived from the same source
- additive/optional-field compatibility and explicit rejection of incompatible versions
- reserved/deprecated identifiers are not silently reused
- compact capability/schema discovery suitable for AI clients without loading the whole catalog
- deterministic validation does not require an AI call

Compare schema/IDL options on code-generation/validation complexity, binary/runtime cost, mixed-version behavior, human debuggability, and context/token overhead. Do not begin adapter product work until this contract boundary is proven.

### Spike 10 — Adapter worker isolation + manifest/broker foundation

With the command-contract boundary proven, test the generic adapter execution boundary before any real UEFN adapter product work.

Prove with a synthetic adapter worker:

- third-party adapter code runs out of process rather than inside RELAY Core
- a versioned manifest declares adapter identity, protocol version, capabilities, required permissions, target/tool requirements, and provenance/trust metadata
- the broker validates manifest and command/capability compatibility before launch
- adapter stdout/stderr and structured messages remain untrusted data
- worker crash/hang/invalid-message behavior cannot crash RELAY Core
- timeout, resource-limit, restart/backoff, and quarantine behavior are explicit
- adapter commands still resolve through the shared command registry rather than creating a second business-logic surface
- inactive adapters add near-zero routine CPU and bounded resident memory
- incompatible or over-permissioned manifests fail closed

Do not implement UEFN features in Spike 10. Use a deterministic synthetic worker so the isolation/broker decision is measured independently of editor behavior.

### Spike 11 — Windows adapter sandbox + egress/capability enforcement

Spike 10 proves fault/lifecycle/resource containment, not a security sandbox. Before treating arbitrary third-party workers as strongly isolated, compare Windows-native containment approaches with a synthetic adversarial worker.

At minimum prove:

- filesystem access outside explicitly brokered/allowed fixture roots is denied by the selected OS boundary
- network access is denied unless the granted adapter policy explicitly permits it
- inherited environment/credential material is minimized and verified
- worker process creation/escape behavior remains bounded
- the worker can still perform the narrow local IPC/data flow required for a useful adapter
- RELAY Core remains outside the restricted worker boundary
- sandbox failure/incompatibility causes fail-closed/quarantine behavior rather than silently running unrestricted
- startup/RAM/latency and developer-tool compatibility cost are measured

Evaluate Windows mechanisms rather than assuming Job Objects alone provide security isolation. Keep this synthetic; do not implement UEFN product behavior merely to test the sandbox.

## Evidence storage lifecycle target

The storage goal is not merely "compress everything."

Order of operations:

1. avoid generating unnecessary evidence
2. deduplicate exact/repeated content
3. normalize/collapse repeated events
4. compress losslessly when exact evidence matters
5. use lower-cost reference formats only when policy permits
6. downsample telemetry over time
7. tier hot/warm/cold evidence
8. expire according to retention
9. protect pinned/critical evidence
10. verify integrity after decompression/restore

Compression happens before encryption when encryption-at-rest is later introduced.

Cross-project/global deduplication is not assumed safe; default scope is project/workspace unless research proves a safe design.

## Phase 1 completion gate

Phase 1 closes only when the team can make evidence-backed stack decisions for:

- implementation language/runtime
- local host/process model
- local IPC
- persistent storage/database
- evidence/blob storage and compression
- indexing/watch/reconciliation
- schema/IDL
- dashboard framework
- packaging/update approach
- adapter worker isolation foundation
- resource scheduling foundation
- logging/diagnostic foundation

And Phase 1 must deliver:

- hello-world `relayd` + `relay` CLI
- structured command round trip
- persistent result round trip
- basic automated tests
- cold/warm startup baseline
- idle CPU/RAM baseline
- architecture decisions recorded in `DECISION_LOG.md`

## New-chat handoff

For a fresh ChatGPT/Codex conversation, use this request:

> Take over DSI RELAY Phase 1. Work in `raiinman/DSI-RELAY`. Start with `AGENTS.md`, `docs/AGENTS.md`, and `docs/PHASE1_START_HERE.md`, then follow the read order there. Phase 0 remains the architecture baseline. Spikes 1–10 are complete; D-153 selects Rust + bundled SQLite as the local core, D-154 selects the JSON command registry contract source, and D-155 selects the out-of-process manifest/broker + Job Object containment foundation while explicitly leaving stronger worker sandboxing open. Begin with Spike 11: Windows adapter sandbox + egress/capability enforcement. Keep it synthetic, narrow, and benchmark-driven; do not jump ahead into UEFN product features.
