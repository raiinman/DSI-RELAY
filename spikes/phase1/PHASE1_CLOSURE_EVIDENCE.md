# Phase 1 closure evidence

Phase 1 closed on 2026-09-25 after Spike 14 passed its final diagnostic-stack gate.

The selected stack is recorded through D-159. Required Phase 1 deliverables are present: relayd + CLI, structured command round trip, persistent result round trip, automated tests, startup/restart baselines, idle CPU/RAM baselines, and durable architecture decisions.

Spike 14 selected bounded structured JSONL as the default local diagnostic record. The final fixture measured 3,984.77 events/s and 0.0211 ms p50 writes with a 16-event durability window. SQLite was faster but added retention maintenance and retained more bytes; ETW was non-durable without a consumer and the non-elevated durable-session attempt was denied. Partial-tail recovery, bounded detail mode, 54 Rust tests, a clean release build, and unchanged resolved-package count passed.

This closes stack selection, not public-release hardening. Phase 2 is next.
