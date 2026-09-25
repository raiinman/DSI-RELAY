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
