# Phase 3 provisional hint-update evidence

Status: PASS on synthetic Windows fixtures, 2026-09-26. Phase 3 remains active.

## Contract

The shared registry adds version-1 `project.index.apply_hints`. It accepts 1–500 project-relative file paths and updates only the hinted indexed rows. It reads only those prior snapshots from SQLite, stats/hashes only hinted filesystem files, applies changes in a generation-guarded transaction, and marks the project index `stale`. A directory hint returns `INDEX_HINT_DIRECTORY` and asks the caller to use authoritative reconciliation. Absolute, escaping, symlink-crossing, missing-baseline, and unreadable paths fail with structured errors.

`project.capabilities` now includes an additive `index_status` field: `missing`, `ready`, `stale`, or `unavailable`. While stale, `project.changes` and dependency-edge reads/writes fail with `INDEX_RECONCILIATION_REQUIRED` instead of presenting provisional state as complete. `project.index.reconcile` still enumerates authoritative filesystem metadata, detects missed changes, and returns the index to `ready`. The stale status survives a hard daemon restart.

The hint-only path is an accelerator. It does not make watcher events authoritative and does not automatically subscribe to OS notifications. A caller or future watcher adapter must provide hints. Project files remain untouched.

## Verification

- A 1,000-file generic fixture hashes only its one hinted change and checks one hinted filesystem path. The following full metadata reconciliation examines all 1,000 files, hashes zero, and confirms the hinted update.
- A second fixture changes one hinted and one unhinted file. The hint-only plan includes only the hinted change; reconciliation finds the missed one.
- A rename is represented when both old and new paths are hinted. A directory hint fails without committing a partial generation.
- The live daemon fixture confirms `stale` status, blocks delta and edge reads, survives a hard restart still stale, then reconciles an unhinted change and exposes both changes through the bounded delta command.
- Scoped Core authority denies a hint-only update to another project.
- `cargo test --workspace` passed 70 non-documentation tests. `cargo build --release --workspace`, targeted `rustfmt --check`, and `git diff --check` passed. Core Clippy completes with existing warnings; repository-wide format and Clippy gates retain the previously documented baseline issues.

One run of the 1,000-file fixture reported 136 ms for baseline hashing, 2 ms for the one-file hint update, and 8 ms for the full metadata reconciliation. These timings are single-workstation samples with warm filesystem effects and are not hardware-tier budgets. The decisive work counts are one versus 1,000 filesystem paths checked and one versus zero files hashed.

## Remaining gates

An OS watcher subscription and continuity-loss signal are still needed to supply hints automatically and schedule reconciliation after drops, downtime, or uncertain bursts. Full reconciliation still scans project metadata, and an unhinted same-size/same-mtime content edit can evade metadata comparison. Foreground-safe scheduling, larger hardware-tier resource budgets, automatic dependency extraction, and the Phase 3 closure review remain open. D-149 and Phase 2 authority assumptions are unchanged.
