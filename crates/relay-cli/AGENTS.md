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
- `project-list --all`, `project-archive`, `project-restore`, and `project-remove` invoke shared lifecycle commands. Removal supplies the exact project-ID confirmation guard.
- `relay parser-install` is an offline bootstrap maintenance path for a local parser package. It requires the daemon stopped, validates and copies a manifest and worker into RELAY state, then publishes a version-1 project grant with an explicit source-delivery opt-in. Normal parser dispatch still requires exact project configuration and is revalidated by the daemon on startup. This bootstrap path does not represent live command parity and must move behind a shared host operation before product beta.

# Verification

- Build/check `relay` as a standalone package so Windows API features are not supplied only by another workspace crate.
- structured status/doctor/command execution.
- malformed machine input fails without prompting.
- incompatible protocol/version errors remain explicit.
- daemon acceptance tests may reuse the exported client transport rather than duplicating the pipe protocol.
- `dashboard-url` fails when the daemon or current dashboard state is unavailable.
- `diagnostics` invokes the shared bounded diagnostic summary command; `doctor` remains the repair-oriented health view.
- `support-bundle` writes a new, summary-only JSON file from shared status and diagnostics commands. Its optional bounded integrated-report input contributes checked counts and resource totals; absent input leaves the run field empty. It omits raw logs, paths, and command arguments.
- `result-list` and `job-list` invoke shared bounded metadata-only commands, with an optional project ID for scoped views.
- `context-compile` invokes the shared byte-budgeted multi-result compiler with optional exact source-qualified pointers and literal focus terms.
- `uefn-inspect` invokes the shared read-only static inspection command for a registered project.
- `uefn-audit` runs that same inspection then records its compact output through shared `result.put`, returning a durable result ID. It cannot claim live editor or runtime validation.
- `asset-validate` sends a bounded local manifest file through the shared project-scoped asset validation command.
- `asset-impact` sends a bounded local manifest and changed project-relative paths through the shared structural impact command; it prints only affected IDs, reason codes, and the explicit creator-app state.
- `blender-mesh-check` invokes the shared read-only native mesh command for one project-relative `.blend` file without accepting a caller-supplied executable or script.
- `krita-inspect` sends that bounded manifest through the shared Krita declaration/link inspection command; native Krita execution remains untested.
- `krita-export` explicitly invokes the shared create-only KRA-to-PNG project-write command with two project-relative paths; it cannot select an executable or script, and returns a nonzero exit when no PNG was published.
- `uefn-discover` invokes an explicit localhost-only MCP capability probe. `verse-analyze` sends a bounded imported capture and optional assertion file through the shared analysis command; it never labels the import as a live UEFN pass.
- `uefn-toolsets` invokes the bounded discovery-only toolset command; it does not dispatch editor tools.
- `uefn-describe` summarizes one advertised toolset's input shape through the shared discovery-only command.
- `verse-record` records that imported analysis through shared `result.put` with an explicit imported kind and durable result ID; it still does not certify live gameplay.
- Offline parser installation rejects missing opt-in, over-permission, digest mismatch, duplicate project grant, and a running host; the daemon accepts the resulting staged grant after restart.

# Child DOX Index

- No child DOX files currently exist.
