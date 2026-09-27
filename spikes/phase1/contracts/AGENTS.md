# Purpose

Own Phase 1 machine-readable RELAY contract sources used across the selected Rust core and derived client surfaces.

# Ownership

- This file governs `spikes/phase1/contracts/`.
- `spikes/phase1/AGENTS.md` remains authoritative for Phase 1 evidence and stack-selection rules.
- These are contract-selection prototypes, not yet the public release contract.

# Local Contracts

- Keep one semantic command source of truth; do not duplicate command IDs, versions, argument/result schemas, effect classes, or discovery summaries across clients.
- Command IDs and per-command contract versions are stable once exposed by a committed registry artifact.
- Reserved and deprecated IDs must never be silently reused.
- Schemas use the declared JSON Schema dialect and only the validated RELAY subset until broader keyword support is explicitly added and tested.
- CLI, dashboard, adapter-discovery, and AI-discovery metadata must be derived from this registry rather than maintained separately.
- Deterministic request/result validation must not require an AI/model call.
- Keep registry discovery compact; clients should retrieve a specific command contract on demand instead of loading the full catalog.

# Work Guidance

- Prefer boring JSON that remains inspectable without specialized tooling.
- Keep descriptions concise because they become client/AI context.
- Additive optional fields are preferred over breaking shape changes.
- Treat registry-format evolution separately from individual command-version evolution.

# Verification

- Run Rust registry/contract tests.
- Run the neutral Spike 9 benchmark/compatibility harness.
- Verify malformed registries, reserved/deprecated collisions, invalid arguments/results, and incompatible command versions fail closed.

# Child DOX Index

- No child DOX files currently exist under this folder.
