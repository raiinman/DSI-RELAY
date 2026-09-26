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

# Verification

- structured status/doctor/command execution.
- malformed machine input fails without prompting.
- incompatible protocol/version errors remain explicit.
- daemon acceptance tests may reuse the exported client transport rather than duplicating the pipe protocol.

# Child DOX Index

- No child DOX files currently exist.
