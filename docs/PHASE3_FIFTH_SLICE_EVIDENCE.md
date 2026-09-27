# Phase 3 Windows notification delivery evidence

Status: PASS on synthetic Windows fixtures, 2026-09-26. Phase 3 remains active.

## Contract

The per-user daemon now runs a bounded recursive OS watcher for indexed local project roots. It shares one subscription when projects use the same canonical root. Events enter a 1,024-message bounded queue and are coalesced into at most 32 project-relative file paths per batch. The trusted watcher invokes the existing version-1 `project.index.apply_hints` command, so file events use targeted hashing and leave the project index `stale` until full reconciliation. The daemon defers watcher hashing while a foreground RELAY client command is active and skips automatic hashing of files above 64 MiB.

Directory/rename ambiguity, callback errors, notification overflow, oversized batches, unreadable or oversized files, and watch setup failures mark the affected index stale. A daemon restart marks every persisted project baseline stale before publishing readiness because notifications during downtime cannot be trusted. The stale marker is durable and blocks delta and dependency-edge reads until `project.index.reconcile` restores ready state. Explicit `verify_content: true` is available when continuity loss could hide a same-size/same-timestamp edit. Project source files are never changed by the watcher.

The watcher uses `notify` 8.2.0 (CC0-1.0). It added 23 resolved lockfile packages across supported target platforms. On this Windows fixture, the optimized daemon grew from 3,086,336 to 3,432,448 bytes (+346,112 bytes). The added dependency is justified by native OS event delivery, bounded callback handling, and cross-platform watcher API support; no polling full-tree scan runs while idle.

## Verification

- A live daemon fixture imported and baselined two projects sharing one filesystem root. A real file creation reached both project hint paths; later metadata reconciliation found no unprocessed file change. A directory event signaled uncertain continuity. A second fixture watched two distinct roots and confirmed an Alpha event left Bravo ready.
- A hard daemon kill and restart marked both persisted project indexes stale before client reads. Explicit content verification restored the first project and hashed its two files.
- The shared-root watcher fixture passed four consecutive runs after the duplicate-root and restart-key fixes. `cargo test --workspace` passed 72 non-documentation tests; `cargo build --release --workspace` passed.
- In a one-second idle sample with two projects and one watched root, daemon working set was 10,817,536 bytes and sampled process CPU time increased by 0 ms. This is one workstation sample, not a hardware-tier budget.

The old Phase 2/3 acceptance fixtures use a test-only switch to preserve their deterministic manual-hint scenarios; the new live fixture exercises the normal daemon watcher path.

## Remaining gates

Automatic foreground-safe scheduling of full reconciliation after continuity loss is still needed. The worker currently defers only while a RELAY client command is active; it does not detect a creator application's foreground workload. Large-file/burst fallback intentionally leaves state stale for explicit recovery. Long-run event storms, many distinct roots, supported hardware tiers, automatic dependency extraction, and the Phase 3 closure review remain open. D-149 and Phase 2 authority boundaries remain intact.
