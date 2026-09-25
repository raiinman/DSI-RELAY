# Phase 1 Benchmark Results

These artifacts are measured spike evidence, not general performance claims.

## Spike 1 — host + CLI

Artifact: `2026-09-24-spike1-node-windows.json`

- startup-to-ready: 91.536 ms p50
- authenticated named-pipe handshake + status: 1.058 ms p50
- full CLI process + status: 141.929 ms p50
- idle host: 118,153,216 bytes RSS; no CPU time observed in the five-second sample
- hard-kill replacement rewrote stale host state in 98.431 ms
- wrong token and incompatible protocol both failed closed

## Spike 2 — SQLite durability

Artifact: `2026-09-24-spike2-node-sqlite-windows.json`

- durable result write: 2.013 ms p50
- durable result read: 1.136 ms p50
- checkpoint update: 1.601 ms p50
- SQLite quick_check: 1.164 ms p50
- results/checkpoints survived a hard process kill
- deliberately corrupted storage produced Degraded state and blocked writes

## Spike 3 — evidence lifecycle

Artifact: `2026-09-24-spike3-evidence-lifecycle-windows.json`

- project-scoped whole-blob dedupe removed 71.7% of referenced raw bytes in the synthetic corpus
- workspace-scoped dedupe removed 80.0%, but crosses the project-isolation boundary
- Zstd level 1 gave strong savings on structured fixtures; incompressible bytes grew slightly
- Zstd level 19 cost seconds per ~9–12 MB structured fixture and is rejected as a default hot-path setting
- SQLite BLOBs were faster, while metadata + files bounded recompression headroom to one blob instead of another full database copy
- level-1 to level-9 idle recompression saved only 0.3573% more bytes on the layout corpus
- log aggregation preserved counts; telemetry downsampling preserved count and global min/max
- image results prove lossless versus lossy roles, but the synthetic image is not representative enough to select a production codec

See D-146 through D-148 in `docs/DECISION_LOG.md` for the current architecture consequences.

## Spike 4 — Windows indexing

Artifact: `2026-09-24-spike4-windows-indexing.json`

- 15,000-file metadata reconciliation scan: 2.420 s
- full content parse: 10.562 s / 16,028,956 logical bytes
- changed-only parse after the live mutation burst: 103.241 ms / 154,660 bytes
- changed-only parsing reduced logical bytes by 99.0354%
- rapid 160-operation burst: 36/170 changed paths surfaced by notifications (21.1765% coverage)
- paced 40-file control at 10 ms spacing: 40/40 paths surfaced, 3 ms p50 detection latency
- recursive watcher sampled 0 CPU ms while idle for 3 seconds on this fixture
- 70 offline changes were recovered by reconciliation; candidate parsing then took 43.399 ms / 61,498 bytes
- USN journal metadata was queryable and file-reference IDs matched Node inode IDs
- USN journal record reading returned Access Denied in the signed-in non-elevated context

Current implication: recursive notifications are fast dirty-path hints, changed-only parsing is strongly favored, reconciliation remains authoritative, and USN may only return as an optional narrowly privileged accelerator if a later benchmark justifies that complexity.

See D-149 in `docs/DECISION_LOG.md`.

## Spike 5 — Resource coexistence

Artifact: `2026-09-24-spike5-resource-coexistence-windows.json`

- real UEFN editor process/window measured; no editor mutation was performed
- baseline UEFN message-pump latency: 5.011 ms median p95 across three runs
- 12-thread Normal background work: 4.992 ms median p95 / 13,257 MiB/s median hashing throughput
- Below-Normal: 4.934 ms median p95 / 11,256 MiB/s median throughput
- Below-Normal + EcoQoS + low memory priority: 4.938 ms median p95 / 12,050 MiB/s median throughput
- weight-based Job Object: 4.783 ms median p95 / 11,111 MiB/s median throughput
- 25% Job hard cap: 4.885 ms median p95 / 6,389 MiB/s median throughput
- full 16-thread saturation still kept Normal work near baseline at 4.932 ms p95 / 14,131 MiB/s
- under saturation, EcoQoS reduced throughput to about 10.1–11.3 GiB/s without a meaningful p95 gain
- under saturation, the 25% hard cap reduced throughput to about 6.1–6.3 GiB/s and worsened UEFN p99 to 12.8–15.1 ms versus about 5.1–5.4 ms for Normal
- one inactive registered project and one hundred inactive projects both sampled 0 ms host CPU over three idle seconds; RSS differed by about 180 KB
- foreground-safe policy deferred optional local-AI and deep-index work while UEFN was detected

Current implication: defer optional heavy work first. Use soft Windows QoS/priority only for work that must continue under foreground activity. Do not use hard CPU caps as a routine foreground-safety default.

See D-150 in `docs/DECISION_LOG.md`.

