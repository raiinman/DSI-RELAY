# Purpose

Own Windows per-user package staging and the future signed-distribution verification contract for RELAY.

# Ownership

- Governs `packaging/` and descendants.
- Builds an unsigned, unpublished local staging archive from supplied release binaries and owns explicit local-development side-by-side install, rollback, and uninstall scripts. It does not sign or publish RELAY.

# Local Contracts

- Stage `relay.exe`, `relayd.exe`, `relay-gateway.exe`, MIT `LICENSE`, RAiiNMAN `NOTICE-RELAY.txt`, current dependency license texts for all three executable graphs, a bounded manifest, and a verifier. Stage only `skills/relay-core/SKILL.md`, `scripts/relay-core.ps1`, and `references/commands.generated.json` under the fixed `skills/relay-core/` payload path.
- Reject missing/non-x64 binaries, personal profile strings in runtime binaries or skill files, reparse-point outputs, and any existing versioned artifact. Keep the installer archive's 512-entry, 256-character entry-path, and 256 MiB expansion bounds. Never overwrite another version or a running daemon.
- Use sorted archive entries and fixed timestamps. Record payload hashes and unresolved signing, trust, and legal reviews explicitly.
- Keep generated local staging output under ignored `packaging/out/`; never commit or upload an unsigned archive as a public binary.
- Local installation requires an explicit unsigned-development opt-in, expected archive digest, verified extraction, a bounded read-only probe of actual RELAY storage schema, declared schema compatibility, and stopped `relayd.exe` before activation or uninstall. The probe rejects WAL/journal sidecars rather than ignoring uncheckpointed data or creating sidecars. The optional caller schema value is only a consistency assertion.
- Install/update/uninstall uses per-user side-by-side version directories, inactive staging, one active pointer, schema-aware rollback, and durable-data preservation. Never delete an unverified version directory or a path outside the explicit program root.
- Explicit rollback names the immediately previous version and its expected archive SHA-256, re-verifies current and target folders and receipts, probes the actual database through the active `relayd.exe`, requires a positive schema compatible with the target, and atomically replaces the pointer while the daemon is stopped. Existing-version activation rejects schema `0`; neither path launches or health-checks the daemon.
- `Verify-SignedDistribution.ps1` is a separate read-only PowerShell 7.2+ Windows release verifier for a detached SHA-256 catalog over an exact payload folder. It requires a trusted Authenticode signature, exact catalog digest, expected publisher subject and signer/root thumbprints from a separately reviewed policy, a current non-revoked code-signing chain, and warning-free timestamped Windows SDK SignTool verification. It must fail closed without a real certificate and never turn unsigned local staging into a trusted package.

# Work Guidance

- Do not treat unsigned staging as release approval or completed clean-account/live integration testing.
- Failed staging may leave a uniquely named directory for manual inspection; never delete an unverified path.

# Verification

- Run the folder verifier against a locally staged package when release binaries are available. Exercise install/update/rollback/uninstall only in a disposable fixture root with an explicit fixture data root. Probe refusal for missing update data, malformed storage, and caller/probe mismatch before pointer replacement.
- Parse the signed verifier and exercise only fail-closed unsigned fixtures until a reviewed CA-issued certificate, catalog, trusted root pin, and Windows SDK SignTool are available. A synthetic signature cannot prove public trust.

# Child DOX Index

- No child DOX files currently exist.
