# Purpose

Own the Rust Phase 1 runtime/IPC challenger used to test whether a lower-footprint Windows host materially improves RELAY's runtime, IPC-security, startup, and dashboard-hosting trade-offs.

# Ownership

- This file governs `spikes/phase1/rust/`.
- `spikes/phase1/AGENTS.md` remains authoritative for Phase 1 evidence and stack-selection rules.
- This challenger does not become production architecture merely by outperforming one Node benchmark.

# Local Contracts

- Keep scope to host/runtime/IPC fundamentals until Rust earns deeper porting.
- Required challenger surface: per-user host, structured handshake, `system.status`, `system.doctor`, `system.shutdown`, clean/hard restart, explicit Windows local-IPC access control, and optional embedded dashboard/static-shell feasibility.
- Use an explicit current-user Windows named-pipe security descriptor; do not rely on the default named-pipe DACL.
- Keep the random application-level auth token as defense in depth even when the OS DACL is explicit.
- Do not port SQLite, indexing, evidence lifecycle, or UEFN product features during this spike.
- Generated binaries and Cargo target output stay out of Git.
- Benchmark exact dependency/build cost, release binary size, startup, idle CPU/RAM, and p50/p95/p99 command latency.
- Preserve structured machine output; human CLI text is not a machine contract.

# Work Guidance

- Prefer a small synchronous implementation before introducing an async runtime.
- Keep Windows API use narrow and document why each unsafe boundary exists.
- Reuse the proven dashboard static assets rather than redesigning UI during a runtime bake-off.
- Treat build/dependency complexity as a measured cost alongside runtime performance.

# Verification

- Run `cargo test`.
- Build `--release`.
- Run the Windows Spike 7 benchmark against the current Node baseline.
- Verify no `target/`, EXE, PDB, credentials, or personal paths are staged.

# Child DOX Index

- No child DOX files currently exist under this folder.
