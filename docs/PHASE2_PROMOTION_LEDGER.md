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

First promotion target: one vertical that registers a project, executes one registry-backed command, writes a durable result and job checkpoint, reports status/doctor health, and survives restart.

Promotion rule: each row moves from evidence to production only with focused tests and a resource check. UEFN-specific behavior remains outside the core.
