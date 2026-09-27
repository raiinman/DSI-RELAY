# Windows per-user install/update plan

`Build-UnsignedStage.ps1` creates local review input only. The companion
`Install-LocalStage.ps1`, `Rollback-LocalStage.ps1`, and
`Uninstall-LocalStage.ps1` exercise the side-by-side path only with an explicit
unsigned-development opt-in. Install and rollback additionally require an
expected archive digest. They do not sign, publish, auto-download, or clear
the public release gates. Fixture-mode install/update activation runs a bounded
daemon health check against an explicit disposable data root and stops it.

For a Windows user test, `Build-LocalTestPackage.ps1` wraps a fresh unsigned
stage in one ZIP. Extract it, double-click `Install-RELAY.cmd`, and launch
`DSI RELAY` from Start. The shortcut starts `relay launch` with a hidden
PowerShell window; failures appear in a Windows dialog. The program opens its
local workspace view in a browser window. The extracted folder also has
`Uninstall-RELAY.cmd`; it verifies the installed program, asks its matching
running daemon to shut down gracefully, and removes the program and shortcut
while preserving RELAY data and projects. This remains an unsigned local test path,
with no publisher trust or automatic update claim.

The unsigned stage contains the CLI, daemon, optional local MCP gateway, and
the compact `skills/relay-core/` runtime skill files. Its manifest and folder
verifier require all three Windows x64 binaries and exactly the skill's
`SKILL.md`, `scripts/relay-core.ps1`, and
`references/commands.generated.json` under that path. Third-party notices
follow all three executable dependency graphs. The gateway remains a local
adapter; staging it does not establish remote client compatibility.

## Signed release verification contract

A future public package may place all distributable files in one `payload/`
directory and a separately signed `payload.cat` beside it. Build the catalog
with Windows `New-FileCatalog -CatalogVersion 2.0` over the completed payload,
then sign and RFC 3161 timestamp the catalog with a reviewed CA-issued code
signing certificate. `Verify-SignedDistribution.ps1` is a separate read-only
gate for that layout; it does not sign or install anything.
It requires PowerShell 7.2 or newer on Windows.

Release policy must supply the exact SHA-256 digest of `payload.cat`, signer
certificate subject and thumbprint, and trusted CA root thumbprint from an
independent reviewed source, not values copied from the downloaded package.
The verifier rejects reparse points and missing core files, checks every
payload file against the SHA-256 catalog with `Test-FileCatalog`, checks the
catalog's trusted Authenticode publisher, builds a current online-revocation
code-signing chain to the pinned root, and requires a Microsoft-signed Windows
SDK SignTool `verify /pa /all /tw` exit code of zero. Warnings fail the gate.
This verifies the folder before any archive or installer activation; the
archive or installer must separately bind to the verified folder digest and
repeat verification after extraction. GitHub artifact attestations can add
provenance but do not replace Windows Authenticode trust.

The certificate, signer/root pins, signed catalog, timestamp service, trusted
delivery metadata, and release installer path are not yet available. The
signed verifier has no passing real-certificate fixture and cannot certify
public distribution at this stage. A new valid signature may still trigger
Windows SmartScreen reputation warnings during early distribution.

Microsoft references: [Test-FileCatalog](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.security/test-filecatalog?view=powershell-7.6), [SignTool](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool), [Get-AuthenticodeSignature](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.security/get-authenticodesignature?view=powershell-7.6), and [SmartScreen reputation](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation).

## Install root

- Use one per-user program root under `%LOCALAPPDATA%\Programs\DSI-RELAY`.
- Keep immutable package versions in `versions/<version>/`; never replace files
  of an active or older version in place.
- Keep application databases, project configuration, results, and diagnostics
  under a separately owned per-user data root. Uninstall must leave that data
  intact unless the user separately requests its removal.
- Keep one `current.json` pointer containing the active version, package hash,
  supported storage schema range, and channel. Write and flush a replacement
  file, then atomically replace the pointer only after verification.

