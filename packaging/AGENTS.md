# Purpose

Own Windows per-user package staging and the future signed-distribution verification contract for RELAY.

# Ownership

- Governs `packaging/` and descendants.
- Builds an unsigned, unpublished local staging archive from supplied release binaries and owns explicit local-development side-by-side install, rollback, and uninstall scripts. `Build-LocalTestPackage.ps1` wraps that stage with double-click per-user install and uninstall entry points. It does not sign or publish RELAY.

# Local Contracts

- Stage `relay.exe`, `relayd.exe`, `relay-gateway.exe`, MIT `LICENSE`, RAiiNMAN `NOTICE-RELAY.txt`, current dependency license texts for all three executable graphs, a bounded manifest, and a verifier. Stage only `skills/relay-core/SKILL.md`, `scripts/relay-core.ps1`, and `references/commands.generated.json` under the fixed `skills/relay-core/` payload path. Stage the fixed `plugins/dsi-relay-chat/` manifest, launcher, and README files for optional desktop Chat installation; no per-user marketplace or credential file belongs in the archive.
- Stage `Launch-RELAY.ps1` beside `relay.exe`; the per-user installer creates a Start menu shortcut that runs this script in a hidden PowerShell window. It invokes the installed CLI's `relay launch` command without capturing child output, so an engine inheriting a pipe cannot hold the launcher open. On failure it shows a short dialog directing the user to run the CLI for details. No development source path is required after installation.
- Reject missing/non-x64 binaries, personal profile strings in runtime binaries or skill files, reparse-point outputs, and any existing versioned artifact. Keep the installer archive's 512-entry, 256-character entry-path, and 256 MiB expansion bounds. Never overwrite another version or a running daemon.
- The package version must exactly equal the CLI, daemon, and gateway Cargo runtime versions so install health and reported version agree. A local-test label belongs in the outer wrapper name, not in a different executable version.
- Use sorted archive entries and fixed timestamps. Record payload hashes and unresolved signing, trust, and legal reviews explicitly.
- Keep generated local staging output under ignored `packaging/out/`; never commit or upload an unsigned archive as a public binary.
- The local test wrapper ZIP contains exactly one inner unsigned stage ZIP and its SHA-256 sidecar plus the install/uninstall scripts. Users extract the outer ZIP, double-click `Install-RELAY.cmd`, then open DSI RELAY from Start. `Uninstall-RELAY.cmd` verifies the active installed package, asks its matching running daemon to shut down gracefully, then removes the verified program and shortcut while preserving data and projects. It never kills an unrelated process.
- Local installation requires an explicit unsigned-development opt-in, expected archive digest, verified extraction, a bounded read-only probe of actual RELAY storage schema, declared schema compatibility, and no running daemon from the installed program root or selected data root before activation or uninstall. Unrelated development daemons using separate data roots do not block. The probe rejects WAL/journal sidecars rather than ignoring uncheckpointed data or creating sidecars. The optional caller schema value is only a consistency assertion.
- The double-click local installer/uninstaller path starts with inbox Windows PowerShell 5.1 and every helper they invoke must remain compatible with that runtime. Do not accidentally introduce PowerShell 7-only APIs into local install, update, rollback, storage-probe, activation-health, or uninstall helpers. The separate signed-distribution verifier may keep its explicit PowerShell 7.2+ requirement.
- Install/update/uninstall uses per-user side-by-side version directories, inactive staging, one active pointer, schema-aware rollback, and durable-data preservation. Never delete an unverified version directory or a path outside the explicit program root. Re-running the exact active version is an idempotent repair/no-op: it may refresh the observed storage schema but must not make `previous_version` point to the active version itself.
- Explicit rollback names the immediately previous version and its expected archive SHA-256, re-verifies current and target folders and receipts, probes the actual database through the active `relayd.exe`, requires a positive schema compatible with the target, and atomically replaces the pointer while the daemon is stopped. Existing-version activation rejects schema `0`.
- Fixture-mode activation launches the staged daemon after the pointer switch against the explicit disposable data root, checks its own PID/version plus status, doctor, and diagnostics through the staged CLI, then stops it before returning. A failed check restores the previous verified pointer only when the daemon is stopped and the actual post-launch schema remains compatible; first activation failure removes the new pointer. No fixture daemon may be left running. Non-fixture local activation and explicit rollback do not launch the daemon.
- `Verify-SignedDistribution.ps1` is a separate read-only PowerShell 7.2+ Windows release verifier for a detached SHA-256 catalog over an exact payload folder. It requires a trusted Authenticode signature, exact catalog digest, expected publisher subject and signer/root thumbprints from a separately reviewed policy, a current non-revoked code-signing chain, and warning-free timestamped Windows SDK SignTool verification. It must fail closed without a real certificate and never turn unsigned local staging into a trusted package.

# Work Guidance

- Do not treat unsigned staging as release approval or completed clean-account/live integration testing.
- Failed staging may leave a uniquely named directory for manual inspection; never delete an unverified path.

# Verification

- Run the folder verifier against a locally staged package when release binaries are available. Exercise install/update/rollback/uninstall only in a disposable fixture root with an explicit fixture data root. Probe refusal for missing update data, malformed storage, and caller/probe mismatch before pointer replacement. Check that fixture activation health always stops its child daemon and that a failed activation restores the prior verified pointer when the data schema remains compatible.
- Parse the local wrapper scripts and, when building a test ZIP, verify that it extracts with one archive and digest, installs to the per-user root, creates a working Start menu shortcut, and removes the program without deleting the data root.
- Parse the signed verifier and exercise only fail-closed unsigned fixtures until a reviewed CA-issued certificate, catalog, trusted root pin, and Windows SDK SignTool are available. A synthetic signature cannot prove public trust.

# Child DOX Index

- No child DOX files currently exist.
