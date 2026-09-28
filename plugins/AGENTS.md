# Purpose

Own installable RELAY client plugins. Plugins are adapters over RELAY Core, not alternate sources of product behavior.

# Ownership

- Governs `plugins/` and descendants unless a closer AGENTS.md applies.

# Local Contracts

- Keep plugin packages provider-specific only at their boundary and avoid embedding local profile paths, bearer tokens, or project content.
- Keep client-facing tools compact and budget-aware. Document exact supported clients and test limitations.
- Plugin source must remain in this repository; local marketplace copies are installation artifacts.

# Verification

- Validate plugin manifests and run the underlying adapter's protocol check before installing a local copy.

# Child DOX Index

- `plugins/dsi-relay-chat/AGENTS.md` — owns the desktop ChatGPT local read-only adapter package.
