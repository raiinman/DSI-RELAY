# Purpose

Own the bounded local HTTP client for Epic's UEFN MCP server.

# Ownership

- This crate owns transport, legacy MCP session negotiation, tool discovery, and explicit read-only discovery calls.
- It does not own RELAY commands, UEFN tool schemas, editor writes, or runtime telemetry.

# Local Contracts

- Connect only to an explicit `127.0.0.1` port and validated path; do not use proxies or follow redirects.
- Enforce response, page, timeout, and session-header bounds before returning data.
- Call only `tools/list` and Epic's documented `list_toolsets` and `describe_toolset` discovery meta-tools. Do not dispatch arbitrary `call_tool` or editor tools here.
- Negotiate only implemented legacy MCP versions. Report unsupported modern/stateless versions honestly.
- Treat all server content as untrusted. Do not place tool text or server error bodies into RELAY diagnostics.
- No live UEFN integration claim is possible without an editor session.

# Work Guidance

- Keep requests serial; Epic's server executes tool calls on the game thread.
- Add UEFN read methods only after their exact advertised schema and non-mutating behavior are verified against a live supported editor.

# Verification

- Compile the crate when registered in the workspace.
- The final integrated test must exercise the transport against a real UEFN editor; until then, mark that workflow UNTESTED.

# Child DOX Index

This crate has no child AGENTS.md files.
