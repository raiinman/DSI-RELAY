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
- `relay launch` is the installed one-action entry point. It reuses a reachable per-user daemon or starts adjacent `relayd.exe` hidden, waits for a live authenticated dashboard link, and opens it in an Edge app window when Edge is installed, otherwise in the Windows default browser. It takes no project-specific arguments, requires no terminal or working-directory setup, exits after opening the app, and returns a failure code if startup or browser handoff fails.
- `relay discover` is an offline, read-only nearby project/tool candidate scan for first-run onboarding; it labels live integrations untested and is a bootstrap exception to shared command parity.
- `relay onboard <project-folder>` composes shared project import, initial baseline, project capability, and read-only indexed UEFN inspection commands into a compact first audit. It reports the project ID if any later command or transport step fails, and never treats a static UEFN marker as editor or runtime validation. Its optional JSON mode reports bounded index counts, capability gaps, and conditional UEFN findings without echoing the project root or indexed paths.
- `project-list --all`, `project-archive`, and `project-restore` invoke shared lifecycle commands. Removal uses `project-removal-plan/get/list/approve/reject` followed by `project-remove <project-id> <approval-id>`, invoking `project.remove@2`; the legacy v1 direct route fails `APPROVAL_REQUIRED`.
- `check-catalog-put` reads a bounded local JSON file and invokes the shared revisioned project catalog write; `check-catalog-get` and `plan-checks` use shared read commands. Plans report only `planned_not_run` checks and an explicit full-catalog fallback when selective coverage is uncertain.
- `check-add-file` reads the current revisioned catalog and appends one direct indexed-file presence check through the same shared catalog write. It requires a project-relative path and unique check ID, fails on a concurrent revision change, and does not claim a live filesystem or creator-app check.
- `run-checks` takes the project ID, after-generation, and exact reviewed plan ID, then invokes the shared keyed indexed-assertion execution command. Its PASS/FAIL statuses describe the guarded indexed snapshot only; native checks remain untested, and no arbitrary script/command is executed.
- `relay parser-install` is an offline bootstrap maintenance path for a local parser package. It requires the daemon stopped, validates and copies a manifest and worker into RELAY state, then publishes a version-1 project grant with an explicit source-delivery opt-in. Normal parser dispatch still requires exact project configuration and is revalidated by the daemon on startup. This bootstrap path does not represent live command parity and must move behind a shared host operation before product beta.

# Verification

- Build/check `relay` as a standalone package so Windows API features are not supplied only by another workspace crate.
- structured status/doctor/command execution.
- malformed machine input fails without prompting.
- incompatible protocol/version errors remain explicit.
- daemon acceptance tests may reuse the exported client transport rather than duplicating the pipe protocol.
- `dashboard-url` fails when the daemon or current dashboard state is unavailable.
- `launch` accepts only the exact loopback dashboard URL shape before handing it to Windows, and never logs that token-bearing URL itself. The current-user protected dashboard state remains the source of the URL.
- `diagnostics` invokes the shared bounded diagnostic summary command; `doctor` remains the repair-oriented health view.
- `support-bundle` writes a new, summary-only JSON file from shared status and diagnostics commands. Its optional bounded integrated-report input contributes checked counts and resource totals; absent input leaves the run field empty. It omits raw logs, paths, and command arguments.
- `result-list` and `job-list` invoke shared bounded metadata-only commands, with an optional project ID for scoped views.
- `context-compile` invokes the shared byte-budgeted multi-result compiler with optional exact source-qualified pointers and literal focus terms.
- `task-context` invokes shared `context.task.compile@1` with a fixed task kind, one project, optional explicit result and removal-approval IDs, exact required pointers, literal focus terms, and a serialized byte budget. It does not expand authority or treat local confirmation as verified human approval.
- `uefn-inspect` invokes the shared read-only static inspection command for a registered project.
- `uefn-audit` runs that same inspection then records its compact output through shared `result.put`, returning a durable result ID. It cannot claim live editor or runtime validation.
- `asset-validate` sends a bounded local manifest file through the shared project-scoped asset validation command.
- `asset-impact` sends a bounded local manifest and changed project-relative paths through the shared structural impact command; it prints only affected IDs, reason codes, and the explicit creator-app state.
- `blender-mesh-check` invokes the shared read-only native mesh command for one project-relative `.blend` file without accepting a caller-supplied executable or script.
- `blender-mesh-record` invokes the same bounded check and stores its compact result through shared `result.put` with kind `BLENDER_MESH_VALIDATION`, returning a durable result ID. A saved unavailable result retains `native_workflow_status: untested`; saving a result does not make the native check pass.
- `krita-inspect` sends that bounded manifest through the shared Krita declaration/link inspection command; native Krita execution remains untested.
- `krita-export` explicitly invokes the shared create-only KRA-to-PNG project-write command with two project-relative paths; it cannot select an executable or script, and returns a nonzero exit unless export and durable build recording both succeed. A successful response carries a `result_id` and source/export identity hashes that match its stored project-scoped build record.
- `krita-reconcile` invokes the shared command for one existing project-local KRA/PNG pair. It stores a candidate result ID after bounded file checks and leaves native origin unverified/untested; users need a fresh `krita-export` target for verified native execution.
- `uefn-discover` invokes an explicit localhost-only MCP capability probe. `verse-analyze` sends a bounded imported capture and optional assertion file through the shared analysis command; it never labels the import as a live UEFN pass.
- `uefn-toolsets` invokes the bounded discovery-only toolset command; it does not dispatch editor tools.
- `uefn-describe` summarizes one advertised toolset's input shape through the shared discovery-only command.
- `verse-record` records that imported analysis through shared `result.put` with an explicit imported kind and durable result ID; it still does not certify live gameplay.
- `verse-file-analyze` sends an active project ID, session ID, and project-relative `.log`/`.jsonl` path through shared `runtime.capture.file.analyze`; the daemon reads and bounds the file. `verse-file-record` stores only that compact analysis through shared `result.put` with kind `PROJECT_FILE_VERSE_CAPTURE_ANALYSIS`. Neither route imports raw file text into a CLI command payload or calls it verified live UEFN evidence.
- Offline parser installation rejects missing opt-in, over-permission, digest mismatch, duplicate project grant, and a running host; the daemon accepts the resulting staged grant after restart.

# Child DOX Index

- No child DOX files currently exist.
