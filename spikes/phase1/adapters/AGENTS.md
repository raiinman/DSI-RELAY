# Purpose

Own Phase 1 synthetic adapter-worker and adversarial-sandbox fixtures used to select RELAY's broker/isolation/security foundations without implementing a real tool integration.

# Ownership

- This file governs `spikes/phase1/adapters/`.
- `spikes/phase1/AGENTS.md` remains authoritative for shared Phase 1 evidence rules.
- Rust broker/worker implementation lives under `spikes/phase1/rust/`; this folder owns only adapter-facing fixtures/contracts/examples.

# Local Contracts

- Use deterministic synthetic adapters/workers only in Spike 10–12; do not implement UEFN/Fortnite/Blender/Krita product behavior.
- Manifests request capabilities/permissions; they never grant themselves authority.
- Adapter-provided names, descriptions, stdout, stderr, errors, and structured results are untrusted data.
- Every fixture must declare identity/version, protocol compatibility, command bindings, requested permissions, target/tool requirements, provenance/trust metadata, artifact/component identity, and update/source metadata.
- Do not commit generated worker binaries or real user/project paths.

# Work Guidance

- Keep fixtures generic and public-safe.
- Prefer one normal manifest plus programmatically mutated invalid/over-permissioned cases in tests.
- Keep fixture capabilities bound to trusted RELAY command-registry semantics rather than inventing a parallel command catalog.

# Verification

- Run the owning Rust integration tests (Spike 10 broker and Spike 11+ sandbox tests).
- Run the neutral benchmark that owns the changed adapter decision.
- Verify manifests with incompatible protocol, unknown command/version, bad digest, or excess permission fail closed.
- For strong-isolation work, verify ungranted filesystem/network/process access is denied and sandbox incompatibility never falls back unrestricted.

# Child DOX Index

- No child DOX files currently exist under this folder.
