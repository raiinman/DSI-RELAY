# Phase 3 production-scale indexing sample

Status: PASS for the measured fixture, 2026-09-26. Hardware-tier budgets remain open.

The repeatable, manually invoked `benchmark_two_watched_projects_at_fifteen_thousand_files` fixture in `crates/relayd/tests/phase3_watcher.rs` creates two distinct 7,500-file generic projects. It runs import and baseline build through the live daemon command path, attaches the OS watcher, samples one second of idle process CPU/RSS, changes one file, waits for the watcher to mark its project stale, reconciles, and explicitly hashes one full project's content. The other project must remain ready during the changed-file step.

Run: `cargo test -p relayd --test phase3_watcher benchmark_two_watched_projects_at_fifteen_thousand_files -- --ignored --nocapture`. The fixture is ignored in routine tests because it creates 15,000 files and is intended for measured tier runs.

| Measurement | This run |
| --- | ---: |
| Host class | 8 physical cores / 16 logical processors, 47.9 GiB RAM |
| Alpha / Bravo baseline, 7,500 files each | 1,013 / 1,032 ms |
| Clean metadata reconciliation, Alpha / Bravo | 48 / 50 ms |
| One changed file to watcher-stale observation, wall time | 138 ms |
| Reconciliation after targeted watcher update | 50 ms, 0 files hashed |
| Explicit Alpha full-content verification | 1,119 ms, 7,500 files hashed |
| Idle daemon working set, two watched roots | 13,709,312 bytes |
| Idle daemon sampled CPU time | 0 ms over 1,000 ms |
| SQLite main + WAL + SHM during fixture | 8,102,144 bytes |

The changed-file watcher path applied its hint before the confirming metadata pass; that pass found no remaining change and hashed zero files. The 138 ms observation includes OS event delivery, 250 ms batching behavior that may have been partly elapsed when the edit occurred, Core processing, named-pipe query polling, and scheduler variance. It is not a per-file hash timing distribution. The SQLite size includes project, command, transaction, usage, idempotency, and index records. The one-second idle sample does not prove long-run power or wakeup behavior.

This is one warm-workstation run, with generic small text files and no active creator application. It does not establish minimum/recommended hardware budgets, tail-latency distributions, foreground interference, large-file behavior, many watched roots, or event-storm recovery. The fixture should be rerun on selected supported tiers before Phase 3 closure.
