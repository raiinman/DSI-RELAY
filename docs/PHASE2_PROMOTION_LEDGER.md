# Phase 2 Promotion Ledger

Purpose: promote Phase 1 evidence into production-shaped core code without silently copying spike shortcuts.

| Slice | Phase 1 evidence | Phase 2 target | Gate |
| --- | --- | --- | --- |
| Host + IPC | Spikes 7-8 | per-user relayd + canonical CLI transport | restart, current-user isolation, protocol negotiation |
| Operational state | Spike 8 | project/result/job store | schema compatibility, damage/degraded behavior, restart persistence |
| Command contracts | Spike 9 | one registry/validator for all surfaces | argument/result/error validation and compact discovery |
| Diagnostics | Spike 14 / D-159 | bounded local diagnostic service | independent health, recovery, redaction, bounded detail |
| Adapter lifecycle | Spikes 10-12 | broker/manifest + strong Windows isolation | fail-closed capability/path/egress policy |
| Dashboard transport | Spike 6 | embedded presentation adapter | no duplicate business logic |
| Update inventory | Spike 13 | component/version/provenance model | verified stage/activate/rollback boundary |

## Current promotion status

Accepted in the first production slice:

- Host + IPC: promoted for the per-user daemon and canonical local CLI transport.
- Operational state: promoted through schema-2 project/result/job/idempotency storage with schema-1 migration coverage.
- Command contracts: promoted into `relay-contracts` with deterministic validation and discovery.
- Diagnostics: promoted into `relay-core` with independent health/degraded behavior.

First-slice acceptance is recorded in `PHASE2_FIRST_SLICE_EVIDENCE.md`.

Accepted in the second production slice:

- Permission/effect enforcement: promoted into `relay-core` using registry metadata.
- Trusted identity/project scope: promoted with transport-owned actor/client/delegator attribution and fail-closed project boundaries.
- Transaction/idempotency boundary: promoted with durable schema-3 transaction records and replay de-duplication.
- Usage/cost metrics: promoted with deterministic command/replay counts and zero-cost baselines for unused model/remote work.
- Data-classification/egress primitives: promoted with strongest-class propagation, local-only default denial, and durable egress decisions.
- Credential-handle boundary: promoted as metadata/revocation only; secret values remain outside command/model-visible payloads.

Second-slice acceptance is recorded in `PHASE2_SECOND_SLICE_EVIDENCE.md`.

Current next promotion target: adapter broker/manifest lifecycle plus the D-155 through D-157 Windows worker-isolation boundary behind production interfaces.

Promotion rule: each row moves from evidence to production only with focused tests and a resource check. UEFN-specific behavior remains outside the core.
