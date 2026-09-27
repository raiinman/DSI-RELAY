# Purpose

Own the static, local RELAY dashboard presentation.

# Ownership

- Governs `crates/relayd/dashboard/`.
- The daemon's `src/dashboard.rs` owns HTTP transport and command dispatch. These assets render returned command data only.

# Local Contracts

- Use the canonical command path through the daemon; do not read project files, SQLite, or other local state from browser code.
- Keep the primary view plain-language, responsive, and keyboard accessible. Put raw command output in Advanced details.
- Insert untrusted project names and details with text nodes, never HTML interpretation.
- The dashboard's URL fragment carries a per-start local token. Browser code sends it only in the local command header and removes it from the visible address after loading.
- No external assets, network services, telemetry, or browser storage.

# Verification

- `cargo test -p relayd --bin relayd dashboard::tests` covers HTTP command scope and token/origin rejection.
- `cargo test -p relayd --test phase5_dashboard` covers live daemon HTTP and pipe parity.

# Child DOX Index

- No child DOX files currently exist.
