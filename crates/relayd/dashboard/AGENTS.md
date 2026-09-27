# Purpose

Own the static, local RELAY dashboard presentation.

# Ownership

- Governs `crates/relayd/dashboard/`.
- The daemon's `src/dashboard.rs` owns HTTP transport and command dispatch. These assets render returned command data only.

# Local Contracts

- Use the canonical command path through the daemon; do not read project files, SQLite, or other local state from browser code.
- Project add uses `project.import` against an existing local folder. Selection is browser-local view state; project actions pass an explicit ID through shared Core commands. Index build, archive, restore, and remove require user confirmation. Remove sends `confirm_project_id` and retains files and RELAY history.
- Guided setup orders existing shared commands as add, select, then check `project.capabilities` and build the local index when needed. Show one next action, explain unavailable host/folder/index states without raw errors, and hand off to Assets, Tests, and Diagnostics only when the local index is ready. UEFN connection guidance remains optional and never marks live editor or play-session workflows as passed.
- List active, archived, and removed project records through `project.list` with `include_inactive`. Archived projects remain selectable for restore or removal; removed records are display-only tombstones.
- Keep the primary view plain-language, responsive, and keyboard accessible. Advanced details expose only allowlisted health metadata; do not render raw command output, arguments, logs, paths, identifiers, or arbitrary error text.
- The Diagnostics & Debug section renders live `system.status`, `system.doctor`, and `diagnostics.summary` data through the shared command path. Component health must not be presented as an end-to-end workflow test result.
- Announce refresh results in the status region, keep Refresh keyboard focusable while loading, and remove stale health/project details when a refresh fails.
- Insert untrusted project names and details with text nodes, never HTML interpretation.
- The dashboard's URL fragment carries a per-start local token. Browser code sends it only in the local command header and removes it from the visible address after loading.
- The project inspection button invokes the shared `uefn.static.inspect` command and renders only bounded counts and capability limits.
- Recent activity and usage use `transaction.list` and `usage.summary` through the same read-only command path; render allowlisted command/state and aggregate counters, never arguments, actor IDs, or raw transaction records.
- Asset manifest selection stays in the local browser and sends only the selected bounded JSON to `assets.manifest.validate` or `assets.krita.inspect`. Bound the encoded HTTP body before sending it. Render counts, allowlisted finding labels, and visible lineage findings without file paths, raw contents, or asset IDs; native creator-app execution remains UNTESTED.
- Asset impact uses the same selected bounded manifest and one project-relative changed path through `assets.impact.analyze`; render only affected counts and allowlisted reason categories, never returned asset IDs or paths.
- Background watcher Pause/Resume uses shared `automation.pause` and `automation.resume` commands. Show `system.status.automation_mode` and make unavailable state explicit.
- Integrated report preview accepts only an explicitly selected local JSON file in the browser, limits its size, and renders allowlisted workflow IDs, statuses, reason codes, and derived counts. It never sends the report or a path to the daemon, and it labels file-provided outcomes unverified by the dashboard.
- The Tests view uses project-scoped `result.list` metadata for imported Verse capture analyses. It does not fetch payloads or describe imported analysis as a live UEFN pass.
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
