# Phase 3 change and dependency-edge evidence

Status: PASS on the synthetic Windows fixture, 2026-09-26. Phase 3 remains active.

This slice adds a bounded, generic delta read and a derived internal-file dependency-edge store. It does not parse any project language or run an adapter.

## Commands and storage

The shared registry adds version-1 `project.changes`, `project.dependencies.replace`, and `project.dependencies.list`. Core implements them once for the daemon, CLI client, dashboard, and AI command surfaces. The live test calls the daemon through the canonical CLI client library.

`project.changes` reads project-relative change records after a requested index generation. It rejects a generation before the current baseline with `INDEX_CONTINUITY_LOST`, a future generation with `INDEX_GENERATION_CONFLICT`, and a delta larger than the requested bound with `INDEX_DELTA_TOO_LARGE` instead of silently omitting records. The default bound is 100 changes and the maximum requested bound is 500.

`project.dependencies.replace` replaces one producer's edges for one indexed source. It requires the current project index generation, the source's indexed SHA-256, bounded producer identity/version, and existing project-relative target paths. Invalid or stale input fails before any edge replacement commits. `project.dependencies.list` returns up to 500 edges and can filter by source path; an oversized result fails explicitly. The source and target must belong to the same project. Client-supplied producer metadata is attribution, not a trust grant.

Operational SQLite advances from schema 4 to 5 in one migration transaction. It adds `project_index_state.baseline_generation` and `project_dependency_edges`, keyed by project and relative source/target/producer. Existing schema-4 index rows, file snapshots, and change history remain. Migrated schema-4 rows set `baseline_generation` to their current generation conservatively, requiring consumers to start a fresh delta cursor. Schemas 1, 2, and 3 still migrate through to 5; their Phase 2 durable state tests pass.

Baseline rebuild advances `baseline_generation`, clears that project's prior changes and edges, and leaves source files untouched. Reconciliation removes edges touching a modified, deleted, or renamed source or target. Unaffected project edges remain.

## Verification

- A live two-project daemon fixture creates one Alpha edge. Bravo's edge list stays empty despite overlapping filenames.
- A changed Alpha target invalidates its edge. Replacing the edge at the new generation succeeds; renaming the source and deleting the target invalidates it again.
- Alpha's modification, rename, and deletion remain queryable after hard daemon restart. Bravo's delta stays empty.
- Core tests reject a stale source hash, missing or escaping target, stale generation, cross-project reads/writes, and a too-small delta bound.
- A schema-4 fixture with existing index rows and change history migrates to schema 5 with a conservative baseline boundary.
- `cargo test --workspace` passed 67 non-documentation tests, and `cargo build --release --workspace` passed.

The existing 120-file-per-project fixture was rerun on schema 5. One sample measured 19/18 ms baseline scans, 2 ms Core processing for one changed file, 6 ms missed-hint reconciliation, 17 ms full rebuild, and 0 sampled CPU ms during three 400 ms idle windows. Idle RSS was 9,641,984 / 10,182,656 / 10,272,768 bytes at 0/1/2 projects. Main SQLite plus WAL/SHM grew from 267,616 to 601,336 bytes through two imports and baselines; this includes command and usage history. These are single-workstation fixture observations, not hardware-tier budgets.

`git diff --check` and targeted `rustfmt --check` for the new test file passed. Workspace-wide format differences and an unrelated existing `relay-adapter` Clippy raw-pointer lint still prevent clean workspace-wide hygiene gates; Core Clippy completes with warnings.

## Limits and next work

No generic parser automatically extracts edges. A caller with state-write authority supplies edges after examining project files through a future parser/adapter contract. Edges represent an indexed snapshot and can become stale until a reconciliation observes the underlying change. The delta API deliberately requires rebuild/recovery when a requested interval exceeds its bound.

The index still enumerates metadata across the tree during authoritative reconciliation. Active OS watcher delivery, stronger continuity detection for unhinted same-size/same-mtime edits, scalable targeted updates, hardware-tier benchmarks, and Phase 3 closure review remain open. D-149 and Phase 2 authority assumptions are unchanged.
