# Purpose

Package the local DSI RELAY MCP connection for ordinary ChatGPT desktop Chat.

# Ownership

- Governs `plugins/dsi-relay-chat/` and descendants.

# Local Contracts

- Root `plugin.json` and `mcp.json` are the portable package; `.codex-plugin/plugin.json` is a compatibility fallback.
- The launcher resolves the currently installed per-user RELAY gateway and runs its stdio mode. Do not place credentials, user paths, or project data in manifests, environment variables, or stdout.
- Expose only the gateway's five bounded read-only discovery and result tools. No editor writes or remote listener may be added without a separate security contract.
- ChatGPT desktop local plugin support does not imply ChatGPT web/mobile support or measured credit savings.

# Verification

- Validate both manifests and test MCP initialize, tools/list, and at least one tools/call through the launcher against a running local RELAY engine.

# Child DOX Index

- No child AGENTS.md files exist.
