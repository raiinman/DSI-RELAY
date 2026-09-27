# Purpose

Own Phase 1 technical spikes, benchmark harnesses, and measured evidence used to select RELAY's durable implementation stack.

# Ownership

- This file governs `spikes/` and all descendants unless a closer AGENTS.md exists.
- Root AGENTS.md remains authoritative for project-wide DOX and product contracts.

# Local Contracts

- Spikes test approved architecture assumptions; they do not silently redefine Phase 0 contracts.
- Keep prototypes minimal, disposable, and explicitly non-production until a decision is approved in `docs/DECISION_LOG.md`.
- Record exact runtime/tool versions, workload, repetitions, resource measurements, failures, and limitations for benchmark evidence.
- Machine-facing spike interfaces use structured versioned data; human text is never the automation contract.
- Never embed personal paths, credentials, private repository names, or machine-specific defaults in distributable prototype code.
- Runtime state and benchmark scratch data must live outside the repository unless intentionally captured as sanitized evidence.
- Do not add UEFN feature work unless a spike specifically requires it to test an architectural assumption.

# Work Guidance

- Prefer standard-library implementations first so dependency cost is visible.
- Keep candidate-specific code isolated under `spikes/phase1/`.
- A passing demo is not enough: test failure/restart behavior and measure resource cost.
- Mark security or compatibility gaps directly in results instead of hiding them.

# Verification

- Run the spike's automated tests.
- Run its benchmark harness on Windows when the decision concerns the Windows host.
- Re-check changed paths against root and this DOX contract before closeout.

# Child DOX Index

- `phase1/AGENTS.md` — owns Phase 1 stack-selection prototypes, benchmark fixtures/results, and candidate-specific verification.
