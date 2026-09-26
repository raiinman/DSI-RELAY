# Phase 3 closure review

Status: NOT READY, 2026-09-26. Keep `phase3/project-discovery` active; do not merge Phase 3 as closed.

This review checks the published `PHASE3_START_HERE.md` completion gate and the broader Phase 3 work order against the eight accepted slice records and `PHASE3_SCALE_BENCHMARK_EVIDENCE.md`. Phase 1 and Phase 2 remain closed.

| Gate | Finding | Evidence / remaining proof |
| --- | --- | --- |
| Multiple projects coexist without state leakage | PASS | Two-root and shared-root fixtures, project-scoped rows, authority denial, delta and edge isolation. |
| Changed-only update | PASS for targeted processing | One hinted file is hashed without a full filesystem walk; live OS notifications feed that path. |
| Capability report through canonical command path | PASS | Live daemon/client fixtures report baseline and index states. |
| Ordinary small changes avoid full rescan | PARTIAL | The low-latency hint path avoids the tree walk but is provisional. Restoring `ready` still performs full metadata enumeration. |
| Changed-file resource/latency budget on supported hardware tiers | OPEN | 120-, 1,000-, and 15,000-file samples exist on one workstation. Numerical minimum/recommended tier budgets and tail distributions are not selected or verified. |
| Inactive projects remain within idle budget | OPEN | One- and two-project idle samples, including the live watcher, show low observed CPU/RSS on one host. Supported-tier and long-run limits remain unset. |
| Continuity-loss reconciliation restores correctness | PASS on synthetic restart; scale proof OPEN | A durable schema-6 flag prevents metadata-only recovery from claiming ready. The idle daemon automatically verifies content after downtime and detects a metadata-invisible edit. Burst, prolonged contention, and large-project completion remain unproved. |
| Storage/schema compatibility documented and tested | PASS | Schema 3→4→5→6 migration, Phase 2 authority preservation, future/damaged-store fail-closed behavior, and hard restart fixtures. |

The wider Phase 3 work order also calls for a dependency-graph foundation and configuration/version metadata. Bounded, project-scoped dependency edges, schema-7 versioned project configuration, and a sandboxed generic parser operation are present. Automatic installation/selection, authorized source delivery, and scheduling of parser observations into Core remain open. The configured adapter ID/version is a declaration, not installed capability or parser authority. The daemon uses signed-in-session input inactivity and active RELAY commands to defer heavy recovery, but creator-app resource contention has not been measured. Project source files remain authoritative; no generic Core rule depends on UEFN/Fortnite or another editor.

## Work required before a passing closure review

1. Extend the bounded recovery scheduler evidence from the passing hard-restart fixture to event bursts, prolonged foreground work, and supported hardware tiers. Prove the 15-second attempt limit does not strand supported projects; add chunked recovery if it does.
2. Connect the accepted sandboxed parser operation to an installed and authorized project binding. Schedule bounded source observations after index changes, commit through Core's guarded dependency replacement, and prove stale edge invalidation/reparse without granting producer metadata authority.
3. Select numerical latency, idle, storage, and foreground-interference budgets for supported minimum/recommended hardware tiers. Run the 15,000-file fixture and a long-run watcher soak on those tiers, then apply release gates.
4. Recheck each gate above and close Phase 3 only when the open and partial items have passing evidence.
