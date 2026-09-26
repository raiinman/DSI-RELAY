# Phase 3 first-slice evidence

Status: PASS on the synthetic Windows fixture, 2026-09-26. Phase 3 remains active.

This is the schema-4 evidence snapshot from first-slice commit `50a2f20`. Later Phase 3 storage evolution is recorded separately.

## Scope and command path

The production `relay-contracts` registry contains four new version-1 commands: `project.import`, `project.index.build`, `project.index.reconcile`, and `project.capabilities`. The existing `project.register` and `project.list` commands remain. The daemon and canonical CLI client use the same validated command envelopes and Core dispatch. The live fixture calls through the CLI client's named-pipe path to the daemon; it does not spawn the CLI executable.

`project.import` canonicalizes a local directory before storing it, returns an opaque project ID without the root, and fails non-interactively for an invalid root. `project.list` returns an opaque local-project reference instead of an absolute local root. Canonical roots remain local operational data in SQLite. Indexed files and changes use project-relative paths.

## Schema and compatibility

Operational SQLite advances from schema 3 to schema 4. The focused transactional migration adds `project_index_state`, `project_files`, and `project_changes`, keyed by project ID. It does not alter Phase 2 authority, result, job, transaction, usage, credential-handle, egress, or idempotency tables. Fresh stores and migrations from schemas 1, 2, and a populated schema 3 are tested. The schema-3 fixture retains project, result, job, idempotency, transaction actor/client/delegator, usage, credential-handle, and egress records after reopening as schema 4. Future-schema and malformed-store fail-closed tests still pass.

Index rows are derived state. A rebuild replaces one project's file inventory and clears that project's change history in one transaction. Reconciliation checks the expected generation and applies touched rows and change records atomically. The index can be recreated from project files without writing those files.

## Acceptance evidence

- The live fixture imports two separate 120-file projects with overlapping relative names. Each baseline has 120 project-scoped rows; Bravo has no Alpha change rows. A scoped Core authority is denied Alpha/Bravo cross-project index build, reconciliation, and capability reads.
- Both baselines survive graceful daemon shutdown/restart and hard process kill/restart. A clean reconciliation after hard restart hashes zero files.
- A single watcher hint for one changed Alpha file hashes one file and reuses 119 snapshots. A changed file with no hint is recovered by authoritative reconciliation and also hashes one file.
- A later reconciliation reports one `renamed`, one `deleted`, and one `added` change, with project-relative paths. It hashes the two new paths and retains a 120-file inventory.
- Invalid/missing roots, an exclusively locked unreadable file, reconciliation before a baseline, and a parent-traversal hint return structured errors. The fixture verifies no partial index state is committed for those failures. A disappearing root reports unavailable capabilities.
- Generic `.txt`, `.json`, and `.src` fixture files contain no editor or engine behavior. No AI call is involved.

## Resource and latency sample

One run of `cargo test -p relayd --test phase3_first_slice -- --nocapture` on the local Windows workstation produced:

| Measurement | Result |
| --- | ---: |
| Initial daemon readiness | 31 ms |
| Alpha / Bravo baseline scan, 120 files each | 21 / 19 ms |
| One-file hinted reconciliation, Core / wall | 2 / 5 ms |
| Missed-hint reconciliation | 6 ms |
| Rename/delete/add reconciliation | 3 ms |
| Full Alpha rebuild recovery control | 17 ms |
| Graceful / hard restart readiness | 22 / 21 ms |
| Idle RSS, 0 / 1 / 2 projects | 9,637,888 / 9,957,376 / 10,194,944 bytes |
| Idle CPU, 0 / 1 / 2 projects | 0 / 0 / 0 sampled ms over 400 ms each |
| SQLite main + WAL + SHM before projects / after two baselines | 242,896 / 576,616 bytes |
| Growth through import and two baselines | 333,720 bytes |
| SQLite size after clean final shutdown | 208,896 bytes |

The storage-growth sample includes project registration and command/usage/transaction history, not index tables alone. The 400 ms idle samples and one run are fixture observations, not hardware-tier budgets or a statistical distribution.

## Verification and limits

`cargo test --workspace` passed 64 non-doc tests, `cargo build --release --workspace` passed, `git diff --check` passed, and the two new Rust files passed targeted `rustfmt --check`. Workspace-wide `cargo fmt --check` reports formatting differences across pre-existing files. Workspace-wide Clippy is blocked by a pre-existing raw-pointer safety lint in `relay-adapter`; `relay-core` Clippy completes with warnings.

Changed-only means changed-candidate content hashing, not constant-time reconciliation: each reconciliation currently enumerates filesystem metadata across the project tree. Caller-provided watcher paths are accepted as hints; an OS watcher subscription is not yet part of this slice. Same-size and same-mtime content changes without a hint can evade this metadata comparison until a stronger continuity signal or rebuild. Rename recognition pairs added/deleted files with equal SHA-256 content; a rename combined with a content edit appears as delete plus add. Symlinks are skipped. No generic ignore policy or dependency-edge extraction is included.

Phase 2's `project.register` version-1 result still echoes its caller-supplied `root_uri` for compatibility. New import results and project-list results do not expose imported canonical local roots. Root paths remain private local state and must be governed by later remote-egress policy.

D-149 remains unchanged: hints are never authoritative, and reconciliation restores missed changes under the stated metadata assumptions. No Phase 2 authority or adapter-isolation assumption changed.
