# Phase 3 closure review

Status: NOT READY, 2026-09-26. Keep `phase3/project-discovery` active; do not merge Phase 3 as closed.

This review checks the published `PHASE3_START_HERE.md` completion gate and the broader Phase 3 work order against the five accepted slice records and `PHASE3_SCALE_BENCHMARK_EVIDENCE.md`. Phase 1 and Phase 2 remain closed.

| Gate | Finding | Evidence / remaining proof |
| --- | --- | --- |
| Multiple projects coexist without state leakage | PASS | Two-root and shared-root fixtures, project-scoped rows, authority denial, delta and edge isolation. |
| Changed-only update | PASS for targeted processing | One hinted file is hashed without a full filesystem walk; live OS notifications feed that path. |
| Capability report through canonical command path | PASS | Live daemon/client fixtures report baseline and index states. |
| Ordinary small changes avoid full rescan | PARTIAL | The low-latency hint path avoids the tree walk but is provisional. Restoring `ready` still performs full metadata enumeration. |
| Changed-file resource/latency budget on supported hardware tiers | OPEN | 120-, 1,000-, and 15,000-file samples exist on one workstation. Numerical minimum/recommended tier budgets and tail distributions are not selected or verified. |
| Inactive projects remain within idle budget | OPEN | One- and two-project idle samples, including the live watcher, show low observed CPU/RSS on one host. Supported-tier and long-run limits remain unset. |
| Continuity-loss reconciliation restores correctness | PASS when invoked; automation OPEN | Durable stale state survives restart. Full-content verification detects metadata-invisible edits. Automatic foreground-safe recovery after drop/downtime is not scheduled yet. |
| Storage/schema compatibility documented and tested | PASS | Schema 3→4→5 migration, Phase 2 authority preservation, future/damaged-store fail-closed behavior, and hard restart fixtures. |

The wider Phase 3 work order also calls for a dependency-graph foundation and configuration/version metadata. Bounded, project-scoped dependency edges are present, but automatic extraction through a generic parser/adapter boundary is still open. The shared command registry supplies version metadata; project configuration metadata has not yet been accepted as a Phase 3 contract. The daemon defers watcher hashing during RELAY client commands; it does not yet detect a creator application's foreground workload for full reconciliation. Project source files remain authoritative; no generic Core rule depends on UEFN/Fortnite or another editor.

## Work required before a passing closure review

1. Add a foreground-aware, bounded recovery scheduler that advances stale projects through metadata or full-content reconciliation after notification loss without forcing heavy scans during active creator work. Prove crash/restart and burst behavior.
2. Define a generic parser/adapter contract for automatic dependency extraction, complete the project configuration/version metadata foundation, and verify stale edge invalidation without granting producer metadata authority.
3. Select numerical latency, idle, storage, and foreground-interference budgets for supported minimum/recommended hardware tiers. Run the 15,000-file fixture and a long-run watcher soak on those tiers, then apply release gates.
4. Recheck each gate above and close Phase 3 only when the open and partial items have passing evidence.
