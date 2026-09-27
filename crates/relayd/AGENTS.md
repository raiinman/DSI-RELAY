# Purpose

Own the production per-user RELAY daemon and local transport boundary.

# Ownership

- Governs `crates/relayd/`.
- Depends on relay-core; core must never depend on this crate.

# Local Contracts

- Normal signed-in-user process; no Session 0 service default.
- Windows named pipe is current-user protected and still requires a per-start application token.
- `host.json` contains that token. Create it with an explicit protected current-user ACL, verify the kernel owner and DACL before writing token bytes, and publish it from a same-directory temporary file.
- Protocol/version negotiation fails closed.
- Host state distinguishes process health from Core recovery health.
- Daemon records transport-safe diagnostics without persisting command arguments/results.
- Restart must preserve Core operational state.
- The daemon's recursive OS watcher feeds bounded file-path hints through Core and marks continuity loss stale. Startup marks persisted baselines stale before readiness; notification events never establish complete truth. Background hint hashing and recovery defer during commands with effects; read-only capability observations do not interrupt an active background scan.
- After a quiet period, the watcher attempts one stale-project recovery while the signed-in Windows session is idle. Recovery checks user input, active RELAY commands, and the callback event epoch during enumeration, hashing, and before commit; a callback during a scan must defer that scan's commit. Metadata-only recovery retains a 15-second attempt limit. Required full-content verification may continue past that limit while idle and quiet so a slow project is not permanently stranded; interrupted attempts retry after backoff. A failed watch subscription cannot restore ready state automatically.
- The daemon loads explicit digest-pinned per-project parser grants from local state at startup. Only the exact configured installed parser with a source-delivery grant receives bounded UTF-8 indexed source bytes through the sandbox. Parser dispatch waits for idle, ready project state and writes edges through Core's generation, source, and configuration guards.
- Parser installation, quarantine, and recovery health is aggregated into live `system.status`/`system.doctor` output and fixed-code transition diagnostics without source text, paths, or project identifiers. Invalid grants fail closed for source delivery while daemon health remains inspectable. `host.json` recovery state is a startup snapshot; clients needing current health use the live commands.
- The read-only dashboard binds to loopback with a per-start token, invokes only selected observe commands through Core, and stores its local URL in a current-user-DACL-protected `dashboard.json` while the daemon runs. CLI prints that URL through `relay dashboard-url`.
- The portable synthetic benchmark creates a new report path without overwriting an existing file, replaces later progress records atomically, bounds each CLI child process and kills it on deadline, records a fixed timeout code, and shuts down its daemon before removing only a validated temporary root without descendant reparse points.

# Verification

- Live daemon/client round trip through `tests/phase2_first_slice.rs`.
- wrong-token and incompatible-protocol/version rejection.
- graceful and hard-kill restart recovery with durable project/result/job verification.
- damaged operational storage starts Degraded without replacement.
- current-user pipe security verification.
- Synthetic Phase 3 two-project import, baseline, reconciliation, privacy, resource, graceful-restart, and hard-restart coverage through `tests/phase3_first_slice.rs`.
- Synthetic Phase 3 change-delta, dependency-edge, provisional hint-update, stale-state, isolation, and hard-restart coverage through `tests/phase3_second_slice.rs`.
- Explicit full-content verification after a metadata-invisible edit through `tests/phase3_second_slice.rs`.
- Live OS notifications, shared-root handling, distinct-root isolation, directory uncertainty, idle resource sample, and hard-restart continuity through `tests/phase3_watcher.rs`.
- Ignored, manually invoked 15,000-file two-project scale fixture in `tests/phase3_watcher.rs`; record host class and avoid treating one run as a tier budget.
- The scale fixture measures watcher-attachment full verification separately from a subsequent clean metadata pass, so these costs are not conflated after schema 6.
- Live idle recovery after hard restart detects a same-size/same-timestamp edit without a manual reconciliation command; debug-only accelerated idle timing is limited to the test fixture.
- Live project-configuration transport test verifies versioned metadata and conflict behavior after a hard restart; parallel fixtures use distinct daemon instance names.
- Live watcher-load tests in `tests/phase3_watcher_load.rs` cover burst uncertainty, automatic verified recovery, separate-root isolation, and a quiet four-root idle sample. Its ignored manual fixtures cover a prolonged event soak, retention of the verification requirement when a callback arrives during recovery, and foreground-write interruption followed by idle retry.
- The watcher unit test verifies required full-content recovery remains eligible after the metadata deadline and still defers for a callback or lost idle state.
- Repeatable five-sample scale measurements use `tests/phase3_scale_report.ps1`; record host facts and distributions without treating a single host as minimum/recommended tier certification.
- Installed parser dispatch, Alpha/Bravo grant isolation, source reparse, target delete/restore, configuration revocation, and bad-digest rejection are covered by `tests/phase3_parser_dispatch.rs`.
- The same parser-dispatch suite verifies an offline staged package and explicit grant activate after daemon startup.
- `tests/phase3_parser_dispatch.rs` also verifies missing/malformed installation health, broker quarantine, fixed-code diagnostics, and repair/restart or source-change recovery.
- The ignored live installed-parser resource fixture in `tests/phase3_parser_resource.rs` measures initial publication, changed-source reparse, foreground latency, idle CPU, and resident memory on one host; `tests/phase3_parser_resource_report.ps1` records repeatable samples and source fingerprints without claiming a supported hardware tier.
- The draft portable runner in `tests/phase3_portable_benchmark.ps1` exercises a staged release daemon on another host; its smoke results are functional evidence until minimum/recommended hardware and creator-app contention are measured.
- The portable runner smoke should verify a successful staged-binary run, an existing report path left byte-identical without starting a daemon, and an induced CLI timeout that leaves a fixed-code failure report with no new daemon or fixture root.
- The portable runner reports fixed failure codes for daemon launch, early exit, readiness timeout, and CLI timeout without copying raw process output into its report.
- `src/dashboard.rs` unit tests and `tests/phase5_dashboard.rs` verify live HTTP token/origin/write rejection and parity with the named-pipe command path.

# Child DOX Index

- `dashboard/AGENTS.md` — owns static local dashboard presentation, plain-language rendering, and browser token handling.
