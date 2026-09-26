# Purpose

Own production Rust implementation promoted from Phase 1 evidence.

# Ownership

- This file governs `crates/` unless a closer AGENTS.md exists.
- Root AGENTS.md and the Phase 2 authority docs remain binding.
- Spike code is evidence/reference only; production code must be promoted deliberately.

# Local Contracts

- RELAY Core owns deterministic business logic and must not depend on a specific AI provider, dashboard, MCP client, or game/tool integration.
- CLI, daemon transport, dashboard, gateways, adapters, and AI clients must converge on the same command contracts.
- Keep project-specific and UEFN-specific behavior outside production core.
- Keep secrets out of model-visible/command payloads; credential handles belong behind later broker interfaces.
- Machine mode is structured and non-interactive.
- Every durable write has explicit schema/version implications and failure/restart behavior.
- Operational storage and diagnostics health remain independent.
- Do not silently copy spike shortcuts; preserve only evidence-backed contracts and rewrite implementation where production boundaries differ.
- No personal paths, private repository names, account identifiers, secrets, or machine-specific defaults in distributable source/docs.

# Work Guidance

- Keep crate boundaries narrow and dependency direction one-way: clients/hosts depend on core contracts, never the reverse.
- Prefer standard library plus already selected dependencies unless a new dependency earns its cost.
- Prefer explicit typed envelopes over ad-hoc JSON maps inside production code.
- Add migration/compatibility tests whenever a promoted schema changes.

# Verification

- Run workspace tests.
- Run release build for affected binaries.
- Run focused restart/damage/version tests for promoted persistence/transport changes.
- Run resource checks against the Phase 1 baseline order of magnitude before promoting a slice.

# Child DOX Index

- `relay-contracts/AGENTS.md` — owns versioned command envelopes, the shared command registry, deterministic schema validation, and client-safe discovery metadata.
- `relay-core/AGENTS.md` — owns deterministic service logic, SQLite state, durable project/result/job records, idempotency, provenance/trust, and diagnostics.
- `relay-adapter/AGENTS.md` — owns generic adapter manifests, broker lifecycle, and qualified Windows worker isolation.
- `relay-cli/AGENTS.md` — owns the canonical human/machine CLI client.
- `relayd/AGENTS.md` — owns the per-user daemon process, Windows local IPC, startup/restart state, and transport boundary.
