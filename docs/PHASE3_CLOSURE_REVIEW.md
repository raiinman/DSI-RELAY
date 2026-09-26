# Phase 3 closure review

Status: NOT READY, 2026-09-26. Keep `phase3/project-discovery` active; do not merge Phase 3 as closed.

This review checks the published `PHASE3_START_HERE.md` completion gate against the nine synthetic slice records, live watcher-load evidence, and repeatable one-host scale samples. Phase 1 and Phase 2 remain closed.

| Gate | Finding | Evidence / remaining proof |
| --- | --- | --- |
| Multiple projects coexist without state leakage | PASS | Two-root and shared-root fixtures, project-scoped rows, authority denial, delta and edge isolation. |
| Changed-only update | PASS for targeted processing | One hinted file is hashed without a full filesystem walk; live OS notifications feed that path. |
| Capability report through canonical command path | PASS | Live daemon/client fixtures report baseline and index states. |
| Ordinary small changes avoid full rescan | PARTIAL | The low-latency hint path avoids the tree walk but is provisional. Restoring `ready` still performs full metadata enumeration. |
| Changed-file resource/latency budget on supported hardware tiers | OPEN | Five live 15,000-file runs passed on one workstation, but numerical minimum/recommended tier budgets and reliable tail distributions are not selected or verified. |
| Inactive projects remain within idle budget | OPEN | Two-project one-second idle samples and a one-minute watcher event soak exist on one host. Supported-tier idle limits and long-run quiet-state cost remain unset. |
| Continuity-loss reconciliation restores correctness | PASS on synthetic restart and one-host burst; tier proof OPEN | The schema-6 flag prevents metadata-only recovery from claiming ready. A 96-file burst, one-minute event soak, and 1 GiB callback-during-recovery fixture kept uncertain state stale until verification. Completion within the 15-second attempt limit on supported tiers remains unproved. |
| Storage/schema compatibility documented and tested | PASS | Schema 3→4→5→6 migration, Phase 2 authority preservation, future/damaged-store fail-closed behavior, and hard restart fixtures. |

The wider Phase 3 work order also calls for a dependency-graph foundation and configuration/version metadata. Bounded, project-scoped dependency edges, schema-7 versioned project configuration, and sandboxed generic parser dispatch are present. One synthetic installed package with an explicit source-delivery grant automatically produced and refreshed edges for its authorized project; a second project without a grant remained empty. This is not a public installation flow or proof with a real tool adapter. The configured adapter ID/version remains a declaration, not installed capability or parser authority. The daemon defers heavy work for signed-in-session input and RELAY writes, while read-only status queries can observe progress. Creator-app resource contention has not been measured. Project source files remain authoritative; no generic Core rule depends on UEFN/Fortnite or another editor.

## Work required before a passing closure review

1. Extend the passing one-host burst and one-minute soak to prolonged creator-app foreground work and supported hardware tiers. Prove the 15-second recovery attempt limit does not strand supported projects; add chunked recovery if it does.
2. Measure installed-parser scheduling cost and surface parser failures in diagnostics before treating automatic extraction as an operational capability. A supported installation command and real tool adapter remain later product work unless the Phase 3 release target includes them.
3. Select numerical latency, idle, storage, and foreground-interference budgets for supported minimum/recommended hardware tiers. Run the repeatable 15,000-file fixture and a longer watcher soak on those tiers, then apply release gates.
4. Recheck each gate above and close Phase 3 only when the open and partial items have passing evidence.
