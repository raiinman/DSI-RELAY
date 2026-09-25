# Purpose

Own the selected Rust Phase 1 local core foundation and its evidence-backed parity spikes. Spike 8 adds operational SQLite state, durability, recovery, and dependency economics while Node remains the compatibility/reference implementation.

# Ownership

- This file governs `spikes/phase1/rust/`.
- `spikes/phase1/AGENTS.md` remains authoritative for Phase 1 evidence and stack-selection rules.
- D-153 selects this subtree as the Phase 1 local core foundation, but selection does not make it release-ready or waive the remaining Phase 1/release-hardening gates.

# Local Contracts

- Preserve the proven Spike 7 host/IPC contracts and Spike 8 operational-state contracts: project registry, durable result IDs, job checkpoints, migration metadata, SQLite integrity checks, degraded damaged-store startup, blocked writes while unavailable, and graceful/hard-kill persistence.
- Further subsystem ports into Rust require their own owning Phase 1 spike or implementation milestone; do not pull indexing, evidence lifecycle, resource scheduling, adapters, or UEFN feature work into unrelated changes.
- Keep SQLite operational metadata/compact results separate from heavyweight evidence; Spike 8 does not reopen the BLOB decision.
- Use an explicit current-user Windows named-pipe security descriptor; do not rely on the default named-pipe DACL.
- Keep the random application-level auth token as defense in depth even when the OS DACL is explicit.
- Generated binaries and Cargo target output stay out of Git.
- Benchmark exact SQLite dependency/license/build cost, release binary-size growth, startup/idle CPU/RAM, durable read/write/checkpoint latency, WAL/database growth, integrity/recovery behavior, and hard-kill persistence.
- Preserve structured machine output and schema compatibility; human CLI text is not a machine contract.
- Spike 9 command IDs, versions, summaries, effect/permission classes, surface visibility, and argument/result schemas come from `../contracts/commands.registry.json`; Rust may validate/derive from that source but must not maintain a competing metadata catalog.

# Work Guidance

- Prefer a small synchronous implementation before introducing an async runtime.
- Keep Windows API use narrow and document why each unsafe boundary exists.
- Keep `rusqlite` default features disabled and bundled SQLite explicit unless a later benchmark deliberately reopens that dependency decision.
- Reuse the proven dashboard static assets rather than redesigning UI during stack-selection work.
- Treat build/dependency complexity as a measured cost alongside runtime performance.

# Verification

- Run `cargo test`.
- Build `--release`.
- Run the neutral Windows comparison that owns the changed decision: Spike 7 for host/IPC or Spike 8 for operational storage.
- Verify no `target/`, EXE, PDB, credentials, personal paths, or user SID values are staged.

# Child DOX Index

- No child DOX files currently exist under this folder.