## Spike 6 — Dashboard shell

Artifact: `2026-09-24-spike6-dashboard-shell-windows.json`

- static shell assets: 9,515 bytes total
- declared runtime dependencies: 0
- host-only idle RSS: 121,917,440 bytes
- standalone host + dashboard proxy idle RSS: 255,938,560 bytes
- standalone proxy overhead versus host-only: +134,021,120 bytes RSS
- embedded dashboard host idle RSS: 132,100,096 bytes
- embedded overhead versus host-only: +10,182,656 bytes RSS
- all shapes sampled 0 ms CPU over the post-cooldown five-second idle window
- direct named-pipe `system.status`: 0.321 ms p50
- standalone dashboard HTTP `system.status`: 1.404 ms p50
- embedded dashboard HTTP `system.status`: 0.457 ms p50
- embedded static root request: 0.325 ms p50
- host-only startup-to-state: 98.304 ms; embedded dashboard startup-to-state: 131.596 ms
- standalone and embedded paths were parity-tested against the same structured command semantics
- degraded storage state, result errors, host-unavailable state, token rejection, blocked side-effect commands, and cross-origin preflight were tested

Current implication: serve the personal dashboard's thin static/HTTP surface from the existing per-user `relayd` process. Do not create another resident dashboard backend solely for UI packaging.

See D-151 in `docs/DECISION_LOG.md`.

## Spike 7 — Runtime + IPC challenger

Artifact: `2026-09-24-spike7-runtime-ipc-challenger-windows.json`

- neutral Node-vs-Rust comparison harness: `spikes/phase1/compare/spike7.mjs`
- Node idle host RSS: 119,095,296 bytes; Rust: 6,529,024 bytes (94.52% lower)
- repeated startup p50: Node 91.340 ms; Rust 29.991 ms
- direct authenticated `system.status` p50: Node 0.284 ms; Rust 0.265 ms
- full CLI-process `status` p50: Node 141.296 ms; Rust 21.476 ms
- hard-kill replacement state: Node 81.895 ms; Rust 27.251 ms; both recovered
- embedded dashboard RSS: Node 127,373,312 bytes; Rust 7,036,928 bytes
- embedded dashboard status p50: Node 0.748 ms; Rust 0.555 ms
- actual Node CLI → Rust host and Rust CLI → Node host interoperability passed
- wrong-token and incompatible-protocol tests failed closed on both candidates
- Rust kernel verification proved a protected current-user-only pipe DACL with one full-control ACE
- Rust release binary: 438,272 bytes; 3 direct crates / 14 resolved Cargo packages
- clean optimized Rust build on cached crates: 12.728 s
- selected source-path cost: Node 556 lines / 0 unsafe mentions; Rust 1,420 lines / 21 unsafe mentions

Current implication: Rust is the preferred candidate for deeper parity, not the final runtime. Node remains the working reference/fallback until Rust proves the durable SQLite/result/checkpoint/migration contracts without erasing its footprint and operability advantage.

See D-152 in `docs/DECISION_LOG.md`.

## Spike 8 — Rust operational-state parity + dependency economics

Artifact: `2026-09-24-spike8-rust-storage-parity-windows.json`

- schema-1 project/result/job durability passed on both Node and Rust after hard kill
- malformed stores and intentionally future schema 999 start `Degraded`, block writes, and are not silently replaced/downgraded
- Node-created databases were opened/read correctly by Rust; Rust-created databases were opened/read correctly by Node
- Node SQLite: 3.53.3; Rust bundled SQLite: 3.53.2
- post-restart idle RSS with SQLite loaded: Node 120,500,224 bytes; Rust 9,367,552 bytes (92.23% lower)
- initial storage-ready startup: Node 87.836 ms; Rust 46.915 ms
- result write p50: Node 1.031 ms; Rust 1.002 ms
- result read p50: Node 0.360 ms; Rust 0.343 ms
- checkpoint p50: Node 0.794 ms; Rust 1.563 ms
- `quick_check` p50: Node 0.382 ms; Rust 0.363 ms
- clean-close DB size matched at 1,462,272 bytes and both cleared WAL/SHM files
- Rust release binary grew from 438,272 bytes (Spike 7) to 2,153,472 bytes with bundled SQLite
- Rust direct dependencies grew from 3 to 5; resolved Cargo packages from 14 to 34
- clean optimized Rust build with cached crates: 32.013 s; incremental no-change release build: 264 ms
- selected dependency metadata: `rusqlite` MIT, `libsqlite3-sys` MIT, `sha2` MIT OR Apache-2.0

Current implication: D-153 selects Rust + bundled SQLite as the Phase 1 local core foundation. Node remains a schema/protocol compatibility reference. Heavyweight evidence remains outside operational SQLite.

See D-153 in `docs/DECISION_LOG.md`.
