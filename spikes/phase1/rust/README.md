# Rust Phase 1 Local Core Foundation

D-153 selects this Rust implementation as the Phase 1 foundation for the RELAY per-user host, local IPC, embedded dashboard transport, and operational SQLite state. It is still a Phase 1 prototype rather than a release-ready product.

## What it proves

- per-user Windows process host
- protocol/version/capability handshake
- structured `system.status`, `system.doctor`, `system.echo`, and `system.shutdown`
- random per-start application auth token
- explicit protected Windows named-pipe DACL scoped to the current user
- kernel verification of the created pipe descriptor with `GetSecurityInfo`
- clean/hard restart behavior
- actual Node CLI ↔ Rust host and Rust CLI ↔ Node host interoperability
- same-process embedded static dashboard feasibility
- schema-version-1 SQLite project/result/job durability in WAL + `synchronous=FULL`
- hard-kill persistence, `quick_check`, damaged-store `Degraded` startup, and blocked writes
- future-schema rejection without downgrade
- bidirectional Node ↔ Rust schema-1 database compatibility
- `rusqlite` with defaults disabled and bundled SQLite as the selected operational-state binding
- one embedded JSON command registry using a bounded JSON Schema 2020-12 profile
- deterministic argument/result/error validation around the command dispatcher
- optional per-command contract versions with fail-closed incompatible-version handling
- registry-derived CLI catalog/help/describe, dashboard exposure, adapter/AI discovery, and command capability IDs
- synthetic on-demand out-of-process adapter broker/worker foundation with manifest validation, SHA-256 artifact verification, provenance, timeout/backoff/quarantine, and registry-bound commands
- query-verified Windows Job Object worker containment: kill-on-close, active-process limit 1, and synthetic process-memory cap

## What it intentionally does not prove yet

- concurrent-reader/writer and long-reader WAL/checkpoint behavior
- VACUUM/compaction and maintenance interruption
- disk-full injection or backup/restore
- future migration interruption beyond safe rejection of an unsupported future schema
- indexing/watch/reconciliation port into Rust
- evidence compression/blob storage port into Rust
- background job scheduling
- real UEFN/Blender/Krita adapters
- OS-enforced third-party worker filesystem/network/registry/local-IPC sandboxing beyond Job Object lifecycle/resource containment
- production HTTP concurrency, installer/update, or desktop packaging

## Run

```powershell
cargo test
cargo build --release
.\target\release\relay-rust-challenger.exe host
```

With the embedded Phase 1 dashboard:

```powershell
.\target\release\relay-rust-challenger.exe host --dashboard
```

In another shell using the same `RELAY_STATE_DIR`:

```powershell
.\target\release\relay-rust-challenger.exe status
.\target\release\relay-rust-challenger.exe doctor
.\target\release\relay-rust-challenger.exe commands
.\target\release\relay-rust-challenger.exe describe project.register 1
.\target\release\relay-rust-challenger.exe help
.\target\release\relay-rust-challenger.exe shutdown
```

Neutral comparisons are owned outside this candidate subtree. Use `../compare/spike7.mjs` for host/IPC evidence, `../compare/spike8.mjs` for operational SQLite parity/dependency economics, `../compare/spike9.mjs` for command-registry/IDL evidence, and `../compare/spike10.mjs` for adapter broker/isolation evidence rather than ad-hoc candidate-only timing.

## Security boundary

The host creates its named pipe with a protected DACL that grants Full Control only to the current user SID. It then asks Windows for the actual created object's security descriptor and fails startup unless the descriptor is protected, owned by the current user, contains exactly one ACE, and grants that user full control.

The random application token remains required as defense in depth.

## Current interpretation

D-153 selects Rust + bundled SQLite as the Phase 1 local core foundation. Spike 8 preserved the schema-1 project/result/job durability contract in both directions with the Node reference and retained a 92.23% post-storage idle-RSS reduction on the measured fixture.

D-154 selects `../contracts/commands.registry.json` plus the bounded JSON Schema 2020-12 validator as the semantic built-in command-contract source. The registry drives runtime validation and discovery metadata without adding another Rust dependency; breaking command changes require explicit command-version evolution rather than silent drift.

D-155 selects the synthetic adapter broker/manifest foundation, including on-demand workers and Job Object lifecycle/resource containment. It does not make the worker a security sandbox; OS-enforced adapter filesystem/network/process capability restrictions remain the next Phase 1 gate.

The selection does not make every Rust subsystem final. Storage maintenance/concurrency hardening, stronger adapter sandboxing, packaging/update, and the remaining Phase 1 foundations still require their own evidence.
