# Purpose

Own the production per-user RELAY daemon and local transport boundary.

# Ownership

- Governs `crates/relayd/`.
- Depends on relay-core; core must never depend on this crate.

# Local Contracts

- Normal signed-in-user process; no Session 0 service default.
- Windows named pipe is current-user protected and still requires a per-start application token.
- The first named-pipe instance is exclusive for each user/`RELAY_INSTANCE` name; a second daemon must fail before replacing host or dashboard state.
- `host.json` contains that token. Create it with an explicit protected current-user ACL, verify the kernel owner and DACL before writing token bytes, and publish it from a same-directory temporary file.
- Protocol/version negotiation fails closed.
- Host state distinguishes process health from Core recovery health.
- Daemon records transport-safe diagnostics without persisting command arguments/results.
- Restart must preserve Core operational state.
- The daemon's recursive OS watcher feeds bounded file-path hints through Core and marks continuity loss stale. Startup marks persisted baselines stale before readiness; notification events never establish complete truth. Background hint hashing and recovery defer during commands with effects; read-only capability observations do not interrupt an active background scan.
- After a quiet period, the watcher attempts one stale-project recovery while the signed-in Windows session is idle. Recovery checks user input, active RELAY commands, and the callback event epoch during enumeration, hashing, and before commit; a callback during a scan must defer that scan's commit. Metadata-only recovery retains a 15-second attempt limit. Required full-content verification may continue past that limit while idle and quiet so a slow project is not permanently stranded; interrupted attempts retry after backoff. A failed watch subscription cannot restore ready state automatically.
- Shared `automation.pause`/`automation.resume` host-control commands toggle optional watcher hint application and idle recovery for the current daemon run. The watcher still observes bounded notifications and can mark an uncertain index stale; an active recovery cooperatively stops before commit. A failed or stopped watcher reports `unavailable`; restart begins unpaused. Foreground commands remain usable.
- The daemon loads explicit digest-pinned per-project parser grants from local state at startup. Only the exact configured installed parser with a source-delivery grant receives bounded UTF-8 indexed source bytes through the sandbox. Parser dispatch waits for idle, ready project state and writes edges through Core's generation, source, and configuration guards.
- Parser installation, quarantine, and recovery health is aggregated into live `system.status`/`system.doctor` output and fixed-code transition diagnostics without source text, paths, or project identifiers. Invalid grants fail closed for source delivery while daemon health remains inspectable. `host.json` recovery state is a startup snapshot; clients needing current health use the live commands.
- The dashboard binds to loopback with a per-start token, invokes allowlisted observe and local project-write commands through Core, and stores its local URL in a current-user-DACL-protected `dashboard.json` while the daemon runs. Origin, host, and token checks protect command dispatch; no dashboard-only capability bypasses Core. CLI prints that URL through `relay dashboard-url`.
- Project removal dispatch uses `project.remove@2` with a durable approved plan; old `project.remove@1` fails in Core. The dashboard can plan, inspect, decide, and execute through local-user authority. This same-user authority and token do not prove a human is present; nonlocal clients do not receive the approval permission by default.
- The dashboard may invoke the bounded `diagnostics.summary` observe command but cannot read raw diagnostic logs or local files.
- Dashboard activity and usage views invoke read-only `transaction.list` and `usage.summary`, displaying only safe summaries.
- Dashboard Assets uses bounded manifest bytes through existing asset analysis commands and renders only finding categories, counts, and lineage summaries. Dashboard Tests uses shared catalog get/put, plan, and explicit execute commands for declared indexed-file checks, plus project-scoped result metadata for imported capture analyses. The dashboard transport allowlist and shared registry surfaces must both admit these commands; indexed assertion outcomes never certify live UEFN or native creator-app workflows.
- Dashboard Assets can invoke the shared, project-scoped Blender mesh check and explicit Krita new-PNG export. The fixed native asset path gate rejects linked, escaping, and existing output paths; Krita uses local-user project-write authority and a keyed request so a successful export records the build. The browser shows only bounded status/count summaries and requires a confirmation before the export.
- The dashboard allowlist includes shared structural asset impact, explicit project index reconciliation, and background watcher pause/resume commands. The browser previews only an explicitly user-selected integrated report file, with no daemon filesystem read route.
- The daemon runs read-only UEFN static inspection as a validated Core extension over an authorized ready-index snapshot; it does not open editor files or claim editor/runtime connectivity.
- Local asset manifest validation runs as an authorized Core extension against a trusted project root, returns bounded finding summaries, and does not launch creator apps.
- Structural asset impact analysis runs through the same authorized project-root extension and returns deterministic IDs and reason codes. It permits missing or stale local files as change inputs and never exposes paths or manifest bytes in results.
- Blender mesh checking runs through an authorized project-relative path gate, rejects traversal and linked path components, then invokes the fixed bounded headless checker. Native unavailability and incomplete outcomes remain explicit; results omit paths and raw process output.
- Krita manifest inspection uses that same validated project scope and reports declared formats and links without launching Krita.
- Explicit CLI Krita export validates existing KRA and new PNG paths beneath the trusted project root, rejecting traversal and linked path components before a fixed native CLI invocation. It requires `project_write` plus `state_write`/`relay_state_write` authority before export. The same original authority invokes shared `result.put` after publication, producing a project-scoped `ASSET_KRITA_EXPORT_BUILD` record with file digests and hashed relative identities. No path, command output, or file bytes are returned. A post-publication record failure returns `ASSET_BUILD_RECORD_FAILED`, leaving the new PNG in place and marking the outer transaction failed.
- `assets.krita.reconcile@1` reads an existing project-local KRA/PNG pair through the bounded path gate and stores only a project-scoped recovery candidate with file digests. It never overwrites the PNG or claims verified native Krita origin; native proof requires a fresh export to a new path.
- `relayd.exe --probe-storage-schema --state-dir <absolute path>` inspects existing SQLite storage read-only without Core startup or migration, emits fixed schema/absence status without a path, and fails closed on damaged or unreadable storage. Unsigned local activation and rollback consume this probe before changing the active pointer.
- The local UEFN MCP probe is explicit and discovery-only; it uses the bounded localhost client, does not invoke editor tools, and reports live workflows untested. Caller-supplied Verse capture analysis returns compact counts/quality/assertions without raw log lines. The file-acquisition command accepts only a bounded linked-component-free `.log`/`.jsonl` under an authorized active project root, returns actual-byte digest/size with the compact analysis, and still labels live UEFN untested.
- Shared `tools.local.discover@1` reads bounded local installation metadata for UEFN, Blender, and Krita through the CLI and dashboard command paths. It launches no creator app, returns no paths, and cannot establish workflow success.
- Shared `codex.projects.discover@1` reads only the bounded private Codex `local-projects` cache to propose existing, plain local project folders. It skips unavailable or linked roots and ChatGPT project mirrors, never reads chats, auth, or sessions, and returns paths only to an authorized local caller. This best-effort cache is not a supported Codex API.
- The combined UEFN inspection command reports ready-index counts and independent local MCP discovery in separate fields. It never binds an MCP endpoint to the indexed project, exposes no indexed paths, and leaves editor identity and native entity/device/spawn/session observations untested.
- Toolset discovery invokes only advertised MCP discovery tools, returns bounded names without descriptions or raw server responses, and does not assert editor identity.
- The portable synthetic benchmark creates a new report path without overwriting an existing file, replaces later progress records atomically, bounds each CLI child process and kills it on deadline, records a fixed timeout code, and shuts down its daemon before removing only a validated temporary root without descendant reparse points.

