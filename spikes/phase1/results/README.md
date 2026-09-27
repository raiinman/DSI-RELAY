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

## Spike 9 — Schema/IDL + command-registry source of truth

Artifact: `2026-09-24-spike9-command-registry-windows.json`

- selected source: plain JSON command registry using the JSON Schema 2020-12 dialect with a bounded RELAY validation profile
- current registry: 13 versioned commands plus reserved/deprecated ID sets
- Rust validates registry structure at startup, arguments before business logic, and successful results/error envelopes before return
- CLI catalog/help/describe, dashboard exposure, adapter discovery, AI discovery, and command capability IDs derive from the same registry
- existing clients that omit `command_version` still work and resolve the current command version; explicit unsupported versions fail `COMMAND_VERSION_INCOMPATIBLE`
- registry/validation added 0 direct dependencies and 0 resolved Cargo packages over the Spike 8 core
- release binary grew by 118,272 bytes to 2,271,744 bytes
- `system.status` with validation: 0.275 ms p50
- compact AI `registry.list`: 0.271 ms p50
- `registry.describe project.register`: 0.220 ms p50
- deterministic invalid-request rejection: 0.189 ms p50
- five-second embedded-dashboard idle sample observed 0 CPU ms and 8,523,776 bytes RSS
- full minified registry: 10,408 bytes; compact AI list: 1,971 bytes (18.94%); one project-register contract: 855 bytes (8.21%)
- transparent 4-bytes/token heuristic: ~2,602 tokens full registry, ~493 compact AI list, ~214 one-command description
- isolated full `jsonschema 0.57.0` no-default-feature cost probe: 80 resolved packages, 4,229,120-byte tiny-validator EXE, 72.5 s first optimized build
- `protoc`, TypeSpec `tsp`, and CUE were not installed on the fixture and were not added as Phase 1 toolchain dependencies

Current implication: D-154 selects the JSON registry + bounded JSON Schema 2020-12 profile as the semantic built-in command-contract source. Public catalog stability and extension contract packaging remain later work.

See D-154 in `docs/DECISION_LOG.md`.


## Spike 10 — Adapter worker isolation + manifest/broker foundation

Artifact: `2026-09-24-spike10-adapter-broker-windows.json`

- selected shape: on-demand out-of-process adapter workers behind a trusted broker
- manifest format 1 carries adapter/publisher identity, protocol range, command bindings, requested permissions, target requirements, artifact/component digests, dependencies, update source, provenance, and review status
- worker executable SHA-256 is verified before launch
- adapter command/version bindings must resolve to the trusted D-154 command registry and adapter surface
- worker hello revalidates identity, protocol, PID, and exact capability set
- command arguments, results, and errors are registry-validated; adapter prose/stdout/stderr remain untrusted data
- query-verified Windows Job Object limits: kill-on-close, active-process limit 1, process-memory limit 33,554,432 bytes
- a synthetic 128 MiB worker reservation was blocked
- manifest/digest/policy/command validation: 0.291 ms p50
- full on-demand worker invocation including launch/handshake/validation/teardown: 7.424 ms p50 / 9.103 ms p95
- worker crash: `ADAPTER_WORKER_EXITED` in 6.203 ms; invalid JSON: `ADAPTER_INVALID_MESSAGE`
- forced 100 ms hang: `ADAPTER_TIMEOUT` in 122.097 ms
- crash/backoff/quarantine, bad digest, over-permissioned manifest, incompatible protocol/command, identity/capability mismatch, bad result schema, undeclared error, hostile stderr, and memory-cap behavior passed synthetic tests
- 0 installed adapters: 4,460,544 bytes RSS / 0 sampled CPU ms / 0 adapter workers
- 100 installed inactive adapters: 5,275,648 bytes RSS / 0 sampled CPU ms / 0 adapter workers
- 100 inactive adapter manifests added 815,104 bytes RSS; install/validation took 56.487 ms
- no new direct Cargo dependency or resolved package was added over Spike 9
- Job Objects are lifecycle/resource containment, not a complete malicious-worker filesystem/network/registry/IPC sandbox

Current implication: D-155 selects the generic adapter broker/manifest/lifecycle foundation. Open third-party adapters remain experimental/controlled until Spike 11 proves an OS-enforced Windows capability/egress sandbox.

See D-155 in `docs/DECISION_LOG.md`.


## Spike 11 — Windows adapter sandbox + egress/capability enforcement

Artifact: `2026-09-25-spike11-windows-adapter-sandbox.json`

