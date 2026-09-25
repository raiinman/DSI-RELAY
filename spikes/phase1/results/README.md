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
