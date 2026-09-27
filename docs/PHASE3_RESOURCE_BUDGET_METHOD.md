# Phase 3 resource-budget measurement method

Status: measurement protocol, 2026-09-26. Minimum- and recommended-class release limits remain unselected until those machines are measured.

## Repeatable scale sample

On Windows, run `crates/relayd/tests/phase3_scale_report.ps1 -Runs 20 -OutputPath <report.json>` from a checkout with a stable build. The script invokes the live two-project, 15,000-file daemon fixture once per run, rejects failed runs or unexpected work counts, and emits raw samples plus nearest-rank median and p95 values. It records CPU model, core counts, RAM, OS, source commit, a Rust-source fingerprint, dirty-tree status, and debug build profile without a user or machine name. It aborts a timing set if Rust source changes during the runs. Keep the raw JSON with the test record; a summary alone cannot explain outliers. The sample uses fresh fixture roots and state on every run, but the build, OS cache, and storage device can remain warm. Record storage type, power mode, competing load, and cold/warm cache policy alongside the JSON.

Twenty runs are the minimum for reporting an empirical p95 from this fixture; at twenty runs it is the nineteenth ordered observation, so it is still a coarse tail estimate. Five runs can expose spread quickly but must be labeled exploratory. Run the fixture on each declared supported hardware tier with the same project contents and build. Do not pool different host tiers into one latency distribution.

The fixture measures baseline hashing, watcher-attachment content verification, clean metadata reconciliation, one-file event-to-stale wall time, post-hint reconciliation, an explicit full-content verification, one second of idle process CPU and working set, and SQLite main/WAL/SHM bytes. It does not isolate a parser, creator application, GPU, filesystem-cache miss, or long-run watcher idle cost. The one-file event clock includes notification delivery, batching, client polling, and scheduling. The post-hint reconciliation may hash zero files because the watcher has already applied the hint. Inspect work counts before interpreting a latency sample.

## Release-gate matrix

| Gate | Required evidence | Decision rule |
| --- | --- | --- |
| Ordinary changed file | At least 20 live fixture runs per supported tier; one-file work counts, event-to-stale wall time, and provisional hint cost. | Select and publish a numerical p95 limit per tier before claiming pass; reject a run set with watcher timeouts or a full content rescan for the ordinary edit. |
| Continuity-loss recovery | At least 20 full-content runs per tier plus burst/soak runs with realistic file sizes and directory shapes. | Every supported fixture must eventually complete while quiet and yield promptly to foreground work or callbacks. Measure tail latency and repeated-interruption progress; add resumable/chunked recovery if needed. A 7,500-file small-text sample alone cannot prove this. |
| Inactive projects | At least five minutes of per-process CPU and working-set samples with two watched roots, then a long-run watcher soak with more roots. | Publish numerical CPU/RAM limits by tier. Record p95 and maximum CPU, working set, wakeup or I/O activity, and whether state remains correct after the window. The existing one-second sample cannot pass this gate. |
| Storage | SQLite main/WAL/SHM before and after baseline and after soak; project source and evidence bytes reported separately. | Publish a cap or growth slope for a defined fixture and verify checkpoint/restart behavior. A single final database size cannot establish a growth limit. |
| Foreground interference | Repeatable interactive creator-app action or frame-time trace with RELAY off/idle/active on each tier, interleaved to reduce drift. | Publish a maximum p95/p99 latency or frame-time delta and reject RELAY work that crosses it. The synthetic fixture has no creator application and cannot pass this gate. |

The scheduler retains a 15-second cap for metadata-only recovery. Required full-content verification may continue longer while the session remains idle and watcher callbacks stay quiet; it still yields to foreground activity. The test matrix should include a minimum-class and a recommended-class machine chosen from the supported creator environment, plus a high-end workstation for regression context. Record full machine specifications and fixture versions in each run report; do not infer a minimum-tier pass from a faster host.

## Measurement hygiene

1. Build once before timing, record the exact source revision, and avoid concurrent builds or test jobs while gathering samples. Record failed runs, retries, antivirus or backup activity, and power-state changes rather than silently dropping them.
2. Use a stable named volume and report whether the project data is local SSD/HDD or networked. Record whether the filesystem cache is warm; do not label a repeated warm run as cold-start evidence.
3. Measure process CPU and memory separately from total-host load. Normalize CPU time by elapsed wall time and logical processor count if percentages are reported.
4. Pair foreground measurements against the same creator-app task without RELAY in a nearby time window. Interleave baseline and RELAY conditions and retain raw traces to distinguish variance from interference.
5. Keep assertions about work counts and correctness separate from timing thresholds. A fast result that leaves an index stale or misses a change is a failed run.

See `PHASE3_SCALE_BENCHMARK_EVIDENCE.md` for the existing single-host observations and `PHASE3_CLOSURE_REVIEW.md` for the open acceptance gates.

## Exploratory live run on one workstation

Five consecutive fixture runs passed on 2026-09-26 using Windows 11, an 8-physical/16-logical-core CPU, and 47.9 GiB RAM. The debug-build checkout was dirty at commit `f3ce18a198506ab25a32f96394c875192fe7bd49`; the Rust-source fingerprint stayed `f9f886cf6d761e21f48bdd6d37093b2c9fa00c40d0633b8ce681caf3ed605283` through the set. The complete machine-readable samples are in `PHASE3_SCALE_FIVE_RUN_2026-09-26.json`.

| Metric | Samples | Minimum | Median | Maximum |
| --- | ---: | ---: | ---: | ---: |
| Baseline hashing per 7,500-file project | 10 | 1,267 ms | 1,319 ms | 1,526 ms |
| Watcher-attachment full verification per project | 10 | 1,324 ms | 1,348 ms | 1,448 ms |
| Clean metadata reconciliation per project | 10 | 54 ms | 60 ms | 68 ms |
| One-file change to watcher-stale observation | 5 | 29 ms | 34 ms | 115 ms |
| Post-hint reconciliation, zero additional files hashed | 5 | 57 ms | 58 ms | 71 ms |
| Explicit 7,500-file full-content verification | 5 | 1,258 ms | 1,377 ms | 1,462 ms |
| Idle daemon working set | 5 | 15,478,784 bytes | 15,781,888 bytes | 15,921,152 bytes |
| Idle process CPU over a one-second window | 5 | 0 ms | 0 ms | 0 ms |
| SQLite main + WAL + SHM at fixture end | 5 | 7,776,664 bytes | 7,776,664 bytes | 7,776,664 bytes |

These are exploratory warm-host observations, not selected limits or a minimum/recommended-tier result. Five observations provide no reliable tail estimate; their empirical p95 would simply equal the maximum. The zero CPU samples are below the one-second timer resolution and do not establish a long-run idle budget. The storage figure includes operational and usage records, not only the index. The small text fixture does not exercise large files or foreground creator-app contention. No creator application was active, so this run supplies no foreground-interference number.
