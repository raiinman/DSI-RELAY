# Phase 3 Start Here

## Status

Phase 3 — Project discovery and indexing — is active.

Phase 2 is closed. The accepted production foundation is the Rust workspace containing `relay-contracts`, `relay-core`, `relayd`, `relay`, and `relay-adapter`.

Phase 3 promotes project discovery/indexing behavior into that production workspace. The generic two-project first slice and bounded change/dependency-edge foundation are accepted; see `PHASE3_FIRST_SLICE_EVIDENCE.md` and `PHASE3_SECOND_SLICE_EVIDENCE.md`. Do not reopen Phase 1 stack selections or Phase 2 Core contracts without contradictory evidence.

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

## Work order

1. project root normalization, add/import, and durable registry extensions
2. two-project isolation fixture and restart persistence
3. baseline file inventory and stable file identity metadata
4. capability report contract
5. watcher hint ingestion plus reconciliation
6. changed-only index update and explicit continuity-loss recovery
7. change/delta and dependency-edge foundations
8. resource/latency benchmark with multiple inactive projects
9. Phase 3 closure review against the published roadmap exit criteria
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

The accepted implementation hashes changed candidates but still enumerates project metadata during reconciliation. Caller-provided watcher hints and project-scoped derived dependency edges are supported. An OS watcher subscription, stronger continuity signal, automatic dependency-edge extraction, and hardware-tier resource budgets remain Phase 3 work.
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

## Handoff

For a fresh conversation:

> Take over DSI RELAY Phase 3 from `phase3/project-discovery`. Read root `AGENTS.md`, `docs/AGENTS.md`, `docs/PHASE3_START_HERE.md`, and both Phase 3 slice evidence records before editing. Phase 1 and Phase 2 are closed; the synthetic project baseline and schema-5 change/dependency foundation are accepted. Preserve the Rust Core/command/authority/adapter contracts. Continue watcher/continuity handling, automatic dependency extraction through a generic adapter boundary, resource evidence, and the Phase 3 closure review. Keep project files authoritative and UEFN-specific behavior outside Core.
