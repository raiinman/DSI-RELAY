---
name: relay-core
description: Use RELAY's local CLI from a coding agent to inspect projects, run shared Core commands, and retrieve compact results. Applies to installed RELAY 0.1.x hosts on Windows.
---

# RELAY Core

Use the local `relay` CLI as a client of RELAY Core. The skill supports generic project and result workflows; load a domain skill for UEFN, runtime testing, or assets when available.

1. Run `scripts/relay-core.ps1 -Action Verify` once per host session. If version or required capabilities differ, stop using this skill's cached command metadata and use live `registry.describe` or update the skill. A stopped host is unavailable, not compatible.
2. Choose a narrow command prefix with `-Action Reference -Prefix project. -Limit 8`. This prints bounded generated metadata. Query the live full contract for a single candidate with `-Action Describe -CommandId project.import` before composing arguments.
3. Invoke with structured arguments in a UTF-8 JSON object file: `scripts/relay-core.ps1 -Action Invoke -CommandId project.import -ArgumentsFile ./arguments.json -AllowStateWrite -IdempotencyKey <unique-key>`. The wrapper sends one request through `relay exec --stdin`, preserving RELAY's JSON envelope and exit status. Do not treat a write timeout as safe to retry with a new key; inspect state or retry with the same key.
4. Prefer `result.describe` and `result.context` with a byte budget before `result.get`. Keep raw payloads and logs outside the agent context unless a specific detail is needed. Use exact required JSON pointers when facts must be retained; an inability to fit them is a failure, not a summary success.

The generated reference contains only compact metadata and is an offline selection aid. The running host's `registry.list` and `registry.describe` own the current contract. The wrapper will reject commands absent from the skill's `ai` surface metadata; update the skill when RELAY changes. RELAY policy and project permissions still decide whether a command executes.

`system.doctor` and `diagnostics.summary` provide bounded health details. Record unavailable real creator-app or hardware workflows as **UNTESTED**; imported or synthetic results cannot establish a live pass.

For a trusted local client that supports MCP `2026-07-28` with a separately protected bearer credential, the optional `relay-gateway` loopback endpoint offers `relay_registry_list`, `relay_registry_describe`, `relay_result_list`, `relay_result_describe`, and `relay_result_context`. Supply an exact project ID for every result read; context reads require a 512–4096 byte budget and return Core-filtered facts only. The local `relay-gateway results|result|context` subcommands use the protected credential without exposing it. Keep the CLI wrapper as the default until an actual MCP client is configured and tested; remote and ChatGPT compatibility remain **UNTESTED**.
