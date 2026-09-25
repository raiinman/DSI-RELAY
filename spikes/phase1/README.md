# Phase 1 Technical Spikes

This subtree contains disposable-but-reproducible prototypes used to select RELAY's implementation stack. It is not the production product.

## Current evidence

- Spike 1 — per-user host, CLI, structured command round trip, handshake, status/doctor, restart behavior
- Spike 2 — SQLite project registry, durable result IDs, job checkpoints, schema/integrity handling
- Spike 3 — evidence deduplication, compression, storage-layout, aggregation/downsampling, and recompression economics
- Spike 4 — Windows full-scan/reconciliation, recursive notifications, changed-only parsing, and USN privilege/continuity behavior

The current Node implementation is a candidate, not a locked runtime. See D-146 through D-149 in `docs/DECISION_LOG.md`.

## Run

From `spikes/phase1/node/`:

```powershell
node --test
node bench\spike1.mjs
node bench\spike2.mjs
node bench\spike3.mjs
node bench\spike4.mjs
```

Start the prototype host:

```powershell
node src\daemon.mjs
bin\relay.cmd status --json
bin\relay.cmd doctor --json
```

Machine execution uses one JSON request on stdin:

```powershell
'{"command":"system.echo","arguments":{"value":42}}' | bin\relay.cmd exec --stdin --json
```

Runtime state defaults to the signed-in user's local application-data area and is not stored in this repository.

## Known open gates

- Node idle memory is too high to lock the final daemon runtime without comparison.
- Named-pipe authentication exists, but explicit Windows DACL/cross-user denial remains unproven.
- SQLite is only provisional for operational metadata/compact results; heavyweight evidence stays out of it by default.
- Evidence compression defaults need real UEFN traces and foreground-interference testing.
- Image-codec selection remains open; lossy reference derivatives are never exact evidence.
- Recursive notifications are hints only; real UEFN fixtures and deliberate watcher-buffer overflow tests remain before public claims.
- USN record reading is not available to the normal host on the current least-privilege fixture; an optional privileged helper needs a separate cost/security justification before implementation.
