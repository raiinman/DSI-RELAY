# Purpose

Own the static, local RELAY dashboard presentation.

# Ownership

- Governs `crates/relayd/dashboard/`.
- The daemon's `src/dashboard.rs` owns HTTP transport and command dispatch. These assets render returned command data only.

# Local Contracts

- Use the canonical command path through the daemon; do not read project files, SQLite, or other local state from browser code.
- Project add uses `project.import` against an existing local folder. Selection is browser-local view state; project actions pass an explicit ID through shared Core commands. Index build, archive, and restore require user confirmation. Remove creates a durable project-scoped approval plan, renders its bounded summary, records approve or reject, then invokes `project.remove@2` with its approval ID. Project files and RELAY history remain.
- Guided setup orders existing shared commands as add, select, then check `project.capabilities` and build the local index when needed. Show one next action, explain unavailable host/folder/index states without raw errors, and hand off to Assets, Tests, and Diagnostics only when the local index is ready. UEFN connection guidance remains optional and never marks live editor or play-session workflows as passed.
- List active, archived, and removed project records through `project.list` with `include_inactive`. Archived projects remain selectable for restore or removal; removed records are display-only tombstones.
- Keep the primary view plain-language, responsive, and keyboard accessible. Advanced details expose only allowlisted health metadata; do not render raw command output, arguments, logs, paths, actor IDs, or arbitrary error text. Tests may show bounded declared check IDs and opaque `RES-` result IDs needed to review a selected run; other internal IDs remain hidden. The approval view shows fixed action/risk/validation/rollback copy and an allowlisted state; the approval ID stays internal to command calls.
- The Diagnostics & Debug section renders live `system.status`, `system.doctor`, and `diagnostics.summary` data through the shared command path. It shows at most 12 retained local event summaries from fixed labels, timestamps, known commands, and failure codes, never raw messages or request details. Component health and local events must not be presented as end-to-end workflow test results.
- Removal planning is a two-step inline review: create and focus a semantic plan region, then use its Approve and remove or Reject plan controls. Recent plans remain inspectable even on a removed project tombstone. A lost approval/removal response is announced as uncertain with a refresh/review step, never as a known failure or success. Dynamic plan lists regain keyboard focus; completed removal and errors focus the project status line. Plan IDs remain internal.
- Diagnostics guidance and integrated report outcomes use live status text. After a report file is selected, focus its bounded summary. Mobile layouts keep approval controls and diagnostic cells readable in one column. These scripted checks do not establish manual assistive-technology acceptance.
- Announce refresh results in the status region, keep Refresh keyboard focusable while loading, and remove stale health/project details when a refresh fails.
- Insert untrusted project names and details with text nodes, never HTML interpretation.
- The dashboard's URL fragment carries a per-start local token. Browser code sends it only in the local command header and removes it from the visible address after loading.
- The project inspection button invokes the shared `uefn.static.inspect` command and renders only bounded counts and capability limits.
- Recent activity and usage use `transaction.list` and `usage.summary` through the same read-only command path; render allowlisted command/state and aggregate counters, never arguments, actor IDs, or raw transaction records.
- Asset manifest selection stays in the local browser and sends only the selected bounded JSON to `assets.manifest.validate` or `assets.krita.inspect`. Bound the encoded HTTP body before sending it. Render counts, allowlisted finding labels, and visible lineage findings without file paths, raw contents, or asset IDs; native creator-app execution remains UNTESTED.
- Asset impact uses the same selected bounded manifest and one project-relative changed path through `assets.impact.analyze`; render only affected counts and allowlisted reason categories, never returned asset IDs or paths.
- Background watcher Pause/Resume uses shared `automation.pause` and `automation.resume` commands. Show `system.status.automation_mode` and make unavailable state explicit. If a control response fails or cannot confirm the requested state, disable the control until a status refresh resolves the uncertainty.
- Integrated report preview accepts only an explicitly selected local JSON file in the browser, limits its size, checks outcome counts and observed-pass consistency, and renders allowlisted workflow IDs, statuses, reason/diagnostic codes, component versions, timing, resource counters, local transport JSON-byte metrics, local context JSON-byte/time comparisons, bounded host/index sampling, reproducible step references, journal digests, and derived counts. Token savings, model quality, remote cost, supported-host budgets, and creator-app interference remain untested without their respective evidence. It never renders arbitrary fields, sends the report or a path to the daemon, or treats file-provided outcomes as independently verified.
- Diagnostics can download a bounded local JSON support summary assembled from fresh shared status, doctor, and diagnostic summaries. Match the CLI support bundle's summary-only privacy fields; add only bounded projected event labels/codes, component check IDs/status, and explicit native/UEFN/hardware UNTESTED context. A selected integrated report contributes only consistency-checked counts, reason codes, timing, and resource totals from at most 256 KiB of local input; it remains file-provided, not independently attested. Never copy raw report fields, log lines, paths, arguments, secrets, or arbitrary response objects into the download. Show a clear save or failure state.
- Tests offers a simple form to append one direct indexed-file presence check to the selected project's current catalog through revision-guarded `project.check_catalog.get/put`. It validates the check ID and project-relative path locally, preserves existing definitions, and requires a refresh after conflict, unavailable index, uncertain save, or an unconfirmed reread. Switching projects clears form inputs and ignores older responses. Saved paths never appear in summaries. Advanced JSON catalog selection remains available in the browser, bounds selected JSON to 24 KiB, and replaces the full catalog through the same revision guard. The view reloads `project.check_catalog.get`, shows revision and declared check IDs/assertion kinds without paths, then plans from the catalog's recorded index generation through `automation.checks.plan`. A full-catalog fallback remains `NOT RUN`; only a reviewed selective plan can be explicitly submitted to `automation.checks.execute`. Show saved result IDs and assertion outcomes, treat stale/uncertain execution as requiring refresh and replanning, and never equate an indexed-file assertion PASS with native or live UEFN behavior.
- The Tests view also uses project-scoped `result.list` metadata for imported Verse capture analyses. It does not fetch payloads or describe imported analysis as a live UEFN pass.
- Stored result summaries use `result.list` metadata only; never fetch payloads or provenance for the overview.
- Saved job summaries use `job.list` metadata only; never fetch checkpoints or provenance for the overview.
- UEFN connection checks are user-initiated discovery probes; the UI does not present an MCP response as verified editor or play-session behavior.
- No external assets, network services, telemetry, or browser storage.

# Verification

- `cargo test -p relayd --bin relayd dashboard::tests` covers HTTP command scope and token/origin rejection.
- `cargo test -p relayd --test phase5_dashboard` covers live daemon HTTP and pipe parity.
- `node --test crates/relayd/dashboard/app.test.mjs` covers dashboard rendering, current-state/error behavior, token handling, and allowlisted project actions without adding a browser runtime dependency.

# Child DOX Index

- No child DOX files currently exist.
