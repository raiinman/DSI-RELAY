# Purpose

Own the production per-user RELAY daemon and local transport boundary.

# Ownership

- Governs `crates/relayd/`.
- Depends on relay-core; core must never depend on this crate.

# Local Contracts

- Normal signed-in-user process; no Session 0 service default.
- Windows named pipe is current-user protected and still requires a per-start application token.
- Protocol/version negotiation fails closed.
- Host state distinguishes process health from Core recovery health.
- Daemon records transport-safe diagnostics without persisting command arguments/results.
- Restart must preserve Core operational state.
- The daemon's recursive OS watcher feeds bounded file-path hints through Core and marks continuity loss stale. Startup marks persisted baselines stale before readiness; notification events never establish complete truth. Background hint hashing and recovery defer during commands with effects; read-only capability observations do not interrupt an active background scan.
- After a quiet period, the watcher attempts one stale-project recovery while the signed-in Windows session is idle. Recovery checks user input, active RELAY commands, and the callback event epoch during enumeration, hashing, and before commit; a callback during a scan must defer that scan's commit. Recovery stops after a bounded attempt and retries after backoff. A failed watch subscription cannot restore ready state automatically.
- The daemon loads explicit digest-pinned per-project parser grants from local state at startup. Only the exact configured installed parser with a source-delivery grant receives bounded UTF-8 indexed source bytes through the sandbox. Parser dispatch waits for idle, ready project state and writes edges through Core's generation, source, and configuration guards.

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
- Live watcher-load tests in `tests/phase3_watcher_load.rs` cover burst uncertainty, automatic verified recovery, and separate-root isolation. Its ignored manual fixtures cover a prolonged event soak and retention of the verification requirement when a callback arrives during recovery.
- Repeatable five-sample scale measurements use `tests/phase3_scale_report.ps1`; record host facts and distributions without treating a single host as minimum/recommended tier certification.
- Installed parser dispatch, Alpha/Bravo grant isolation, source reparse, target delete/restore, configuration revocation, and bad-digest rejection are covered by `tests/phase3_parser_dispatch.rs`.

# Child DOX Index

- No child DOX files currently exist.
