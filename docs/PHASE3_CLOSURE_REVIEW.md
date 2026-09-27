# Phase 3 closure review

Status: NOT READY, 2026-09-26. Keep `phase3/project-discovery` active; do not merge Phase 3 as closed.

This review checks the published `PHASE3_START_HERE.md` completion gate against the ten synthetic slice records, live watcher-load and parser-resource evidence, and repeatable one-host scale samples. Phase 1 and Phase 2 remain closed.

| Gate | Finding | Evidence / remaining proof |
| --- | --- | --- |
| Multiple projects coexist without state leakage | PASS | Two-root and shared-root fixtures, project-scoped rows, authority denial, delta and edge isolation. |
| Changed-only update | PASS for targeted processing | One hinted file is hashed without a full filesystem walk; live OS notifications feed that path. |
| Capability report through canonical command path | PASS | Live daemon/client fixtures report baseline and index states. |
| Ordinary small changes avoid full rescan | PARTIAL | The low-latency hint path avoids the tree walk but is provisional. Restoring `ready` still performs full metadata enumeration. |
| Changed-file resource/latency budget on supported hardware tiers | OPEN | Five live 15,000-file runs passed on one workstation, but numerical minimum/recommended tier budgets and reliable tail distributions are not selected or verified. |
| Inactive projects remain within idle budget | OPEN | A four-root five-second quiet sample measured 16 ms daemon CPU and stable RSS on one host; a one-minute watcher event soak also passed. Supported-tier idle limits and long-run quiet-state cost remain unset. |
| Continuity-loss reconciliation restores correctness | PASS on synthetic restart and one-host burst; tier proof OPEN | The schema-6 flag prevents metadata-only recovery from claiming ready. A 96-file burst, one-minute event soak, and 1 GiB callback-during-recovery fixture kept uncertain state stale until verification. Required full verification can now continue beyond 15 seconds while idle and quiet; supported-tier completion and foreground behavior remain unproved. |
| Storage/schema compatibility documented and tested | PASS | Schema 3→4→5→6 migration, Phase 2 authority preservation, future/damaged-store fail-closed behavior, and hard restart fixtures. |

The wider Phase 3 work order also calls for a dependency-graph foundation and configuration/version metadata. Bounded, project-scoped dependency edges, schema-7 versioned project configuration, and sandboxed generic parser dispatch are present. One synthetic installed package with an explicit source-delivery grant automatically produced and refreshed edges for its authorized project; a second project without a grant remained empty. Live status and doctor now expose bounded parser installation, failure, quarantine, and recovery state. A first offline CLI installer can validate and stage a local package and explicit grant; it remains outside the shared live command path and is not a public installation lifecycle. Three one-host parser-cost runs covered 15 reparses with publish times of 1,121–1,159 ms. This is not proof with a real tool adapter. The configured adapter ID/version remains a declaration, not installed capability or parser authority. The daemon defers heavy work for signed-in-session input and RELAY writes, while read-only status queries can observe progress. Synthetic foreground writes interrupted a background 256 MiB verification, but creator-app resource contention has not been measured. Project source files remain authoritative; no generic Core rule depends on UEFN/Fortnite or another editor.

## Work required before a passing closure review

1. Extend the passing one-host burst and one-minute soak to prolonged creator-app foreground work and supported hardware tiers. Measure large required full verification on slower machines and whether repeated foreground interruption prevents completion; add resumable/chunked recovery if needed.
2. Validate the new one-host installed-parser cost and live health behavior on supported tiers. Turn the offline package-staging command into a shared live installation lifecycle and verify a real tool adapter before a product beta.
3. Select numerical latency, idle, storage, and foreground-interference budgets for supported minimum/recommended hardware tiers. Run the repeatable 15,000-file fixture and a longer watcher soak on those tiers, then apply release gates.
4. Use the local portable synthetic benchmark draft to recruit opt-in tier measurements only after the public-preview distribution gates in `PUBLIC_PREVIEW_PATH.md` pass. It does not substitute for a creator-app session or product beta.
5. Recheck each gate above and close Phase 3 only when the open and partial items have passing evidence.
