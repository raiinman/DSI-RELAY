# Spike 14 benchmark notes

Final run: 2026-09-25.

JSONL default candidate:
- 3,984.77 events/s
- 0.0211 ms p50 write
- 3.5707 ms p50 sync at the 16-event durability window
- 3,735,997 retained bytes
- 4,710,400 bytes idle RSS
- 0 ms sampled CPU over five seconds

Immediate-sync control: 267.24 events/s and 3.7591 ms p50.

SQLite matched the 16-event window at 27,725.64 events/s and 0.405 ms p50 commit, but retained 4,247,552 bytes and required 91.1948 ms of explicit retention maintenance. Its idle RSS was 5,738,496 bytes.

ETW without a consumer retained zero bytes. Starting a durable session in the normal non-elevated user context was access denied.

Recovery removed an 11-byte torn JSONL tail, preserved the complete event, and reported zero invalid lines. Temporary detail mode used 4.1866x the bytes/event with a 1.1x p50 latency ratio.

Verification: 54 Rust tests passed, clean optimized release build completed in 33.171 seconds, and no resolved Cargo package was added.

Decision: D-159 selects bounded JSONL for the default local diagnostic record; ETW remains optional deep tracing.
