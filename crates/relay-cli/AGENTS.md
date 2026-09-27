# Purpose

Own the canonical RELAY command-line client.

# Ownership

- Governs `crates/relay-cli/`.
- Depends on relay-core contracts; it does not own business logic.

# Local Contracts

- Human-friendly commands and structured machine mode call the same daemon/Core command surface.
- The named-pipe client transport is exposed as a small library module for acceptance tests and future local clients; it contains no business logic.
- Machine mode never prompts.
- Output envelopes remain versioned and parseable.
- CLI must not maintain a second command catalog; discovery/help comes from the shared registry.
- No dashboard-only or CLI-only business capability.
- `relay dashboard-url` prints the daemon's current local dashboard link after confirming the host is reachable; the dashboard remains a client of the same Core commands.
- `relay parser-install` is an offline bootstrap maintenance path for a local parser package. It requires the daemon stopped, validates and copies a manifest and worker into RELAY state, then publishes a version-1 project grant with an explicit source-delivery opt-in. Normal parser dispatch still requires exact project configuration and is revalidated by the daemon on startup. This bootstrap path does not represent live command parity and must move behind a shared host operation before product beta.

# Verification

- Build/check `relay` as a standalone package so Windows API features are not supplied only by another workspace crate.
- structured status/doctor/command execution.
- malformed machine input fails without prompting.
- incompatible protocol/version errors remain explicit.
- daemon acceptance tests may reuse the exported client transport rather than duplicating the pipe protocol.
- `dashboard-url` fails when the daemon or current dashboard state is unavailable.
- Offline parser installation rejects missing opt-in, over-permission, digest mismatch, duplicate project grant, and a running host; the daemon accepts the resulting staged grant after restart.

# Child DOX Index

- No child DOX files currently exist.
