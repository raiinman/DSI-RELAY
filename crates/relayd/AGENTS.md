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

# Verification

- Live daemon/client round trip through `tests/phase2_first_slice.rs`.
- wrong-token and incompatible-protocol/version rejection.
- graceful and hard-kill restart recovery with durable project/result/job verification.
- damaged operational storage starts Degraded without replacement.
- current-user pipe security verification.
- Synthetic Phase 3 two-project import, baseline, reconciliation, privacy, resource, graceful-restart, and hard-restart coverage through `tests/phase3_first_slice.rs`.
- Synthetic Phase 3 change-delta, dependency-edge, provisional hint-update, stale-state, isolation, and hard-restart coverage through `tests/phase3_second_slice.rs`.

# Child DOX Index

- No child DOX files currently exist.
