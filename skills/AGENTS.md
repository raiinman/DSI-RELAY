# Purpose

Own local AI skill packages distributed with RELAY.

# Ownership

- This file governs `skills/` unless a child AGENTS.md is closer.
- Product command identity, schemas, effects, and permissions remain owned by the shared command registry under `crates/relay-contracts/`.

# Local Contracts

- Keep entrypoint instructions compact and load command details only for the task at hand.
- Generate command reference data from the registry; do not hand-maintain a second catalog.
- Skills and wrappers are clients of RELAY Core. They must not implement business behavior or bypass Core policy.
- Declare skill generation and compatible RELAY versions, then check host capabilities before use.
- Do not claim creator-app or hardware workflows passed based on local synthetic data.

# Work Guidance

- Prefer structured CLI requests and bounded result/context retrieval.
- Keep wrappers noninteractive and preserve RELAY's structured response and exit status.

# Verification

- Regenerate references after registry changes and check generated output for drift.
- Validate skill frontmatter and script syntax when a package changes.

# Child DOX Index

- `skills/relay-core/AGENTS.md` — owns the core local coding-agent skill, generated command reference, and its thin PowerShell wrappers.
