# Purpose

Own cross-runtime Phase 1 comparison harnesses that must evaluate implementation candidates with the same workload and measurement method.

# Ownership

- This file governs `spikes/phase1/compare/`.
- `spikes/phase1/AGENTS.md` remains authoritative for Phase 1 evidence rules.
- Candidate-specific implementation remains in its own Node/Rust subtree; this folder does not become another runtime.

# Local Contracts

- Use the same protocol requests, repetition counts, process metrics, restart faults, and dashboard HTTP method for every candidate being compared.
- Do not quietly compensate for one candidate's weakness with a candidate-specific benchmark path.
- Keep machine/user identifiers out of committed results; reduce ACL evidence to structural booleans/counts.
- Cross-runtime compatibility tests must use the actual candidate client where practical, not only a synthetic parser.
- Benchmark artifacts remain under `spikes/phase1/results/`.

# Work Guidance

- Prefer one comparison harness over separately reported candidate benchmarks when making a head-to-head decision.
- Record both performance and engineering costs: build time, binary/runtime size, dependency graph, unsafe/native boundary, and open feature-porting cost.

# Verification

- Run the comparison harness from a clean Git working state or record unrelated changes explicitly.
- Verify candidate processes and temporary state are cleaned up after the run.
- Run `git diff --check` and secret/personal-path scans before commit.

# Child DOX Index

- No child DOX files currently exist under this folder.
