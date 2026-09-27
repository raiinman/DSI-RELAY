# Phase 2 First Slice Evidence

Status: PASS — first production-shaped Core vertical accepted on 2026-09-25.

## Scope

This slice promotes Phase 1 contracts into production Rust crates only:

- `relay-contracts` — versioned envelopes and shared registry validation
- `relay-core` — deterministic service logic, SQLite state, idempotency, provenance/trust, diagnostics
- `relayd` — per-user Windows host and protected named-pipe transport
- `relay` — canonical human/machine CLI client

No UEFN, Fortnite, editor-specific, dashboard-only, MCP-only, or AI-provider business logic is included.

## Acceptance result

1. CLI and other local clients converge on one daemon/Core command path: PASS.
2. Registry validation runs before business logic: PASS.
3. Result/error envelopes carry explicit schema and command versions: PASS.
4. Project/result/job state survives graceful and hard-kill restarts: PASS.
5. Durable idempotency metadata has a schema-2 SQLite boundary: PASS.
6. Core remains project/tool agnostic: PASS.
7. Malformed state/input, damaged storage, restart, and incompatible versions are tested: PASS.
8. Runtime footprint remains within the Phase 1 Rust baseline order of magnitude: PASS.


## Verification

`cargo test --workspace` passed with 32 non-doc tests:

- 9 contract/registry tests
- 17 Core storage/service/diagnostic tests
- 4 daemon security/state tests
- 2 live daemon integration tests

The live integration suite starts the real daemon, drives it through the production CLI client transport, verifies schema-2 state, then exercises graceful restart, hard-kill restart, idempotency replay, damaged-store Degraded behavior, and incompatible command versions.

A release workspace build also passed. One clean-ish fixture build completed in 27.62 seconds; this is a workstation measurement, not a public build-time budget.

## Live vertical behavior

The live fixture proved:

- `system.status` and `system.doctor` report Healthy with storage, diagnostics, and IPC independently visible
- one project registration persists
- one durable result ID persists
- one durable job/checkpoint persists
- replaying the same `result.put` idempotency key returns the same result ID with `replayed=true`
- durable result/job records preserve provenance and trust metadata
- graceful shutdown clears transient host state and restart restores durable state
- forced daemon termination leaves durable SQLite state recoverable on the next host start
- command version 99 fails `COMMAND_VERSION_INCOMPATIBLE`
- malformed machine input exits non-interactively with structured error output
- deliberately malformed SQLite starts the daemon Degraded, blocks storage writes, preserves the damaged bytes, and leaves diagnostics/transport reachable


## Resource check

After real project/result/job work and a hard-kill restart, the production-shaped Phase 2 daemon sampled:

- 8,871,936 bytes RSS
- 1,204,224 bytes private memory
- 3 threads
- 103 handles
- 0 ms CPU over a five-second idle sample

The selected Phase 1 Rust + SQLite Spike 8 reference sampled 9,367,552 bytes RSS, 2,838,528 bytes private memory, 4 threads, 107 handles, and 0 ms CPU over five seconds after restart.

The Phase 2 slice is therefore still in the same low-footprint class and did not erase the selected runtime advantage.

## Migration/compatibility

Production operational storage is schema 2. Focused tests prove a Phase 1 schema-1 fixture migrates without data loss. Future-schema and malformed stores fail closed without replacement.

The machine command envelope remains schema 1 and command contracts remain version 1. This separates storage evolution from public command-envelope evolution.

## Next promotion target

With the first vertical accepted, Phase 2 can advance into the next Core foundations:

- permission/effect enforcement
- transaction boundary
- usage/cost metrics
- data classification and egress-policy primitives
- credential-handle boundary
- actor/delegator attribution

Adapter/sandbox promotion follows after these Core contracts are coherent.
