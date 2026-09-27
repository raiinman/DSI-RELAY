# Phase 3 Start Here

## Status

Phase 3 — Project discovery and indexing — is active.

Phase 2 is closed. The accepted production foundation is the Rust workspace containing `relay-contracts`, `relay-core`, `relayd`, `relay`, and `relay-adapter`.

Phase 3 promotes project discovery/indexing behavior into that production workspace. The generic two-project baseline, bounded change/dependency-edge foundation, provisional hint-update path, explicit content-verification recovery, bounded Windows watcher delivery, idle recovery scheduler, versioned project configuration, sandboxed parser operation, explicitly installed parser dispatch, and live parser health are accepted on synthetic fixtures; see the ten Phase 3 slice evidence records. The active development line also adds project archive, restore, and removal tombstones; restoring an archive invalidates its old index. Live watcher load, five-run 15,000-file single-host scale, and three-run installed-parser cost are recorded in `PHASE3_WATCHER_LOAD_EVIDENCE.md`, `PHASE3_RESOURCE_BUDGET_METHOD.md`, and `PHASE3_PARSER_RESOURCE_EVIDENCE.md`. `PHASE3_CLOSURE_REVIEW.md` remains NOT READY. Do not reopen Phase 1 stack selections or Phase 2 Core contracts without contradictory evidence.

## Goal

Make RELAY understand multiple local projects deterministically without requiring AI enumeration or repeated full scans.

Phase 3 must establish:

- generic project discovery/add/import
- strict per-project isolation
- durable baseline state
- incremental watcher/reconciliation
- changed-only indexing
- project capability reporting
- change/delta records
- dependency-graph foundations
- configuration/version metadata
- restart/continuity-loss recovery
- measured inactive-project and changed-file resource cost
## Locked foundations

Preserve:

- one per-user daemon and canonical CLI transport
- shared command registry and deterministic schema validation
- schema-3 authority/transaction/usage/credential/egress contracts unless a Phase 3 migration explicitly advances storage
- trusted actor/client attribution and project-scope enforcement
- bounded structured JSONL diagnostics
- generic adapter broker and qualified Windows sandbox
- notification hints plus authoritative reconciliation from D-149
- foreground-safe resource scheduling from D-150
- derived indexes are rebuildable; project files remain authoritative

Project-specific names, tool rules, and fixtures do not belong in Core.

## Completed construction order

1. project root normalization, add/import, and durable registry extensions
2. two-project isolation fixture and restart persistence
3. baseline file inventory and stable file identity metadata
4. capability report contract
5. watcher hint ingestion plus reconciliation
6. changed-only index update and explicit continuity-loss recovery
7. change/delta and dependency-edge foundations
8. resource/latency benchmark with multiple inactive projects
9. Carry the remaining real-adapter and hardware gaps into the final integrated acceptance run after the documented product workflows are built; keep Phase 3 open meanwhile
## First slice

The first slice passed the `PHASE3_FIRST_SLICE.md` acceptance contract on its synthetic fixture. Use its evidence record as the baseline for the remaining Phase 3 work.

The first slice must prove one production-shaped generic vertical:

- add/import two synthetic projects
- canonicalize project roots without leaking machine-specific paths into public contracts
- keep project state/index rows isolated
- persist baseline file inventory across daemon restart
- produce a compact project capability report
- detect one changed file without reparsing unchanged files
- treat watcher events as hints and reconcile against authoritative filesystem state
- recover a deliberately missed change through reconciliation
- expose deterministic structured commands through the existing registry/CLI path
- measure idle overhead for multiple registered projects and changed-file latency

No real editor integration or UEFN-specific parsing belongs in the first slice.

The accepted implementation offers a targeted hint-only update that avoids a full filesystem metadata walk but marks the index stale. The daemon supplies bounded recursive Windows notification hints and marks uncertain continuity stale after event errors, overflow, ambiguous directories, and restart. Metadata reconciliation enumerates the tree and recovers missed changes with observable metadata differences. After continuity loss, schema 6 requires `verify_content: true` to restore ready state and detect same-size/same-timestamp edits. The idle recovery loop defers when a notification arrives during scanning; live burst, one-minute soak, callback-race, and synthetic foreground-write fixtures passed on one Windows host. Project-scoped derived dependency edges, schema-7 versioned project configuration, and sandboxed parser dispatch with an explicit per-project installation grant are supported. Live status and doctor report bounded parser health. Public parser installation, real tool adapters, prolonged creator-app contention, and supported-tier resource budgets remain Phase 3 work.

## Phase 3 rules

- Never trust a watcher event as complete truth.
- Never require a full rescan for an ordinary small change once a healthy baseline exists.
- Never allow project A paths/state/results/index rows to appear through project B scope.
- Canonical filesystem paths are local operational data; compact public/machine results should prefer project-relative paths and opaque project IDs.
- File content, filenames, tool metadata, project config, and adapter output are untrusted data.
- Discovery must be deterministic and local before any AI call.
- Capability reports must distinguish detected, available, unavailable, incompatible, and unknown states.
- Continuity-loss and stale-baseline states must be explicit.
- Index records are derived and may be rebuilt; user/project source files are authoritative.
- Foreground creator work continues to outrank optional indexing work.
## Phase 3 completion gate

Phase 3 closes only when:

- multiple projects coexist without state leakage
- changed-only index update is demonstrated
- a project capability report works through the canonical command path
- ordinary small changes do not require a full rescan
- changed-file processing meets the selected resource/latency budget on the fixture
- inactive registered projects remain within the idle resource budget
- continuity-loss reconciliation restores correctness
- Phase 3 storage/schema evolution and compatibility behavior are documented and tested

## Current development handoff

`development/relay-v0.1` is the active construction line. Read root `AGENTS.md`, the applicable child AGENTS files, and `ROADMAP.md` before changing code. This file records Phase 3 foundations and remaining acceptance limits; it no longer directs work to the older `phase3/project-discovery` branch. Build the documented workflows across Phases 3–11 before one integrated acceptance run. Keep `PHASE3_CLOSURE_REVIEW.md` as historical gate evidence, the preview binary unpublished, and `main` unmerged. Real adapter, creator-app, and hardware checks remain UNTESTED when their environments are unavailable.