# Verification

- Live daemon/client round trip through `tests/phase2_first_slice.rs`.
- wrong-token and incompatible-protocol/version rejection.
- graceful and hard-kill restart recovery with durable project/result/job verification.
- damaged operational storage starts Degraded without replacement.
- current-user pipe security verification.
- A second daemon cannot claim the same named pipe while the first holds it; the name is reusable after the first exits.
- Synthetic Phase 3 two-project import, baseline, reconciliation, privacy, resource, graceful-restart, and hard-restart coverage through `tests/phase3_first_slice.rs`.
- Synthetic Phase 3 change-delta, dependency-edge, provisional hint-update, stale-state, isolation, and hard-restart coverage through `tests/phase3_second_slice.rs`.
- Explicit full-content verification after a metadata-invisible edit through `tests/phase3_second_slice.rs`.
- Live OS notifications, shared-root handling, distinct-root isolation, directory uncertainty, idle resource sample, and hard-restart continuity through `tests/phase3_watcher.rs`.
- Ignored, manually invoked 15,000-file two-project scale fixture in `tests/phase3_watcher.rs`; record host class and avoid treating one run as a tier budget.
- The scale fixture measures watcher-attachment full verification separately from a subsequent clean metadata pass, so these costs are not conflated after schema 6.
- Live idle recovery after hard restart detects a same-size/same-timestamp edit without a manual reconciliation command; debug-only accelerated idle timing is limited to the test fixture.
- Live project-configuration transport test verifies versioned metadata and conflict behavior after a hard restart; parallel fixtures use distinct daemon instance names.
- Live watcher-load tests in `tests/phase3_watcher_load.rs` cover burst uncertainty, automatic verified recovery, separate-root isolation, and a quiet four-root idle sample. Its ignored manual fixtures cover a prolonged event soak, retention of the verification requirement when a callback arrives during recovery, and foreground-write interruption followed by idle retry.
- The watcher unit test verifies required full-content recovery remains eligible after the metadata deadline and still defers for a callback or lost idle state.
- Repeatable five-sample scale measurements use `tests/phase3_scale_report.ps1`; record host facts and distributions without treating a single host as minimum/recommended tier certification.
- Installed parser dispatch, Alpha/Bravo grant isolation, source reparse, target delete/restore, configuration revocation, and bad-digest rejection are covered by `tests/phase3_parser_dispatch.rs`.
- The same parser-dispatch suite verifies an offline staged package and explicit grant activate after daemon startup.
- `tests/phase3_parser_dispatch.rs` also verifies missing/malformed installation health, broker quarantine, fixed-code diagnostics, and repair/restart or source-change recovery.
- The ignored live installed-parser resource fixture in `tests/phase3_parser_resource.rs` measures initial publication, changed-source reparse, foreground latency, idle CPU, and resident memory on one host; `tests/phase3_parser_resource_report.ps1` records repeatable samples and source fingerprints without claiming a supported hardware tier.
- The draft portable runner in `tests/phase3_portable_benchmark.ps1` exercises a staged release daemon on another host; its smoke results are functional evidence until minimum/recommended hardware and creator-app contention are measured.
- The portable runner smoke should verify a successful staged-binary run, an existing report path left byte-identical without starting a daemon, and an induced CLI timeout that leaves a fixed-code failure report with no new daemon or fixture root.
- The portable runner reports fixed failure codes for daemon launch, early exit, readiness timeout, and CLI timeout without copying raw process output into its report.
- `src/dashboard.rs` unit tests and `tests/phase5_dashboard.rs` verify live HTTP token/origin rejection, scoped project writes, unrelated-write rejection, and parity with the named-pipe command path.

# Child DOX Index

- `dashboard/AGENTS.md` — owns static local dashboard presentation, plain-language rendering, and browser token handling.
