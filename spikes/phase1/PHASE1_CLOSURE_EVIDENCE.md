# Phase 1 closure evidence

Phase 1 closed on 2026-09-25 after Spike 14 passed its final diagnostic-stack gate.

The selected stack is recorded through D-159. Required Phase 1 deliverables are present: relayd + CLI, structured command round trip, persistent result round trip, automated tests, startup/restart baselines, idle CPU/RAM baselines, and durable architecture decisions.

Spike 14 selected bounded structured JSONL as the default local diagnostic record. The reconciled exact-tree rerun measured 3,685.9 events/s, 0.0196 ms p50 append latency, and 3.8777 ms p50 sync-boundary latency with a 16-event durability window. SQLite was faster but added retention maintenance and retained more bytes; ETW was non-durable without a consumer and the non-elevated durable-session attempt was denied. Partial-tail recovery, bounded detail mode, 54 Rust tests, the optimized release build, 17 Node reference tests, and unchanged resolved-package count passed.

This closes stack selection, not public-release hardening. Phase 2 is next.
