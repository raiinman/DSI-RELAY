# Phase 1 Start Here

## Status

Phase 1 — Technical spike and stack selection — is active.

Phase 0 is closed as the approved architecture/research baseline. Phase 1 must test the assumptions rather than quietly replacing them.

Current prototype progress:

- Spike 1 complete: per-user host + CLI + structured command round trip is proven on Windows. Node remains provisional because its idle RSS is material and explicit named-pipe DACL/cross-user denial is still unproven.
- Spike 2 complete: project registry, durable result IDs, job checkpoints, schema migration metadata, integrity checks, hard-kill persistence, and degraded corrupted-store startup are proven with the provisional SQLite candidate.
- Spike 3 complete: exact whole-blob dedupe, Zstandard cost curves, SQLite-BLOB versus metadata+file layout, log aggregation, telemetry downsampling, idle recompression, and lossless/reference image handling have benchmark evidence.
- Spike 4 complete: Windows recursive notifications are proven as fast hints but not authoritative under bursts; reconciliation + changed-only parsing is the least-privilege baseline, and USN journal reading is deferred to an optional privileged-helper experiment because non-elevated reads were denied.
- Spike 5 complete: real UEFN message-pump coexistence confirms foreground-safe deferral as the primary protection; soft Windows QoS remains available for work that must continue, hard CPU caps are rejected as a routine default, and 100 registered inactive projects remained effectively idle.
- Spike 6 complete: dashboard command-contract parity is proven; a standalone proxy costs another resident runtime, so the provisional default is a dependency-free static/HTTP shell hosted inside `relayd`.
- Current next spike: Spike 7 — Runtime + IPC challenger. Compare the provisional Node host against a lower-footprint Windows candidate before locking implementation language/runtime or IPC.
- Benchmark artifacts live under `spikes/phase1/results/`; provisional architecture consequences are recorded in D-146 through D-151.

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

> Take over DSI RELAY Phase 1. Work in `raiinman/DSI-RELAY`. Start with `AGENTS.md`, `docs/AGENTS.md`, and `docs/PHASE1_START_HERE.md`, then follow the read order there. Phase 0 is closed and must remain the architecture baseline unless a Phase 1 prototype provides evidence that a decision needs revision. Begin with Spike 1: local host + CLI + structured command round trip. Keep the implementation minimal and benchmark-driven; do not jump ahead into the full UEFN product.
