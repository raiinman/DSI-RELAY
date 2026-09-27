# Purpose

Own local assembly and verification of RELAY distribution artifacts.

# Ownership

- Governs `release/` and descendants.
- This folder does not approve publication, signing, licensing, or release claims.

# Local Contracts

- A measurement preview uses release-built `relay.exe` and `relayd.exe` and the reviewed synthetic benchmark runner only. Do not bundle test fixture workers, private project content, credentials, or machine-specific paths.
- Include a versioned manifest, payload hashes, exact dependency inventory and available license files. Record unresolved review items explicitly.
- Pin the reviewed lockfile, per-crate license choices and notice-file fingerprint, and bundled SQLite source bytes/features. A dependency or source change reopens the matching technical review.
- Testers inspect and share reports manually; packaging must not add uploads or telemetry.
- Preserve deterministic file order and archive timestamps so identical inputs produce identical archive bytes on the same packaging runtime.
- Never overwrite an existing versioned artifact. Clean up only a validated staging directory created by the current run.

# Work Guidance

- Keep public-facing text plain and distinguish synthetic measurement from a supported product release.

# Verification

- Build a local review bundle from release binaries, verify its hashes, and smoke the contained runner.
- Confirm no personal path, username, machine name, token, or report data enters the archive.
- Compare the reviewed third-party notice fingerprint and SQLite provenance pin against the exact packaged graph before clearing technical review flags.

# Child DOX Index

- No child DOX files currently exist.
