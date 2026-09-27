# Phase 4 Context Gauntlet: first deterministic fixture

Status: a bounded local comparison, not Phase 4 closure or a model benchmark.

Run `cargo test -p relay-core --test context_gauntlet -- --nocapture` from the repository root. The test writes each synthetic payload as a durable result, reads the full payload through `result.get`, reads the compact body through `result.context` with a 1,024-byte budget, asserts expected exact JSON Pointer/value pairs, and prints one JSON measurement per fixture. It repeats each context read and checks byte-for-byte equality. The temporary database is removed after each case. No network, model, tokenizer, or paid service is used.

The compared byte counts are UTF-8 serialized JSON for the **stored payload alone** and the **`result.context` body alone**. The complete `result.get` response envelope, command framing, transport overhead, and model tokens are excluded. These figures describe this fixed synthetic workload, not typical user results.

| Fixture | Full payload bytes | Context body bytes | Exact facts asserted | Finding |
| --- | ---: | ---: | ---: | --- |
| Target at start of 101 entries | 3,515 | 388 | 2 | Target ID and generation retained. |
| Target in middle of 101 entries | 3,515 | 390 | 2 | Same exact values retained. |
| Target at end of 101 entries | 3,515 | 392 | 2 | Same exact values retained. |
| Stale then current state | 204 | 553 | 4 | Old, stale, and current state facts remain distinguishable by pointer; compact body is larger than full payload. |
| 600 repeated warnings | 18,044 | 1,009 | 1 | Target ID retained; 11 repeated warning codes selected, 589 scalar values omitted. There is no deduplication. |
| Coordinate in noisy logs | 2,365 | 331 | 1 | Target ID retained; all three coordinate numbers omitted by the current allowlist. |

These numbers were observed on two consecutive Windows runs of the command above; both produced identical JSON measurements. The assertions are the durable, repeatable evidence. The placement entries contain prose noise; they do not test competition against hundreds of higher-priority allowlisted facts. The stale-state case checks exact fact retention, not whether a model or client will choose the latest state. The coordinate case is a known retention failure for task-critical but non-allowlisted fields. A truncated context must fetch the full result by ID whenever omitted fields matter.

Next work: task-aware exact-field selection, stale-state resolution, repeated-fact deduplication, harder placement competition, and a model/client benchmark with real token, recall, success, latency, and cost measures. Any remote use still requires the separate data-egress decision.
