# Phase 3 explicit content-verification evidence

Status: PASS on synthetic Windows fixtures, 2026-09-26. Phase 3 remains active.

## Contract

Version-1 `project.index.reconcile` accepts an additive optional `verify_content` boolean and reports the selected value in its result. The default remains `false`: enumerate all filesystem metadata, hash new, hinted, or metadata-changed files, and retain changed-only cost for ordinary reconciliation. With `verify_content: true`, hash every regular file in the project before comparing the stored content digests. Both modes apply project-scoped changes through the existing generation-guarded transaction. No schema migration or project-source write is required.

The explicit content pass is for uncertain continuity, including a watcher outage or a file changed without a size or timestamp change. It is a full read of project content and must be scheduled with foreground workload priority in mind. The `ready` marker means a full-tree reconciliation completed for the stored generation; it is not proof that a metadata-only pass detected an unobservable content edit.

## Verification

- In the live daemon fixture, a file was rewritten with different same-length bytes, then its previous modification time was restored. Ordinary metadata reconciliation hashed zero files and reported no change. The explicit content pass hashed the project's full file count and reported the changed project-relative path.
- In the 1,000-file generic fixture, one hinted update checked and hashed one path. A following metadata pass checked 1,000 paths and hashed zero. The explicit content pass checked and hashed all 1,000 paths without reporting a false change.
- `cargo test --workspace` passed 70 non-documentation tests, including the live daemon and scale fixtures. `cargo build --release --workspace` and `git diff --check` passed.

One warm-filesystem scale run measured 132 ms for initial baseline hashing, 2 ms for one hinted update, 7 ms for metadata reconciliation, and 132 ms for full content verification. These are single-workstation samples, not hardware-tier budgets. The work counts, rather than these timings, define the contract.

## Remaining gates

An OS watcher subscription, automatic continuity-loss signaling and deferred recovery scheduling, automatic dependency extraction, and supported hardware-tier budgets remain open. Full content verification should run after genuinely uncertain continuity; running it for every ordinary edit would defeat the changed-only resource goal. D-149 and Phase 2 authority assumptions are unchanged.
