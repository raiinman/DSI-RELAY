# Purpose

Own the static, local RELAY dashboard presentation.

# Ownership

- Governs `crates/relayd/dashboard/`.
- The daemon's `src/dashboard.rs` owns HTTP transport and command dispatch. These assets render returned command data only.

# Local Contracts

- Use the canonical command path through the daemon; do not read project files, SQLite, or other local state from browser code.
- Keep the primary view plain-language, responsive, and keyboard accessible. Advanced details expose only allowlisted health metadata; do not render raw command output, arguments, logs, paths, identifiers, or arbitrary error text.
- The Diagnostics & Debug section renders live `system.status`, `system.doctor`, and `diagnostics.summary` data through the shared command path. Component health must not be presented as an end-to-end workflow test result.
- Announce refresh results in the status region, keep Refresh keyboard focusable while loading, and remove stale health/project details when a refresh fails.
- Insert untrusted project names and details with text nodes, never HTML interpretation.
- The dashboard's URL fragment carries a per-start local token. Browser code sends it only in the local command header and removes it from the visible address after loading.
- The project inspection button invokes the shared `uefn.static.inspect` command and renders only bounded counts and capability limits.
- Recent activity and usage use `transaction.list` and `usage.summary` through the same read-only command path; render allowlisted command/state and aggregate counters, never arguments, actor IDs, or raw transaction records.
- Asset manifest selection stays in the local browser and sends only the selected bounded JSON to `assets.manifest.validate` or `assets.krita.inspect`; the dashboard displays finding counts and makes creator-app limits explicit.
- Stored result summaries use `result.list` metadata only; never fetch payloads or provenance for the overview.
- UEFN connection checks are user-initiated discovery probes; the UI does not present an MCP response as verified editor or play-session behavior.
- No external assets, network services, telemetry, or browser storage.

# Verification

- `cargo test -p relayd --bin relayd dashboard::tests` covers HTTP command scope and token/origin rejection.
- `cargo test -p relayd --test phase5_dashboard` covers live daemon HTTP and pipe parity.
- `node --test crates/relayd/dashboard/app.test.mjs` covers dashboard rendering, current-state/error behavior, token handling, and the read-only command set without adding a browser runtime dependency.

# Child DOX Index

- No child DOX files currently exist.
