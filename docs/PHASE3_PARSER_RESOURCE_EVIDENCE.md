# Phase 3 installed-parser resource sample

Status: PASS for the measured synthetic fixture on one workstation, 2026-09-26. Supported-tier and foreground budgets remain open.

The manually invoked `crates/relayd/tests/phase3_parser_resource.rs` fixture starts the live daemon with one configured project and no parser grant, samples five seconds of idle process CPU and working set, then restarts with a digest-pinned installed synthetic parser. It verifies content after restart, waits for an initial edge, changes the 28-byte JSON source five times, reconciles each change, and measures time from reconcile completion until the new edge is visible through the canonical command path. A final five-second sample observes the installed daemon after those reparses. The sandbox worker is built and installed in its own temporary package directory. The fixture disables the OS watcher to isolate parser dispatch and uses a debug-only one-millisecond user-idle threshold; the normal production threshold is 30 seconds.

Run three repeat fixtures with `crates/relayd/tests/phase3_parser_resource_report.ps1 -Runs 3 -OutputPath <report.json>`. The script builds the synthetic worker, rejects fixture failures or unexpected work counts, checks that Rust source does not change during the set, and records the source revision and fingerprint. The raw three-run result is `PHASE3_PARSER_RESOURCE_THREE_RUN_2026-09-26.json`.

| Measurement | Observations | Minimum | Median | Maximum |
| --- | ---: | ---: | ---: | ---: |
| Initial edge publication after verified reconcile | 3 | 1,338 ms | 1,349 ms | 1,378 ms |
| Reparsed edge publication after a source edit and reconcile | 15 | 1,121 ms | 1,134 ms | 1,159 ms |
| Unbound daemon idle CPU over five seconds | 3 | 0 ms | 0 ms | 0 ms |
| Installed daemon idle CPU over five seconds | 3 | 0 ms | 0 ms | 16 ms |
| Unbound daemon working set | 3 | 11,894,784 bytes | 11,894,784 bytes | 11,907,072 bytes |
| Installed daemon working set after reparses | 3 | 16,166,912 bytes | 16,261,120 bytes | 16,289,792 bytes |

The sample host ran Windows 11 with 8 physical/16 logical CPU cores and 47.9 GiB RAM. The debug checkout was dirty at commit `c2fec0d7c72888e532434ebfbfb1ab57bc4d6953`; its Rust-source fingerprint stayed `79c273f6878c0be0d6a95d526a53a0653a50af39dbc2b6a7687d3c89f5b21120` during the three-run set. Each run passed.

An earlier one-run diagnostic, before the parser retry fix, observed 5,129–5,190 ms for five rapid reparses. The successful parse left a retry timestamp that delayed a later changed source. The parser scheduler now clears a successful attempt and keys retry suppression to the ready generation and source digest. The stable three-run set above measures that corrected behavior; the diagnostic is retained here to explain why the latency changed, not as an accepted performance result.

These measurements combine the parser poll interval, sandbox worker launch, Core edge commit, and client query polling. They do not isolate worker execution time. The no-grant and installed idle samples come from sequential daemon processes with different command histories, so their working-set difference is an operational comparison rather than a precise cost attributed only to polling. The five-second windows are too short for a long-run idle budget, and the 16 ms process-CPU observation is timer-granular. The fixture has one tiny source and no creator application; it cannot establish large-project parser throughput, minimum/recommended hardware limits, or foreground-interference safety. The synthetic fixture worker remains test-only and is not a public distribution component.
