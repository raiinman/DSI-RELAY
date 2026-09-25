# Purpose

Own Phase 1 stack-selection prototypes, benchmark fixtures, and measured evidence used to promote or reject implementation choices.

# Ownership

- This file governs `spikes/phase1/` and all descendants unless a closer AGENTS.md exists.
- `spikes/AGENTS.md` remains authoritative for shared spike rules.
- Durable product architecture remains owned by `docs/`; this subtree supplies evidence, not an alternate architecture.

# Local Contracts

- Keep each spike narrow enough to identify the decision it informs.
- A candidate implementation is not production code and does not become a final stack choice merely because it passes.
- Benchmark fixtures must be synthetic or sanitized; do not copy private project content into committed evidence.
- Store committed benchmark outputs under `spikes/phase1/results/` with runtime/tool versions, workload, measurements, pass/fail state, and limitations.
- Record architecture consequences in `docs/DECISION_LOG.md`; do not hide decisions only inside benchmark JSON.
- Failure/recovery behavior is part of the prototype contract.
- Preserve exact identifiers/numerical evidence through compression and result round trips.
- Expensive compression or maintenance work needs a measured break-even, not a storage-ratio-only justification.

# Work Guidance

- Prefer dependency-free/platform-provided capabilities first so dependency cost remains visible.
- Use synthetic fixtures that include compressible, repetitive, and incompressible cases.
- Keep large evidence out of the operational metadata database unless a benchmark explicitly tests that layout.
- Mark unresolved security, compatibility, hardware-tier, and foreground-interference questions in the result rather than inferring success.

# Verification

- Run `node --test` for the current Node candidate.
- Run the benchmark harness for any decision whose implementation or workload changed.
- Run `git diff --check`.
- Scan changed files for personal paths, credentials, tokens, and account identifiers before commit.

# Child DOX Index

- `compare/AGENTS.md` — owns neutral cross-runtime comparison harnesses and shared candidate measurements.
- `node/AGENTS.md` — owns the provisional Node.js runtime candidate, tests, benchmark harnesses, and platform-helper boundaries.
- `rust/AGENTS.md` — owns the lower-footprint Rust runtime/IPC challenger and its Windows host-security benchmark boundary.
