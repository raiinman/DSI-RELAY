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
- `relay discover` is an offline, read-only nearby project/tool candidate scan for first-run onboarding; it labels live integrations untested and is a bootstrap exception to shared command parity.
- `relay onboard <project-folder>` composes shared project import and initial baseline commands. It reports the project ID even if baseline construction fails, so a partial import is never hidden.
- `relay parser-install` is an offline bootstrap maintenance path for a local parser package. It requires the daemon stopped, validates and copies a manifest and worker into RELAY state, then publishes a version-1 project grant with an explicit source-delivery opt-in. Normal parser dispatch still requires exact project configuration and is revalidated by the daemon on startup. This bootstrap path does not represent live command parity and must move behind a shared host operation before product beta.

# Verification

- Build/check `relay` as a standalone package so Windows API features are not supplied only by another workspace crate.
- structured status/doctor/command execution.
- malformed machine input fails without prompting.
- incompatible protocol/version errors remain explicit.
- daemon acceptance tests may reuse the exported client transport rather than duplicating the pipe protocol.
- `dashboard-url` fails when the daemon or current dashboard state is unavailable.
- `diagnostics` invokes the shared bounded diagnostic summary command; `doctor` remains the repair-oriented health view.
- `support-bundle` writes a new, summary-only JSON file from shared status and diagnostics commands. It omits raw logs, paths, command arguments, and integrated-run claims until a real run exists.
- `uefn-inspect` invokes the shared read-only static inspection command for a registered project.
- `uefn-audit` runs that same inspection then records its compact output through shared `result.put`, returning a durable result ID. It cannot claim live editor or runtime validation.
- `asset-validate` sends a bounded local manifest file through the shared project-scoped asset validation command.
- `krita-inspect` sends that bounded manifest through the shared Krita declaration/link inspection command; native Krita execution remains untested.
- `uefn-discover` invokes an explicit localhost-only MCP capability probe. `verse-analyze` sends a bounded imported capture and optional assertion file through the shared analysis command; it never labels the import as a live UEFN pass.
- Offline parser installation rejects missing opt-in, over-permission, digest mismatch, duplicate project grant, and a running host; the daemon accepts the resulting staged grant after restart.

# Child DOX Index

- No child DOX files currently exist.
