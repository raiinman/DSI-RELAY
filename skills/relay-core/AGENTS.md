# Purpose

Own the compact `relay-core` skill for local coding agents using the RELAY CLI.

# Ownership

- `SKILL.md` gives the minimal task workflow and points to optional command detail.
- `scripts/generate-reference.ps1` generates `references/commands.generated.json` from the shared registry.
- `scripts/relay-core.ps1` checks compatibility, shows a bounded reference, describes one live command, or invokes one structured request.

# Local Contracts

- `commands.generated.json` is derived data. Edit the registry, then regenerate; never manually change command metadata here.
- The script must validate host version and required capability IDs before a session treats the skill as compatible.
- Invocation uses the daemon's shared command bus through `relay exec --stdin`; it cannot create a private skill operation.
- The optional authenticated loopback MCP gateway offers only registry discovery plus project-scoped result metadata and byte-budgeted exact facts. It uses the same shared commands and does not replace the CLI wrapper or grant public/remote access.
- Write requests need an explicit opt-in and keyed commands need a caller-supplied idempotency key.
- Generated reference output is bounded by prefix and limit. Full argument schemas come from live `registry.describe` only when needed.

# Work Guidance

- Keep `SKILL.md` short. Read or print only the relevant command prefix for a task.
- Keep scripts noninteractive and do not transform success or error envelopes.

# Verification

- Run the reference generator and compare its output to the checked-in generated file.
- Parse the PowerShell scripts and validate `SKILL.md` frontmatter.

# Child DOX Index

- No child DOX files currently exist; this file owns the package subtree.
