# DSI RELAY Chat adapter

This desktop-only plugin exposes RELAY's five compact, read-only MCP tools to ordinary ChatGPT Chat. Install and launch DSI RELAY first. Then install this plugin from a local marketplace in ChatGPT desktop and start a new Chat. Ask ChatGPT to use DSI RELAY to list available capabilities. RELAY must remain running on this computer while Chat uses it.

The plugin launches the installed `relay-gateway.exe` in standard MCP stdio mode. Its manifest contains no bearer token, project path, or account identifier. The gateway reads the current local RELAY connection state inside its process. ChatGPT web and mobile cannot run this local plugin. This adapter does not prove token or credit savings; measure representative tasks before claiming them.

Created by RAiiNMAN. MIT licensed with the RELAY source repository.