- measured backend: dynamically loaded Microsoft `Experimental_CreateProcessInSandbox` / SandboxSpec 0.1.0 on the Windows fixture; D-156 does not release-lock this experimental export
- sandboxed worker verified `TokenIsAppContainer=true`
- mailbox read/write succeeded through one explicit read/write path grant
- worker executable directory was readable but a write into that read-only grant failed
- ungranted sibling-directory read and write both failed
- outbound TCP was denied by default with WSAEACCES 10013
- explicit network grant succeeded only through RELAY's capability allowlist + `internetClient` + SandboxSpec egress default-allow
- synthetic parent secret and `USERPROFILE` were absent from the worker environment
- child-process creation failed; outer Job Object still verified active-process limit 1, 32 MiB process-memory limit, and kill-on-close
- unsupported sandbox-spec version failed `SANDBOX_SPEC_INCOMPATIBLE` before launch
- unknown capability name failed `SANDBOX_CAPABILITY_UNSUPPORTED` in RELAY before reaching Windows
- fresh sandbox creation: 30.037 ms p50 / 53.149 ms p95
- full denied-worker mailbox round trip: 63.200 ms p50 / 108.509 ms p95
- Spike 10 unsandboxed synthetic invocation reference: 7.424 ms p50; strong isolation added about 55.776 ms p50 (8.51x)
- explicit network-granted run completed in 98.106 ms while keeping filesystem/process restrictions
- Spike 11 added one direct dependency (`flatbuffers` 25.12.19, Apache-2.0) and three resolved packages; core release EXE stayed 2,271,744 bytes because unused sandbox code is stripped from the current daemon
- clean optimized all-bin build: 36.074 s; synthetic sandbox worker: 256,000 bytes

Current implication: D-156 selects the strong AppContainer/process-sandbox security semantics for untrusted adapters, but the experimental processmodel backend remains provisional. Spike 12 must reproduce the same guarantees through a stable Windows process-container/fallback matrix before public open third-party adapters.

See D-156 in `docs/DECISION_LOG.md`.


## Spike 12 — Stable Windows sandbox backend + OS-tier matrix

Artifact: `2026-09-25-spike12-stable-windows-sandbox-matrix.json`

- selected release candidate on qualified builds: documented AppContainer/LPAC launch through `CreateAppContainerProfile` + `SECURITY_CAPABILITIES` / `STARTUPINFOEX` + `CreateProcessW`
- backend matrix version 1; measured Windows build 26200 selects `stable_lpac_brokered_egress`
- experimental-only and unmeasured stable-API hosts select `disabled`; there is no unrestricted fallback
- untrusted worker gets exactly one direct read/write grant: a broker-owned ephemeral mailbox
- worker/project paths may be read-only; project/user writes are broker-mediated
- direct worker network capabilities are rejected; egress is brokered through an explicit target allowlist
- AppContainer token verified; blocked filesystem read/write, direct TCP, inherited parent secret, `USERPROFILE`, and child-process creation all remained denied
- outer Job Object still query-verifies one process, 32 MiB memory limit, and kill-on-close
- temporary mailbox DACL is restored, temporary Low-Integrity label is removed, and the worker-tree descriptor is restored exactly after exit
- stable prelaunch setup: 19.941 ms p50 / 25.328 ms p95
- stable AppContainer `CreateProcessW` launch: 5.517 ms p50 / 9.187 ms p95
- full stable restored round trip: 63.261 ms p50 / 86.259 ms p95
- experimental Spike 11 reference: 65.297 ms p50 / 84.733 ms p95
- stable path was 0.9688× the experimental p50 on the final rerun
- no new direct dependency, resolved package, or selected core-binary growth versus Spike 11
- clean optimized all-binary build: 40.017 s; stable probe 390,144 bytes; synthetic worker 248,832 bytes
- only build 26200 is physically qualified so far; additional Windows builds require the same adversarial suite before entering the release allowlist

Current implication: D-157 selects stable AppContainer/LPAC + brokered egress for strongly isolated untrusted Windows adapters on qualified builds. The experimental processmodel backend is benchmark/reference-only.

See D-157 in `docs/DECISION_LOG.md`.

## Spike 13 — Packaging/update approach

Artifact: `2026-09-25-spike13-packaging-update-windows.json`

