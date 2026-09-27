# Purpose

Own the Phase 9 authenticated loopback HTTP and MCP gateway prototype for bounded read-only RELAY access.

# Ownership

- This crate owns its HTTP parser, bearer-token check, bounded request policy, and forwarding adapter.
- `relay-contracts` owns command identities, versions, and argument schemas.
- `relay::client` owns the existing local daemon transport. RELAY Core still owns command execution.

# Local Contracts

- Bind only IPv4 loopback. Reject wrong Host, Origin, unauthenticated requests, transfer encoding, oversized messages, and malformed or duplicate security headers.
- Expose only `registry.list`, `registry.describe`, `result.list`, `result.describe`, and `result.context`, plus protocol/capability negotiation. Require bounded registry prefixes; restrict describe to AI-exposed commands. Result reads require an explicit project ID, metadata-only outputs, and a 4096-byte maximum Core context budget; `result.context` first verifies result ownership through `result.describe`.
- `/mcp` implements only stateless MCP Streamable HTTP `2026-07-28` with `server/discover`, `tools/list`, and `tools/call` for those five fixed tools. Reject unsupported protocol versions explicitly; do not claim compatibility with older handshake MCP clients.
- Never expose raw shell, full project data or result payloads, write commands, daemon token, local paths, or raw host status. Validate shared arguments and results, project membership, response sizes, and command capabilities. A single in-flight bounded host call prevents a stuck local pipe from accumulating workers.
- The library accepts a caller-provided high-entropy revocable bearer token. The local executable generates a fresh 256-bit token on each start, stores only a current-user DPAPI-encrypted credential blob under local app data, and never accepts or prints a token through CLI arguments, environment variables, or logs. Remove the blob on clean shutdown. No browser CORS grant is sent.
- The executable binds only `127.0.0.1:8765`; its `probe`, `list`, `describe`, `results`, `result`, and `context` subcommands read the protected credential and send local read-only requests without printing the token.
- This local HTTP prototype has no TLS, public bind, cloud relay, or independent delegated identity. It cannot be used as evidence that the documented remote workflow passed.
- Keep `remote_client_status: untested` until a real remote client can authenticate and complete the documented workflow.

# Work Guidance

- Keep fixed error codes and strict byte limits. Treat transport failures as unavailable without echoing local details.
- Keep test transport injection behind the same command allowlist; tests may use loopback but must not imply internet reachability.

# Verification

- Run focused parsing, authorization, origin/Host, MCP header, command-allowlist, and version-negotiation tests.
- Compile this crate as a workspace member.
- The executable token round-trip and local probe are construction checks; they do not prove a public remote or ChatGPT client workflow.

# Child DOX Index

- No child DOX files currently exist; this file owns the crate.
