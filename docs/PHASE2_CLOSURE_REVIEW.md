# Phase 2 Closure Review

Status: PASS — Phase 2 closed on 2026-09-26.

## Scope

Phase 2 promoted the selected Phase 1 contracts into production-shaped Rust crates without adding tool-specific product behavior.

Accepted slices:

- Slice 1: daemon/CLI transport, shared command registry, project/result/job/idempotency storage, provenance/trust, diagnostics health, restart persistence, damaged-store behavior.
- Slice 2: permission/effect enforcement, trusted identity and project scope, transaction/usage/credential/egress state, local-only policy, revocation, migrations, restart persistence.
- Slice 3: generic adapter manifest/broker lifecycle, integrity/provenance revalidation, bounded failure/quarantine behavior, and qualified AppContainer/LPAC + Job Object worker isolation.

Evidence is recorded in `PHASE2_FIRST_SLICE_EVIDENCE.md`, `PHASE2_SECOND_SLICE_EVIDENCE.md`, and `PHASE2_THIRD_SLICE_EVIDENCE.md`.
## Published Phase 2 exit criteria

| Exit criterion | Evidence | Result |
| --- | --- | --- |
| Same operation callable human-friendly and structured machine mode | One daemon/Core command path serves CLI machine execution and human CLI surfaces; registry validation precedes business logic | PASS |
| Durable result ID works across processes | Slice 1 live daemon integration persists and retrieves result/job/project state across graceful and forced restart | PASS |
| No interactive prompt in machine mode | Canonical `relay exec --stdin` and structured CLI paths fail non-interactively with structured errors | PASS |
| Measured job metadata exists | Durable jobs/checkpoints, transaction rows, provenance/trust fields, usage metrics, actor/client/delegator attribution, and restart persistence are measured | PASS |

All published Phase 2 exit criteria pass.

## Production foundation accepted

The production workspace now contains:

- `relay-contracts` — shared command registry and versioned machine envelopes
- `relay-core` — deterministic service logic, authority/policy, persistence, diagnostics, transactions, usage, credential-handle and egress foundations
- `relayd` — per-user Windows daemon and protected local transport
- `relay` — canonical CLI client
- `relay-adapter` — generic manifest/broker lifecycle and qualified Windows sandbox boundary
## Compatibility and recovery evidence

Phase 2 preserves the selected Phase 1 contracts while advancing operational storage to schema 3.

Verified behavior includes:

- schema 1 → 3 and schema 2 → 3 migration without loss of prior durable state
- future-schema and malformed stores fail closed without replacement
- graceful and hard-kill restart persistence
- idempotent replay does not duplicate durable side effects
- trusted transport identity overrides spoofed actor/client/delegator request context
- local-only egress remains default deny for project data
- credential values remain outside command/model-visible payloads
- untrusted adapter workers fail closed on unqualified Windows builds
- adapter integrity, permissions, capabilities, result/error schemas, crash/hang/backoff/quarantine, and security restoration are tested

No UEFN, Fortnite, Blender, Krita, or other real tool-specific behavior was promoted into Core.
## Resource acceptance

The selected production-shaped Rust core remains in the low-footprint class established during Phase 1.

Representative accepted measurements:

- first-slice daemon after real durable work and restart: 8,871,936 bytes RSS, 0 sampled CPU ms over five seconds
- second-slice daemon after authority/policy/egress work: 8,458,240 bytes RSS, 0 sampled CPU ms
- adapter foundation with 100 validated inactive adapters: 5,955,584 bytes RSS, 0 sampled CPU ms, 0 resident adapter workers
- thirty full sandboxed adapter invocations under foreground load: 113.6006 ms p50

These measurements are fixture evidence, not public hardware budgets.

## Final closure verification

A final `cargo test --workspace` run on the exact accepted production tree passed 54 non-doc tests with 0 failures:

- 3 adapter backend-selection tests
- 8 adapter lifecycle/sandbox integration tests
- 9 shared contract/registry tests
- 27 Core service/storage/policy/diagnostic tests
- 4 daemon security/state tests
- 2 first-slice live daemon/restart tests
- 1 second-slice authority/policy live daemon test

No production source changes were required by the closure run.

## Deferred work

The following are later-phase implementation or release-hardening work and do not reopen Phase 2:

- project discovery/indexing and multi-project change tracking
- production dashboard promotion
- production updater implementation/signing bootstrap
- real tool adapters
- Context Compiler and AI-facing cost engine
- broader Windows sandbox qualification
- additional hardware-tier and soak coverage
## Closure consequence

Phase 2 is complete.

Phase 3 may begin from the accepted production workspace. The next milestone is project discovery and indexing: multi-project registration/import, project isolation, capability reporting, baseline state, watcher/reconciliation updates, changed-only processing, dependency/change graph foundations, and compatibility/configuration metadata.

Phase 3 must preserve the Phase 2 command, authority, persistence, diagnostics, adapter, and recovery contracts rather than rebuilding them.
