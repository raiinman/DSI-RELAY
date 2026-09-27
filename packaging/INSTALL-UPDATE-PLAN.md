# Windows per-user install/update plan

`Build-UnsignedStage.ps1` creates local review input only. The companion
`Install-LocalStage.ps1` and `Uninstall-LocalStage.ps1` exercise the side-by-side
path only with an explicit unsigned-development opt-in and expected archive
digest. They do not sign, publish, auto-download, launch the daemon, or clear
the public release gates.

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
   reparse-point defenses. Re-verify extracted bytes.
4. Require the per-user daemon to stop cleanly. If it is running or cannot be
   stopped, leave the current version active and defer activation.
5. Run a pre-activation health check against the staged binaries and a safe
   storage compatibility check. Avoid irreversible migrations before the new
   version is active and recoverable.
6. Atomically replace `current.json`, then launch the new daemon and check
   health. On failure, reactivate the last compatible verified version.

The local development script verifies the staged package and requires a caller
supplied observed storage schema (0 only for absent data) before activation.
It does not launch the daemon or prove that the observed schema came from the
actual database. Production must read schema and complete post-activation
health verification through trusted code before this becomes an updater.

## Rollback and uninstall

- Rollback only to a previously verified version whose declared schema range
  accepts the current data. A downgrade that cannot read the database is
  blocked; it must never silently alter or discard user data.
- Uninstall stops only the current user's daemon, removes the program pointer
  and the version directories owned by this installation, and leaves separately
  owned data and project files untouched.
- Concurrent versions and an interrupted update must resolve from the active
  pointer without guessing which executable to run.

## Open release gates

Publisher signature/trust bootstrap, automated delivery, clean-account
install/update/uninstall, schema migration and rollback, dependency notice
review, legal review, and the final integrated RELAY workflows remain open.
