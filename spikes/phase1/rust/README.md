# Rust Runtime / IPC Challenger

This is the Phase 1 lower-footprint Windows challenger for the RELAY per-user host. It is not yet the production runtime.

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

## What it intentionally does not prove yet

- SQLite operational-state parity
- result/checkpoint/migration durability
- indexing/watch/reconciliation
- evidence compression/blob storage
- background job scheduling
- UEFN/adapters
- production HTTP concurrency or desktop packaging

Those stay out until the runtime challenger earns them.

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
.\target\release\relay-rust-challenger.exe shutdown
```

The neutral Node-vs-Rust comparison is owned by `../compare/spike7.mjs`; use that artifact rather than ad-hoc candidate-only timing when making runtime decisions.

## Security boundary

The host creates its named pipe with a protected DACL that grants Full Control only to the current user SID. It then asks Windows for the actual created object's security descriptor and fails startup unless the descriptor is protected, owned by the current user, contains exactly one ACE, and grants that user full control.

The random application token remains required as defense in depth.

## Current interpretation

Spike 7 promotes Rust to the preferred candidate for deeper parity because it materially improves host/dashboard footprint, startup/CLI latency, restart time, and explicit Windows IPC security on the measured fixture.

That is not a final language lock. Spike 8 must test whether the advantage survives durable SQLite state and its dependency/build economics.
