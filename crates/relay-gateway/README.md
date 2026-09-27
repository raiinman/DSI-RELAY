# Local discovery gateway

`remote_client_status: untested`

Start the per-user gateway while `relayd` is running:

```powershell
cargo run -p relay-gateway --bin relay-gateway
```

In another terminal, send a local negotiation request:

```powershell
cargo run -p relay-gateway --bin relay-gateway -- probe
```

For a read-only discovery request, list commands with a bounded prefix or describe one command:

```powershell
cargo run -p relay-gateway --bin relay-gateway -- list registry.
cargo run -p relay-gateway --bin relay-gateway -- describe registry.list
```

`probe` sends `POST /v1/negotiate` with `{"protocol_min":1,"protocol_max":1}` to `127.0.0.1:8765`. `list` and `describe` send authenticated `POST /v1/execute` requests limited to the two registry commands. Each client command loads the bearer token from a current-user DPAPI-protected local credential; the token is never entered on a command line or printed. Press Ctrl+C in the server terminal to stop it and remove the credential file. A crash may leave an encrypted stale file; the next successful start replaces it with a new token.

This gateway exposes only bounded `registry.list` and `registry.describe` discovery. It does not expose project data, writes, public networking, or a ChatGPT connection.

## Local MCP endpoint

The same loopback listener exposes `POST /mcp` for the exact stateless MCP Streamable HTTP protocol version `2026-07-28`. It supports `server/discover`, `tools/list`, and `tools/call` for two tools: `relay_registry_list` and `relay_registry_describe`. Tool calls still pass through the same bounded shared registry commands. The endpoint returns JSON responses, requires the current bearer token, validates Host and Origin, and checks the protocol, method, and tool-name headers against the JSON-RPC body. Earlier handshake-based MCP versions receive an explicit unsupported-version error; GET streams, sessions, subscriptions, and public remote access are not offered.

The launcher keeps the bearer token in a per-user DPAPI credential and does not print or export it. The MCP endpoint is currently for local integration with a trusted credential broker or test client; ordinary external MCP clients have no configured token handoff. This construction check does not establish a working ChatGPT or public remote client connection.

Protocol reference: [MCP 2026-07-28 Streamable HTTP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http), [discovery](https://modelcontextprotocol.io/specification/2026-07-28/server/discover), and [tools](https://modelcontextprotocol.io/specification/2026-07-28/server/tools).
