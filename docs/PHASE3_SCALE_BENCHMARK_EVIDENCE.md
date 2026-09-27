# Phase 3 production-scale indexing sample

Status: PASS for the measured fixture, 2026-09-26. Hardware-tier budgets remain open.

The repeatable, manually invoked `benchmark_two_watched_projects_at_fifteen_thousand_files` fixture in `crates/relayd/tests/phase3_watcher.rs` creates two distinct 7,500-file generic projects. It runs import and baseline build through the live daemon command path, fully verifies content after watcher attachment, measures a separate clean metadata reconciliation, samples one second of idle process CPU/RSS, changes one file, waits for the watcher to mark its project stale, reconciles, and explicitly hashes one full project's content. The other project must remain ready during the changed-file step.

Run: `cargo test -p relayd --test phase3_watcher benchmark_two_watched_projects_at_fifteen_thousand_files -- --ignored --nocapture`. The fixture is ignored in routine tests because it creates 15,000 files and is intended for measured tier runs.

| Measurement | Earlier run | Schema-7 run |
| --- | ---: | ---: |
| Host class | 8 physical / 16 logical cores, 47.9 GiB RAM | Same workstation |
| Alpha / Bravo baseline, 7,500 files each | 1,013 / 1,032 ms | 947 / 946 ms |
| Watcher-attachment content verification, Alpha / Bravo | Not separately measured | 958 / 949 ms; 7,500 files hashed each |
| Clean metadata reconciliation, Alpha / Bravo | 48 / 50 ms | 46 / 46 ms; 0 files hashed |
| One changed file to watcher-stale observation, wall time | 138 ms | 353 ms |
| Reconciliation after targeted watcher update | 50 ms, 0 files hashed | 47 ms, 0 files hashed |
| Explicit Alpha full-content verification | 1,119 ms, 7,500 files hashed | 946 ms, 7,500 files hashed |
| Idle daemon working set, two watched roots | 13,709,312 bytes | 15,458,304 bytes |
| Idle daemon sampled CPU time | 0 ms over 1,000 ms | 0 ms over 1,000 ms |
| SQLite main + WAL + SHM during fixture | 8,102,144 bytes | 7,776,664 bytes |

The changed-file watcher path applied its hint before the confirming metadata pass; that pass found no remaining change and hashed zero files. The watcher-stale observations include OS event delivery, 250 ms batching, Core processing, named-pipe query polling, and scheduler variance. They are not per-file hash timing distributions. The newer fixture separates required watcher-attachment content verification from a clean metadata-only pass; the earlier run preceded schema-6 verification requirements. SQLite size includes project, command, transaction, usage, idempotency, and index records. The one-second idle samples do not prove long-run power or wakeup behavior.

These are two warm-workstation runs, with generic small text files and no active creator application. They do not establish minimum/recommended hardware budgets, tail-latency distributions, foreground interference, large-file behavior, many watched roots, or event-storm recovery. The fixture should be rerun on selected supported tiers before Phase 3 closure.