## Install or update

1. Obtain the package over a trusted channel. Verify a trusted publisher
   signature over the archive digest and verify every packaged file hash.
   Unsigned staging packages cannot pass this production gate. The local
   development script instead requires `-AllowUnsignedLocalDevelopment` and
   an explicit expected SHA-256 digest, and records that channel in its pointer.
2. Confirm Windows architecture, version, free space, package version, and
   storage schema compatibility before touching the active pointer.
3. Extract into a new versioned inactive directory, using path traversal and
   reparse-point defenses. Re-verify extracted bytes and retain the installed
   manifest digest in that version's receipt.
4. Require the per-user daemon to stop cleanly. If it is running or cannot be
   stopped, leave the current version active and defer activation.
5. Run a pre-activation health check against the staged binaries and a safe
   storage compatibility check. Avoid irreversible migrations before the new
   version is active and recoverable.
6. Atomically replace `current.json`, then launch the new daemon and check
   health. On failure, reactivate the last compatible verified version.

The local development script verifies the staged package, stops activation if
`relayd.exe` is running, then invokes the staged binary's bounded read-only
schema probe against `%LOCALAPPDATA%\DSI\RELAY\relay.sqlite3`. The probe
does not create or migrate a database and fails on unreadable or damaged data.
Uncheckpointed WAL/journal sidecars also block activation; the operator must
recover and cleanly close the database before retrying.
Schema 0 is accepted only for a first activation with no active version.
Replacement activation requires a positive actual schema within the target
package's declared range. `-ObservedStorageSchema` remains optional and must
equal the probed value when supplied. Fixture activations require an explicit
fixture data root. A custom data root is unavailable outside fixture mode;
activation refuses an ambient `RELAY_STATE_DIR` override. After switching the
pointer in fixture mode, `Test-LocalActivationHealth.ps1` launches the staged
daemon in that data root with a unique local instance, waits at most 12 seconds
for its own host record, asks the staged CLI for status, doctor, and diagnostics
with an eight-second bound per command, and stops the daemon before returning.
The installer checks the post-launch storage schema and records it in the
pointer. To keep automatic reversal schema-aware, fixture updates require the
new package's maximum declared schema to fit the previous version's declared
range. On a failed health check, it restores the previous verified pointer if
the daemon has stopped and the actual schema is still compatible. A failed
first activation removes the new pointer. The version files and separately
owned data are retained for inspection; an unclean or incompatible database
requires manual recovery and blocks automatic pointer restoration. Non-fixture
activation remains pointer-only. Production still needs post-activation health
verification and a trusted signed delivery path.

## Rollback and uninstall

- Rollback only to a previously verified version whose declared schema range
  accepts the current data. A downgrade that cannot read the database is
  blocked; it must never silently alter or discard user data.
- `Rollback-LocalStage.ps1` requires explicit unsigned-development opt-in, the
  exact immediately previous version and its expected archive SHA-256. It
  refuses a running daemon, re-verifies the active and target folders and
  receipts, then probes actual storage through the active binary. Rollback
  requires a positive schema within the target package's declared range;
  an optional caller schema must match the probe. It then atomically switches
  `current.json`. It never launches the daemon or alters the data root.
- Local-development uninstall requires the daemon to be stopped; it does not
  terminate any process. It removes only the program pointer and verified
  version directories owned by this installation, leaving separately owned
  data and project files untouched.
- Concurrent versions and an interrupted update must resolve from the active
  pointer without guessing which executable to run.
- A single-version uninstall refuses the active version and its current
  rollback target. `-AllVersions` validates every version before removing the
  owned program inventory; project files and separately owned RELAY data stay
  untouched.

## Open release gates

Publisher signature/trust bootstrap, automated delivery, clean-account
install/update/uninstall, schema migration and rollback, dependency notice
review, legal review, and the final integrated RELAY workflows remain open.
