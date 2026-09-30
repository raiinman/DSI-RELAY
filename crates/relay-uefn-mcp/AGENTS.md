# Purpose

Own the bounded local HTTP client for Epic's UEFN MCP server.

# Ownership

- This crate owns transport, legacy MCP session negotiation, tool discovery, and a private guarded editor-tool invocation primitive.
- It does not own RELAY commands, effect authorization, UEFN tool semantics, or runtime telemetry.

# Local Contracts

- Connect only to an explicit `127.0.0.1` port and validated path; do not use proxies or follow redirects.
- Enforce response, page, timeout, and session-header bounds before returning data.
- Discovery calls use only `tools/list`, `list_toolsets`, and `describe_toolset`. The private invocation primitive may use advertised `call_tool` only after fresh toolset discovery, exact toolset/tool matching, and strict argument-schema validation; unsupported schema shapes fail closed.
- The caller must supply an explicit effect classification and obtain authority outside this crate before invoking. The declaration is not proof of the tool's actual effect. No general RELAY command exposes this primitive.
- Build `describe_toolset` arguments from the advertised input schema. Toolset names accept bounded structured JSON or Unreal's plain-text top-level `- Name: description` catalog; discard descriptions and continuation lines. Parameter schemas remain structured JSON only. Reject unknown shapes, duplicate names, and excessive catalogs.
- Empty server display names are accepted when protocol and tool capability checks succeed; display metadata never establishes editor identity.
- Negotiate only implemented legacy MCP versions. Report unsupported modern/stateless versions honestly.
- Treat all server content as untrusted. Do not place tool text or server error bodies into RELAY diagnostics.
- Editor results remain opaque and bounded; their raw content has no Debug or default logging surface and may be consumed only for authorized result storage.
- No live UEFN integration claim is possible without an editor session.

# Work Guidance

- Keep requests serial; Epic's server executes tool calls on the game thread.
- Add UEFN read methods only after their exact advertised schema and non-mutating behavior are verified against a live supported editor.

# Verification

- Compile the crate when registered in the workspace.
- Exercise the invocation guard against a local fake MCP fixture; it is not live UEFN evidence.
- The ignored `live_editor_toolset_discovery` test probes a running local editor on port 8000 without invoking editor actions. Passing discovery proves only that connection and catalog exchange.
- The final integrated test must exercise the transport against a real UEFN editor; until then, mark that workflow UNTESTED.

# Child DOX Index

This crate has no child AGENTS.md files.
