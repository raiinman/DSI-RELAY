# Purpose

Own the provisional Node.js Phase 1 runtime candidate, including host/CLI prototypes, storage/index/resource spike code, tests, and benchmark harnesses.

# Ownership

- This file governs `spikes/phase1/node/` and descendants unless a closer AGENTS.md exists.
- `spikes/phase1/AGENTS.md` remains authoritative for Phase 1 benchmark evidence and stack-selection rules.
- Passing work here does not lock Node as RELAY's final runtime.

# Local Contracts

- Prefer Node built-ins and platform capabilities before adding package dependencies; dependency cost must be justified by benchmark evidence.
- Keep human CLI text separate from structured machine contracts.
- Runtime state, temporary databases, generated executables, and benchmark scratch data stay outside the repository.
- Tests must cover failure/restart behavior when the changed code affects durability, IPC, indexing, or resource scheduling.
- Synthetic benchmark fixtures must not contain private project data.
- Do not hard-code personal paths, repository names, credentials, account identifiers, or one-project behavior.
- Platform-specific native behavior belongs behind explicit helpers/modules rather than leaking into generic command semantics.

# Work Guidance

- Keep spike code minimal and measurable rather than evolving it into the full Phase 2 product.
- Record benchmark artifacts under `spikes/phase1/results/`, not beside source.
- Treat high-end-workstation results as fixture-specific until hardware-tier evidence exists.

# Verification

- Run `node --test`.
- Run `node --check` on changed benchmark/CLI modules.
- Re-run any benchmark whose decision-affecting implementation changed.
- Scan for generated binaries and private paths before commit.

# Child DOX Index

- `windows/AGENTS.md` — owns Windows-native Phase 1 interoperability helpers and their safety/build rules.
