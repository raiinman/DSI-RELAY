# Phase 1 Technical Spikes

This subtree contains disposable-but-reproducible prototypes used to select RELAY's implementation stack. It is not the production product.

## Current evidence

- Spike 1 — per-user host, CLI, structured command round trip, handshake, status/doctor, restart behavior
- Spike 2 — SQLite project registry, durable result IDs, job checkpoints, schema/integrity handling
- Spike 3 — evidence deduplication, compression, storage-layout, aggregation/downsampling, and recompression economics
- Spike 4 — Windows full-scan/reconciliation, recursive notifications, changed-only parsing, and USN privilege/continuity behavior
- Spike 5 — live-UEFN resource coexistence, Windows priority/EcoQoS/Job Object controls, foreground-safe deferral, and inactive-project idle cost
- Spike 6 — dashboard shell command parity, local HTTP security boundary, and standalone-vs-embedded process/resource economics
- Spike 7 — neutral Node-vs-Rust runtime/IPC challenger, explicit Windows pipe DACL verification, cross-runtime protocol compatibility, restart/resource/build economics
- Spike 8 — Rust operational SQLite parity, degraded/future-schema recovery, bidirectional Node/Rust database compatibility, and dependency/build economics
- Spike 9 — single JSON command registry, bounded JSON Schema 2020-12 validation, command-version compatibility, and derived CLI/dashboard/adapter/AI discovery metadata
- Spike 10 — synthetic out-of-process adapter workers, versioned manifests, registry-bound capabilities, integrity/provenance checks, timeout/backoff/quarantine, and query-verified Windows Job Object resource containment
- Spike 11 — synthetic adversarial AppContainer/process sandboxing, explicit filesystem grants, default-deny/explicit-grant network egress, environment minimization, child-process denial, and experimental-backend cost
- Spike 12 — stable documented AppContainer/LPAC launch, mailbox-only direct writes, brokered egress, security-descriptor cleanup, measured-build allowlisting, and fail-closed OS-tier selection

D-153 selects Rust + bundled SQLite as the Phase 1 local core foundation. D-154 selects the JSON command registry + bounded JSON Schema 2020-12 profile as the semantic command-contract source. D-155 selects the on-demand out-of-process adapter broker/manifest foundation. D-156 selects the strong adapter-isolation semantics. D-157 selects the stable AppContainer/LPAC + brokered-egress release candidate on qualified Windows builds and makes the experimental backend reference-only. Node remains a compatibility/reference implementation rather than the intended shipping daemon. See D-146 through D-157 in `docs/DECISION_LOG.md`.

## Run

From `spikes/phase1/node/`:

```powershell
node --test
node bench\spike1.mjs
node bench\spike2.mjs
node bench\spike3.mjs
node bench\spike4.mjs
node bench\spike5.mjs
node bench\spike6.mjs
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

Rust challenger:

```powershell
cd ..\rust
cargo test
cargo build --release
.\target\release\relay-rust-challenger.exe host --dashboard
```

Neutral stack comparisons:

```powershell
cd ..\compare
node spike7.mjs
node spike8.mjs
node spike9.mjs
node spike10.mjs
node spike11.mjs
node spike12.mjs
```

## Known open gates

- Rust + bundled SQLite is selected for operational metadata/compact results, but concurrency, WAL pressure, VACUUM/maintenance interruption, disk-full injection, backup/restore, and future migration interruption remain release-hardening gates.
- Heavyweight evidence stays out of operational SQLite by default; evidence compression defaults still need real UEFN traces and foreground-interference testing.
- Image-codec selection remains open; lossy reference derivatives are never exact evidence.
- Recursive notifications are hints only; real UEFN fixtures and deliberate watcher-buffer overflow tests remain before public claims.
- USN record reading is not available to the normal host on the current least-privilege fixture; an optional privileged helper needs a separate cost/security justification before implementation.
- Resource coexistence still needs Fortnite play-session frame-time testing and minimum/recommended hardware fixtures.
- GPU local-model coexistence remains open; Spike 5 proves defer policy and CPU background controls, not local-LLM VRAM scheduling.
- Process-name creator detection is a spike mechanism; production scheduling should consume trusted adapter/project activity state rather than hard-code one application's executable name.
- The dashboard is still read-only in Phase 1; write/approval authorization and client identity remain open.
- D-154 selects the JSON command registry + bounded JSON Schema 2020-12 profile; signed/external extension contract packaging and the eventual public stability policy remain later work.
- D-157 selects the stable AppContainer/LPAC + brokered-egress backend only on qualified Windows builds; build 26200 is the sole physically measured build so far, and real UEFN/Blender/Krita adapter compatibility inside this boundary remains open.
- Node remains a compatibility/reference implementation for schema-1 interoperability testing, not the intended shipping daemon.
- Rust's Win32 boundary is larger and uses explicit `unsafe` code; keep that boundary narrow and regression-tested rather than spreading native calls across RELAY Core.