- selected default personal/direct Windows path: signed per-user side-by-side versions with verify → stage → atomic activation → schema-aware rollback
- optional Windows-managed channel: MSIX + App Installer when a trusted signing/Store path is available
- benchmark ran non-elevated in user session 1 with zero RELAY services before/after
- raw synthetic two-component payload: 2,486,784 bytes
- side-by-side v2 ZIP: 1,317,397 bytes + 1,239-byte CMS signature
- side-by-side v2 build/sign/verify: 1,168.660 ms
- v1 install/verify/activate: 412.970 ms; v2 stage: 387.298 ms; v2 activation: 6.510 ms
- compatible rollback to v1: 4.873 ms; uninstall: 4.383 ms
- tampered ZIP failed `UPDATE_SIGNATURE_INVALID` before extraction and v1 remained active
- staging v2 did not change the active version until activation
- simulated newer storage schema blocked v1 rollback with `UPDATE_STORAGE_SCHEMA_INCOMPATIBLE` while v2 stayed active
- active metadata retained channel/source plus core+adapter version, SHA-256, and provenance
- uninstall removed binaries/version state while the external marker + SQLite fixture survived
- custom updater evidence code: 245 lines / 7,297 bytes
- MSIX v2: 1,349,288 bytes; build/sign/verify: 1,263.638 ms
- signed MSIX tamper verification failed closed and unpacked core/adapter hashes matched the release inventory
- generated App Installer metadata defines HTTPS distribution, on-launch/background update checks, and disables automatic downgrade
- non-admin AppX registration of the self-signed fixture was blocked by `0x80073CF0` / certificate trust `0x800B0109`; no elevation was used to paper over that production trust dependency
- no package, test certificate, or RELAY service remained after cleanup

Current implication: D-158 selects the signed side-by-side updater model for the default personal/direct path. MSIX/App Installer remains useful for Store or managed/trusted-signing distribution, but its lifecycle was not claimed as physically proven on this self-signed non-admin fixture.

See D-158 in `docs/DECISION_LOG.md`.


## Spike 14 — Logging/diagnostic foundation

Artifact: `2026-09-25-spike14-diagnostics-foundation-windows.json`

- selected default candidate: bounded structured JSONL
- optional Windows deep-trace hook: ETW; not the default durable support record
- SQLite remains selected for operational state but is not selected as the default raw diagnostic store
- event envelope includes stable ID/time/severity/component, project/job/result references, correlation/causation, source/trust, completeness/sampling, capture mode, and structured attributes
- deterministic sensitive-key/path redaction occurs before persistence
- normal capture ceiling: 4 KiB/event; explicit detail mode ceiling: 32 KiB/event and maximum 15-minute window
- benchmark JSONL retention: 4 x 1 MiB; Phase 1 module defaults remain 4 x 512 KiB
- default JSONL (sync every 16): 3,685.9 events/s, 0.0196 ms append p50, 3.8777 ms durability-boundary p50
- strict JSONL sync-every-event control: 239.4 events/s / 4.1647 ms p50
- bounded JSONL retained 3,735,997 bytes after 10,000 input events and explicitly recorded 5,289 evicted events
- JSONL aggregate read: 19.707 ms
- SQLite with the same 16-event durability window: 24,350.6 events/s / 0.0030 ms insert p50 / 0.4272 ms commit-boundary p50
- SQLite retained 4,247,552 bytes after pruning to 4,096 rows; measured retention maintenance was 108.608 ms across prune/checkpoint/VACUUM/post-VACUUM checkpoint
- idle sample: JSONL 5,197,824 bytes RSS; SQLite 6,242,304; ETW 5,087,232; all sampled 0 CPU ms over five seconds
- ETW without a consumer retained 0 bytes; non-elevated durable `logman` session creation returned Access Denied
- temporary detail mode amplified bytes/event by 4.187x and p50 latency by 1.106x while remaining within the detail ceiling
- partial-tail recovery removed 11 incomplete bytes, preserved the prior complete event, and left zero invalid lines
- live-host tests prove command arguments/results are not logged and diagnostic failure independently degrades `status` / `doctor`
- no new direct dependency or resolved Cargo package; existing `windows-sys` only enabled its ETW feature surface
- clean optimized core/probe build: 37.447 s; core EXE 2,360,832 bytes; diagnostic probe 2,091,008 bytes

Current implication: D-159 selects bounded JSONL for normal local diagnostics/support evidence, with explicit periodic durability and retention completeness metadata. ETW stays optional for deliberate deep tracing; elevated ETW loss/export behavior remains later work.

See D-159 in `docs/DECISION_LOG.md`.
